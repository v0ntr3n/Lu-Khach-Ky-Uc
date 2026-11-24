//! Pet (`Đệ tử`) grant, type-switching, and power-milestone operations for [`AdminControl`].

use nro_core::AccountId;
use nro_db::player_repo::PlayerSnapshot;
use nro_world::{ZoneCommand, s2c_packets};
use tracing::info;

use super::AdminControl;
use super::admin_control_ops::try_send_if_ready;
use crate::router::{
    self, effective_appearance, pkt_fusion, recalculate_session_stats, recalculate_snapshot_stats,
    sprite_restore_packets,
};
use crate::runtime::SharedServerState;

/// Hard ceiling on Pet (`Đệ tử`) power and potential (`200_000_000_000`).
const MAX_PET_POWER: i64 = 200_000_000_000;

/// Maps `PetSnapshot::type_pet` (`0..=4`) to its Vietnamese display label.
#[must_use]
pub const fn pet_type_label(type_pet: i8) -> &'static str {
    match type_pet {
        0 => "Thường",
        1 => "Mabư",
        2 => "Uub",
        3 => "Kid Beer",
        4 => "Kid Jiren",
        _ => "Khác",
    }
}

/// Extracts `(has_pet, pet_name, pet_type, pet_power)` from an optional [`PlayerSnapshot`].
pub(super) fn pet_summary(snap: Option<&PlayerSnapshot>) -> (bool, String, i8, i64) {
    let Some(pet) = snap.and_then(|s| s.pet.as_ref()) else {
        return (false, String::new(), -1, 0);
    };
    let name = if pet.name.is_empty() {
        "Đệ tử".to_owned()
    } else {
        pet.name.clone()
    };
    (true, name, pet.type_pet, pet.stats.power)
}

impl AdminControl {
    /// Grants a fresh Pet (`type_pet` in `0..=4`) or switches an existing Pet's type while
    /// preserving any equipped `items_body` slots and cleanly unfusing fused masters.
    pub fn grant_or_set_pet(&self, account_id: i32, type_pet: i8) -> Result<String, String> {
        if !(0..=4).contains(&type_pet) {
            return Err(format!(
                "Loại Đệ tử không hợp lệ ({type_pet}); cần giá trị từ 0 đến 4."
            ));
        }

        let label = pet_type_label(type_pet);
        if let Some(sid) = self.find_online_session_id(account_id) {
            let online = if let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid) {
                if router::is_player_in_trade(entry.snapshot.id) {
                    return Err("Người chơi đang giao dịch, vui lòng thử lại sau!".to_owned());
                }
                let was_fused = entry
                    .snapshot
                    .pet
                    .as_ref()
                    .is_some_and(|p| p.fusion_type != 0);
                apply_fresh_pet_preserving_gear(&self.shared, &mut entry.snapshot, type_pet);
                recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                Some((
                    entry.snapshot.id,
                    entry.entity_key,
                    was_fused,
                    entry.snapshot.clone(),
                    entry.handle.clone(),
                ))
            } else {
                None
            };

            if let Some((pid, ek_opt, was_fused, snap, handle)) = online {
                self.persist_snapshot(account_id, &snap)?;
                if was_fused {
                    let (eh, eb, el, _, is_disg) =
                        effective_appearance(&self.shared.templates, &snap);
                    try_send_if_ready(&handle, account_id, pkt_fusion(pid, 1));
                    try_send_if_ready(
                        &handle,
                        account_id,
                        s2c_packets::inventory_body(eh, &snap.items_body),
                    );
                    for p in sprite_restore_packets(pid, eh, eb, el, is_disg)
                        .into_iter()
                        .flatten()
                    {
                        try_send_if_ready(&handle, account_id, p);
                    }
                    if let Some(ek) = ek_opt
                        && let Ok(ztx) = self
                            .shared
                            .world_router
                            .resolve_sender(snap.location.map_id, snap.location.zone_id)
                    {
                        let _ = ztx.try_send(ZoneCommand::UpdatePlayerParts(ek, (eh, eb, el)));
                        let _ = ztx.try_send(ZoneCommand::UpdatePlayerStats(ek, snap.stats));
                    }
                }
                try_send_if_ready(&handle, account_id, s2c_packets::me_load_point(&snap.stats));
                try_send_if_ready(&handle, account_id, s2c_packets::have_pet(true));
                if let Some(detail_pkt) =
                    crate::router_pet::build_pet_detail_pkt(&self.shared, &snap)
                {
                    try_send_if_ready(&handle, account_id, detail_pkt);
                }
                if let Ok(toast) =
                    s2c_packets::thong_bao(&format!("Admin đã cấp Đệ tử {label} cho bạn!"))
                {
                    try_send_if_ready(&handle, account_id, toast);
                }
                let _ = self.run_async(crate::router_pet::sync_online_pet_map_state(
                    &self.shared,
                    pid,
                ));
                info!(
                    account_id,
                    player_id = pid.0,
                    type_pet,
                    was_fused,
                    "admin UI granted/switched online pet"
                );
                return Ok(format!(
                    "Đã cấp/đổi Đệ tử {label} (online) cho tài khoản #{account_id}."
                ));
            }
        }

