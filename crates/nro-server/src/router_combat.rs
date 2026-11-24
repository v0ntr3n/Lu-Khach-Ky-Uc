//! Core combat cast preparation, TNSM scaling, and mob hit resolution.

#[path = "router_combat_hits.rs"]
mod router_combat_hits;
#[path = "router_combat_lifecycle.rs"]
mod router_combat_lifecycle;
#[path = "router_combat_pickup.rs"]
mod router_combat_pickup;

use nro_boss::PlayerView;
use nro_core::{EntityKey, Location, MapId};
use nro_player::{combat::TnsmContext, npoint::NPoint, skills::SkillKind};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};
use tokio::sync::oneshot;

use super::ServerSessionHandler;
use super::router_combat_calc::{
    apply_lifesteal, attacker_pen_and_lifesteal, compute_attack_damage, in_range_px,
    skill_max_range_px,
};
use crate::runtime::{LeaderboardSlot, unix_now_ms};
use crate::state::ActivePlayerEntry;

#[inline]
pub(super) const fn is_offline_map(map_id: MapId) -> bool {
    matches!(map_id.0, 21..=23 | 39..=41 | 45..=50)
}

pub(self) fn dedup_take<T: Copy + PartialEq>(
    items: impl IntoIterator<Item = T>,
    limit: usize,
) -> Vec<T> {
    let mut out = Vec::with_capacity(limit.min(16));
    for item in items {
        if !out.contains(&item) {
            out.push(item);
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}

pub(self) struct CastPrep {
    pub(self) skill_kind: Option<SkillKind>,
    pub(self) skill_point: u8,
    pub(self) skill_paint_id: u8,
    pub(self) mob_dmg: u64,
    pub(self) pvp_dmg: u64,
    pub(self) is_crit: bool,
    pub(self) new_mp: i64,
    pub(self) now_ms: i64,
    pub(self) max_fight: usize,
    pub(self) max_range_px: i64,
    pub(self) attacker_pen: (i32, i32, i32),
    pub(self) attacker_drains: (i32, i32, i32),
}

impl ServerSessionHandler {
    #[inline]
    pub(crate) const fn is_offline_map_id(map_id: MapId) -> bool {
        is_offline_map(map_id)
    }

    pub(crate) fn active_entry_to_player_view(entry: &ActivePlayerEntry) -> PlayerView {
        let s = &entry.snapshot;
        PlayerView {
            id: s.id,
            clan_id: (s.clan_id.0 >= 0).then_some(s.clan_id),
            gender: s.gender,
            location: s.location,
            stats: s.stats,
            power: s.stats.power,
            flag: u8::try_from(entry.c_flag.max(0)).unwrap_or(0),
            gold: u64::try_from(s.wallet.gold.max(0)).unwrap_or(0),
            has_pet: s.pet.is_some(),
            carrying_namek_ball: false,
            active_task: (
                u16::try_from(s.main_task.task_id.max(0)).unwrap_or(0),
                u8::try_from(s.main_task.sub_index.max(0)).unwrap_or(0),
            ),
            whis_top_level: u32::try_from(s.thachdauwhis.max(0)).unwrap_or(0),
            last_chat: None,
        }
    }

    pub(crate) fn compute_player_tnsm(
        &self,
        entry: &ActivePlayerEntry,
        base_tnsm: u64,
        now_ms: i64,
    ) -> u64 {
        let mut np = NPoint::new(entry.snapshot.stats);
        for opt in entry
            .snapshot
            .items_body
            .iter()
            .flatten()
            .flat_map(|it| it.options.iter())
        {
            if matches!(opt.id, 88 | 101 | 156) {
                np.mods.tl_tnsm.push(opt.param);
            }
        }
        let (charms, intr) = (&entry.snapshot.charms, &entry.snapshot.intrinsic);
        let ctx = TnsmContext {
            charms_tri_tue: (
                charms.td_tri_tue_ms > now_ms,
                charms.td_tri_tue3_ms > now_ms,
                charms.td_tri_tue4_ms > now_ms,
            ),
            vip_active: (
                entry.snapshot.vip_state.1 > now_ms,
                entry.snapshot.vip_state.2,
            ),
            bonus_flags: (false, false, false, false),
            intrinsic: (
                u8::try_from(intr.intrinsic_id.max(0)).unwrap_or(0),
                i32::from(intr.param1),
            ),
            pet_master_bonus: None,
            world_bonus: (
                matches!(entry.snapshot.location.map_id.0, 135..=138),
                u8::try_from(entry.c_flag.max(0)).unwrap_or(0),
                self.shared.config.rates.exp_multiplier,
            ),
            parity: self.shared.config.parity_mode,
        };
        np.cal_suc_manh_tiem_nang(base_tnsm, &ctx)
    }

    pub(self) fn prepare_attack_cast(
        &self,
        entry: &mut ActivePlayerEntry,
        sid: u64,
        revert_pkt: &mut Option<(Location, [Option<Packet>; 2])>,
    ) -> Option<CastPrep> {
        let (sel, now_ms, pid, loc) = (
            entry.selected_skill,
            unix_now_ms(),
            entry.snapshot.id,
            entry.snapshot.location,
        );
        super::router_skill::clear_ttnl(pid);
        let pruned = super::router_economy::prune_expired_session_buffs(sid, now_ms)
            | super::router_skill::prune_expired_skill_buffs(pid, now_ms);
        if pruned {
            super::recalculate_session_stats(
                sid,
                &mut entry.snapshot,
                self.shared.config.parity_mode,
            );
            if let (Some(ek), Ok(tx)) = (
                entry.entity_key,
                self.shared
                    .world_router
                    .resolve_sender(loc.map_id, loc.zone_id),
            ) {
                let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
            }
            let _ = entry
                .handle
                .try_send(s2c_packets::me_load_point(&entry.snapshot.stats));
        }
        if super::router_skill::take_monkey_revert_pending(pid) {
            *revert_pkt = Some((loc, self.revert_monkey_appearance_entry(entry, loc)));
        }
        if !matches!(sel, 0..=5 | 9..=11 | 17 | 20 | 24..=26) {
            return None;
        }
        let sk_idx = entry
            .snapshot
            .skills
            .iter()
            .position(|sk| sk.template_id == i32::from(sel) && sk.point > 0)?;
        let st = self
            .shared
            .templates
            .skills
            .iter()
            .find(|s| i16::from(s.id) == sel)?;
        let pt = entry.snapshot.skills[sk_idx].point;
        let lv = st.levels.iter().find(|l| l.point == pt)?;
        if now_ms.saturating_sub(entry.snapshot.skills[sk_idx].last_time_use_ms)
            < i64::from(lv.cool_down.max(0)).saturating_sub(50)
        {
            return None;
        }
        let hp_cost = if sel == 9 {
            (entry.snapshot.stats.hp_max.max(0) / 10).max(1)
        } else {
            0
        };
        if hp_cost > 0 && entry.snapshot.stats.hp <= hp_cost {
            return None;
        }
        let mana_cost = match st.mana_use_type {
            2 => {
                if entry.snapshot.stats.mp <= 0 {
                    return None;
                }
                entry.snapshot.stats.mp
            }
            1 => (entry.snapshot.stats.mp_max.max(0) * i64::from(lv.mana_use.max(0))) / 100,
            _ => i64::from(lv.mana_use.max(0)),
        };
        if entry.snapshot.stats.mp < mana_cost {
            return None;
        }
        if hp_cost > 0 {
            entry.snapshot.stats.hp = entry.snapshot.stats.hp.saturating_sub(hp_cost);
            let _ = entry
                .handle
                .try_send(s2c_packets::me_load_hp(entry.snapshot.stats.hp));
        }
        let new_mp = entry.snapshot.stats.mp.saturating_sub(mana_cost).max(0);
        entry.snapshot.stats.mp = new_mp;
        entry.snapshot.skills[sk_idx].last_time_use_ms = now_ms;
        let (skill_kind, skill_point) = (
            SkillKind::from_repr(sel as u8),
            u8::try_from(pt.clamp(1, 7)).unwrap_or(1),
        );
        let skill_paint_id = u8::try_from(lv.id.clamp(0, 255)).unwrap_or(0);
        let (mob_dmg, pvp_dmg, is_crit) = compute_attack_damage(
            entry,
            sid,
            skill_kind.unwrap_or(SkillKind::Dragon),
            i32::from(lv.damage.max(100)),
            skill_point,
            now_ms,
            self.shared.config.parity_mode,
        );
        let (attacker_pen, attacker_drains) =
            attacker_pen_and_lifesteal(&entry.snapshot.items_body);
        Some(CastPrep {
            skill_kind,
            skill_point,
            skill_paint_id,
            mob_dmg,
            pvp_dmg,
            is_crit,
            new_mp,
            now_ms,
            max_fight: usize::try_from(lv.max_fight.max(1)).unwrap_or(1),
            max_range_px: skill_max_range_px(skill_kind, skill_point),
            attacker_pen,
            attacker_drains,
        })
    }

    pub(self) fn filter_mobs_in_range(
        &self,
        loc: Location,
        raw_mobs: &[u8],
        cast: &CastPrep,
    ) -> Vec<u8> {
        let Some(map_tpl) = self
            .shared
            .templates
            .maps
            .iter()
            .find(|m| m.id == loc.map_id)
        else {
            return dedup_take(raw_mobs.iter().copied(), cast.max_fight);
        };
        let in_range = raw_mobs.iter().copied().filter(|&slot| {
            map_tpl
                .mobs
                .get(usize::from(slot))
                .is_some_and(|&(_, _, _, mx, my)| {
                    in_range_px(
                        loc,
                        Location {
                            map_id: loc.map_id,
                            zone_id: loc.zone_id,
                            x: mx,
                            y: my,
                        },
                        cast.max_range_px,
                    )
                })
        });
        dedup_take(in_range, cast.max_fight)
    }

    pub(self) async fn resolve_mob_hits(
        &self,
        session: &SessionHandle,
        sid: u64,
        ek: EntityKey,
        loc: Location,
        valid_mobs: Vec<u8>,
        cast: &CastPrep,
    ) -> Result<(), ProtocolError> {
        if valid_mobs.is_empty() {
            return Ok(());
        }
        let Ok(zone_tx) = self
            .shared
            .world_router
            .resolve_sender(loc.map_id, loc.zone_id)
        else {
            return Ok(());
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        let cmd = ZoneCommand::AttackMobs {
            attacker: ek,
            skill_paint_id: cast.skill_paint_id,
            mob_ids: valid_mobs,
            damage: cast.mob_dmg,
            is_crit: cast.is_crit,
            reply: reply_tx,
        };
        if zone_tx.send(cmd).await.is_ok()
            && let Ok((tnsm, killed_mobs)) = reply_rx.await
        {
            if tnsm > 0 {
                let now_ms = unix_now_ms();
                let res = self
                    .shared
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
                        let drained = apply_lifesteal(
                            &mut entry.snapshot.stats,
                            i64::try_from(cast.mob_dmg).unwrap_or(i64::MAX),
                            true,
                            cast.attacker_drains,
                        );
                        let _ = zone_tx
                            .try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                        let exp = i32::try_from(
                            total_tnsm.min(u64::try_from(i32::MAX).unwrap_or(u64::MAX)),
                        )
                        .unwrap_or(i32::MAX);
                        (exp, drained.then_some(entry.snapshot.stats))
                    });
                if let Some((exp, drained_stats)) = res {
                    self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                    session.send(s2c_packets::player_up_exp(2, exp)).await?;
                    if let Some(st) = drained_stats {
                        session.send(s2c_packets::me_load_point(&st)).await?;
                    }
                    self.advance_task_on_power(session, sid).await?;
                }
            }
            for mob_tid in killed_mobs {
                self.advance_task_on_mob_kill(session, sid, mob_tid).await?;
            }
        }
        Ok(())
    }
}
