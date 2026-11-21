//! Magic Tree handlers and state/packet helpers.

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_core::PlayerId;
use nro_db::player_repo::PlayerSnapshot;
use nro_economy::{InventoryError, MagicTreeState, UseItemError};
use nro_protocol::{Packet, PacketError, ProtocolError, SessionHandle};
use nro_world::s2c_packets;

use super::{
    ServerSessionHandler, send_thong_bao, snapshot_to_economy_inv, sync_economy_inv_to_snapshot,
    wallet_money_pkt,
};
use crate::runtime::unix_now_ms;

static MAGIC_TREE_MENU_STATE: OnceLock<DashMap<PlayerId, bool>> = OnceLock::new();

fn magic_tree_menu_state() -> &'static DashMap<PlayerId, bool> {
    MAGIC_TREE_MENU_STATE.get_or_init(DashMap::new)
}

pub(super) fn clear_magic_tree_menu_state(pid: PlayerId) {
    magic_tree_menu_state().remove(&pid);
}

enum MagicTreeAction {
    Harvest,
    StartUpgrade,
    FastFinish,
    CancelUpgrade,
}

impl ServerSessionHandler {
    pub(super) async fn handle_magic_tree(&self, session: &SessionHandle, sid: u64, action: i8) -> Result<(), ProtocolError> {
        let now_ms = unix_now_ms();
        let Some(snap) = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
            tick_magic_tree(&mut e.snapshot, now_ms);
            e.snapshot.clone()
        }) else {
            return Ok(());
        };
        match action {
            1 => session.send(build_magic_tree_menu_pkt(&snap)?).await?,
            0 | 2 => session.send(build_magic_tree_load(&snap)?).await?,
            _ => {}
        }
        Ok(())
    }

    pub(super) async fn handle_magic_tree_confirm(&self, session: &SessionHandle, sid: u64, menu_id: i8) -> Result<(), ProtocolError> {
        let now_ms = unix_now_ms();
        let mut harvested_ok = false;
        let outcome = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
            if !matches!(entry.snapshot.location.map_id.0, 21..=23) {
                return (Vec::new(), None);
            }
            if super::router_shop::is_in_trade(entry.snapshot.id) {
                return (Vec::new(), Some("Không thể thao tác khi đang giao dịch!".to_owned()));
            }
            tick_magic_tree(&mut entry.snapshot, now_ms);
            let mut mt = snapshot_to_econ_magic_tree(&entry.snapshot);
            if let Some((_, cached_upgrading)) = magic_tree_menu_state().remove(&entry.snapshot.id)
                && cached_upgrading != mt.is_upgrading
            {
                return (
                    Vec::new(),
                    Some("Trạng thái cây đậu đã thay đổi, vui lòng mở lại menu!".to_owned()),
                );
            }
            let action = resolve_magic_tree_action(&mt, menu_id);
            let Some(action) = action else {
                return (Vec::new(), None);
            };
            let mut inv = snapshot_to_economy_inv(&entry.snapshot);
            match action {
                MagicTreeAction::Harvest => match mt.harvest(&mut inv, now_ms) {
                    Ok(0) => (Vec::new(), Some("Chưa có đậu thần để thu hoạch!".to_owned())),
                    Ok(harvested) => {
                        sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                        sync_econ_magic_tree_to_snapshot(&mt, &mut entry.snapshot);
                        harvested_ok = true;
                        (
                            vec![
                                s2c_packets::magic_tree_harvest(0, i32::from(mt.level) * 60),
                                s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                            ],
                            Some(format!("Bạn vừa thu hoạch được {harvested} hạt đậu thần!")),
                        )
                    }
                    Err(UseItemError::Inventory(InventoryError::ContainerFull(_))) => (
                        Vec::new(),
                        Some("Hành trang đã đầy, không thể thu hoạch đậu!".to_owned()),
                    ),
                    Err(_) => (Vec::new(), Some("Không thể thu hoạch đậu thần lúc này!".to_owned())),
                },
                MagicTreeAction::StartUpgrade => match mt.start_upgrade(&mut inv, now_ms) {
                    Ok(()) => {
                        sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                        sync_econ_magic_tree_to_snapshot(&mt, &mut entry.snapshot);
                        let mut pkts = Vec::with_capacity(2);
                        if let Ok(load_pkt) = build_magic_tree_load(&entry.snapshot) {
                            pkts.push(load_pkt);
                        }
                        pkts.push(wallet_money_pkt(&entry.snapshot.wallet));
                        let next_lv = (mt.level + 1).min(10);
                        (pkts, Some(format!("Bắt đầu nâng cấp Đậu thần lên cấp {next_lv}!")))
                    }
                    Err(UseItemError::Inventory(InventoryError::InsufficientCurrency(_, req, _))) => (
                        Vec::new(),
                        Some(format!("Không đủ vàng để nâng cấp Đậu thần (cần {req} vàng)!")),
                    ),
                    Err(UseItemError::PreconditionFailed(_)) => (
                        Vec::new(),
                        Some("Đậu thần đã đạt cấp tối đa hoặc đang nâng cấp!".to_owned()),
                    ),
                    Err(_) => (Vec::new(), Some("Không thể nâng cấp Đậu thần lúc này!".to_owned())),
                },
                MagicTreeAction::FastFinish => {
                    let was_upgrading = mt.is_upgrading;
                    match mt.fast_finish(&mut inv, now_ms) {
                        Ok(()) => {
                            sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                            sync_econ_magic_tree_to_snapshot(&mt, &mut entry.snapshot);
                            let mut pkts = Vec::with_capacity(2);
                            if let Ok(load_pkt) = build_magic_tree_load(&entry.snapshot) {
                                pkts.push(load_pkt);
                            }
                            pkts.push(wallet_money_pkt(&entry.snapshot.wallet));
                            let msg = if was_upgrading {
                                format!("Đã nâng cấp nhanh Đậu thần lên cấp {}!", mt.level)
                            } else {
                                "Đậu thần đã kết hạt xong!".to_owned()
                            };
                            (pkts, Some(msg))
                        }
                        Err(UseItemError::Inventory(InventoryError::InsufficientCurrency(_, req, _))) => (
                            Vec::new(),
                            Some(format!("Không đủ ngọc xanh (cần {req} ngọc)!")),
                        ),
                        Err(_) => (Vec::new(), Some("Không thể hoàn thành nhanh lúc này!".to_owned())),
                    }
                }
                MagicTreeAction::CancelUpgrade => match mt.cancel_upgrade(&mut inv, now_ms) {
                    Ok(refund) => {
                        sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                        sync_econ_magic_tree_to_snapshot(&mt, &mut entry.snapshot);
                        let mut pkts = Vec::with_capacity(2);
                        if let Ok(load_pkt) = build_magic_tree_load(&entry.snapshot) {
                            pkts.push(load_pkt);
                        }
                        pkts.push(wallet_money_pkt(&entry.snapshot.wallet));
                        (pkts, Some(if refund > 0 { format!("Đã hủy nâng cấp Đậu thần, hoàn lại {refund} vàng!") } else { "Đã hủy nâng cấp Đậu thần!".to_owned() }))
                    }
                    Err(_) => (
                        Vec::new(),
                        Some("Đậu thần không đang trong quá trình nâng cấp!".to_owned()),
                    ),
                },
            }
        });
        if let Some((pkts, msg)) = outcome {
            for pkt in pkts {
                session.send(pkt).await?;
            }
            if let Some(text) = msg {
                send_thong_bao(session, &text)?;
            }
        }
        if harvested_ok {
            self.advance_task_on_harvest(session, sid).await?;
        }
        Ok(())
    }
}