        let mut snap = self.load_offline_snapshot(account_id)?;
        apply_fresh_pet_preserving_gear(&self.shared, &mut snap, type_pet);
        recalculate_snapshot_stats(&mut snap, self.shared.config.parity_mode);
        self.persist_snapshot(account_id, &snap)?;
        info!(
            account_id,
            player_id = snap.id.0,
            type_pet,
            "admin UI granted/switched offline pet"
        );
        Ok(format!(
            "Đã cấp/đổi Đệ tử {label} (offline) cho tài khoản #{account_id}."
        ))
    }

    /// Adds `delta_power` (saturating, capped at `200_000_000_000`) to both `pet.stats.power`
    /// and `pet.stats.tiem_nang`, recalculates stats/skills (and fused master stats), and syncs map state.
    pub fn add_pet_power(&self, account_id: i32, delta_power: i64) -> Result<String, String> {
        if delta_power <= 0 {
            return Err("Lượng sức mạnh cộng thêm cho Đệ tử phải > 0.".to_owned());
        }

        if let Some(sid) = self.find_online_session_id(account_id) {
            let online = if let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid) {
                if router::is_player_in_trade(entry.snapshot.id) {
                    return Err("Người chơi đang giao dịch, vui lòng thử lại sau!".to_owned());
                }
                let (pet_name, type_pet, new_power) =
                    apply_pet_power_delta(&self.shared, &mut entry.snapshot, delta_power)?;
                let is_fused = entry
                    .snapshot
                    .pet
                    .as_ref()
                    .is_some_and(|p| p.fusion_type != 0);
                if is_fused {
                    recalculate_session_stats(
                        sid,
                        &mut entry.snapshot,
                        self.shared.config.parity_mode,
                    );
                }
                Some((
                    entry.snapshot.id,
                    entry.entity_key,
                    is_fused,
                    entry.snapshot.clone(),
                    entry.handle.clone(),
                    pet_name,
                    type_pet,
                    new_power,
                ))
            } else {
                None
            };

            if let Some((pid, ek_opt, is_fused, snap, handle, pet_name, type_pet, new_power)) =
                online
            {
                self.persist_snapshot(account_id, &snap)?;
                if is_fused {
                    try_send_if_ready(&handle, account_id, s2c_packets::me_load_point(&snap.stats));
                    if let Some(ek) = ek_opt
                        && let Ok(ztx) = self
                            .shared
                            .world_router
                            .resolve_sender(snap.location.map_id, snap.location.zone_id)
                    {
                        let _ = ztx.try_send(ZoneCommand::UpdatePlayerStats(ek, snap.stats));
                    }
                }
                if let Some(detail_pkt) =
                    crate::router_pet::build_pet_detail_pkt(&self.shared, &snap)
                {
                    try_send_if_ready(&handle, account_id, detail_pkt);
                }
                if let Ok(toast) = s2c_packets::thong_bao(&format!(
                    "Admin đã cộng +{delta_power} SM cho Đệ tử {pet_name} (SM hiện tại: {new_power})!"
                )) {
                    try_send_if_ready(&handle, account_id, toast);
                }
                let _ = self.run_async(crate::router_pet::sync_online_pet_map_state(
                    &self.shared,
                    pid,
                ));
                info!(
                    account_id,
                    player_id = pid.0,
                    delta_power,
                    type_pet,
                    new_power,
                    "admin UI added online pet power"
                );
                return Ok(format!(
                    "Đã cộng +{delta_power} SM cho Đệ tử {pet_name} ({}) online của TK #{account_id} (SM: {new_power}).",
                    pet_type_label(type_pet),
                ));
            }
        }

        let mut snap = self.load_offline_snapshot(account_id)?;
        let (pet_name, type_pet, new_power) =
            apply_pet_power_delta(&self.shared, &mut snap, delta_power)?;
        if snap.pet.as_ref().is_some_and(|p| p.fusion_type != 0) {
            recalculate_snapshot_stats(&mut snap, self.shared.config.parity_mode);
        }
        self.persist_snapshot(account_id, &snap)?;
        info!(
            account_id,
            player_id = snap.id.0,
            delta_power,
            type_pet,
            new_power,
            "admin UI added offline pet power"
        );
        Ok(format!(
            "Đã cộng +{delta_power} SM cho Đệ tử {pet_name} ({}) offline của TK #{account_id} (SM: {new_power}).",
            pet_type_label(type_pet),
        ))
    }

    fn find_online_session_id(&self, account_id: i32) -> Option<u64> {
        if let Some(sid) = self
            .shared
            .clients
            .accounts
            .get(&AccountId(account_id))
            .map(|v| *v.value())
        {
            return Some(sid);
        }
        self.shared.clients.authenticated.iter().find_map(|entry| {
            (entry.value().snapshot.account_id == Some(AccountId(account_id)))
                .then_some(*entry.key())
        })
    }
}

