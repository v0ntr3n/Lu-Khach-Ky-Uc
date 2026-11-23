//! Capsule transport (`-91`), spacetime tunnel (`-105`), Yardrat teleport (`18`), and session buff multipliers.

use nro_consts::const_map;
use nro_core::{CoreStats, ItemInstance, MapId, PlayerId, TemplateId, ZoneId};
use nro_economy::{BuffSlot, ItemCatalog};
use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::s2c_packets;

use super::super::{
    DbItemCatalog, ServerSessionHandler, capsule_dests, send_thong_bao, snapshot_to_economy_inv,
    sync_economy_inv_to_snapshot,
};
use super::session_buffs;
use crate::runtime::unix_now_ms;

impl ServerSessionHandler {
    pub(in crate::router) async fn handle_map_transport(
        &self,
        session: &SessionHandle,
        sid: u64,
        opt_idx: Option<i8>,
    ) -> Result<(), ProtocolError> {
        let Some(raw_idx) = opt_idx.and_then(|i| usize::try_from(i).ok()) else {
            return Ok(());
        };
        let mut in_trade = false;
        let prep = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                if super::super::router_shop::is_in_trade(entry.snapshot.id) {
                    in_trade = true;
                    return None;
                }
                if entry.snapshot.stats.hp <= 0 {
                    return None;
                }
                let pid = entry.snapshot.id;
                let (is_194, dst_map) = capsule_dests()
                    .get(&pid)
                    .and_then(|v| v.value().1.get(raw_idx).map(|&m| (v.value().0, m)))?;
                let deducted = if is_194 {
                    if !entry.snapshot.items_bag.iter().any(|s| {
                        s.as_ref()
                            .is_some_and(|it| it.template_id == TemplateId(194) && it.quantity > 0)
                    }) {
                        return None;
                    }
                    None
                } else {
                    let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                    let refund_item = inv
                        .bag
                        .iter()
                        .flatten()
                        .find(|it| it.template_id == TemplateId(193))
                        .map_or_else(
                            || {
                                ItemInstance::with_options(
                                    TemplateId(193),
                                    1,
                                    Vec::new(),
                                    unix_now_ms(),
                                )
                            },
                            |it| ItemInstance {
                                template_id: TemplateId(193),
                                quantity: 1,
                                options: it.options.clone(),
                                create_time_ms: it.create_time_ms,
                            },
                        );
                    inv.sub_template_from_bag(TemplateId(193), 1).ok()?;
                    sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                    Some((
                        refund_item,
                        s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                    ))
                };
                capsule_dests().remove(&pid);
                Some((
                    dst_map,
                    if entry.snapshot.have_tennis_space_ship {
                        3
                    } else {
                        1
                    },
                    deducted,
                ))
            });
        if in_trade {
            return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
        }
        if let Some((dst_map, ship, deducted)) = prep {
            let res = self
                .switch_player_zone_with_ship(
                    session,
                    sid,
                    Some(dst_map),
                    ZoneId(0),
                    Some((250, 5)),
                    ship,
                )
                .await;
            if matches!(res, Ok(true)) {
                if let Some((_, pkt)) = deducted {
                    session.send(pkt).await?;
                }
            } else {
                if let Some((refund_item, _)) = deducted {
                    let stackable = DbItemCatalog(&self.shared.templates)
                        .meta_of(TemplateId(193))
                        .stackable;
                    let bp = self
                        .shared
                        .clients
                        .authenticated
                        .get_mut(&sid)
                        .map(|mut entry| {
                            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                            let _ = inv.add_to_bag(refund_item, stackable);
                            sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                            s2c_packets::inventory_bag(&entry.snapshot.items_bag)
                        });
                    if res.is_ok()
                        && let Some(pkt) = bp
                    {
                        session.send(pkt).await?;
                    }
                }
                res?;
            }
        }
        Ok(())
    }

    pub(in crate::router) async fn handle_transport_complete(
        &self,
        session: &SessionHandle,
        sid: u64,
    ) -> Result<(), ProtocolError> {
        let Some(dst) = self.shared.clients.authenticated.get(&sid).and_then(|e| {
            (e.snapshot.stats.hp > 0
                && !super::super::router_shop::is_in_trade(e.snapshot.id)
                && matches!(e.snapshot.location.map_id, MapId(27 | 102)))
            .then_some(if e.snapshot.location.map_id == MapId(102) {
                MapId(27)
            } else {
                MapId(102)
            })
        }) else {
            return Ok(());
        };
        self.switch_player_zone(session, sid, Some(dst), ZoneId(0), Some((250, 336)))
            .await
    }

    pub(in crate::router) async fn handle_goto_player(
        &self,
        session: &SessionHandle,
        sid: u64,
        target_pid: PlayerId,
    ) -> Result<(), ProtocolError> {
        let Some((can_act, has_yardrat)) = self.shared.clients.authenticated.get(&sid).map(|e| {
            let can_act =
                e.snapshot.stats.hp > 0 && !super::super::router_shop::is_in_trade(e.snapshot.id);
            let has_yardrat = e.is_admin
                || e.snapshot.items_body.iter().flatten().any(|it| {
                    (592..=594).contains(&it.template_id.0) || it.options.iter().any(|o| o.id == 33)
                });
            (can_act, has_yardrat)
        }) else {
            return Ok(());
        };
        if !can_act {
            return send_thong_bao(session, "Không thể dịch chuyển lúc này!");
        }
        if !has_yardrat {
            return send_thong_bao(
                session,
                "Cần mặc cải trang Yardrat để dịch chuyển tới người chơi!",
            );
        }
        let target_loc = self.shared.clients.players.get(&target_pid).and_then(|s| {
            self.shared
                .clients
                .authenticated
                .get(s.value())
                .map(|e| e.snapshot.location)
        });
        if let Some(tloc) = target_loc {
            let non_normal = self
                .shared
                .templates
                .maps
                .iter()
                .find(|m| m.id == tloc.map_id)
                .is_some_and(|m| m.map_type != const_map::MAP_NORMAL);
            if non_normal
                || matches!(
                    tloc.map_id.0,
                    21..=23 | 39..=41 | 45..=51 | 53..=62 | 85..=91 | 112..=120 | 127..=129
                )
                || tloc.map_id.0 >= 135
            {
                return send_thong_bao(session, "Không thể dịch chuyển đến khu vực này!");
            }
            self.switch_player_zone(
                session,
                sid,
                Some(tloc.map_id),
                tloc.zone_id,
                Some((tloc.x, tloc.y)),
            )
            .await
        } else {
            session
                .send(s2c_packets::thong_bao("Không tìm thấy người chơi!")?)
                .await
        }
    }
}

