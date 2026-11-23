//! Account moderation, VND, currency, and item-grant operations for [`AdminControl`].

use nro_core::{AccountId, ItemInstance, TemplateId};
use nro_db::player_repo::{LIMIT_GOLD, PlayerSnapshot};
use nro_protocol::{Packet, SessionHandle};
use nro_world::s2c_packets;
use smallvec::SmallVec;
use tracing::{info, warn};

use super::AdminControl;
use crate::router::{
    self, snapshot_to_economy_inv, sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use crate::runtime::unix_now_ms;

impl AdminControl {
    /// Disconnects the live session for `account_id` if currently online.
    pub fn kick_user(&self, account_id: i32) -> Result<String, String> {
        let Some(sid) = self
            .shared
            .clients
            .accounts
            .get(&AccountId(account_id))
            .map(|v| *v.value())
        else {
            return Err(format!("Tài khoản #{account_id} hiện không online."));
        };
        if let Some(handle) = self.shared.clients.sessions.get(&sid) {
            handle.disconnect();
            info!(account_id, "admin UI kicked online account");
            return Ok(format!("Đã kick tài khoản #{account_id}."));
        }
        Err(format!(
            "Không tìm thấy phiên kết nối cho tài khoản #{account_id}."
        ))
    }

    /// Updates `ban` flag in `fjall` and kicks the account immediately when banning.
    pub fn set_ban(&self, account_id: i32, ban: bool) -> Result<String, String> {
        let updated = self
            .shared
            .accounts_repo
            .set_ban(AccountId(account_id), ban)
            .map_err(|err| format!("Lỗi DB khi cập nhật ban: {err}"))?;
        if !updated {
            return Err(format!("Không tìm thấy tài khoản #{account_id}."));
        }
        if ban {
            let _ = self.kick_user(account_id);
        }
        info!(account_id, ban, "admin UI updated account ban status");
        Ok(if ban {
            format!("Đã khóa (ban) tài khoản #{account_id}.")
        } else {
            format!("Đã mở khóa (unban) tài khoản #{account_id}.")
        })
    }

    /// Updates member activation (`active`) in `fjall` and in live session state.
    pub fn set_active(&self, account_id: i32, active: bool) -> Result<String, String> {
        let updated = self
            .shared
            .accounts_repo
            .set_active(AccountId(account_id), active)
            .map_err(|err| format!("Lỗi DB khi cập nhật kích hoạt: {err}"))?;
        if !updated {
            return Err(format!("Không tìm thấy tài khoản #{account_id}."));
        }
        if let Some(sid) = self
            .shared
            .clients
            .accounts
            .get(&AccountId(account_id))
            .map(|v| *v.value())
            && let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid)
        {
            entry.activated = active;
        }
        info!(account_id, active, "admin UI updated account activation");
        Ok(if active {
            format!("Đã kích hoạt thành viên cho tài khoản #{account_id}.")
        } else {
            format!("Đã hủy kích hoạt thành viên cho tài khoản #{account_id}.")
        })
    }

    /// Adjusts account VND balance in `fjall`, returning an error if the account does not exist.
    pub fn add_vnd(&self, account_id: i32, delta: i64) -> Result<String, String> {
        if delta == 0 {
            return Err("Số VND cần cộng/trừ phải khác 0.".to_owned());
        }
        let exists = self
            .shared
            .accounts_repo
            .get_by_id(AccountId(account_id))
            .map_err(|err| format!("Lỗi DB khi kiểm tra tài khoản: {err}"))?
            .is_some();
        if !exists {
            return Err(format!("Không tìm thấy tài khoản #{account_id}."));
        }
        if delta > 0 {
            self.rt
                .block_on(
                    self.shared
                        .accounts_repo
                        .add_vnd(AccountId(account_id), delta),
                )
                .map_err(|err| format!("Lỗi cộng VND: {err}"))?;
            Ok(format!("Đã cộng +{delta} VND cho tài khoản #{account_id}."))
        } else {
            let amount = i32::try_from(delta.unsigned_abs()).unwrap_or(i32::MAX);
            let ok = self
                .rt
                .block_on(
                    self.shared
                        .accounts_repo
                        .sub_vnd(AccountId(account_id), amount),
                )
                .map_err(|err| format!("Lỗi trừ VND: {err}"))?;
            if ok {
                Ok(format!("Đã trừ {amount} VND của tài khoản #{account_id}."))
            } else {
                Err(format!(
                    "Tài khoản #{account_id} không đủ VND để trừ {amount}."
                ))
            }
        }
    }

    /// Grants or deducts gold/gem/ruby for an online or offline character, rejecting mid-trade mutations.
    pub fn grant_currency(
        &self,
        account_id: i32,
        gold_delta: i64,
        gem_delta: i32,
        ruby_delta: i32,
    ) -> Result<String, String> {
        if gold_delta == 0 && gem_delta == 0 && ruby_delta == 0 {
            return Err("Nhập ít nhất 1 giá trị Vàng / Ngọc / Hồng ngọc khác 0.".to_owned());
        }
        if let Some(sid) = self
            .shared
            .clients
            .accounts
            .get(&AccountId(account_id))
            .map(|v| *v.value())
        {
            let online = if let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid) {
                if router::is_player_in_trade(entry.snapshot.id) {
                    return Err("Người chơi đang giao dịch, vui lòng thử lại sau!".to_owned());
                }
                apply_wallet_deltas(&mut entry.snapshot, gold_delta, gem_delta, ruby_delta);
                let notify = s2c_packets::thong_bao(&format!(
                    "Admin cập nhật ví: Vàng {gold_delta:+}, Ngọc {gem_delta:+}, Hồng ngọc {ruby_delta:+}"
                ));
                Some((
                    entry.snapshot.clone(),
                    entry.handle.clone(),
                    wallet_money_pkt(&entry.snapshot.wallet),
                    notify,
                ))
            } else {
                None
            };
            if let Some((snap, handle, money_pkt, notify_pkt)) = online {
                self.persist_snapshot(account_id, &snap)?;
                try_send_if_ready(&handle, account_id, money_pkt);
                if let Ok(notify) = notify_pkt {
                    try_send_if_ready(&handle, account_id, notify);
                }
                return Ok(format!(
                    "Đã cập nhật tiền online cho tài khoản #{account_id}."
                ));
            }
        }
        let mut snap = self.load_offline_snapshot(account_id)?;
        apply_wallet_deltas(&mut snap, gold_delta, gem_delta, ruby_delta);
        self.persist_snapshot(account_id, &snap)?;
        Ok(format!(
            "Đã cập nhật tiền offline cho tài khoản #{account_id}."
        ))
    }

    /// Gives `qty` of `template_id` into the player's bag (online or offline).
    pub fn give_item(&self, account_id: i32, template_id: i16, qty: u32) -> Result<String, String> {
        if template_id < 0 || qty == 0 {
            return Err("Template ID phải >= 0 và số lượng phải > 0.".to_owned());
        }
        let Some(tpl) = self
            .shared
            .templates
            .items
            .iter()
            .find(|it| it.id == i32::from(template_id))
        else {
            return Err(format!("Item template {template_id} not found"));
        };
        let allow_stack = tpl.is_up_to_up;
        let item = ItemInstance {
            template_id: TemplateId(template_id),
            quantity: qty,
            options: SmallVec::new(),
            create_time_ms: unix_now_ms(),
        };
        if let Some(sid) = self
            .shared
            .clients
            .accounts
            .get(&AccountId(account_id))
            .map(|v| *v.value())
        {
            let online = if let Some(mut entry) = self.shared.clients.authenticated.get_mut(&sid) {
                if router::is_player_in_trade(entry.snapshot.id) {
                    return Err("Người chơi đang giao dịch, vui lòng thử lại sau!".to_owned());
                }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                inv.add_to_bag(item.clone(), allow_stack)
                    .map_err(|err| format!("Hành trang đầy hoặc lỗi thêm đồ: {err}"))?;
                sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                let bag_pkt = s2c_packets::inventory_bag(&entry.snapshot.items_bag);
                let notify = s2c_packets::thong_bao(&format!(
                    "Admin vừa tặng bạn x{qty} vật phẩm #{template_id}!"
                ));
                Some((
                    entry.snapshot.clone(),
                    entry.handle.clone(),
                    bag_pkt,
                    notify,
                ))
            } else {
                None
            };
            if let Some((snap, handle, bag_pkt, notify_pkt)) = online {
                self.persist_snapshot(account_id, &snap)?;
                try_send_if_ready(&handle, account_id, bag_pkt);
                if let Ok(msg) = notify_pkt {
                    try_send_if_ready(&handle, account_id, msg);
                }
                return Ok(format!(
                    "Đã tặng x{qty} item #{template_id} (online) cho tài khoản #{account_id}."
                ));
            }
        }
        let mut snap = self.load_offline_snapshot(account_id)?;
        let mut inv = snapshot_to_economy_inv(&snap);
        inv.add_to_bag(item, allow_stack)
            .map_err(|err| format!("Hành trang đầy hoặc lỗi thêm đồ: {err}"))?;
        sync_economy_inv_to_snapshot(&inv, &mut snap);
        self.persist_snapshot(account_id, &snap)?;
        Ok(format!(
            "Đã tặng x{qty} item #{template_id} (offline) cho tài khoản #{account_id}."
        ))
    }

    pub(super) fn load_offline_snapshot(&self, account_id: i32) -> Result<PlayerSnapshot, String> {
        self.rt
            .block_on(
                self.shared
                    .players_repo
                    .load_by_account(AccountId(account_id), self.shared.config.parity_mode),
            )
            .map_err(|err| format!("Lỗi đọc nhân vật: {err}"))?
            .ok_or_else(|| format!("Tài khoản #{account_id} chưa tạo nhân vật."))
    }

    pub(super) fn persist_snapshot(
        &self,
        account_id: i32,
        snap: &PlayerSnapshot,
    ) -> Result<(), String> {
        self.rt
            .block_on(self.shared.players_repo.save_player(
                snap,
                None,
                self.shared.config.parity_mode,
                unix_now_ms(),
            ))
            .map_err(|err| {
                warn!(account_id, player_id = snap.id.0, error = %err, "failed to persist admin snapshot");
                format!("Lỗi lưu nhân vật: {err}")
            })
    }
}

pub(super) fn try_send_if_ready(handle: &SessionHandle, account_id: i32, pkt: Packet) {
    if handle.sent_key()
        && let Err(err) = handle.try_send(pkt)
    {
        warn!(account_id, error = %err, "failed to send admin packet to online session");
    }
}

fn apply_wallet_deltas(
    snap: &mut PlayerSnapshot,
    gold_delta: i64,
    gem_delta: i32,
    ruby_delta: i32,
) {
    snap.wallet.gold = snap
        .wallet
        .gold
        .saturating_add(gold_delta)
        .clamp(0, LIMIT_GOLD);
    snap.wallet.gem = snap.wallet.gem.saturating_add(gem_delta).max(0);
    snap.wallet.ruby = snap.wallet.ruby.saturating_add(ruby_delta).max(0);
}
