//! In-world packet guard checks and `ClientPacket` dispatch table (`dispatch_in_world`).

use nro_core::ZoneId;
use nro_protocol::{ClientPacket, ProtocolError, SessionHandle, SocialAction};
use nro_world::s2c_packets;
use tracing::debug;

use super::{ServerSessionHandler, router_shop, router_skill_effects, send_thong_bao};
use crate::runtime::unix_now_ms;

impl ServerSessionHandler {
    pub(super) async fn dispatch_in_world(
        &self,
        session: &SessionHandle,
        packet: ClientPacket,
    ) -> Result<(), ProtocolError> {
        let sid = session.id();
        if matches!(
            &packet,
            ClientPacket::PlayerMove(..)
                | ClientPacket::ZoneChange(..)
                | ClientPacket::MapWaypointChange(_)
                | ClientPacket::MapTransport(_)
                | ClientPacket::TransportComplete
                | ClientPacket::GotoPlayer(_)
                | ClientPacket::AttackMobs(..)
                | ClientPacket::AttackPlayers(_)
                | ClientPacket::AttackMulti(..)
                | ClientPacket::SkillNotFocus(..)
                | ClientPacket::SkillUp { .. }
                | ClientPacket::PickMapItem(_)
                | ClientPacket::NpcMenuOpen(_)
                | ClientPacket::NpcMenuConfirm(..)
                | ClientPacket::DauThanConfirm(..)
                | ClientPacket::MagicTree(_)
                | ClientPacket::ShopBuy(..)
                | ClientPacket::ShopSell(..)
                | ClientPacket::ConsignShop(_)
                | ClientPacket::GetItemMove(..)
                | ClientPacket::UseItem(..)
                | ClientPacket::BoxSort
                | ClientPacket::CombinePreview(..)
                | ClientPacket::Trade(_)
                | ClientPacket::PetChangeStatus(_)
                | ClientPacket::Fusion(_)
                | ClientPacket::LuckyRoundOpen(..)
                | ClientPacket::TaskGet { .. }
                | ClientPacket::ClientInput(_)
                | ClientPacket::PvpChallenge(..)
                | ClientPacket::ChangeFlag(nro_protocol::FlagAction::Choose(_))
        ) && self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .is_some_and(|e| e.snapshot.stats.hp <= 0)
        {
            return if matches!(&packet, ClientPacket::PlayerMove(..)) {
                Ok(())
            } else {
                send_thong_bao(session, "Không thể thực hiện khi đã chết!")
            };
        }
        if matches!(
            &packet,
            ClientPacket::PlayerMove(..)
                | ClientPacket::AttackMobs(..)
                | ClientPacket::AttackPlayers(_)
                | ClientPacket::AttackMulti(..)
                | ClientPacket::SkillNotFocus(..)
                | ClientPacket::ZoneChange(..)
                | ClientPacket::MapWaypointChange(_)
                | ClientPacket::MapTransport(_)
                | ClientPacket::GotoPlayer(_)
        ) && router_skill_effects::is_player_cc_locked(session.player_id(), unix_now_ms())
        {
            return if matches!(&packet, ClientPacket::PlayerMove(..)) {
                Ok(())
            } else {
                send_thong_bao(session, "Không thể thực hiện khi đang bị khống chế!")
            };
        }
        if matches!(
            &packet,
            ClientPacket::ClientInput(_)
                | ClientPacket::CombinePreview(_, _)
                | ClientPacket::MapTransport(_)
                | ClientPacket::TransportComplete
                | ClientPacket::NpcMenuOpen(_)
                | ClientPacket::NpcMenuConfirm(_, _)
                | ClientPacket::DauThanConfirm(_, _, _)
                | ClientPacket::LuckyRoundOpen(_, _)
                | ClientPacket::AchievementConfirm(_)
                | ClientPacket::MagicTree(_)
                | ClientPacket::PickMapItem(_)
                | ClientPacket::ReviveMeLive
                | ClientPacket::BoxSort
                | ClientPacket::ClanCreateInfo(_)
                | ClientPacket::ZoneChange(_, _)
                | ClientPacket::MapWaypointChange(_)
                | ClientPacket::GotoPlayer(_)
                | ClientPacket::ReturnHomeMeBack
                | ClientPacket::EnemyList(SocialAction::PrimaryTarget(_))
                | ClientPacket::TaskGet { .. }
                | ClientPacket::PetChangeStatus(_)
                | ClientPacket::Fusion(_)
        ) && self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .is_some_and(|e| router_shop::is_in_trade(e.snapshot.id))
        {
            return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
        }
        match packet {
            ClientPacket::ClientOk => self.handle_client_ok(session, sid).await?,
            ClientPacket::MagicTree(action) => self.handle_magic_tree(session, sid, action).await?,
            ClientPacket::FinishLoadMap => self.handle_finish_load_map(session, sid).await?,
            ClientPacket::LoadRms(key) => self.handle_load_rms(session, sid, &key).await?,
            ClientPacket::PlayerMove(_t, x, maybe_y) => {
                self.handle_player_move(session, sid, x, maybe_y).await?;
            }
            ClientPacket::OpenZoneUi => self.handle_open_zone_ui(session, sid).await?,
            ClientPacket::ZoneChange(_raw, Some(new_zone)) => {
                self.switch_player_zone(session, sid, None, new_zone, None)
                    .await?;
            }
            ClientPacket::ZoneChange(_raw, None) => {
                if let Some(loc) = self
                    .shared
                    .clients
                    .authenticated
                    .get(&sid)
                    .map(|e| e.snapshot.location)
                {
                    let max_z = self
                        .shared
                        .templates
                        .maps
                        .iter()
                        .find(|m| m.id == loc.map_id)
                        .map_or(1, |m| {
                            if Self::is_offline_map_id(m.id) || m.map_type == 1 {
                                u16::from(m.zones.max(1))
                            } else {
                                u16::from(m.zones.max(2))
                            }
                        });
                    if max_z > 1 {
                        self.switch_player_zone(
                            session,
                            sid,
                            None,
                            ZoneId((loc.zone_id.0 + 1) % max_z),
                            None,
                        )
                        .await?;
                    } else {
                        session.send(s2c_packets::reset_point(loc)).await?;
                        send_thong_bao(session, "Không thể đổi khu vực tại bản đồ này!")?;
                    }
                }
            }
            ClientPacket::MapWaypointChange(is_off) => {
                self.handle_waypoint_change(session, sid, is_off).await?;
            }
            ClientPacket::AttackMobs(mobs, _) => {
                self.handle_attack_mobs(session, sid, &mobs).await?;
            }
            ClientPacket::AttackPlayers(targets) => self.handle_attack_players(sid, &targets),
            ClientPacket::AttackMulti(mobs, players) => {
                self.handle_attack_multi(session, sid, &mobs, &players)
                    .await?;
            }
            ClientPacket::SkillNotFocus(st, sp) => {
                self.handle_skill_not_focus(session, sid, st, sp).await?;
            }
            ClientPacket::PotentialUp(stat_idx, points) => {
                self.handle_potential_up(session, sid, stat_idx, points)
                    .await?;
            }
            ClientPacket::SkillUp { template_id, point } => {
                self.handle_skill_up(session, sid, template_id, point)
                    .await?;
            }
            ClientPacket::SkillSelect(tid) => {
                if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid)
                    && e.snapshot
                        .skills
                        .iter()
                        .any(|sk| sk.template_id == i32::from(tid.0) && sk.point > 0)
                {
                    e.selected_skill = tid.0;
                }
            }
            ClientPacket::ChangeOnSkill(sc) => {
                if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) {
                    e.snapshot.skills_shortcut = sc;
                }
            }
            ClientPacket::BoxSort => self.handle_box_sort(session, sid).await?,
            ClientPacket::ChangeName { name, .. } => {
                self.handle_change_name(session, sid, &name).await?;
            }
            ClientPacket::PickMapItem(id) => self.handle_pick_map_item(session, sid, id).await?,
            ClientPacket::ReturnHomeMeBack => self.handle_revive(session, sid, true).await?,
            ClientPacket::ReviveMeLive => self.handle_revive(session, sid, false).await?,
            ClientPacket::ChatMap(raw) => self.handle_chat_map(session, sid, &raw).await?,
            ClientPacket::ChatGlobal(text) => self.handle_chat_global(session, sid, &text)?,
            ClientPacket::ChatPrivate(target, text) => {
                self.handle_whisper(session, sid, target, &text).await?;
            }
            ClientPacket::NpcMenuOpen(npc_id) => {
                self.handle_npc_menu(session, sid, npc_id, None).await?;
            }
            ClientPacket::NpcMenuConfirm(npc_id, sel) => {
                self.handle_npc_menu(session, sid, npc_id, Some(sel))
                    .await?;
            }
            ClientPacket::DauThanConfirm(4, menu_id, _) => {
                self.handle_magic_tree_confirm(session, sid, menu_id)
                    .await?;
            }
            ClientPacket::DauThanConfirm(npc_id, menu_id, _) => {
                self.handle_npc_menu(session, sid, i16::from(npc_id), Some(menu_id))
                    .await?;
            }
            ClientPacket::TaskGet { npc_id, .. } => {
                if let Some(say) = self
                    .advance_task_on_npc(session, sid, i16::from(npc_id))
                    .await?
                {
                    session
                        .send(s2c_packets::npc_open_say(i16::from(npc_id), &say, None)?)
                        .await?;
                }
            }
            ClientPacket::ShopBuy(bt, id, qty) => {
                self.handle_shop_buy(session, sid, bt, id, qty).await?;
            }
            ClientPacket::ShopSell(act, wh, idx) => {
                self.handle_shop_sell(session, sid, act, wh, idx).await?;
            }
            ClientPacket::ConsignShop(act) => {
                self.handle_consign_action(session, sid, act).await?;
            }
            ClientPacket::ClientInput(fields) => {
                self.handle_client_input(session, sid, &fields).await?;
            }
            ClientPacket::GetItemMove(mt, idx) => {
                self.handle_get_item_move(session, sid, mt, idx).await?;
            }
            ClientPacket::UseItem(at, wh, idx, tid) => {
                self.handle_use_item(session, sid, at, wh, idx, tid).await?;
            }
            ClientPacket::MapTransport(idx) => {
                self.handle_map_transport(session, sid, idx).await?;
            }
            ClientPacket::TransportComplete => self.handle_transport_complete(session, sid).await?,
            ClientPacket::GotoPlayer(pid) => self.handle_goto_player(session, sid, pid).await?,
            ClientPacket::CombinePreview(act, bag_indices) => {
                self.handle_combine_preview_cmd(session, sid, act, &bag_indices)
                    .await?;
            }
            ClientPacket::Trade(action) => self.handle_trade(session, sid, action).await?,
            ClientPacket::ChangeFlag(act) => self.handle_change_flag(session, sid, act).await?,
            ClientPacket::PetShowInfo => self.handle_pet_info(session, sid).await?,
            ClientPacket::PetChangeStatus(st) => {
                self.handle_pet_status(session, sid, st, false).await?;
            }
            ClientPacket::Fusion(ft) => self.handle_pet_status(session, sid, ft, true).await?,
            ClientPacket::IntrinsicMenu(sub) => {
                self.handle_intrinsic_open(session, sid, sub).await?;
            }
            ClientPacket::PlayerMenu(pid) => {
                self.handle_player_menu(session, sid, pid, false).await?;
            }
            ClientPacket::GetPlayerMenu(pid) => {
                self.handle_player_menu(session, sid, pid, true).await?;
            }
            ClientPacket::PlayerMenuAction(pid, mid) => {
                self.handle_player_menu_action(session, sid, pid, mid)
                    .await?;
            }
            ClientPacket::PvpChallenge(act, pty, pid) => {
                self.handle_pvp_challenge(session, sid, act, pty, pid)
                    .await?;
            }
            ClientPacket::FriendList(act) => self.handle_social(session, sid, act, true).await?,
            ClientPacket::EnemyList(act) => self.handle_social(session, sid, act, false).await?,
            ClientPacket::ClanCreateInfo(act) => {
                self.handle_clan_create_info(session, sid, &act).await?;
            }
            ClientPacket::ClanSearch(q) => self.handle_clan_search(session, sid, &q).await?,
            ClientPacket::ClanMemberList(cid) => {
                self.handle_clan_member_list(session, sid, cid).await?;
            }
            ClientPacket::ClanMessage(act) => self.handle_clan_message(session, sid, &act).await?,
            ClientPacket::ClanInfo(cid) => self.handle_clan_info_req(session, sid, cid).await?,
            ClientPacket::ClanLeaveMenu => self.handle_clan_leave_menu(session, sid).await?,
            ClientPacket::ClanInvite(act) => self.handle_clan_invite(session, sid, act).await?,
            ClientPacket::ClanRemote(pid, role) => {
                self.handle_clan_remote(session, sid, pid, role).await?;
            }
            ClientPacket::ClanDonate(msg_id) => {
                self.handle_clan_donate(session, sid, msg_id).await?;
            }
            ClientPacket::ClanJoinDecision(msg_id, dec) => {
                self.handle_clan_join_decision(session, sid, msg_id, dec)
                    .await?;
            }
            ClientPacket::SuperRankOrAdminBoss(_) => {
                session
                    .send(s2c_packets::thong_bao("Tính năng đang được phát triển!")?)
                    .await?;
            }
            ClientPacket::Radar(act) => self.handle_radar(session, sid, act).await?,
            ClientPacket::TopQuery(name, page) => {
                self.handle_top_query(session, sid, &name, page).await?;
            }
            ClientPacket::LuckyRoundOpen(ty, cnt) => {
                self.handle_lucky_round(session, sid, ty, cnt).await?;
            }
            ClientPacket::AchievementConfirm(idx) => {
                self.handle_achievement(session, sid, Some(idx)).await?;
            }
            ClientPacket::LockInventory(pin) => self.handle_lock_inventory(session, sid, pin)?,
            _ => debug!(
                sid,
                op = <&'static str>::from(&packet),
                "processed auxiliary world packet"
            ),
        }
        Ok(())
    }
}
