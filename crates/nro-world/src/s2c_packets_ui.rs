//! Interactive UI, shop, consign, trade-offer, skill/EXP, disguise, and task `S2C` [`Packet`] builders.

use nro_consts::cmd_message;
use nro_core::{ItemInstance, ItemOption, PlayerId};
use nro_protocol::{Packet, PacketError};

use super::{
    MAX_SAFE_UTF_BYTES, build_pkt, build_pkt_res, clamp_u64_to_wire_i32, pid_i32,
    sub_command_writer, write_utf_lossy,
};

/// Single text field descriptor for [`client_input_open`] (`cmd -125`, `Controller2.cs:783-808`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputFieldWire<'a> {
    /// Prompt label shown above the input box.
    pub label: &'a str,
    /// Input validator (`0` = numeric, `1` = any text, `2` = password).
    pub input_type: i8,
}

/// Builds opcode `-125` (`CLIENT_INPUT`) modal text-input dialog (`Controller2.cs:783-808`).
pub fn client_input_open(
    title: &str,
    fields: &[InputFieldWire<'_>],
) -> Result<Packet, PacketError> {
    let n = fields.len().min(127);
    build_pkt_res(cmd_message::CLIENT_INPUT, 4 + title.len() + n * 16, |w| {
        w.write_utf(title)?;
        w.write_i8(n as i8);
        for f in &fields[..n] {
            w.write_utf(f.label)?;
            w.write_i8(f.input_type);
        }
        Ok(())
    })
}

/// Builds opcode `7` (`ITEM_SALE`) NPC item sale confirmation prompt (`Controller.cs:3483-3490`).
pub fn item_sale_confirm(
    where_slot: i8,
    slot_index: i16,
    prompt: &str,
) -> Result<Packet, PacketError> {
    build_pkt_res(cmd_message::ITEM_SALE, 5 + prompt.len(), |w| {
        w.write_i8(where_slot);
        w.write_i16(slot_index);
        w.write_utf(prompt)
    })
}

/// Shop item entry serialized inside [`shop_open`] (`cmd -44`, `Controller.cs:2398-2471`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShopItemWire<'a> {
    /// Item template ID (`-1` emits an empty slot marker).
    pub template_id: i16,
    /// Gold cost (`buyCoin`).
    pub buy_coin_gold: i32,
    /// Green gem / ruby cost (`buyGold`).
    pub buy_gold_gem: i32,
    /// Required power for skill/special shops (`type_shop == 1`).
    pub power_require: i64,
    /// Exchange currency icon ID (`type_shop == 3`).
    pub icon_spec: i16,
    /// Exchange currency amount (`type_shop == 3`).
    pub buy_spec: i32,
    /// Unlock/purchase reason string (`type_shop == 4`).
    pub reason: &'a str,
    /// Stack quantity (`type_shop == 2 | 8`).
    pub quantity: i32,
    /// Consignment metadata `(item_or_listing_id, buy_type, is_me, seller_name)` for `type_shop == 2`.
    pub consign: Option<(i16, i8, i8, &'a str)>,
    /// Attached item options (falls back to `[(73, 0)]` when empty).
    pub options: &'a [ItemOption],
    /// `"Mới"` badge flag (`newItem`).
    pub is_new: bool,
    /// Optional costume preview parts `(head, body, leg, bag)`.
    pub part_temp: Option<(i16, i16, i16, i16)>,
}

/// Shop tab serialized inside [`shop_open`] (`cmd -44`, `Controller.cs:2375-2472`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShopTabWire<'a> {
    /// Tab caption (`"\n"` splits into two header lines on client).
    pub name: &'a str,
    /// Total page count for consignment tabs (`type_shop == 2`).
    pub max_page: u8,
    /// Items listed in this tab.
    pub items: &'a [ShopItemWire<'a>],
}

