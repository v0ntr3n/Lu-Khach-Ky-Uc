//! Non-targeted skill handler (`cmd -45`, `SKILL_NOT_FOCUS`) matching `Controller.cs:2071-2350`.

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_consts::cmd_message;
use nro_core::PlayerId;
use nro_protocol::{Packet, PacketWriter, ProtocolError, SessionHandle, SkillSpecialArgs};
use nro_world::{ZoneCommand, s2c_packets};

use super::{ServerSessionHandler, send_thong_bao};
use crate::runtime::unix_now_ms;

const MONKEY_HEADS: [i16; 7] = [192, 195, 196, 199, 197, 200, 198];
const MAX_TTNL_TICKS: u8 = 10;

static TTNL_STATE: OnceLock<DashMap<PlayerId, (i64, u8)>> = OnceLock::new();
fn ttnl_state() -> &'static DashMap<PlayerId, (i64, u8)> {
    TTNL_STATE.get_or_init(DashMap::new)
}

static SHIELD_UNTIL: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
fn shield_until() -> &'static DashMap<PlayerId, i64> {
    SHIELD_UNTIL.get_or_init(DashMap::new)
}

pub(super) fn is_shield_active(pid: PlayerId, now_ms: i64) -> bool {
    shield_until().get(&pid).is_some_and(|t| *t.value() > now_ms)
}

pub(super) fn clear_shield(pid: PlayerId) {
    shield_until().remove(&pid);
}

pub(super) fn clear_ttnl(pid: PlayerId) {
    ttnl_state().remove(&pid);
}

pub(super) fn clear_skill_state(pid: PlayerId) {
    ttnl_state().remove(&pid);
    shield_until().remove(&pid);
}

fn build_skill_not_focus_pkt(
    status: i8, pid: i32, skill_paint_id: i16, special: Option<SkillSpecialArgs>,
) -> Packet {
    let mut w = PacketWriter::with_capacity(20);
    let wire_status = match status {
        2 => 1,
        9 | 10 => 6,
        20 if special.is_some() => 20,
        0 | 1 | 3 | 4 | 5 | 6 | 7 | 8 => status,
        _ => 0,
    };
    w.write_i8(wire_status);
    w.write_i32(pid);
    w.write_i16(skill_paint_id);
    match wire_status {
        0 => {
            w.write_i8(0);
            w.write_i8(0);
        }
        4 | 7 => w.write_i16(3_000),
        20 => {
            let dir = special.map_or(1_i8, |a| a.dir);
            w.write_i8(0);
            w.write_i8(dir);
            w.write_i16(1_500);
            w.write_i8(0);
            w.write_i8(0);
            w.write_i8(-1);
        }
        _ => {}
    }
    w.finish(cmd_message::SKILL_NOT_FOCUS)
}

impl ServerSessionHandler {
    pub(crate) fn clear_ttnl_for_player(pid: PlayerId) {
        clear_ttnl(pid);
    }

