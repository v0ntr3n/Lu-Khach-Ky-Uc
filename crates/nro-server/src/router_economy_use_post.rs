//! Post-use radar card updates, Pea/Porata preconditions, broadcast assembly, and world/session packet dispatch.

use nro_consts::{cmd_message, const_player};
use nro_core::{TemplateId, ZoneId};
use nro_db::{
    StrNum,
    json_columns::{RadarCardOption, RadarCardRecord},
    player_repo::PlayerSnapshot,
};
use nro_economy::TimedBuffState;
use nro_protocol::{Packet, PacketWriter, ProtocolError, SessionHandle};
use nro_world::{GroundDropKind, ZoneCommand, s2c_packets};

use super::super::{
    ServerSessionHandler, effective_appearance, is_own_home_map, pkt_fusion,
    recalculate_session_stats, send_thong_bao, sprite_restore_packets, wallet_money_pkt,
};
use super::router_economy_use_fx::UseItemPostState;
use crate::data_game_tpl;
use crate::runtime::{ActivePlayerEntry, unix_now_ms};

pub(super) fn check_pea_or_porata_precond(
    snap: &PlayerSnapshot,
    item_tid: TemplateId,
    is_pea: bool,
    prev_fusion: i8,
    now_ms: i64,
) -> Option<&'static str> {
    if is_pea && snap.stats.hp > 0 {
        let st = &snap.stats;
        let master_full =
            st.hp >= st.hp_max && st.mp >= st.mp_max && st.stamina >= st.max_stamina.max(1_000);
        let pet_full = snap.pet.as_ref().is_none_or(|p| {
            p.fusion_type != const_player::NON_FUSION
                || (p.stats.hp > 0
                    && p.stats.hp >= p.stats.hp_max
                    && p.stats.mp >= p.stats.mp_max
                    && p.stats.stamina >= p.stats.max_stamina.max(1_000))
        });
        if master_full && pet_full {
            return Some("HP, KI và Thể lực đã đầy!");
        }
    }
    if matches!(item_tid.0, 454 | 921 | 1819)
        && prev_fusion == const_player::NON_FUSION
        && super::super::router_secondary::is_pet_fusion_on_cooldown(snap.id, now_ms)
    {
        return Some("Vui lòng đợi 5 giây trước khi hợp thể lại!");
    }
    None
}

impl ServerSessionHandler {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn collect_use_item_bcasts_and_base_pkts(
        &self,
        entry: &ActivePlayerEntry,
        item_tid: TemplateId,
        now_ms: i64,
        buffs: &TimedBuffState,
        prev_buffs: &TimedBuffState,
        prev_stamina: u16,
        prev_app: (i16, i16, i16, i16, bool),
        prev_fusion: i8,
        cur_fusion: i8,
        post: &mut UseItemPostState,
    ) -> [Packet; 4] {
        if buffs != prev_buffs {
            let icon_id = self
                .shared
                .templates
                .items
                .iter()
                .find(|r| r.id == i32::from(item_tid.0))
                .map_or(0_i16, |r| r.icon_id as i16);
            for &(slot, exp) in &buffs.active {
                if exp > now_ms
                    && !prev_buffs
                        .active
                        .iter()
                        .any(|&(ps, pe)| ps == slot && pe == exp)
                {
                    let mut w = PacketWriter::with_capacity(4);
                    w.write_i16(icon_id);
                    w.write_i16(((exp - now_ms) / 1_000).clamp(0, i64::from(i16::MAX)) as i16);
                    post.extra_pkts.push(w.finish(cmd_message::ITEM_TIME));
                }
            }
        }
        if entry.snapshot.stats.stamina != prev_stamina {
            post.extra_pkts.push(s2c_packets::stamina(
                i16::try_from(entry.snapshot.stats.stamina).unwrap_or(i16::MAX),
            ));
        }
        let (eh, eb, el, ebag, is_disg) =
            effective_appearance(&self.shared.templates, &entry.snapshot);
        let (pid, loc) = (entry.snapshot.id, entry.snapshot.location);
        let mut bcasts = Vec::new();
        if cur_fusion != prev_fusion {
            let wire_fusion = match cur_fusion {
                const_player::NON_FUSION => 1_i8,
                const_player::HOP_THE_PORATA
                | const_player::HOP_THE_PORATA2
                | const_player::HOP_THE_PORATA3 => 6_i8,
                other => other,
            };
            bcasts.push(pkt_fusion(pid, wire_fusion));
        }
        if cur_fusion != prev_fusion
            || (eh, eb, el, is_disg) != (prev_app.0, prev_app.1, prev_app.2, prev_app.4)
        {
            bcasts.extend(
                sprite_restore_packets(pid, eh, eb, el, is_disg)
                    .into_iter()
                    .flatten(),
            );
        }
        if ebag != prev_app.3 {
            bcasts.push(s2c_packets::send_flag_bag(pid, ebag));
        }
        if !bcasts.is_empty() {
            post.zone_bcasts = Some((loc, bcasts));
        }
        if let (Some(ek), Ok(tx)) = (
            entry.entity_key,
            self.shared
                .world_router
                .resolve_sender(loc.map_id, loc.zone_id),
        ) {
            let _ = tx.try_send(ZoneCommand::UpdatePlayerParts(ek, (eh, eb, el)));
            let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
        }
        [
            s2c_packets::inventory_body(eh, &entry.snapshot.items_body),
            s2c_packets::inventory_bag(&entry.snapshot.items_bag),
            s2c_packets::me_load_point(&entry.snapshot.stats),
            wallet_money_pkt(&entry.snapshot.wallet),
        ]
    }

