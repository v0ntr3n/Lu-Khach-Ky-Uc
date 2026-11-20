//! Deterministic 4-stage `--benchmark-replay` workload for LLVM Profile-Guided Optimization.
//!
//! Profiles real hot paths across `nro-protocol`, `nro-player`, `nro-economy`, `nro-boss`,
//! and `nro-world` entirely in memory without requiring network sockets or external daemons:
//! 1. Rolling XOR session cipher + asymmetric 16-bit/24-bit packet framing (`nro_protocol`).
//! 2. `NPoint::cal_point` & `SetClothes` equipment/buff stat recalculation (`nro_player`).
//! 3. `CombineService` star-socket rolls (`nro_economy`) + `evaluate_boss_damage` caps (`nro_boss`).
//! 4. `ZoneSoaArena` spatial player/mob updates + `s2c_packets` binary serialization (`nro_world`).

use std::time::{Duration, Instant};

use anyhow::ensure;
use bytes::BytesMut;
use nro_boss::mechanics::{BossArchetype, DamageRule, evaluate_boss_damage};
use nro_boss::spawner::BossGroupManager;
use nro_core::{
    AccountId, CoreStats, GameRng, Gender, ItemInstance, ItemOption, Location, MapId, ParityMode,
    PlayerId, SessionKey, TemplateId, ZoneId,
};
use nro_economy::{BuiltinCatalog, CombineContext, CombineOp, CombineService, Inventory};
use nro_player::Player;
use nro_protocol::{
    ClientPacket, Packet, PacketWriter, XorSessionCipher, decode_c2s_frame, decode_s2c_frame,
    encode_c2s_frame, encode_s2c_frame,
};
use nro_world::{EntityKind, PlayerIngress, ZoneSoaArena, s2c_packets};
use slotmap::SlotMap;
use smallvec::smallvec;
use tracing::info;

use crate::config::ServerConfig;

/// Fixed golden seed ensuring bit-exact repeatability across PGO instrumentation runs.
pub const PGO_GOLDEN_SEED: u64 = 0x2026_0E20_5AFE_BEEF;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

/// Telemetry captured for one of the four PGO replay stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageMetrics {
    /// 1-based stage ordinal (`1..=4`).
    pub stage_index: u8,
    /// Short identifier of the profiled subsystem.
    pub stage_name: &'static str,
    /// Number of domain operations executed during the stage.
    pub operations: u64,
    /// Wall-clock duration spent inside the stage.
    pub elapsed: Duration,
    /// Deterministic 64-bit state checksum produced by the stage.
    pub digest: u64,
}

/// Aggregated report returned by [`run_four_stage_replay`].
#[derive(Debug, Clone)]
pub struct ReplaySummary {
    /// Per-stage timing, operation count, and checksum records.
    pub stages: [StageMetrics; 4],
    /// Combined 64-bit digest folded across all four stages.
    pub combined_digest: u64,
}

impl ReplaySummary {
    /// Total domain operations executed across all four stages.
    #[must_use]
    pub fn total_operations(&self) -> u64 {
        self.stages.iter().fold(0_u64, |acc, s| acc.saturating_add(s.operations))
    }

    /// Total wall-clock time accumulated across all four stages.
    #[must_use]
    pub fn total_elapsed(&self) -> Duration {
        self.stages.iter().fold(Duration::ZERO, |acc, s| acc.saturating_add(s.elapsed))
    }
}

/// Executes all four deterministic PGO profiling stages scaled by `replay_scale`.
pub fn run_four_stage_replay(config: &ServerConfig, replay_scale: u32) -> anyhow::Result<ReplaySummary> {
    ensure!(replay_scale >= 1, "replay_scale must be >= 1");
    let scale = u64::from(replay_scale);
    let mut rng = GameRng::from_seed(PGO_GOLDEN_SEED ^ u64::from(config.active_seasonal_event_id));

    let s1 = stage_1_protocol_xor_framing(scale, &mut rng)?;
    let s2 = stage_2_player_npoint_recalc(scale, config.parity_mode, &mut rng);
    let s3 = stage_3_combine_and_boss_caps(scale, config.parity_mode, &mut rng)?;
    let s4 = stage_4_zone_soa_and_s2c(scale, &mut rng);

    let stages = [s1, s2, s3, s4];
    let mut combined_digest = FNV_OFFSET_BASIS;
    for stage in &stages {
        combined_digest = fnv_mix(combined_digest, stage.digest);
        info!(
            stage = stage.stage_index,
            name = stage.stage_name,
            ops = stage.operations,
            elapsed_ms = stage.elapsed.as_secs_f64() * 1_000.0,
            digest = format_args!("{:#018x}", stage.digest),
            "PGO benchmark stage finished"
        );
    }
    Ok(ReplaySummary { stages, combined_digest })
}

