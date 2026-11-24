//! `TuSat` AoE resolution and ally `Trị Thương` / `Huýt Sáo` propagation for `SKILL_NOT_FOCUS`.

use nro_core::{CoreStats, EntityKey, Location, PlayerId};
use nro_player::skills::{SkillKind, apply_tri_thuong_target, utility_skill_params};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};
use tokio::sync::oneshot;

use super::super::ServerSessionHandler;
use super::router_skill_state::{apply_huyt_sao_to_snapshot, stats_to_npoint};
use crate::runtime::{ActivePlayerEntry, LeaderboardSlot, unix_now_ms};

impl ServerSessionHandler {
    pub(super) fn prepare_tu_sat_aoe(
        &self,
        entry: &mut ActivePlayerEntry,
        pt_u8: u8,
        skill_paint_id: i16,
    ) -> Option<(EntityKey, u8, Vec<u8>, u64)> {
        let caster_hp = entry.snapshot.stats.hp.max(1);
        let loc = entry.snapshot.location;
        let range_px = i64::from(utility_skill_params(pt_u8).3);
        let mob_ids: Vec<u8> = self
            .shared
            .templates
            .maps
            .iter()
            .find(|m| m.id == loc.map_id)
            .map_or_default(|m| {
                m.mobs
                    .iter()
                    .enumerate()
                    .take(127)
                    .filter_map(|(idx, &(_, _, _, mx, my))| {
                        let dx = i64::from(mx) - i64::from(loc.x);
                        let dy = i64::from(my) - i64::from(loc.y);
                        (dx * dx + dy * dy <= range_px * range_px).then_some(idx as u8)
                    })
                    .collect()
            });
        entry.snapshot.stats.hp = 0;
        let ek = entry.entity_key?;
        if mob_ids.is_empty() {
            return None;
        }
        let paint_u8 = u8::try_from(skill_paint_id.clamp(0, 255)).unwrap_or(0);
        Some((ek, paint_u8, mob_ids, u64::try_from(caster_hp).unwrap_or(1)))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn finish_skill_not_focus(
        &self,
        session: &SessionHandle,
        sid: u64,
        status: i8,
        sel: i16,
        pid_obj: PlayerId,
        loc: Location,
        stats: CoreStats,
        skill_pkt: Packet,
        monkey_pkt: Option<Packet>,
        pruned_sync: Option<(Location, Option<CoreStats>, Option<[Option<Packet>; 2]>)>,
        tu_sat_aoe: Option<(EntityKey, u8, Vec<u8>, u64)>,
        ally_effect: Option<(i16, u8, i8, i64)>,
        cc_cast: Option<(u8, i8, i64)>,
    ) -> Result<(), ProtocolError> {
        let tu_sat_died = status == 7 && sel == 14;
        if tu_sat_died {
            self.handle_player_death_cleanup(pid_obj, loc, sid);
        }
        if let Some((pt_u8, c_flag, now_ms)) = cc_cast {
            self.apply_cast_cc_targets(sid, pid_obj, c_flag, loc, sel, pt_u8, now_ms);
        }
        self.broadcast_zone_or_self(loc, sid, &skill_pkt);
        if let Some(mpkt) = monkey_pkt {
            self.broadcast_zone_or_self(loc, sid, &mpkt);
        } else if let Some((ploc, _, Some(rpkts))) = pruned_sync {
            for p in rpkts.into_iter().flatten() {
                self.broadcast_zone_or_self(ploc, sid, &p);
            }
        }
        if let Some((ek, paint_u8, mob_ids, dmg)) = tu_sat_aoe
            && let Ok(zone_tx) = self
                .shared
                .world_router
                .resolve_sender(loc.map_id, loc.zone_id)
        {
            let (reply_tx, reply_rx) = oneshot::channel();
            let cmd = ZoneCommand::AttackMobs {
                attacker: ek,
                skill_paint_id: paint_u8,
                mob_ids,
                damage: dmg,
                is_crit: false,
                reply: reply_tx,
            };
            if zone_tx.send(cmd).await.is_ok()
                && let Ok((tnsm, killed_mobs)) = reply_rx.await
            {
                if tnsm > 0 {
                    let now_ms = unix_now_ms();
                    let wire_exp =
                        self.shared
                            .clients
                            .authenticated
                            .get_mut(&sid)
                            .map(|mut entry| {
                                let total_tnsm = self.compute_player_tnsm(&entry, tnsm, now_ms);
                                let gain = i64::try_from(total_tnsm).unwrap_or(i64::MAX);
                                entry.snapshot.stats.power =
                                    entry.snapshot.stats.power.saturating_add(gain);
                                entry.snapshot.stats.tiem_nang =
                                    entry.snapshot.stats.tiem_nang.saturating_add(gain);
                                let _ = zone_tx.try_send(ZoneCommand::UpdatePlayerStats(
                                    ek,
                                    entry.snapshot.stats,
                                ));
                                i32::try_from(
                                    total_tnsm.min(u64::try_from(i32::MAX).unwrap_or(u64::MAX)),
                                )
                                .unwrap_or(i32::MAX)
                            });
                    if let Some(exp) = wire_exp {
                        self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                        session.send(s2c_packets::player_up_exp(2, exp)).await?;
                        self.advance_task_on_power(session, sid).await?;
                    }
                }
                for mob_tid in killed_mobs {
                    self.advance_task_on_mob_kill(session, sid, mob_tid).await?;
                }
            }
        }
        if let Some((eff_sel, pt_u8, caster_flag, now_ms)) = ally_effect
            && !super::super::router_combat::is_offline_map(loc.map_id)
        {
            self.apply_ally_skill_targets(sid, loc, eff_sel, pt_u8, caster_flag, now_ms);
        }
        let stats = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .map_or(stats, |e| e.snapshot.stats);
        session.send(s2c_packets::me_load_point(&stats)).await?;
        if matches!(status, 2 | 6 | 7) || matches!(sel, 7 | 21) {
            session.send(s2c_packets::me_load_hp(stats.hp)).await?;
            session.send(s2c_packets::me_load_mp(stats.mp)).await?;
        }
        if tu_sat_died {
            let (me_die, peer_die) = s2c_packets::char_die_packets(pid_obj, 0, loc, stats.power);
            session.send(me_die).await?;
            self.broadcast_to_zone_peers(loc, Some(sid), &peer_die);
        }
        Ok(())
    }

    fn apply_ally_skill_targets(
        &self,
        sid: u64,
        loc: Location,
        eff_sel: i16,
        pt_u8: u8,
        caster_flag: i8,
        now_ms: i64,
    ) {
        let ally_range = super::super::router_combat_calc::skill_max_range_px(
            SkillKind::from_repr(eff_sel as u8),
            pt_u8,
        );
        let ally_sids: Vec<u64> = self
            .shared
            .clients
            .authenticated
            .iter()
            .filter_map(|e| {
                let (a_sid, aloc) = (*e.key(), e.snapshot.location);
                let same_flag = (caster_flag == 0 && e.c_flag == 0)
                    || (caster_flag > 0 && caster_flag == e.c_flag && caster_flag != 8);
                (a_sid != sid
                    && aloc.map_id == loc.map_id
                    && aloc.zone_id == loc.zone_id
                    && same_flag
                    && super::super::router_combat_calc::in_range_px(loc, aloc, ally_range))
                .then_some(a_sid)
            })
            .collect();
        for a_sid in ally_sids {
            let upd = self
                .shared
                .clients
                .authenticated
                .get_mut(&a_sid)
                .and_then(|mut ally| {
                    let aloc = ally.snapshot.location;
                    if aloc.map_id != loc.map_id
                        || aloc.zone_id != loc.zone_id
                        || !super::super::router_combat_calc::in_range_px(loc, aloc, ally_range)
                    {
                        return None;
                    }
                    let (mut was_dead, mut ally_monkey_revert) = (false, None);
                    if eff_sel == 7 {
                        was_dead = ally.snapshot.stats.hp <= 0;
                        let mut np = stats_to_npoint(ally.snapshot.stats);
                        apply_tri_thuong_target(&mut np, pt_u8);
                        ally.snapshot.stats.hp = i64::from(np.hp).min(ally.snapshot.stats.hp_max);
                        ally.snapshot.stats.mp = i64::from(np.mp).min(ally.snapshot.stats.mp_max);
                    } else if eff_sel == 21
                        && ally.snapshot.stats.hp > 0
                        && apply_huyt_sao_to_snapshot(
                            a_sid,
                            &mut ally.snapshot,
                            pt_u8,
                            now_ms,
                            self.shared.config.parity_mode,
                        )
                    {
                        ally_monkey_revert = Some(self.revert_monkey_appearance_entry(&ally, loc));
                    }
                    if let (Some(aek), Ok(tx)) = (
                        ally.entity_key,
                        self.shared
                            .world_router
                            .resolve_sender(loc.map_id, loc.zone_id),
                    ) {
                        let _ =
                            tx.try_send(ZoneCommand::UpdatePlayerStats(aek, ally.snapshot.stats));
                    }
                    Some((
                        ally.handle.clone(),
                        ally.snapshot.id,
                        ally.snapshot.location,
                        ally.snapshot.stats,
                        was_dead,
                        ally_monkey_revert,
                    ))
                });
            if let Some((ah, apid, aloc, astats, was_dead, ally_monkey_revert)) = upd {
                if let Some(rpkts) = ally_monkey_revert {
                    for p in rpkts.into_iter().flatten() {
                        self.broadcast_zone_or_self(aloc, a_sid, &p);
                    }
                }
                if was_dead {
                    let (me_live_pkt, peer_live_pkt) =
                        s2c_packets::char_revive_packets(apid, astats.hp, astats.hp_max, aloc);
                    let _ = ah.try_send(me_live_pkt);
                    self.broadcast_to_zone_peers(aloc, Some(a_sid), &peer_live_pkt);
                }
                let _ = ah.try_send(s2c_packets::me_load_point(&astats));
                let _ = ah.try_send(s2c_packets::me_load_hp(astats.hp));
                if eff_sel == 7 {
                    let _ = ah.try_send(s2c_packets::me_load_mp(astats.mp));
                }
            }
        }
    }
}
