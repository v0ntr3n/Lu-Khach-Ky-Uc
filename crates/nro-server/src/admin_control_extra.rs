//! Extended item-search, player heal/notify, bulk online operations, and row projection for [`AdminControl`].

use nro_core::AccountId;
use nro_db::player_repo::PlayerSnapshot;
use nro_db::templates::MapTemplateRecord;
use nro_protocol::SessionHandle;
use nro_world::s2c_packets;
use tracing::info;

use super::admin_control_ops::try_send_if_ready;
use super::admin_control_pet::pet_summary;
use super::{AdminControl, AdminItemInfo, AdminUserRow};

impl AdminControl {
    /// Searches item templates by ID or case-insensitive name, prioritizing exact ID matches.
    #[must_use]
    pub fn search_items(&self, query: &str, limit: usize) -> Vec<AdminItemInfo> {
        if limit == 0 {
            return Vec::new();
        }
        let q = query.trim();
        if q.is_empty() {
            return self
                .shared
                .templates
                .items
                .iter()
                .filter_map(|it| {
                    let id = i16::try_from(it.id).ok()?;
                    Some(AdminItemInfo {
                        id,
                        name: it.name.clone(),
                        stackable: it.is_up_to_up,
                    })
                })
                .take(limit)
                .collect();
        }

        let q_lower = q.to_lowercase();
        let mut exact = Vec::new();
        let mut partial = Vec::new();
        for it in &self.shared.templates.items {
            let Ok(id) = i16::try_from(it.id) else {
                continue;
            };
            let id_str = it.id.to_string();
            if id_str == q {
                exact.push(AdminItemInfo {
                    id,
                    name: it.name.clone(),
                    stackable: it.is_up_to_up,
                });
            } else if partial.len() < limit
                && (id_str.contains(q) || it.name.to_lowercase().contains(&q_lower))
            {
                partial.push(AdminItemInfo {
                    id,
                    name: it.name.clone(),
                    stackable: it.is_up_to_up,
                });
            }
        }
        exact.extend(partial);
        exact.truncate(limit);
        exact
    }

    /// Restores full HP and KI for an online player, persists the snapshot, and updates the client.
    pub fn heal_player(&self, account_id: i32) -> Result<String, String> {
        let Some(sid) = self
            .shared
            .clients
            .accounts
            .get(&AccountId(account_id))
            .map(|v| *v.value())
        else {
            return Err(format!("Tài khoản #{account_id} hiện không online."));
        };

        let online = if let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid) {
            let max_hp = entry.snapshot.stats.hp_max.max(1);
            let max_mp = entry.snapshot.stats.mp_max.max(1);
            entry.snapshot.stats.hp = max_hp;
            entry.snapshot.stats.mp = max_mp;
            Some((entry.snapshot.clone(), entry.handle.clone()))
        } else {
            None
        };

        let Some((snap, handle)) = online else {
            return Err(format!("Tài khoản #{account_id} hiện không online."));
        };

