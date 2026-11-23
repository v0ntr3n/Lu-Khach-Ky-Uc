//! Player/boss hit resolution and C2S attack packet handlers (`handle_attack_mobs`, `handle_attack_players`, `handle_attack_multi`).

use std::sync::Arc;

use nro_boss::EngineEffect;
use nro_core::{Location, PlayerId};
use nro_player::skills::SkillKind;
use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};

use super::super::ServerSessionHandler;
use super::super::router_combat_calc::{
    apply_incoming_hit_to_player, apply_lifesteal, in_range_px, next_combat_rng,
};
use super::super::router_combat_pvp::lock_pvp;
use super::{CastPrep, dedup_take};

impl ServerSessionHandler {
    fn apply_attacker_lifesteal(
        &self,
        attacker_sid: u64,
        loc: Location,
        dealt_dmg: i64,
        drains: (i32, i32, i32),
    ) {
        if dealt_dmg <= 0 || (drains.0 <= 0 && drains.1 <= 0) {
            return;
        }
        let healed = self
            .shared
            .clients
            .authenticated
            .get_mut(&attacker_sid)
            .and_then(|mut atk| {
                if !apply_lifesteal(&mut atk.snapshot.stats, dealt_dmg, false, drains) {
                    return None;
                }
                if let (Some(aek), Ok(tx)) = (
                    atk.entity_key,
                    self.shared
                        .world_router
                        .resolve_sender(loc.map_id, loc.zone_id),
                ) {
                    let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(aek, atk.snapshot.stats));
                }
                Some((atk.handle.clone(), atk.snapshot.stats))
            });
        if let Some((ah, astats)) = healed {
            let _ = ah.try_send(s2c_packets::me_load_point(&astats));
        }
    }

    fn resolve_player_hits(
        &self,
        attacker_sid: u64,
        atk_pid: PlayerId,
        atk_flag: i8,
        loc: Location,
        targets: &[PlayerId],
        cast: &CastPrep,
    ) -> Vec<EngineEffect> {
        let raw_dmg = i64::try_from(cast.pvp_dmg).unwrap_or(i64::MAX);
        let piercing = matches!(
            cast.skill_kind,
            Some(SkillKind::Makankosappo | SkillKind::TuSat)
        );
        let mut boss_effects = Vec::new();
        for &tpid in targets {
            if tpid.0 < 0 {
                let Some(atk_view) = self
                    .shared
                    .clients
                    .authenticated
                    .get(&attacker_sid)
                    .map(|e| Self::active_entry_to_player_view(&e))
                else {
                    continue;
                };
                let mut rng = next_combat_rng(attacker_sid, cast.now_ms);
                let (dealt, mut fx) = {
                    let mut mgr = self.shared.bosses.lock();
                    let Some(bi) = mgr.instances().get(&tpid.0) else {
                        continue;
                    };
                    if bi.map_id != loc.map_id
                        || bi.zone_id != loc.zone_id
                        || !in_range_px(loc, bi.location, cast.max_range_px)
                    {
                        continue;
                    }
                    let before = bi.stats.hp;
                    let fx = mgr.apply_player_hit(
                        &atk_view,
                        tpid.0,
                        raw_dmg,
                        piercing,
                        false,
                        &mut rng,
                        cast.now_ms as u64,
                    );
                    let after = mgr.instances().get(&tpid.0).map_or(0, |b| b.stats.hp);
                    (before.saturating_sub(after).max(0), fx)
                };
                self.apply_attacker_lifesteal(attacker_sid, loc, dealt, cast.attacker_drains);
                boss_effects.append(&mut fx);
                continue;
            }
            if tpid == atk_pid {
                continue;
            }
            let Some(tsid) = self.shared.clients.players.get(&tpid).map(|s| *s.value()) else {
                continue;
            };
            if tsid == attacker_sid {
                continue;
            }
            let pvp_pair_ok = lock_pvp().contains_key(&(atk_pid, tpid))
                || lock_pvp().contains_key(&(tpid, atk_pid));
            let hit_res = self
                .shared
                .clients
                .authenticated
                .get_mut(&tsid)
                .and_then(|mut tgt| {
                    if tgt.snapshot.location.map_id != loc.map_id
                        || tgt.snapshot.location.zone_id != loc.zone_id
                        || tgt.snapshot.stats.hp <= 0
                    {
                        return None;
                    }
                    if !in_range_px(loc, tgt.snapshot.location, cast.max_range_px) {
                        return None;
                    }
                    let flags_hostile = (atk_flag == 8 || tgt.c_flag == 8)
                        || (atk_flag > 0 && tgt.c_flag > 0 && atk_flag != tgt.c_flag);
                    if !pvp_pair_ok && !flags_hostile {
                        return None;
                    }
                    let e = &mut *tgt;
                    let (out, reflected) = apply_incoming_hit_to_player(
                        tsid,
                        tpid,
                        &mut e.snapshot.stats,
                        &e.snapshot.items_body,
                        raw_dmg,
                        cast.skill_kind,
                        cast.attacker_pen,
                        piercing,
                        cast.now_ms,
                    );
                    let final_dmg = if out.dodged {
                        0
                    } else {
                        i64::from(out.damage_taken.max(0))
                    };
                    let _ = tgt
                        .handle
                        .try_send(s2c_packets::me_load_point(&tgt.snapshot.stats));
                    if let (Some(tek), Ok(tx)) = (
                        tgt.entity_key,
                        self.shared
                            .world_router
                            .resolve_sender(loc.map_id, loc.zone_id),
                    ) {
                        let _ =
                            tx.try_send(ZoneCommand::UpdatePlayerStats(tek, tgt.snapshot.stats));
                    }
                    let died = tgt.snapshot.stats.hp <= 0;
                    Some((
                        tgt.handle.clone(),
                        tgt.snapshot.location,
                        tgt.snapshot.stats.hp,
                        tgt.snapshot.stats.power,
                        final_dmg,
                        reflected,
                        died,
                    ))
                });
            if let Some((th, tloc, hp_after, tpower, dealt, reflected, died)) = hit_res {
                if dealt > 0 {
                    super::super::router_skill::clear_ttnl(tpid);
                }
                self.broadcast_zone_or_self(
                    loc,
                    attacker_sid,
                    &s2c_packets::have_attack_player(tpid, hp_after, dealt, cast.is_crit, -1),
                );
                self.apply_attacker_lifesteal(attacker_sid, loc, dealt, cast.attacker_drains);
                if reflected > 0 {
                    boss_effects.push(EngineEffect::DamagePlayer {
                        target: atk_pid,
                        damage: reflected,
                        piercing: true,
                    });
                }
                if let Some(sk) = cast.skill_kind {
                    let (flags, durs) = nro_player::skills::cc_durations_ms(sk, cast.skill_point);
                    if flags.contains(nro_player::skills::CcFlags::BLIND_DCTT) && durs[1] > 0 {
                        let _ = self.shared.clients.authenticated.get(&tsid).map(|e| {
                            super::super::router_skill_effects::apply_boss_debuff_to_player(
                                tpid,
                                nro_boss::PlayerDebuffKind::Stun,
                                u64::from(durs[1]),
                                &e.snapshot.items_body,
                                cast.now_ms,
                            )
                        });
                    }
                }
                if died {
                    self.handle_player_death_cleanup_with_killer(tpid, tloc, tsid, Some(atk_pid));
                    let (me_die, peer_die) = s2c_packets::char_die_packets(tpid, 0, tloc, tpower);
                    let _ = th.try_send(me_die);
                    self.broadcast_to_zone_peers(tloc, Some(tsid), &peer_die);
                }
            }
        }
        boss_effects
    }

    pub(in crate::router) async fn handle_attack_mobs(
        &self,
        session: &SessionHandle,
        sid: u64,
        mobs: &[u8],
    ) -> Result<(), ProtocolError> {
        let mut revert_pkt = None;
        let prep = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                let ek = entry.entity_key?;
                let cast = self.prepare_attack_cast(&mut entry, sid, &mut revert_pkt)?;
                Some((ek, entry.snapshot.location, cast))
            });
        if let Some((rloc, rpkt)) = revert_pkt {
            self.broadcast_zone_or_self(rloc, sid, &rpkt);
        }
        if let Some((ek, loc, cast)) = prep {
            session.send(s2c_packets::me_load_mp(cast.new_mp)).await?;
            let valid = self.filter_mobs_in_range(loc, mobs, &cast);
            self.resolve_mob_hits(session, sid, ek, loc, valid, &cast)
                .await?;
        }
        Ok(())
    }

    pub(in crate::router) fn handle_attack_players(&self, sid: u64, targets: &[PlayerId]) {
        if targets.is_empty() {
            return;
        }
        let mut revert_pkt = None;
        let prep = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                let cast = self.prepare_attack_cast(&mut entry, sid, &mut revert_pkt)?;
                let _ = entry.handle.try_send(s2c_packets::me_load_mp(cast.new_mp));
                Some((
                    entry.snapshot.id,
                    entry.c_flag,
                    entry.snapshot.location,
                    cast,
                ))
            });
        if let Some((rloc, rpkt)) = revert_pkt {
            self.broadcast_zone_or_self(rloc, sid, &rpkt);
        }
        if let Some((atk_pid, cur_flag, loc, cast)) = prep {
            let valid = dedup_take(targets.iter().copied(), cast.max_fight);
            let effects = self.resolve_player_hits(sid, atk_pid, cur_flag, loc, &valid, &cast);
            if !effects.is_empty() {
                let shared = Arc::clone(&self.shared);
                tokio::spawn(async move {
                    shared.dispatch_boss_effects(effects).await;
                });
            }
        }
    }

    pub(in crate::router) async fn handle_attack_multi(
        &self,
        session: &SessionHandle,
        sid: u64,
        mobs: &[u8],
        players: &[PlayerId],
    ) -> Result<(), ProtocolError> {
        let mut revert_pkt = None;
        let prep = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                let ek = entry.entity_key?;
                let cast = self.prepare_attack_cast(&mut entry, sid, &mut revert_pkt)?;
                Some((
                    ek,
                    entry.snapshot.id,
                    entry.c_flag,
                    entry.snapshot.location,
                    cast,
                ))
            });
        if let Some((rloc, rpkt)) = revert_pkt {
            self.broadcast_zone_or_self(rloc, sid, &rpkt);
        }
        if let Some((ek, atk_pid, cur_flag, loc, cast)) = prep {
            session.send(s2c_packets::me_load_mp(cast.new_mp)).await?;
            let valid_mobs = self.filter_mobs_in_range(loc, mobs, &cast);
            let remaining = cast.max_fight.saturating_sub(valid_mobs.len());
            self.resolve_mob_hits(session, sid, ek, loc, valid_mobs, &cast)
                .await?;
            if remaining > 0 && !players.is_empty() {
                let effects = self.resolve_player_hits(
                    sid,
                    atk_pid,
                    cur_flag,
                    loc,
                    &dedup_take(players.iter().copied(), remaining),
                    &cast,
                );
                if !effects.is_empty() {
                    self.shared.dispatch_boss_effects(effects).await;
                }
            }
        }
        Ok(())
    }
}