fn stage_1_protocol_xor_framing(scale: u64, rng: &mut GameRng) -> anyhow::Result<StageMetrics> {
    let start = Instant::now();
    let (mut c2s_read, mut c2s_write) = XorSessionCipher::default_nro().split();
    let (mut s2c_read, mut s2c_write) = XorSessionCipher::default_nro().split();
    c2s_read.set_sent_key(true);
    c2s_write.set_sent_key(true);
    s2c_read.set_sent_key(true);
    s2c_write.set_sent_key(true);

    let mut wire = BytesMut::with_capacity(2_048);
    let mut digest = FNV_OFFSET_BASIS;
    let trips = scale.saturating_mul(16);

    for i in 0..trips {
        wire.clear();
        let mut pw = PacketWriter::with_capacity(16);
        pw.write_i8(1);
        pw.write_i16((rng.next_u32() & 0x03FF) as i16);
        pw.write_i16((rng.next_u32() & 0x01FF) as i16);
        encode_c2s_frame(&pw.finish(-7), &mut c2s_write, &mut wire)?;
        if let Some(raw) = decode_c2s_frame(&mut wire, &mut c2s_read)? {
            let decoded = ClientPacket::decode(raw.cmd, &mut raw.reader(), 231)?;
            if let ClientPacket::PlayerMove(_, x, Some(y)) = decoded {
                digest = fnv_mix(digest, (i64::from(x) ^ i64::from(y)).cast_unsigned());
            }
        }

        wire.clear();
        let out_cmd = if i.trailing_zeros() >= 3 { -87 } else { -73 };
        let s2c_pkt = Packet::new(out_cmd, &b"Boss Super Broly xuat hien tai Rung Nguyen Sinh!"[..]);
        encode_s2c_frame(&s2c_pkt, &mut s2c_write, &mut wire)?;
        if let Some(back) = decode_s2c_frame(&mut wire, &mut s2c_read)? {
            digest = fnv_mix(digest, back.payload.len() as u64);
        }
    }

    Ok(StageMetrics {
        stage_index: 1,
        stage_name: "protocol_xor_framing",
        operations: trips.saturating_mul(2),
        elapsed: start.elapsed(),
        digest,
    })
}

fn stage_2_player_npoint_recalc(scale: u64, parity: ParityMode, rng: &mut GameRng) -> StageMetrics {
    let start = Instant::now();
    let mut players: Vec<Player> = (0..32_i64)
        .map(|idx| {
            let gender = Gender::from_i8_lossy((idx % 3) as i8);
            let core = CoreStats {
                hpg: 50_000 + i64::from(rng.next_u32() % 250_000),
                mpg: 40_000 + i64::from(rng.next_u32() % 200_000),
                dameg: 2_500 + i64::from(rng.next_u32() % 25_000),
                power: 25_000_000_000,
                limit_power: 6,
                ..CoreStats::default()
            };
            let loc = Location { map_id: MapId(5), zone_id: ZoneId(0), x: 240, y: 336 };
            let mut pl = Player::new(PlayerId(idx + 1), AccountId((idx + 1) as i32), gender, core, loc);
            for slot in 0..5 {
                pl.inventory.items_body[slot] = Some(ItemInstance {
                    template_id: TemplateId(555 + slot as i16),
                    quantity: 1,
                    options: smallvec![ItemOption::new(77, 15), ItemOption::new(50, 12), ItemOption::new(135, 1)],
                    create_time_ms: 0,
                });
            }
            pl
        })
        .collect();

    let mut digest = FNV_OFFSET_BASIS;
    let sweeps = scale.saturating_mul(4);
    for sweep in 0..sweeps {
        let in_cold = sweep.trailing_zeros() >= 2;
        for pl in &mut players {
            pl.effects.monkey_level = ((sweep & 7) as u8) % 4;
            pl.recalculate_stats(in_cold, false, sweep * 1_000, parity);
            digest = fnv_mix(digest, (i64::from(pl.npoint.hp_max) ^ i64::from(pl.npoint.dame)).cast_unsigned());
        }
    }

    StageMetrics {
        stage_index: 2,
        stage_name: "player_npoint_recalc",
        operations: sweeps.saturating_mul(players.len() as u64),
        elapsed: start.elapsed(),
        digest,
    }
}