/// Builds opcode `-44` (`SHOP`) matching v2.5.0 `Controller.cs:2351-2518` byte-for-byte.
pub fn shop_open(type_shop: i8, tabs: &[ShopTabWire<'_>]) -> Result<Packet, PacketError> {
    let tab_n = tabs.len().min(u8::MAX as usize);
    build_pkt_res(cmd_message::SHOP, 32 + tab_n * 64, |w| {
        w.write_i8(type_shop);
        w.write_u8(tab_n as u8);
        let fallback = [ItemOption::new(73, 0)];
        for tab in &tabs[..tab_n] {
            w.write_utf(tab.name)?;
            if type_shop == 2 {
                w.write_u8(tab.max_page);
            }
            let item_n = tab.items.len().min(u8::MAX as usize);
            w.write_u8(item_n as u8);
            for it in &tab.items[..item_n] {
                w.write_i16(it.template_id);
                if it.template_id == -1 {
                    continue;
                }
                match type_shop {
                    8 => {
                        w.write_i32(it.buy_coin_gold);
                        w.write_i32(it.buy_gold_gem);
                        w.write_i32(it.quantity);
                    }
                    4 => w.write_utf(it.reason)?,
                    0 => {
                        w.write_i32(it.buy_coin_gold);
                        w.write_i32(it.buy_gold_gem);
                    }
                    1 => w.write_i64(it.power_require),
                    2 => {
                        let (listing_id, buy_type, is_me, _) =
                            it.consign.unwrap_or((0, 0, 0, ""));
                        w.write_i16(listing_id);
                        w.write_i32(it.buy_coin_gold);
                        w.write_i32(it.buy_gold_gem);
                        w.write_i8(buy_type);
                        w.write_i32(it.quantity);
                        w.write_i8(is_me);
                    }
                    3 => {
                        w.write_i16(it.icon_spec);
                        w.write_i32(it.buy_spec);
                    }
                    _ => {}
                }
                let opts: &[ItemOption] = if it.options.is_empty() {
                    &fallback
                } else {
                    it.options
                };
                let opt_n = opts.len().min(u8::MAX as usize);
                w.write_u8(opt_n as u8);
                for o in &opts[..opt_n] {
                    w.write_i16(o.id.min(i16::MAX as u16) as i16);
                    w.write_i32(o.param);
                }
                w.write_i8(i8::from(it.is_new));
                if let Some((head, body, leg, bag)) = it.part_temp {
                    w.write_i8(1);
                    for part in [head, body, leg, bag] {
                        w.write_i16(part);
                    }
                } else {
                    w.write_i8(0);
                }
                if type_shop == 2 {
                    let seller = it.consign.map_or("", |(_, _, _, s)| s);
                    w.write_utf(seller)?;
                }
            }
        }
        Ok(())
    })
}

/// Consignment listing entry serialized inside [`consign_tab_page`] (`cmd -100`, `Controller2.cs:971-1027`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConsignItemWire<'a> {
    /// Item template ID (`-1` emits an empty slot marker).
    pub template_id: i16,
    /// Consignment listing ID (`itemId`).
    pub listing_id: i16,
    /// Gold price (`buyCoin`).
    pub buy_coin_gold: i32,
    /// Gem / ruby price (`buyGold`).
    pub buy_gold_gem: i32,
    /// Listing status / buy mode byte (`buyType`).
    pub buy_type: i8,
    /// Stack quantity.
    pub quantity: i32,
    /// `1` if owned by the viewing player (`isMe`), else `0`.
    pub is_me: i8,
    /// Attached item options (falls back to `[(73, 0)]` when empty).
    pub options: &'a [ItemOption],
    /// Optional costume preview parts `(head, body, leg, bag)`.
    pub part_temp: Option<(i16, i16, i16, i16)>,
    /// Seller character name (`nameNguoiKyGui`, client `intVERSION >= 237`).
    pub seller_name: &'a str,
}