        self.persist_snapshot(account_id, &snap)?;
        try_send_if_ready(&handle, account_id, s2c_packets::me_load_point(&snap.stats));
        if let Ok(notify) = s2c_packets::thong_bao("Admin đã hồi đầy HP và KI cho bạn!") {
            try_send_if_ready(&handle, account_id, notify);
        }
        info!(account_id, "admin UI healed online player");
        Ok(format!("Đã hồi đầy HP và KI cho tài khoản #{account_id}."))
    }

    /// Sends a direct server notification message (`thong_bao`) to a single online player.
    pub fn notify_player(&self, account_id: i32, message: &str) -> Result<String, String> {
        let trimmed = message.trim();
        if trimmed.is_empty() {
            return Err("Nội dung thông báo không được để trống.".to_owned());
        }

        let Some(sid) = self
            .shared
            .clients
            .accounts
            .get(&AccountId(account_id))
            .map(|v| *v.value())
        else {
            return Err(format!("Tài khoản #{account_id} hiện không online."));
        };

        let Some(handle) = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .map(|entry| entry.handle.clone())
        else {
            return Err(format!("Tài khoản #{account_id} hiện không online."));
        };

        let pkt = s2c_packets::thong_bao(trimmed)
            .map_err(|err| format!("Lỗi tạo gói tin thông báo: {err}"))?;
        try_send_if_ready(&handle, account_id, pkt);
        info!(account_id, message = trimmed, "admin UI notified player");
        Ok(format!(
            "Đã gửi thông báo riêng tới tài khoản #{account_id}."
        ))
    }

    /// Persists snapshots for all currently online authenticated players to the database.
    #[allow(clippy::unnecessary_wraps)]
    pub fn save_all_online(&self) -> Result<String, String> {
        let snapshots: Vec<(i32, PlayerSnapshot)> = self
            .shared
            .clients
            .authenticated
            .iter()
            .filter_map(|entry| {
                entry
                    .value()
                    .snapshot
                    .account_id
                    .map(|aid| (aid.0, entry.value().snapshot.clone()))
            })
            .collect();

        let total = snapshots.len();
        let mut saved = 0_usize;
        let mut failed = 0_usize;
        for (account_id, snap) in &snapshots {
            if self.persist_snapshot(*account_id, snap).is_ok() {
                saved += 1;
            } else {
                failed += 1;
            }
        }
        info!(saved, failed, total, "admin UI saved all online players");
        if failed == 0 {
            Ok(format!("Đã lưu {saved}/{total} nhân vật online xuống DB."))
        } else {
            Ok(format!(
                "Đã lưu {saved}/{total} nhân vật online xuống DB ({failed} lỗi)."
            ))
        }
    }

    /// Disconnects all currently online authenticated players.
    #[allow(clippy::unnecessary_wraps)]
    pub fn kick_all_online(&self) -> Result<String, String> {
        let handles: Vec<SessionHandle> = self
            .shared
            .clients
            .authenticated
            .iter()
            .map(|entry| entry.value().handle.clone())
            .collect();
        let count = handles.len();
        for handle in &handles {
            handle.disconnect();
        }
        info!(count, "admin UI disconnected all online players");
        Ok(format!("Đã ngắt kết nối {count} người chơi online."))
    }
}

pub(super) fn matches_query(
    needle: &str,
    acc_id: i32,
    username: &str,
    pid: Option<i64>,
    pname: &str,
) -> bool {
    if needle.is_empty() {
        return true;
    }
    acc_id.to_string().contains(needle)
        || pid.is_some_and(|p| p.to_string().contains(needle))
        || username.to_ascii_lowercase().contains(needle)
        || pname.to_ascii_lowercase().contains(needle)
}

pub(super) fn row_from_snap(
    account_id: i32,
    username: String,
    snap: Option<PlayerSnapshot>,
    maps: &[MapTemplateRecord],
    online: bool,
    ban: bool,
    active: bool,
    is_admin: bool,
    vnd: i32,
    fallback_gold: i64,
) -> AdminUserRow {
    let (has_pet, pet_name, pet_type, pet_power) = pet_summary(snap.as_ref());
    let (player_id, player_name, map_id, map_name, zone_id, power, gold, gem, ruby) = snap
        .map_or_else(
            || {
                (
                    None,
                    "(Chưa tạo NV)".to_owned(),
                    0,
                    "-".to_owned(),
                    0,
                    0,
                    fallback_gold,
                    0,
                    0,
                )
            },
            |s| {
                let map_name = maps.iter().find(|m| m.id == s.location.map_id).map_or_else(
                    || format!("Map {}", s.location.map_id.0),
                    |m| m.name.clone(),
                );
                (
                    Some(s.id.0),
                    s.name,
                    s.location.map_id.0,
                    map_name,
                    s.location.zone_id.0,
                    s.stats.power,
                    s.wallet.gold,
                    s.wallet.gem,
                    s.wallet.ruby,
                )
            },
        );
    AdminUserRow {
        account_id,
        username,
        player_id,
        player_name,
        online,
        ban,
        active,
        is_admin,
        map_id,
        map_name,
        zone_id,
        power,
        gold,
        gem,
        ruby,
        vnd,
        has_pet,
        pet_name,
        pet_type,
        pet_power,
    }
}
