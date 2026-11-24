//! Auxiliary shop purchase paths (`LuckyRoundBox` withdrawals and `BuyBack` repurchase).

use nro_core::{ItemInstance, TemplateId};
use nro_db::{player_repo::PlayerSnapshot, templates::ShopRecord};
use nro_economy::ItemCatalog;
use nro_protocol::Packet;
use nro_world::s2c_packets;

use super::super::super::{
    DbItemCatalog, snapshot_to_economy_inv, sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use super::super::{buyback_items_snapshot, buyback_ledgers};

pub(super) fn buy_from_lucky_round_box(
    snap: &mut PlayerSnapshot,
    bt: i8,
    item_id: TemplateId,
    tag: Option<String>,
    shop: Option<&ShopRecord>,
    catalog: &DbItemCatalog<'_>,
) -> (Vec<Packet>, Option<(i16, String)>) {
    let mut inv = snapshot_to_economy_inv(snap);
    let find_slot = |box_items: &[Option<ItemInstance>]| {
        usize::try_from(item_id.0)
            .ok()
            .and_then(|n| {
                box_items
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.is_some())
                    .nth(n)
                    .map(|(i, _)| i)
            })
            .or_else(|| {
                box_items
                    .iter()
                    .position(|s| s.as_ref().is_some_and(|it| it.template_id == item_id))
            })
    };
    let msg = match bt {
        1 if item_id.0 < 0 => {
            inv.lucky_round_box.iter_mut().for_each(|s| *s = None);
            Ok("Đã xóa toàn bộ vật phẩm trong rương phụ!".to_owned())
        }
        1 => find_slot(&inv.lucky_round_box).map_or_else(
            || Err("Không tìm thấy vật phẩm trong rương phụ!".to_owned()),
            |slot| {
                inv.lucky_round_box[slot] = None;
                Ok("Đã xóa vật phẩm khỏi rương phụ!".to_owned())
            },
        ),
        2 => {
            let mut moved = 0_usize;
            for i in 0..inv.lucky_round_box.len() {
                let Some(it) = inv.lucky_round_box[i].clone() else {
                    continue;
                };
                let stackable = catalog.meta_of(it.template_id).stackable;
                if inv.add_to_bag(it, stackable).is_ok() {
                    inv.lucky_round_box[i] = None;
                    moved += 1;
                } else {
                    break;
                }
            }
            if moved > 0 {
                Ok(format!("Đã nhận {moved} vật phẩm vào hành trang!"))
            } else {
                Err("Rương phụ trống hoặc hành trang đã đầy!".to_owned())
            }
        }
        _ => {
            if let Some(slot) = find_slot(&inv.lucky_round_box)
                && let Some(it) = inv.lucky_round_box[slot].clone()
            {
                let stackable = catalog.meta_of(it.template_id).stackable;
                inv.add_to_bag(it, stackable)
                    .map(|()| {
                        inv.lucky_round_box[slot] = None;
                        "Đã nhận vật phẩm vào hành trang!".to_owned()
                    })
                    .map_err(|e| format!("Không thể nhận vật phẩm: {e}"))
            } else {
                Err("Không tìm thấy vật phẩm trong rương phụ!".to_owned())
            }
        }
    };
    if msg.is_ok() {
        sync_economy_inv_to_snapshot(&inv, snap);
    }
    let mut pkts = vec![s2c_packets::inventory_bag(&snap.items_bag)];
    if let Ok(tb) = s2c_packets::thong_bao(match &msg {
        Ok(s) | Err(s) => s.as_str(),
    }) {
        pkts.push(tb);
    }
    let reopen = msg
        .is_ok()
        .then(|| tag.map(|tg| (shop.map_or(19, |s| i16::from(s.npc_id)), tg)))
        .flatten();
    (pkts, reopen)
}

pub(super) fn buy_from_buyback_ledger(
    snap: &mut PlayerSnapshot,
    item_id: TemplateId,
    tag: Option<String>,
    shop: Option<&ShopRecord>,
    catalog: &DbItemCatalog<'_>,
) -> (Vec<Packet>, Option<(i16, String)>) {
    let (pid, mut inv) = (snap.id, snapshot_to_economy_inv(snap));
    let res: Result<(), String> = (|| {
        let items = buyback_items_snapshot(pid);
        let hidx = usize::try_from(item_id.0)
            .ok()
            .filter(|&idx| idx < items.len())
            .or_else(|| items.iter().position(|it| it.template_id == item_id))
            .ok_or_else(|| "Không tìm thấy vật phẩm mua lại!".to_owned())?;
        buyback_ledgers()
            .entry(pid)
            .or_default()
            .repurchase(&mut inv, hidx, catalog)
            .map(|_| ())
            .map_err(|e| format!("{e}"))
    })();
    if res.is_ok() {
        sync_economy_inv_to_snapshot(&inv, snap);
    }
    let mut pkts = vec![
        s2c_packets::inventory_bag(&snap.items_bag),
        wallet_money_pkt(&snap.wallet),
    ];
    let msg = match &res {
        Ok(()) => "Mua lại vật phẩm thành công!".to_owned(),
        Err(e) => format!("Không thể mua lại: {e}"),
    };
    if let Ok(tb) = s2c_packets::thong_bao(&msg) {
        pkts.push(tb);
    }
    let reopen = res
        .is_ok()
        .then(|| tag.map(|tg| (shop.map_or(0, |s| i16::from(s.npc_id)), tg)))
        .flatten();
    (pkts, reopen)
}
