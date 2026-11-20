//! Operator / administrator chat command execution (`exec_admin_chat_cmd` and base-stat overrides).

use nro_boss::BossStatus;
use nro_consts::const_npc;
use nro_core::{ItemInstance, ItemOption, ZoneId};
use nro_protocol::{AdminCommand as ChatAdminCommand, ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};

use super::{
    ServerSessionHandler, con_meo_state, send_thong_bao, snapshot_to_economy_inv,
    sync_economy_inv_to_snapshot,
};
use crate::data_game_tpl::pkt_task_get;
use crate::runtime::{LeaderboardSlot, unix_now_ms};

impl ServerSessionHandler {
    pub(super) async fn exec_admin_chat_cmd(
        &self, session: &SessionHandle, sid: u64, cmd: ChatAdminCommand,
    ) -> Result<(), ProtocolError> {
        match cmd {
            ChatAdminCommand::TeleportMap { map_id } => self.switch_player_zone(session, sid, Some(map_id), ZoneId(0), None).await?,
            ChatAdminCommand::AddPlayerPower { power } => {
                if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) {
                    e.snapshot.stats.power = e.snapshot.stats.power.saturating_add(power);
                    e.snapshot.stats.tiem_nang = e.snapshot.stats.tiem_nang.saturating_add(power);
                    let _ = session.try_send(s2c_packets::me_load_point(&e.snapshot.stats));
                    self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                }
                self.advance_task_on_power(session, sid).await?;
            }
            ChatAdminCommand::SpawnItems { item_id, quantity, options } => {
                let Some(tpl_it) = self.shared.templates.items.iter().find(|i| i.id == i32::from(item_id.0)) else {
                    return send_thong_bao(session, "Item ID không tồn tại!");
                };
                let opts: Vec<ItemOption> = options.into_iter().map(|o| ItemOption::new(o.option_id.clamp(0, i32::from(u16::MAX)) as u16, o.param)).collect();
                let qty = quantity.max(1) as u32;
                if let Some(msg) = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
                    let (mut inv, now) = (snapshot_to_economy_inv(&e.snapshot), unix_now_ms());
                    let ok = if tpl_it.is_up_to_up {
                        inv.add_to_bag(ItemInstance::with_options(item_id, qty, opts.clone(), now), true).is_ok()
                    } else {
                        (0..qty.min(20)).any(|_| inv.add_to_bag(ItemInstance::with_options(item_id, 1, opts.clone(), now), false).is_ok())
                    };
                    sync_economy_inv_to_snapshot(&inv, &mut e.snapshot);
                    let _ = session.try_send(s2c_packets::inventory_bag(&e.snapshot.items_bag));
                    if ok { format!("Đã nhận {} x{qty}", tpl_it.name) } else { "Hành trang đã đầy!".to_owned() }
                }) {
                    send_thong_bao(session, &msg)?;
                }
            }
            ChatAdminCommand::SetMainTask { task_id } => {
                let tid = task_id.clamp(0, 126) as i8;
                if let Some(snap) = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
                    e.snapshot.main_task.task_id = tid;
                    e.snapshot.main_task.sub_index = 0;
                    e.snapshot.main_task.count = 0;
                    e.snapshot.clone()
                }) {
                    self.shared.leaderboards.mark_dirty(LeaderboardSlot::Task);
                    session.send(pkt_task_get(&self.shared.templates, &snap)).await?;
                    send_thong_bao(session, &format!("Đã chuyển sang nhiệm vụ {tid}!"))?;
                }
            }
            ChatAdminCommand::ReleaseSkillCooldowns => {
                if let Some((stats, ek, loc)) = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
                    e.snapshot.stats.hp = e.snapshot.stats.hp_max.max(1);
                    e.snapshot.stats.mp = e.snapshot.stats.mp_max.max(1);
                    e.snapshot.stats.stamina = e.snapshot.stats.max_stamina.max(1_000);
                    for sk in &mut e.snapshot.skills { sk.last_time_use_ms = 0; }
                    (e.snapshot.stats, e.entity_key, e.snapshot.location)
                }) {
                    session.send(s2c_packets::me_load_point(&stats)).await?;
                    session.send(s2c_packets::me_load_hp(stats.hp)).await?;
                    session.send(s2c_packets::me_load_mp(stats.mp)).await?;
                    if let (Some(ek), Ok(tx)) = (ek, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                        let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, stats));
                    }
                }
            }
            ChatAdminCommand::ShowCoordinates => {
                if let Some(loc) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.location) {
                    let mname = self.shared.templates.maps.iter().find(|m| m.id == loc.map_id).map_or("Map", |m| m.name.as_str());
                    send_thong_bao(session, &format!("{mname} (ID {}) - Khu {} | X: {}, Y: {}", loc.map_id.0, loc.zone_id.0, loc.x, loc.y))?;
                }
            }
            ChatAdminCommand::SetBaseDamage { damage } => self.set_admin_base_stat(session, sid, Some(i64::from(damage)), None, None).await?,
            ChatAdminCommand::SetBaseHp { hp_base } => self.set_admin_base_stat(session, sid, None, Some(i64::from(hp_base)), None).await?,
            ChatAdminCommand::SetBaseKi { mp_base } => self.set_admin_base_stat(session, sid, None, None, Some(i64::from(mp_base))).await?,
            ChatAdminCommand::NudgeDown { delta_y } => {
                if let Some(loc) = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
                    e.snapshot.location.y = e.snapshot.location.y.saturating_add(delta_y);
                    let loc = e.snapshot.location;
                    if let (Some(ek), Ok(tx)) = (e.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                        let _ = tx.try_send(ZoneCommand::MovePlayer(ek, loc));
                    }
                    loc
                }) {
                    session.send(s2c_packets::reset_point(loc)).await?;
                }
            }
            ChatAdminCommand::AddPetPower { power } => {
                let pet_pkt = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut e| {
                    let pet = e.snapshot.pet.as_mut()?;
                    pet.stats.power = pet.stats.power.saturating_add(power);
                    pet.stats.tiem_nang = pet.stats.tiem_nang.saturating_add(power);
                    self.pkt_pet_detail(&e.snapshot)
                });
                if let Some(pkt) = pet_pkt {
                    session.send(pkt).await?;
                    send_thong_bao(session, &format!("Đã cộng {power} sức mạnh cho đệ tử!"))?;
                } else {
                    send_thong_bao(session, "Bạn chưa có đệ tử!")?;
                }
            }
            ChatAdminCommand::OpenGiveItemForm => self.handle_open_input(session, sid, "admin_give_item").await?,
            ChatAdminCommand::OpenGetItemForm => self.handle_open_input(session, sid, "admin_get_item").await?,
            ChatAdminCommand::OpenAdminDashboard => {
                let online = self.shared.clients.authenticated.len();
                if let Some(pid) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.id) {
                    con_meo_state().insert(pid, const_npc::MENU_ADMIN);
                }
                session.send(s2c_packets::npc_open_menu(
                    i16::from(const_npc::CON_MEO),
                    &format!("Bảng điều khiển Quản Trị Viên\nNgười chơi đang trực tuyến: {online}"),
                    &["Đóng"],
                    None,
                )?).await?;
            }
            ChatAdminCommand::ShowBrolyBossList => {
                let lines: Vec<String> = {
                    let mgr = self.shared.bosses.lock();
                    mgr.instances()
                        .values()
                        .filter(|b| b.lifecycle.status != BossStatus::Rest)
                        .take(20)
                        .map(|b| {
                            let mname = self.shared.templates.maps.iter().find(|m| m.id == b.map_id).map_or("Map", |m| m.name.as_str());
                            format!("{:?} - {mname} (ID {}, Khu {}) [HP: {}/{}]", b.boss_type, b.map_id.0, b.zone_id.0, b.stats.hp, b.stats.hp_max)
                        })
                        .collect()
                };
                let msg = if lines.is_empty() {
                    "Hiện chưa có Boss nào đang hoạt động!".to_owned()
                } else {
                    format!("Danh sách Boss đang hoạt động ({}):\n{}", lines.len(), lines.join("\n"))
                };
                send_thong_bao(session, &msg)?;
            }
            _ => send_thong_bao(session, "Lệnh quản trị này chưa được bật trên server!")?,
        }
        Ok(())
    }

    async fn set_admin_base_stat(
        &self, session: &SessionHandle, sid: u64, dmg: Option<i64>, hp: Option<i64>, mp: Option<i64>,
    ) -> Result<(), ProtocolError> {
        if let Some((p, stats, ek, loc)) = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
            if let Some(d) = dmg { e.snapshot.stats.dameg = d.max(1); }
            if let Some(h) = hp { e.snapshot.stats.hpg = h.max(1); }
            if let Some(m) = mp { e.snapshot.stats.mpg = m.max(1); }
            super::recalculate_session_stats(sid, &mut e.snapshot, self.shared.config.parity_mode);
            e.snapshot.stats.hp = e.snapshot.stats.hp_max;
            e.snapshot.stats.mp = e.snapshot.stats.mp_max;
            (s2c_packets::me_load_point(&e.snapshot.stats), e.snapshot.stats, e.entity_key, e.snapshot.location)
        }) {
            session.send(p).await?;
            if let (Some(ek), Ok(tx)) = (ek, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, stats));
            }
        }
        Ok(())
    }
}
