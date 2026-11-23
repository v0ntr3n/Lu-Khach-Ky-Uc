//! Boss [`EngineEffect`] dispatcher for [`SharedServerState`].

use std::sync::Arc;

use nro_boss::{EngineEffect, PlayerDebuffKind};
use nro_consts::cmd_message;
use nro_core::ItemOption;
use nro_world::{GroundDropKind, ZoneCommand, s2c_packets};
use smallvec::SmallVec;
use tracing::{debug, info, warn};

use super::{LeaderboardSlot, SharedServerState, unix_now_ms};
use crate::router::ServerSessionHandler;

impl SharedServerState {
    fn apply_boss_damage_to_player(
        self: &Arc<Self>,
        target: nro_core::PlayerId,
        damage: i64,
        piercing: bool,
    ) {
        let now_ms = unix_now_ms();
        let target_sid = self.clients.players.get(&target).map(|s| *s.value());
        let (mut died_info, mut took_damage, mut hit_broadcast) = (None, false, None);
        if let Some(sid) = target_sid
            && let Some(mut entry) = self.clients.authenticated.get_mut(&sid)
        {
            let prev_hp = entry.snapshot.stats.hp;
            let e = &mut *entry;
            let (out, _) = crate::router::apply_incoming_hit_to_player(
                sid,
                target,
                &mut e.snapshot.stats,
                &e.snapshot.items_body,
                damage.max(0),
                None,
                (0, 0, 0),
                piercing,
                now_ms,
            );
            let final_dmg = if out.dodged {
                0
            } else {
                i64::from(out.damage_taken.max(0))
            };
            took_damage = final_dmg > 0;
            if entry.handle.sent_key() {
                let _ = entry
                    .handle
                    .try_send(s2c_packets::me_load_point(&entry.snapshot.stats));
            }
            let loc = entry.snapshot.location;
            if !ServerSessionHandler::is_offline_map_id(loc.map_id) {
                hit_broadcast = Some((loc, entry.snapshot.stats.hp, final_dmg));
            }
            if let (Some(ek), Ok(ztx)) = (
                entry.entity_key,
                self.world_router.resolve_sender(loc.map_id, loc.zone_id),
            ) {
                let _ = ztx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
            }
            if prev_hp > 0 && entry.snapshot.stats.hp == 0 {
                died_info = Some((sid, entry.handle.clone(), loc, entry.snapshot.stats.power));
            }
            debug!(
                player_id = target.0,
                damage,
                final_dmg,
                piercing,
                hp = entry.snapshot.stats.hp,
                "applied boss damage effect"
            );
        }
        if let Some((loc, hp_after, dmg)) = hit_broadcast {
            self.broadcast_zone_pkt(
                loc.map_id,
                loc.zone_id,
                &s2c_packets::have_attack_player(target, hp_after, dmg, false, -1),
            );
        }
        if took_damage {
            ServerSessionHandler::clear_ttnl_for_player(target);
        }
        if let Some((victim_sid, victim_handle, victim_loc, victim_power)) = died_info {
            ServerSessionHandler::new(Arc::clone(self))
                .handle_player_death_cleanup(target, victim_loc, victim_sid);
            let (me_die_pkt, peer_die_pkt) =
                s2c_packets::char_die_packets(target, 0, victim_loc, victim_power);
            if victim_handle.sent_key() {
                let _ = victim_handle.try_send(me_die_pkt);
            }
            if !ServerSessionHandler::is_offline_map_id(victim_loc.map_id) {
                for rec in &self.clients.authenticated {
                    if *rec.key() != victim_sid
                        && rec.value().snapshot.location.map_id == victim_loc.map_id
                        && rec.value().snapshot.location.zone_id == victim_loc.zone_id
                        && rec.value().handle.sent_key()
                    {
                        let _ = rec.value().handle.try_send(peer_die_pkt.clone());
                    }
                }
            }
        }
    }