pub(in crate::router) fn apply_session_buffs_before_clamp(sid: u64, stats: &mut CoreStats) {
    let Some(buffs) = session_buffs().get(&sid) else {
        return;
    };
    let now_ms = unix_now_ms();
    let has = |p: fn(BuffSlot) -> bool| buffs.has_active(now_ms, p);
    if has(|s| s == BuffSlot::BoHuyet(true)) {
        stats.hp_max = stats.hp_max.saturating_mul(22) / 10;
    } else if has(|s| s == BuffSlot::BoHuyet(false)) {
        stats.hp_max = stats.hp_max.saturating_mul(2);
    }
    if has(|s| s == BuffSlot::BoKhi(true)) {
        stats.mp_max = stats.mp_max.saturating_mul(22) / 10;
    } else if has(|s| s == BuffSlot::BoKhi(false)) {
        stats.mp_max = stats.mp_max.saturating_mul(2);
    }
    if has(|s| s == BuffSlot::CuongNo(true)) {
        stats.dame = stats.dame.saturating_mul(22) / 10;
    } else if has(|s| s == BuffSlot::CuongNo(false)) {
        stats.dame = stats.dame.saturating_mul(2);
    }
    if has(|s| matches!(s, BuffSlot::MealTier1(_))) {
        stats.dame = stats.dame.saturating_add(stats.dame / 10);
    }
    if has(|s| matches!(s, BuffSlot::MealTier2(8060 | 880))) {
        stats.dame = stats.dame.saturating_add(stats.dame / 20);
    }
    if has(|s| matches!(s, BuffSlot::MealTier2(8062 | 882))) {
        stats.hp_max = stats.hp_max.saturating_add(stats.hp_max / 20);
    }
    for t in 1..=3_u8 {
        if buffs.has_active(now_ms, |s| s == BuffSlot::NuocMia(t)) {
            stats.hp_max = stats.hp_max.saturating_add(stats.hp_max / 10);
        }
    }
    if has(|s| s == BuffSlot::NuocMia(2)) {
        stats.crit = stats.crit.saturating_add(10);
    }
    if has(|s| s == BuffSlot::NuocMia(3)) {
        stats.dame = stats.dame.saturating_add(stats.dame / 10);
        stats.def = stats.def.saturating_add(stats.def / 10);
    }
    let max_i32 = i64::from(i32::MAX);
    stats.hp_max = stats.hp_max.clamp(1, max_i32);
    stats.mp_max = stats.mp_max.clamp(1, max_i32);
    stats.dame = stats.dame.clamp(0, max_i32);
}