fn stage_3_combine_and_boss_caps(scale: u64, parity: ParityMode, rng: &mut GameRng) -> anyhow::Result<StageMetrics> {
    let start = Instant::now();
    let _mgr = BossGroupManager::with_builtin_catalog(parity, 1_000)?;
    let mut inv = Inventory::new(16, 16, 12);
    inv.gold = 190_000_000_000;
    inv.gem = 5_000_000;
    inv.set_bag_slot(0, Some(ItemInstance::new(TemplateId(555), 1)))?;
    let ctx = CombineContext::default_hardened();

    let mut boss_stats = CoreStats { hp: 16_000_000, hp_max: 16_000_000, dame: 150_000, ..CoreStats::default() };
    let mut archetype = BossArchetype::Broly { is_super: false };
    let rule = DamageRule::BrolyGrowth { is_super: false };
    let mut shield = false;
    let mut effects = Vec::with_capacity(4);
    let mut digest = FNV_OFFSET_BASIS;
    let rounds = scale.saturating_mul(16);

    for _ in 0..rounds {
        if inv.bag_item(0)?.options.iter().any(|o| o.id == 107 && o.param >= 8) {
            inv.set_bag_slot(0, Some(ItemInstance::new(TemplateId(555), 1)))?;
        }
        inv.gold = 190_000_000_000;
        let out = CombineService::execute(&mut inv, CombineOp::PhaLeHoaTrangBi, &[0], &ctx, &BuiltinCatalog, rng)?;
        digest = fnv_mix(digest, u64::from(out.success));

        if boss_stats.hp <= 100_000 {
            boss_stats.hp = boss_stats.hp_max;
        }
        effects.clear();
        let hit = evaluate_boss_damage(
            PlayerId(-100),
            0,
            &mut boss_stats,
            &mut shield,
            &rule,
            &mut archetype,
            None,
            nro_boss::SkillKind::Kamejoko,
            850_000,
            false,
            false,
            false,
            parity,
            rng,
            &mut effects,
        );
        digest = fnv_mix(digest, hit.applied_damage.cast_unsigned());
    }

    Ok(StageMetrics {
        stage_index: 3,
        stage_name: "combine_and_boss_caps",
        operations: rounds.saturating_mul(2),
        elapsed: start.elapsed(),
        digest,
    })
}

fn stage_4_zone_soa_and_s2c(scale: u64, rng: &mut GameRng) -> StageMetrics {
    let start = Instant::now();
    let mut arena = ZoneSoaArena::default();
    let mut sm: SlotMap<SessionKey, ()> = SlotMap::with_key();
    let mut keys = Vec::with_capacity(24);
    for i in 0..24_i64 {
        let sk = sm.insert(());
        let ing = PlayerIngress {
            session: (AccountId((i + 1) as i32), sk),
            identity: (PlayerId(i + 1), None),
            gender: Gender::from_i8_lossy((i % 3) as i8),
            name: format!("Warrior{i}"),
            loc: Location { map_id: MapId(5), zone_id: ZoneId(0), x: 180 + (i as i16) * 12, y: 336 },
            stats: CoreStats { hp: 250_000, hp_max: 250_000, dame: 18_000, ..CoreStats::default() },
            parts: (64, 65, 66),
            ship_type: 1,
            outbound_tx: None,
        };
        keys.push(arena.insert_player(ing));
    }
    let _mob_key = arena.insert_non_player(
        EntityKind::Mob,
        None,
        String::from("KhungLong"),
        Location { map_id: MapId(5), zone_id: ZoneId(0), x: 400, y: 336 },
        CoreStats::default(),
    );

    let mut digest = FNV_OFFSET_BASIS;
    let ticks = scale.saturating_mul(8);
    for tick in 0..ticks {
        let idx = (tick as usize) % keys.len();
        let loc = Location {
            map_id: MapId(5),
            zone_id: ZoneId(0),
            x: (rng.next_u32() % 1_000) as i16,
            y: 336,
        };
        let mv_pkt = s2c_packets::player_move(PlayerId((idx + 1) as i64), loc);
        let die_pkt = s2c_packets::char_die_packets(PlayerId((idx + 1) as i64), 0, loc, 25_000_000_000);
        digest = fnv_mix(digest, (mv_pkt.payload.len() + die_pkt.0.payload.len()) as u64);
    }

    StageMetrics {
        stage_index: 4,
        stage_name: "zone_soa_and_s2c",
        operations: ticks.saturating_mul(keys.len() as u64),
        elapsed: start.elapsed(),
        digest,
    }
}

#[inline]
const fn fnv_mix(state: u64, value: u64) -> u64 {
    (state ^ value).wrapping_mul(FNV_PRIME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_stage_pgo_replay_is_bit_exact_and_deterministic() -> anyhow::Result<()> {
        let cfg = ServerConfig::default();
        let r1 = run_four_stage_replay(&cfg, 2)?;
        let r2 = run_four_stage_replay(&cfg, 2)?;
        assert_eq!(r1.combined_digest, r2.combined_digest);
        assert!(r1.total_operations() > 0);
        for (a, b) in r1.stages.iter().zip(&r2.stages) {
            assert_eq!(a.digest, b.digest);
            assert_eq!(a.operations, b.operations);
        }
        Ok(())
    }
}