fn snapshot_to_econ_magic_tree(snap: &PlayerSnapshot) -> MagicTreeState {
    let mt = &snap.magic_tree;
    let level = mt.level.clamp(1, 10).cast_unsigned();
    let max_peas = 5 + level.saturating_sub(1) * 2;
    let curr_peas = mt.curr_peas.max(0).cast_unsigned().min(max_peas);
    MagicTreeState {
        level,
        curr_peas,
        is_upgrading: mt.is_upgrade,
        timer_ms: if mt.is_upgrade {
            mt.last_time_upgrade_ms
        } else {
            mt.last_time_harvest_ms
        },
    }
}

fn sync_econ_magic_tree_to_snapshot(mt: &MagicTreeState, snap: &mut PlayerSnapshot) {
    snap.magic_tree.level = mt.level.clamp(1, 10) as i8;
    snap.magic_tree.curr_peas = mt.curr_peas.min(mt.max_peas()) as i8;
    snap.magic_tree.is_upgrade = mt.is_upgrading;
    if mt.is_upgrading {
        snap.magic_tree.last_time_upgrade_ms = mt.timer_ms;
    } else {
        snap.magic_tree.last_time_harvest_ms = mt.timer_ms;
    }
}

fn resolve_magic_tree_action(mt: &MagicTreeState, menu_id: i8) -> Option<MagicTreeAction> {
    if mt.is_upgrading {
        match menu_id {
            0 => Some(MagicTreeAction::FastFinish),
            1 => Some(MagicTreeAction::CancelUpgrade),
            _ => None,
        }
    } else {
        let can_upgrade = mt.upgrade_gold_cost().is_some();
        let can_fast_harvest = mt.curr_peas < mt.max_peas();
        match menu_id {
            0 => Some(MagicTreeAction::Harvest),
            1 if can_upgrade => Some(MagicTreeAction::StartUpgrade),
            1 if can_fast_harvest => Some(MagicTreeAction::FastFinish),
            2 if can_upgrade && can_fast_harvest => Some(MagicTreeAction::FastFinish),
            _ => None,
        }
    }
}