    pub(super) fn apply_radar_card_update(
        &self,
        sid: u64,
        snap: &mut PlayerSnapshot,
        tid: TemplateId,
        post: &mut UseItemPostState,
    ) {
        let card_upd = if let Some(card) = snap.radar_cards.iter_mut().find(|c| c.id.0 == tid.0) {
            let mut leveled = false;
            card.amount.0 = card.amount.0.saturating_add(1);
            if card.max.0 > 0 && card.amount.0 >= card.max.0 {
                card.level.0 = card.level.0.saturating_add(1);
                card.amount.0 = 0;
                leveled = true;
            }
            Some((
                card.amount.0,
                card.max.0,
                card.level.0,
                leveled,
                card.used.0 != 0,
            ))
        } else if let Some(tpl) = self.shared.templates.radars.iter().find(|r| r.id == tid.0) {
            let opts = tpl
                .options
                .iter()
                .map(|o| RadarCardOption {
                    id: StrNum(o.id),
                    active: StrNum(o.active_card),
                    param: StrNum(i32::from(o.param)),
                })
                .collect();
            let (amt, lvl) = if tpl.max <= 1 { (0, 1) } else { (1, 0) };
            snap.radar_cards.push(RadarCardRecord {
                id: StrNum(tid.0),
                amount: StrNum(amt),
                max: StrNum(tpl.max),
                option: opts,
                level: StrNum(lvl),
                used: StrNum(0),
            });
            Some((amt, tpl.max, lvl, lvl > 0, false))
        } else {
            None
        };
        if let Some((amt, max, lvl, leveled, used)) = card_upd {
            let mut w3 = PacketWriter::with_capacity(5);
            w3.write_i8(3);
            w3.write_i16(tid.0);
            w3.write_i8(amt);
            w3.write_i8(max);
            post.extra_pkts.push(w3.finish(127));
            if leveled {
                let mut w2 = PacketWriter::with_capacity(4);
                w2.write_i8(2);
                w2.write_i16(tid.0);
                w2.write_i8(lvl);
                post.extra_pkts.push(w2.finish(127));
            }
            if used {
                recalculate_session_stats(sid, snap, self.shared.config.parity_mode);
            }
        }
        if let Ok(tb) = s2c_packets::thong_bao("Đã cập nhật thẻ sưu tầm!") {
            post.extra_pkts.push(tb);
        }
    }

    pub(super) async fn finish_use_item_post(
        &self,
        session: &SessionHandle,
        sid: u64,
        packets: Option<[Packet; 4]>,
        post: UseItemPostState,
    ) -> Result<(), ProtocolError> {
        if let Some(base_pkts) = packets {
            for p in base_pkts {
                session.send(p).await?;
            }
            for p in post.extra_pkts {
                session.send(p).await?;
            }
        }
        let now_ms = unix_now_ms();
        if post.refresh_home_egg
            && let Some((snap, ek)) =
                self.shared
                    .clients
                    .authenticated
                    .get_mut(&sid)
                    .and_then(|mut e| {
                        is_own_home_map(e.snapshot.location.map_id, e.snapshot.gender).then(|| {
                            super::super::router_world::tick_magic_tree(&mut e.snapshot, now_ms);
                            (e.snapshot.clone(), e.entity_key)
                        })
                    })
        {
            session
                .send(data_game_tpl::pkt_map_info(
                    &self.shared.assets,
                    &self.shared.templates,
                    &snap,
                ))
                .await?;
            session
                .send(super::super::router_world::build_magic_tree_load(&snap)?)
                .await?;
            for egg_pkt in crate::router_pet::build_home_egg_pkts(&snap, now_ms) {
                session.send(egg_pkt).await?;
            }
            if let (Some(ek), Ok(tx)) = (
                ek,
                self.shared
                    .world_router
                    .resolve_sender(snap.location.map_id, snap.location.zone_id),
            ) {
                let _ = tx.send(ZoneCommand::SyncZoneState(ek)).await;
            }
        }
        if let Some(pid) = post.pet_sync_pid {
            crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
        }
        if let Some((tid, loc, pid, clan_id)) = post.satellite_drop
            && let Ok(tx) = self
                .shared
                .world_router
                .resolve_sender(loc.map_id, loc.zone_id)
        {
            let _ = tx
                .send(ZoneCommand::DropItem(
                    tid,
                    1,
                    loc,
                    Some(pid),
                    GroundDropKind::Satellite { clan_id },
                ))
                .await;
        }
        if post.power_gained {
            self.advance_task_on_power(session, sid).await?;
        }
        if let Some((loc, bcasts)) = post.zone_bcasts {
            for p in bcasts {
                self.broadcast_zone_or_self(loc, sid, &p);
            }
        }
        if let Some(msg) = post.err_msg {
            send_thong_bao(session, &msg)?;
        }
        if let Some((pid, loc, power)) = post.died_info {
            self.handle_player_death_cleanup(pid, loc, sid);
            let (me_die, peer_die) = s2c_packets::char_die_packets(pid, 0, loc, power);
            session.send(me_die).await?;
            self.broadcast_to_zone_peers(loc, Some(sid), &peer_die);
        }
        if let Some(dst_map) = post.teleport_target {
            self.switch_player_zone(session, sid, Some(dst_map), ZoneId(0), None)
                .await?;
        }
        Ok(())
    }
}