fn apply_fresh_pet_preserving_gear(
    shared: &SharedServerState,
    snap: &mut PlayerSnapshot,
    type_pet: i8,
) {
    let mut fresh = crate::router_pet::make_fresh_pet(type_pet, snap.gender);
    if let Some(prev_pet) = snap.pet.take() {
        for (idx, maybe_item) in prev_pet.items_body.into_iter().enumerate() {
            if let Some(item) = maybe_item {
                while fresh.items_body.len() <= idx {
                    fresh.items_body.push(None);
                }
                fresh.items_body[idx] = Some(item);
            }
        }
    }
    snap.pet = Some(fresh);
    crate::router_pet::recalculate_pet_in_snapshot(shared, snap);
}

fn apply_pet_power_delta(
    shared: &SharedServerState,
    snap: &mut PlayerSnapshot,
    delta_power: i64,
) -> Result<(String, i8, i64), String> {
    let Some(pet) = snap.pet.as_mut() else {
        return Err("Người chơi chưa có Đệ tử!".into());
    };
    pet.stats.power = pet
        .stats
        .power
        .saturating_add(delta_power)
        .min(MAX_PET_POWER);
    pet.stats.tiem_nang = pet
        .stats
        .tiem_nang
        .saturating_add(delta_power)
        .min(MAX_PET_POWER);
    let pet_name = if pet.name.is_empty() {
        "Đệ tử".to_owned()
    } else {
        pet.name.clone()
    };
    let type_pet = pet.type_pet;
    crate::router_pet::recalculate_pet_in_snapshot(shared, snap);
    let new_power = snap.pet.as_ref().map_or(0, |updated| updated.stats.power);
    Ok((pet_name, type_pet, new_power))
}