pub(super) fn build_magic_tree_menu_pkt(snap: &PlayerSnapshot) -> Result<Packet, PacketError> {
    let mt = snapshot_to_econ_magic_tree(snap);
    magic_tree_menu_state().insert(snap.id, mt.is_upgrading);
    let mut owned: Vec<String> = Vec::with_capacity(3);
    if mt.is_upgrading {
        owned.push(format!("Nâng cấp nhanh\n({} ngọc)", mt.fast_upgrade_gem_cost()));
        owned.push("Hủy nâng cấp".to_owned());
    } else {
        owned.push("Thu hoạch".to_owned());
        if let Some(gold) = mt.upgrade_gold_cost() {
            owned.push(if gold == 0 { "Nâng cấp\n(Miễn phí)".to_owned() } else { format!("Nâng cấp\n({gold} vàng)") });
        }
        if mt.curr_peas < mt.max_peas() {
            owned.push(format!("Kết hạt nhanh\n({} ngọc)", mt.fast_harvest_gem_cost()));
        }
    }
    let refs: Vec<&str> = owned.iter().map(String::as_str).collect();
    s2c_packets::magic_tree_menu(&refs)
}

pub(super) fn tick_magic_tree(snap: &mut PlayerSnapshot, now_ms: i64) {
    let mut mt = snapshot_to_econ_magic_tree(snap);
    mt.tick(now_ms);
    sync_econ_magic_tree_to_snapshot(&mt, snap);
    if snap.main_task.task_id == 0
        && snap.main_task.sub_index == 4
        && !snap.magic_tree.is_upgrade
        && snap.magic_tree.curr_peas <= 0
    {
        snap.magic_tree.curr_peas = 1;
    }
}

pub(super) fn build_magic_tree_load(snap: &PlayerSnapshot) -> Result<Packet, PacketError> {
    let mt = &snap.magic_tree;
    let level = mt.level.clamp(1, 10);
    let max_peas = i16::from(5 + (level - 1) * 2);
    let curr_peas = i16::from(mt.curr_peas.max(0)).min(max_peas);
    let (x, y, icon_id) = match i8::from(snap.gender) {
        1 => (372_i16, 336_i16, 84_i16),
        2 => (348_i16, 336_i16, 372_i16),
        _ => (348_i16, 336_i16, 84_i16),
    };
    let name = format!("Đậu thần cấp {level}");
    let now_ms = unix_now_ms();
    let seconds = if mt.is_upgrade {
        ((mt.last_time_upgrade_ms.saturating_sub(now_ms)) / 1_000).max(0) as i32
    } else if curr_peas < max_peas {
        let interval = i64::from(level) * 60;
        let elapsed_secs = (now_ms.saturating_sub(mt.last_time_harvest_ms) / 1_000).max(0);
        (interval - (elapsed_secs % interval)) as i32
    } else {
        0
    };
    s2c_packets::magic_tree_load(icon_id, &name, x, y, level, curr_peas, max_peas, &name, seconds, mt.is_upgrade)
}