/// Builds opcode `-100` (`KIGUI`) consignment tab page snapshot (`Controller2.cs:971-1027`).
pub fn consign_tab_page(
    tab_id: i8,
    max_page: i8,
    curr_page: i8,
    items: &[ConsignItemWire<'_>],
) -> Result<Packet, PacketError> {
    let item_n = items.len().min(u8::MAX as usize);
    build_pkt_res(cmd_message::KIGUI, 4 + item_n * 32, |w| {
        w.write_i8(tab_id);
        w.write_i8(max_page);
        w.write_i8(curr_page);
        w.write_u8(item_n as u8);
        let fallback = [ItemOption::new(73, 0)];
        for it in &items[..item_n] {
            w.write_i16(it.template_id);
            if it.template_id == -1 {
                continue;
            }
            w.write_i16(it.listing_id);
            w.write_i32(it.buy_coin_gold);
            w.write_i32(it.buy_gold_gem);
            w.write_i8(it.buy_type);
            w.write_i32(it.quantity);
            w.write_i8(it.is_me);
            let opts: &[ItemOption] = if it.options.is_empty() { &fallback } else { it.options };
            let opt_n = opts.len().min(i8::MAX as usize);
            w.write_i8(opt_n as i8);
            for o in &opts[..opt_n] {
                w.write_i16(o.id.min(i16::MAX as u16) as i16);
                w.write_i32(o.param);
            }
            if let Some((head, body, leg, bag)) = it.part_temp {
                w.write_i8(1);
                for part in [head, body, leg, bag] {
                    w.write_i16(part);
                }
            } else {
                w.write_i8(0);
            }
            w.write_utf(it.seller_name)?;
        }
        Ok(())
    })
}

/// Builds opcode `-43` (`USE_ITEM`) item confirmation dialog (`Controller.cs:1310-1318`).
#[must_use]
pub fn use_item_confirm(item_action: i8, where_slot: i8, index: i8, prompt: &str) -> Packet {
    build_pkt(cmd_message::USE_ITEM, 5 + prompt.len().min(MAX_SAFE_UTF_BYTES), |w| {
        w.write_i8(item_action);
        w.write_i8(where_slot);
        w.write_i8(index);
        write_utf_lossy(w, prompt);
    })
}

/// Builds `-30` sub `5` (`ME_LOAD_HP`) 64-bit self HP update.
#[must_use]
pub fn me_load_hp(hp: i64) -> Packet {
    let mut w = sub_command_writer(cmd_message::ME_LOAD_HP, 8);
    w.write_i64(hp.max(0));
    w.finish(cmd_message::SUB_COMMAND)
}

/// Builds `-30` sub `6` (`ME_LOAD_MP`) 64-bit self MP/Ki update.
#[must_use]
pub fn me_load_mp(mp: i64) -> Packet {
    let mut w = sub_command_writer(cmd_message::ME_LOAD_MP, 8);
    w.write_i64(mp.max(0));
    w.finish(cmd_message::SUB_COMMAND)
}

/// Builds `-30` sub `23` (`ME_ADD_SKILL`) newly learned skill notification.
#[must_use]
pub fn me_add_skill(skill_id: i16) -> Packet {
    let mut w = sub_command_writer(cmd_message::ME_ADD_SKILL, 2);
    w.write_i16(skill_id);
    w.finish(cmd_message::SUB_COMMAND)
}

/// Builds `-30` sub `62` (`ME_UPDATE_SKILL`) upgraded skill notification.
#[must_use]
pub fn me_update_skill(skill_id: i16) -> Packet {
    let mut w = sub_command_writer(cmd_message::ME_UPDATE_SKILL, 2);
    w.write_i16(skill_id);
    w.finish(cmd_message::SUB_COMMAND)
}

/// Builds opcode `-3` (`PLAYER_UP_EXP`, `0` = power, `1` = potential, `2` = both).
#[must_use]
pub fn player_up_exp(mode: i8, quant: i32) -> Packet {
    build_pkt(cmd_message::PLAYER_UP_EXP, 5, |w| {
        w.write_i8(mode);
        w.write_i32(quant.max(0));
    })
}

/// Builds opcode `54` (`PLAYER_ATTACK_NPC`) peer mob-attack animation broadcast (`max 10` mobs).
#[must_use]
pub fn peer_attack_mobs(attacker: PlayerId, skill_paint_id: u8, mobs: &[u8]) -> Packet {
    let n = mobs.len().min(10);
    build_pkt(cmd_message::PLAYER_ATTACK_NPC, 5 + n, |w| {
        w.write_i32(pid_i32(attacker));
        w.write_u8(skill_paint_id);
        for &mob_id in &mobs[..n] {
            w.write_u8(mob_id);
        }
    })
}

/// Builds opcode `-90` (`UPDATE_BODY`) disguise/monkey transformation or revert packet.
#[must_use]
pub fn update_body(player_id: PlayerId, disguise: Option<(i16, i16, i16, i8)>) -> Packet {
    build_pkt(cmd_message::UPDATE_BODY, 12, |w| {
        if let Some((head, body, leg, monkey)) = disguise {
            w.write_i8(1);
            w.write_i32(pid_i32(player_id));
            for part in [head, body, leg] {
                w.write_i16(part);
            }
            w.write_i8(monkey);
        } else {
            w.write_i8(-1);
            w.write_i32(pid_i32(player_id));
        }
    })
}

/// Builds opcode `-64` (`UPDATE_BAG`) character flag-bag / clan-emblem update.
#[must_use]
pub fn send_flag_bag(player_id: PlayerId, bag_id: i16) -> Packet {
    build_pkt(cmd_message::UPDATE_BAG, 6, |w| {
        w.write_i32(pid_i32(player_id));
        w.write_i16(bag_id);
    })
}

/// Builds empty opcode `41` (`TASK_NEXT`) advancing the client's main task sub-step.
#[must_use]
pub const fn task_next() -> Packet {
    Packet::empty(cmd_message::TASK_NEXT)
}

/// Builds opcode `43` (`TASK_UPDATE`) updating the client's current task progress counter.
#[must_use]
pub fn task_update(count: i16) -> Packet {
    build_pkt(cmd_message::TASK_UPDATE, 2, |w| {
        w.write_i16(count);
    })
}

/// Builds opcode `-86` (`GIAO_DICH`, sub `2`) removing an untradeable item from the sender's trade slot.
#[must_use]
pub fn trade_remove_rejected_item(bag_index: i8) -> Packet {
    build_pkt(cmd_message::GIAO_DICH, 2, |w| {
        w.write_i8(2);
        w.write_i8(bag_index);
    })
}

/// Builds opcode `-86` (`GIAO_DICH`, sub `6`) partner locked trade offer snapshot (`Controller.cs:715-757`).
#[must_use]
pub fn trade_lock_offer(gold: u64, items: &[ItemInstance]) -> Packet {
    let n = items.len().min(127);
    build_pkt(cmd_message::GIAO_DICH, 6 + n * 14, |w| {
        w.write_i8(6);
        w.write_i32(clamp_u64_to_wire_i32(gold));
        w.write_i8(n as i8);
        let fallback = [ItemOption::new(73, 0)];
        for it in &items[..n] {
            w.write_i16(it.template_id.0);
            w.write_i32(it.quantity.min(i32::MAX as u32) as i32);
            let opts: &[ItemOption] = if it.options.is_empty() {
                &fallback
            } else {
                it.options.as_slice()
            };
            let opt_n = opts.len().min(u8::MAX as usize);
            w.write_u8(opt_n as u8);
            for o in &opts[..opt_n] {
                w.write_i16(o.id.min(i16::MAX as u16) as i16);
                w.write_i32(o.param);
            }
        }
    })
}