    pub(super) async fn handle_skill_not_focus(
        &self, session: &SessionHandle, sid: u64, status: i8, special: Option<SkillSpecialArgs>,
    ) -> Result<(), ProtocolError> {
        let outcome = self.shared.clients.authenticated.get_mut(&sid).and_then(|mut entry| {
            if entry.snapshot.stats.hp <= 0 {
                return None;
            }
            let (sel, now_ms) = (special.map_or(entry.selected_skill, |a| i16::from(a.skill_id)), unix_now_ms());
            let status_ok = match status {
                1 | 2 => sel == 8,
                6 => sel == 13,
                7 => sel == 14,
                9 => sel == 19,
                _ => true,
            };
            if !status_ok {
                return None;
            }
            let sk_idx = entry.snapshot.skills.iter().position(|sk| sk.template_id == i32::from(sel) && sk.point > 0)?;
            let st = self.shared.templates.skills.iter().find(|s| i16::from(s.id) == sel)?;
            let pt = entry.snapshot.skills[sk_idx].point;
            let lv = st.levels.iter().find(|l| l.point == pt)?;
            let skill_paint_id = lv.id.max(0);
            let skip_mp = matches!(status, 2 | 3 | 5);
            if !skip_mp {
                let cd = i64::from(lv.cool_down.max(0)).saturating_sub(50);
                if now_ms.saturating_sub(entry.snapshot.skills[sk_idx].last_time_use_ms) < cd {
                    return None;
                }
            }
            let mana_cost = if skip_mp {
                0
            } else {
                match st.mana_use_type {
                    2 => {
                        if entry.snapshot.stats.mp <= 0 {
                            return Some(Err("Không đủ KI để sử dụng kỹ năng!"));
                        }
                        entry.snapshot.stats.mp
                    }
                    1 => (entry.snapshot.stats.mp_max.max(0) * i64::from(lv.mana_use.max(0))) / 100,
                    _ => i64::from(lv.mana_use.max(0)),
                }
            };
            if mana_cost > 0 && entry.snapshot.stats.mp < mana_cost {
                return Some(Err("Không đủ KI để sử dụng kỹ năng!"));
            }
            let pid_obj = entry.snapshot.id;
            match status {
                1 => {
                    entry.snapshot.stats.mp = entry.snapshot.stats.mp.saturating_sub(mana_cost).max(0);
                    entry.snapshot.skills[sk_idx].last_time_use_ms = now_ms;
                    ttnl_state().insert(pid_obj, (now_ms, 0));
                }
                2 => {
                    let remove_after = {
                        let mut st_entry = ttnl_state().get_mut(&pid_obj)?;
                        let (last_tick_ms, ticks_used) = *st_entry;
                        if ticks_used >= MAX_TTNL_TICKS || now_ms.saturating_sub(last_tick_ms) < 950 {
                            return None;
                        }
                        let next_ticks = ticks_used.saturating_add(1);
                        *st_entry = (now_ms, next_ticks);
                        let pct = i64::from(lv.damage.max(10));
                        let hp_gain = (entry.snapshot.stats.hp_max.saturating_mul(pct) / 100).max(1);
                        let mp_gain = (entry.snapshot.stats.mp_max.saturating_mul(pct) / 100).max(1);
                        entry.snapshot.stats.hp = entry.snapshot.stats.hp.saturating_add(hp_gain).min(entry.snapshot.stats.hp_max);
                        entry.snapshot.stats.mp = entry.snapshot.stats.mp.saturating_add(mp_gain).min(entry.snapshot.stats.mp_max);
                        next_ticks >= MAX_TTNL_TICKS
                            || (entry.snapshot.stats.hp >= entry.snapshot.stats.hp_max
                                && entry.snapshot.stats.mp >= entry.snapshot.stats.mp_max)
                    };
                    if remove_after {
                        ttnl_state().remove(&pid_obj);
                    }
                }
                3 => {
                    ttnl_state().remove(&pid_obj);
                }
                4 => {}
                _ => {
                    entry.snapshot.stats.mp = entry.snapshot.stats.mp.saturating_sub(mana_cost).max(0);
                    if status == 7 && sel == 14 {
                        entry.snapshot.stats.hp = 0;
                    }
                    if status == 9 {
                        shield_until().insert(pid_obj, now_ms.saturating_add(30_000));
                    }
                    if !skip_mp {
                        entry.snapshot.skills[sk_idx].last_time_use_ms = now_ms;
                    }
                }
            }
            let (pid, loc, stats) = (pid_obj.0 as i32, entry.snapshot.location, entry.snapshot.stats);
            if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, stats));
            }
            let skill_pkt = build_skill_not_focus_pkt(status, pid, skill_paint_id, special);
            let die_pkts = (status == 7 && sel == 14)
                .then(|| s2c_packets::char_die_packets(pid_obj, 0, loc, stats.power));
            let monkey_pkt = (status == 6).then(|| {
                let idx = usize::try_from(pt.clamp(1, 7) - 1).unwrap_or(0);
                s2c_packets::update_body(pid_obj, Some((MONKEY_HEADS[idx], 193, 194, 1)))
            });
            Some(Ok((pid_obj, loc, stats, skill_pkt, die_pkts, monkey_pkt)))
        });
        match outcome {
            Some(Err(msg)) => send_thong_bao(session, msg)?,
            Some(Ok((pid_obj, loc, stats, skill_pkt, die_pkts, monkey_pkt))) => {
                self.broadcast_zone_or_self(loc, sid, &skill_pkt);
                if let Some(mpkt) = monkey_pkt {
                    self.broadcast_zone_or_self(loc, sid, &mpkt);
                }
                session.send(s2c_packets::me_load_point(&stats)).await?;
                if matches!(status, 2 | 7) {
                    session.send(s2c_packets::me_load_hp(stats.hp)).await?;
                    session.send(s2c_packets::me_load_mp(stats.mp)).await?;
                }
                if let Some((me_die, peer_die)) = die_pkts {
                    self.handle_player_death_cleanup(pid_obj, loc, sid);
                    session.send(me_die).await?;
                    self.broadcast_to_zone_peers(loc, Some(sid), &peer_die);
                }
            }
            None => {}
        }
        Ok(())
    }
}