    pub(crate) async fn dispatch_boss_effects(self: &Arc<Self>, effects: Vec<EngineEffect>) {
        for effect in effects {
            match effect {
                EngineEffect::SpawnDynamicBoss {
                    boss_type,
                    map_id,
                    zone_id,
                    location,
                    scaled_stats,
                } => {
                    info!(
                        boss_type = boss_type.0,
                        map_id = map_id.0,
                        zone_id = zone_id.0,
                        x = location.x,
                        y = location.y,
                        scaled = scaled_stats.is_some(),
                        "boss spawned"
                    );
                    if !matches!(boss_type.0, 46..=50 | 96) {
                        let map_label = self
                            .templates
                            .maps
                            .iter()
                            .find(|m| m.id == map_id)
                            .map_or_else(|| format!("Map {}", map_id.0), |m| m.name.clone());
                        self.clients.broadcast_server_notify(&format!(
                            "BOSS #{} vừa xuất hiện tại {map_label} khu {}",
                            boss_type.0, zone_id.0
                        ));
                    }
                }
                EngineEffect::DamagePlayer {
                    target,
                    damage,
                    piercing,
                } => self.apply_boss_damage_to_player(target, damage, piercing),
                EngineEffect::DropItem(drop) => {
                    if let Ok(ztx) = self
                        .world_router
                        .resolve_sender(drop.location.map_id, drop.location.zone_id)
                    {
                        let kind = match drop.item.template_id.0 {
                            14..=20 => GroundDropKind::EarthDragonBall,
                            353..=359 | 362 => GroundDropKind::NamekBall,
                            372..=378 => GroundDropKind::BlackBall,
                            _ => GroundDropKind::Standard,
                        };
                        let options: SmallVec<[ItemOption; 4]> =
                            drop.item.options.iter().copied().collect();
                        let _ = ztx.try_send(ZoneCommand::DropItemWithOptions(
                            drop.item.template_id,
                            drop.item.quantity,
                            drop.location,
                            drop.owner,
                            kind,
                            options,
                        ));
                    }
                }
                EngineEffect::GrantTnsm { target, amount } => {
                    let gain = amount.max(0);
                    if gain > 0
                        && let Some(sid) = self.clients.players.get(&target).map(|s| *s.value())
                        && let Some(mut entry) = self.clients.authenticated.get_mut(&sid)
                    {
                        entry.snapshot.stats.power =
                            entry.snapshot.stats.power.saturating_add(gain);
                        entry.snapshot.stats.tiem_nang =
                            entry.snapshot.stats.tiem_nang.saturating_add(gain);
                        if entry.handle.sent_key() {
                            let _ = entry
                                .handle
                                .try_send(s2c_packets::me_load_point(&entry.snapshot.stats));
                        }
                        self.leaderboards.mark_dirty(LeaderboardSlot::Power);
                    }
                }
                EngineEffect::GrantEventPoints { target, points } => {
                    let delta = i32::try_from(points).unwrap_or(i32::MAX);
                    if let Some(sid) = self.clients.players.get(&target).map(|s| *s.value())
                        && let Some(mut entry) = self.clients.authenticated.get_mut(&sid)
                    {
                        entry.snapshot.event_point =
                            entry.snapshot.event_point.saturating_add(delta);
                    }
                }
                EngineEffect::StealGold { target, amount } => {
                    let stolen = i64::try_from(amount).unwrap_or(i64::MAX);
                    if let Some(sid) = self.clients.players.get(&target).map(|s| *s.value())
                        && let Some(mut entry) = self.clients.authenticated.get_mut(&sid)
                    {
                        entry.snapshot.wallet.gold =
                            entry.snapshot.wallet.gold.saturating_sub(stolen).max(0);
                        if entry.handle.sent_key() {
                            let w = &entry.snapshot.wallet;
                            let pkt = s2c_packets::send_money(
                                w.gold.max(0).cast_unsigned(),
                                u64::from(w.gem.max(0).cast_unsigned()),
                                u64::from(w.ruby.max(0).cast_unsigned()),
                            );
                            let _ = entry.handle.try_send(pkt);
                        }
                    }
                }
                EngineEffect::WipeZoneMobs { map_id, zone_id } => {
                    if let Ok(ztx) = self.world_router.resolve_sender(map_id, zone_id) {
                        let _ = ztx.try_send(ZoneCommand::WipeMobs);
                    }
                    info!(
                        map_id = map_id.0,
                        zone_id = zone_id.0,
                        "boss wiped zone mobs"
                    );
                }
                EngineEffect::ChangePk { entity, mode } => {
                    if let Some((map_id, zone_id)) = self.resolve_entity_zone(entity)
                        && !ServerSessionHandler::is_offline_map_id(map_id)
                    {
                        let mut pk =
                            s2c_packets::sub_command_writer(cmd_message::UPDATE_TYPE_PK, 5);
                        pk.write_i32(entity.0 as i32);
                        pk.write_i8((mode as u8).cast_signed());
                        self.broadcast_zone_pkt(
                            map_id,
                            zone_id,
                            &pk.finish(cmd_message::SUB_COMMAND),
                        );
                    }
                }
                EngineEffect::GrantNormalPet { target } => {
                    if let Some((sid, handle)) = self
                        .clients
                        .players
                        .get(&target)
                        .map(|s| *s.value())
                        .and_then(|sid| {
                            self.clients
                                .authenticated
                                .get(&sid)
                                .map(|e| (sid, e.handle.clone()))
                        })
                    {
                        let shared = Arc::clone(self);
                        tokio::spawn(async move {
                            if let Err(err) = ServerSessionHandler::new(shared)
                                .grant_starter_pet_for_effect(&handle, sid)
                                .await
                            {
                                warn!(player_id = target.0, error = %err, "boss grant normal pet failed");
                            }
                        });
                    }
                }
                EngineEffect::AwardMayDamPoints { target, points } => {
                    let delta = i32::try_from(points).unwrap_or(i32::MAX);
                    if let Some(sid) = self.clients.players.get(&target).map(|s| *s.value())
                        && let Some(mut entry) = self.clients.authenticated.get_mut(&sid)
                    {
                        entry.snapshot.point_maydam =
                            entry.snapshot.point_maydam.saturating_add(delta);
                    }
                    self.notify_online_player(
                        target,
                        &format!("Bạn nhận được +{points} điểm Máy Đấm!"),
                    );
                }
                EngineEffect::UnlockSuperDivineWater { target } => {
                    if let Some(sid) = self.clients.players.get(&target).map(|s| *s.value())
                        && let Some(mut entry) = self.clients.authenticated.get_mut(&sid)
                    {
                        entry.snapshot.sieu_than_thuy.0 = true;
                    }
                    self.notify_online_player(target, "Bạn đã mở khóa Siêu Thần Thủy!");
                }
                EngineEffect::DebuffPlayer {
                    target,
                    kind,
                    duration_ms,
                } => {
                    let applied = self
                        .clients
                        .players
                        .get(&target)
                        .map(|s| *s.value())
                        .and_then(|sid| {
                            self.clients.authenticated.get(&sid).map(|e| {
                                crate::router::apply_boss_debuff_to_player(
                                    target,
                                    kind,
                                    u64::from(duration_ms),
                                    &e.snapshot.items_body,
                                    unix_now_ms(),
                                )
                            })
                        })
                        .unwrap_or(true);
                    if !applied {
                        self.notify_online_player(target, "Bạn đã kháng hiệu ứng choáng!");
                        continue;
                    }
                    let secs = duration_ms.div_ceil(1_000).max(1);
                    let label = match kind {
                        PlayerDebuffKind::Stun => "bị choáng",
                        PlayerDebuffKind::PetrifyStone => "bị hóa đá",
                        PlayerDebuffKind::Chocolate => "bị biến thành Sô-cô-la",
                        PlayerDebuffKind::BodyChange => "bị hoán đổi cơ thể",
                        PlayerDebuffKind::GravitySlow => "bị làm chậm trọng lực",
                        PlayerDebuffKind::HalloweenCurse(_) => "bị dính lời nguyền Halloween",
                        PlayerDebuffKind::Contagion {
                            lethal_on_expire: true,
                        } => "bị nhiễm độc chí mạng",
                        PlayerDebuffKind::Contagion {
                            lethal_on_expire: false,
                        } => "bị thiêu đốt",
                    };
                    self.notify_online_player(
                        target,
                        &format!("Bạn đã {label} trong {secs} giây!"),
                    );
                }
                EngineEffect::Chat { speaker, text } => {
                    if let Some((map_id, zone_id)) = self.resolve_entity_zone(speaker)
                        && !ServerSessionHandler::is_offline_map_id(map_id)
                        && let Ok(pkt) = s2c_packets::chat_map(speaker, &text)
                    {
                        self.broadcast_zone_pkt(map_id, zone_id, &pkt);
                    } else {
                        debug!(speaker = speaker.0, %text, "boss chat");
                    }
                }
                EngineEffect::TeleportPlayer {
                    target,
                    map_id,
                    zone_id,
                    location,
                } => {
                    if let Some((sid, handle)) = self
                        .clients
                        .players
                        .get(&target)
                        .map(|s| *s.value())
                        .and_then(|sid| {
                            self.clients
                                .authenticated
                                .get(&sid)
                                .map(|e| (sid, e.handle.clone()))
                        })
                    {
                        let shared = Arc::clone(self);
                        tokio::spawn(async move {
                            if let Err(err) = ServerSessionHandler::new(shared)
                                .teleport_player(
                                    &handle,
                                    sid,
                                    map_id,
                                    zone_id,
                                    (location.x, location.y),
                                )
                                .await
                            {
                                warn!(player_id = target.0, map_id = map_id.0, zone_id = zone_id.0, error = %err, "boss teleport player failed");
                            }
                        });
                    }
                }
                EngineEffect::DistributeClanCapsules(grants) => {
                    for (target, _clan_id, capsules) in grants {
                        self.notify_online_player(
                            target,
                            &format!("Bang hội nhận được +{capsules} Capsule Bang!"),
                        );
                    }
                }
            }
        }
    }
}
