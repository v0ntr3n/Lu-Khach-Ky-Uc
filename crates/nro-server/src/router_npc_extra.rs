//! Intrinsic menus, `CON_MEO` confirms (`112`), Lucky Round (`-127`), and Achievements (`-76`).

use nro_consts::{cmd_message, const_npc};
use nro_core::{ClanId, GameRng};
use nro_economy::{LuckyRoundMode, LuckyRoundService};
use nro_player::progression::{INTRINSIC_GOLD_MILLIONS, INTRINSIC_MIN_POWER};
use nro_protocol::{Packet, PacketWriter, ProtocolError, SessionHandle};
use nro_world::s2c_packets;

use super::{
    ServerSessionHandler, con_meo_state, snapshot_to_economy_inv, sync_economy_inv_to_snapshot,
    wallet_money_pkt,
};
use crate::runtime::unix_now_ms;

impl ServerSessionHandler {
    pub(super) async fn handle_intrinsic_open(&self, session: &SessionHandle, sid: u64, sub: i8) -> Result<(), ProtocolError> {
        if sub == 1 { return session.send(self.build_intrinsic_catalog_pkt()?).await; }
        let Some((pid, intr)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.intrinsic)) else { return Ok(()); };
        if let Some(rec) = self.shared.templates.intrinsics.iter().find(|r| r.id == i32::from(intr.intrinsic_id)) {
            session.send(s2c_packets::intrinsic_info(rec.icon as i16, &rec.name.replacen('#', &intr.param1.to_string(), 1))?).await?;
        } else {
            session.send(s2c_packets::intrinsic_info(0, "Chưa mở nội tại")?).await?;
        }
        let open_m = INTRINSIC_GOLD_MILLIONS[usize::from(intr.count_open.clamp(0, 7) as u8)];
        let gold_lbl = format!("Mở Nội Tại\n({open_m}Tr Vàng)");
        con_meo_state().insert(pid, const_npc::INTRINSIC);
        session.send(s2c_packets::npc_open_menu(
            i16::from(const_npc::CON_MEO), "Bạn muốn thực hiện thao tác Nội Tại nào?",
            &["Xem tất cả\nNội Tại", &gold_lbl, "Mở VIP\n(100 Ngọc)", "Đóng"], None,
        )?).await
    }

    async fn execute_dissolve_clan(&self, session: &SessionHandle, sid: u64) -> Result<(), ProtocolError> {
        let Some((pid, cid)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.clan_id)) else {
            return Ok(());
        };
        if cid.0 < 0 {
            return session.send(s2c_packets::thong_bao("Bạn không ở trong bang hội nào!")?).await;
        }
        let Ok(clans) = self.shared.clans_repo.load_all().await else {
            return session.send(s2c_packets::thong_bao("Không thể giải tán bang hội lúc này!")?).await;
        };
        let is_leader = clans
            .iter()
            .find(|c| c.id == cid)
            .is_some_and(|c| c.members.iter().any(|m| m.id == pid.0 as i32 && m.role == 0));
        if !is_leader {
            return session.send(s2c_packets::thong_bao("Chỉ bang chủ mới có thể giải tán bang hội!")?).await;
        }
        if self.shared.clans_repo.delete_clan(cid).await.is_err() {
            return session.send(s2c_packets::thong_bao("Không thể giải tán bang hội lúc này!")?).await;
        }
        for mut entry in self.shared.clients.authenticated.iter_mut() {
            if entry.snapshot.clan_id == cid {
                entry.snapshot.clan_id = ClanId(-1);
            }
        }
        let mut w = PacketWriter::with_capacity(4);
        w.write_i32(-1);
        session.send(w.finish(cmd_message::CLAN_INFO)).await?;
        session.send(s2c_packets::thong_bao("Đã giải tán bang hội!")?).await
    }

    pub(super) async fn handle_con_meo_confirm(&self, session: &SessionHandle, sid: u64, select: u8) -> Result<(), ProtocolError> {
        let Some(pid) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.id) else { return Ok(()); };
        let mode = con_meo_state().remove(&pid).map_or(const_npc::INTRINSIC, |(_, v)| v);
        if mode == const_npc::CONFIRM_LEAVE_CLAN {
            return if select == 0 { self.execute_leave_clan(session, sid).await } else { Ok(()) };
        }
        if mode == const_npc::CONFIRM_DISSOLUTION_CLAN {
            return if select == 0 { self.execute_dissolve_clan(session, sid).await } else { Ok(()) };
        }
        if !matches!(mode, const_npc::INTRINSIC | const_npc::CONFIRM_OPEN_INTRINSIC | const_npc::CONFIRM_OPEN_INTRINSIC_VIP) {
            return Ok(());
        }
        match select {
            0 => session.send(self.build_intrinsic_catalog_pkt()?).await?,
            1 | 2 => {
                let is_vip = select == 2;
                let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    if entry.snapshot.stats.power < INTRINSIC_MIN_POWER {
                        return Err(format!("Cần ít nhất {INTRINSIC_MIN_POWER} sức mạnh để mở Nội Tại!"));
                    }
                    let g_i16 = i16::from(i8::from(entry.snapshot.gender));
                    let pool: Vec<_> = self.shared.templates.intrinsics.iter().filter(|r| r.gender == g_i16 || r.gender == 3).collect();
                    if pool.is_empty() { return Err("Chưa có dữ liệu Nội Tại!".to_owned()); }
                    if is_vip {
                        if entry.snapshot.wallet.gem < 100 { return Err("Cần 100 ngọc xanh để mở Nội Tại VIP!".to_owned()); }
                        entry.snapshot.wallet.gem -= 100;
                        entry.snapshot.intrinsic.count_open = 0;
                    } else {
                        let open_idx = usize::from(entry.snapshot.intrinsic.count_open.clamp(0, 7) as u8);
                        let gold_cost = INTRINSIC_GOLD_MILLIONS[open_idx] * 1_000_000;
                        let gold_cost_i64 = gold_cost as i64;
                        if entry.snapshot.wallet.gold < gold_cost_i64 {
                            return Err(format!("Cần {gold_cost} vàng để mở Nội Tại!"));
                        }
                        entry.snapshot.wallet.gold -= gold_cost_i64;
                        if entry.snapshot.intrinsic.count_open < 7 {
                            entry.snapshot.intrinsic.count_open += 1;
                        }
                    }
                    let mut rng = GameRng::from_seed(unix_now_ms().cast_unsigned() ^ sid);
                    let chosen = pool[rng.next_range(0, (pool.len() - 1) as i32) as usize];
                    let p1 = rng.next_range(chosen.param_from_1, chosen.param_to_1.max(chosen.param_from_1)) as i16;
                    entry.snapshot.intrinsic.intrinsic_id = chosen.id as i8;
                    entry.snapshot.intrinsic.param1 = p1;
                    let info = chosen.name.replacen('#', &p1.to_string(), 1);
                    Ok((chosen.icon as i16, info, wallet_money_pkt(&entry.snapshot.wallet)))
                });
                match res {
                    Some(Ok((icon, info, money_pkt))) => {
                        session.send(s2c_packets::intrinsic_info(icon, &info)?).await?;
                        session.send(money_pkt).await?;
                        session.send(s2c_packets::thong_bao(&format!("Đã mở Nội Tại: {info}"))?).await?;
                    }
                    Some(Err(msg)) => session.send(s2c_packets::thong_bao(&msg)?).await?,
                    None => {}
                }
            }
            _ => { if let Some(mp) = self.money_pkt_for(sid) { session.send(mp).await?; } }
        }
        Ok(())
    }

    fn build_intrinsic_catalog_pkt(&self) -> Result<Packet, ProtocolError> {
        let mut w = PacketWriter::with_capacity(1024);
        w.write_i8(1);
        let tabs = [(0_i16, "Trái Đất\nNội Tại"), (1, "Namếc\nNội Tại"), (2, "Xayda\nNội Tại")];
        w.write_i8(tabs.len() as i8);
        for (g, title) in tabs {
            w.write_utf(title)?;
            let cnt = self.shared.templates.intrinsics.iter().filter(|r| r.gender == g).take(60).count();
            w.write_i8(cnt as i8);
            for r in self.shared.templates.intrinsics.iter().filter(|r| r.gender == g).take(60) {
                w.write_i16(r.icon as i16);
                let (lo, hi) = (r.param_from_1, r.param_to_1);
                w.write_utf(&r.name.replacen('#', &format!("{lo}..{hi}"), 1))?;
            }
        }
        Ok(w.finish(cmd_message::SPEACIAL_SKILL))
    }

    pub(super) async fn handle_lucky_round(&self, session: &SessionHandle, sid: u64, ty: Option<i8>, opt_cnt: Option<i8>) -> Result<(), ProtocolError> {
        let is_vip = ty == Some(2);
        let Some(cnt) = opt_cnt else {
            let mut w = PacketWriter::with_capacity(24);
            w.write_i8(0);
            w.write_i8(7);
            for b in 419_i16..=425 { w.write_i16(b); }
            w.write_i8(if is_vip { 2 } else { 1 });
            w.write_i32(if is_vip { 250_000_000 } else { 4 });
            w.write_i16(if is_vip { -1 } else { 821 });
            return session.send(w.finish(cmd_message::LUCKY_ROUND)).await;
        };
        let mode = if is_vip { LuckyRoundMode::VipGold } else { LuckyRoundMode::StandardGem };
        let n = cnt.clamp(1, 7).cast_unsigned();
        let res = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            if inv.empty_bag_slots() < usize::from(n) { return Err("Hành trang đã đầy!".to_owned()); }
            let prev_len = inv.lucky_round_box.len();
            let mut rng = GameRng::from_seed(unix_now_ms().cast_unsigned() ^ sid);
            let spun = LuckyRoundService::spin(&mut inv, mode, n, &mut rng)
                .map_err(|err| format!("Vòng quay: {err}"))?;
            let mut icons = Vec::with_capacity(usize::from(spun));
            let won_items: Vec<_> = inv.lucky_round_box.drain(prev_len..).flatten().collect();
            for item in won_items {
                let tpl = self.shared.templates.items.iter().find(|t| t.id == i32::from(item.template_id.0));
                let icon = tpl.map_or(420_i16, |t| t.icon_id as i16);
                inv.add_to_bag(item, tpl.is_some_and(|t| t.is_up_to_up)).map_err(|_| "Hành trang đã đầy!".to_owned())?;
                icons.push(icon);
            }
            sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
            let mut w = PacketWriter::with_capacity(2 + icons.len() * 2);
            w.write_i8(1);
            w.write_i8(icons.len() as i8);
            for icon in icons { w.write_i16(icon); }
            Ok([w.finish(cmd_message::LUCKY_ROUND), s2c_packets::inventory_bag(&entry.snapshot.items_bag), wallet_money_pkt(&entry.snapshot.wallet)])
        });
        match res {
            Some(Ok(pkts)) => { for p in pkts { session.send(p).await?; } }
            Some(Err(msg)) => session.send(s2c_packets::thong_bao(&msg)?).await?,
            None => {}
        }
        Ok(())
    }

    pub(super) async fn handle_achievement(&self, session: &SessionHandle, sid: u64, claim_idx: Option<i8>) -> Result<(), ProtocolError> {
        if let Some(raw_idx) = claim_idx {
            let Ok(idx) = usize::try_from(raw_idx) else { return Ok(()); };
            if let Some((ack, money)) = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
                let &(ach_id, _, _, ruby_rw, req) = self.shared.templates.achievements.get(idx)?;
                while entry.snapshot.achievements.len() <= idx { entry.snapshot.achievements.push((0, false)); }
                let met = if matches!(ach_id, 1 | 2 | 17) {
                    entry.snapshot.stats.power >= req
                } else {
                    entry.snapshot.achievements.get(idx).map_or(0, |a| a.0) >= req
                };
                if entry.snapshot.achievements[idx].1 || !met { return None; }
                entry.snapshot.achievements[idx].1 = true;
                entry.snapshot.wallet.ruby = entry.snapshot.wallet.ruby.saturating_add(ruby_rw.max(1));
                let mut w = PacketWriter::with_capacity(2);
                w.write_i8(1);
                w.write_u8(idx as u8);
                Some((w.finish(cmd_message::ARCHIVEMENT), wallet_money_pkt(&entry.snapshot.wallet)))
            }) {
                session.send(ack).await?;
                session.send(money).await?;
            }
            return Ok(());
        }
        let Some((pwr, achs)) = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.stats.power, e.snapshot.achievements.clone())) else { return Ok(()); };
        let count = self.shared.templates.achievements.len().min(50);
        if count == 0 { return session.send(s2c_packets::thong_bao("Chưa có dữ liệu thành tích!")?).await; }
        let mut w = PacketWriter::with_capacity(4 + count * 36);
        w.write_i8(0);
        w.write_i8(count as i8);
        for (i, (ach_id, i1, i2, ruby, req)) in self.shared.templates.achievements.iter().take(50).enumerate() {
            let rec = achs.get(i).copied().unwrap_or((0, false));
            let met = if matches!(*ach_id, 1 | 2 | 17) { pwr >= *req } else { rec.0 >= *req };
            w.write_utf(i1)?;
            w.write_utf(i2)?;
            w.write_i16((*ruby).clamp(0, i32::from(i16::MAX)) as i16);
            w.write_bool(met);
            w.write_bool(rec.1);
        }
        session.send(w.finish(cmd_message::ARCHIVEMENT)).await
    }
}
