//! Positional JSON array parsers and writers for `player` wallet, location, core stats,
//! magic-tree, triple-escaped item containers, social lists, intrinsics, tasks, and buff timers.
//!
//! Re-exports [`crate::json_ext`] so callers importing `nro_db::json_columns::*` access all codecs.

pub use crate::json_ext::*;

use nro_core::{
    CoreStats, Gender, ItemInstance, ItemOption, Location, MapId, ParityMode, TemplateId, ZoneId,
};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::{
    DbError, DbResult, JsonStr, PositionalArray, gender_to_sql, is_legacy_parity,
    parse_lenient_json_vec,
};

pub(crate) fn json_err(column: &'static str, source: serde_json::Error) -> DbError {
    DbError::JsonColumn { column, source }
}

/// Currency wallet stored in `player.data_inventory`: `[gold, gem, ruby, coupon, event]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InventoryWallet {
    pub gold: i64,
    pub gem: i32,
    pub ruby: i32,
    pub coupon: i32,
    pub event: i32,
}

pub fn parse_data_inventory(raw: &str, mode: ParityMode) -> DbResult<InventoryWallet> {
    let a = PositionalArray::parse("data_inventory", raw)?;
    if a.len() < 3 {
        return Err(DbError::InvalidColumnFormat {
            column: "data_inventory",
            reason: format!("expected >= 3 elements, got {}", a.len()),
        });
    }
    Ok(InventoryWallet {
        gold: a.i64_or(0, 0),
        gem: a.i32_or(1, 0),
        ruby: a.i32_or(2, 0),
        coupon: a.i32_or(3, 0),
        event: if is_legacy_parity(mode) { 0 } else { a.i32_or(4, 0) },
    })
}

pub fn write_data_inventory(w: &InventoryWallet) -> DbResult<String> {
    serde_json::to_string(&(w.gold, w.gem, w.ruby, w.coupon, w.event))
        .map_err(|e| json_err("data_inventory", e))
}

pub fn parse_data_location(raw: &str) -> DbResult<Location> {
    let (m, x, y): (i32, i32, i32) =
        serde_json::from_str(raw).map_err(|e| json_err("data_location", e))?;
    Ok(Location { map_id: MapId(m as i16), zone_id: ZoneId(0), x: x as i16, y: y as i16 })
}

#[must_use]
pub fn sanitize_login_location(mut loc: Location, gender: Gender) -> Location {
    let _ = nro_consts::MAP_NORMAL;
    let m = i32::from(loc.map_id.0);
    if m == 51 || (53..=62).contains(&m) || (85..=91).contains(&m) || (114..=120).contains(&m) {
        loc.map_id = MapId(i16::from(gender_to_sql(gender)) + 21);
        loc.x = 300;
        loc.y = 336;
    } else if m == 112 {
        loc.y = 408;
    } else if m == 113 || m == 129 {
        loc.y = 360;
    } else if m == 49 {
        loc.map_id = MapId(45);
        loc.x = 359;
        loc.y = 408;
    }
    loc
}

pub fn write_data_location(loc: &Location) -> DbResult<String> {
    serde_json::to_string(&(i32::from(loc.map_id.0), i32::from(loc.x), i32::from(loc.y)))
        .map_err(|e| json_err("data_location", e))
}

pub fn parse_data_point(raw: &str) -> DbResult<CoreStats> {
    let a = PositionalArray::parse("data_point", raw)?;
    if a.len() < 14 {
        return Err(DbError::InvalidColumnFormat {
            column: "data_point",
            reason: format!("expected 14 elements, got {}", a.len()),
        });
    }
    Ok(CoreStats {
        limit_power: a.i32_or(0, 0).clamp(0, 9) as u8,
        power: a.i64_or(1, 0),
        tiem_nang: a.i64_or(2, 0),
        stamina: a.i32_or(3, 0).clamp(0, 65_535) as u16,
        max_stamina: a.i32_or(4, 0).clamp(0, 65_535) as u16,
        hpg: a.i64_or(5, 100),
        mpg: a.i64_or(6, 100),
        dameg: a.i64_or(7, 10),
        defg: a.i32_or(8, 0),
        critg: a.i32_or(9, 0).clamp(0, 255) as u8,
        critdragon: a.i32_or(10, 0).clamp(-128, 127) as i8,
        hp: a.i64_or(12, 100),
        mp: a.i64_or(13, 100),
        ..CoreStats::default()
    })
}

pub fn write_data_point(s: &CoreStats) -> DbResult<String> {
    serde_json::to_string(&(
        s.limit_power, s.power, s.tiem_nang, s.stamina, s.max_stamina, s.hpg, s.mpg, s.dameg,
        s.defg, s.critg, s.critdragon, 0_i32, s.hp, s.mp,
    ))
    .map_err(|e| json_err("data_point", e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MagicTreeState {
    pub level: i8,
    pub curr_peas: i8,
    pub is_upgrade: bool,
    pub last_time_harvest_ms: i64,
    pub last_time_upgrade_ms: i64,
}

pub fn parse_data_magic_tree(raw: &str) -> DbResult<MagicTreeState> {
    let (l, p, u, h, up): (i8, i8, u8, i64, i64) =
        serde_json::from_str(raw).map_err(|e| json_err("data_magic_tree", e))?;
    Ok(MagicTreeState {
        level: l,
        curr_peas: p,
        is_upgrade: u == 1,
        last_time_harvest_ms: h,
        last_time_upgrade_ms: up,
    })
}

pub fn write_data_magic_tree(m: &MagicTreeState) -> DbResult<String> {
    serde_json::to_string(&(
        m.level, m.curr_peas, u8::from(m.is_upgrade), m.last_time_harvest_ms, m.last_time_upgrade_ms,
    ))
    .map_err(|e| json_err("data_magic_tree", e))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawItemSlot(
    pub TemplateId,
    pub i32,
    pub JsonStr<Vec<JsonStr<(i32, i32)>>>,
    #[serde(default)] pub i64,
);

pub fn parse_items_container(column: &'static str, raw: &str) -> DbResult<Vec<Option<ItemInstance>>> {
    let slots: Vec<JsonStr<RawItemSlot>> = parse_lenient_json_vec(column, raw)?;
    let mut items: Vec<Option<ItemInstance>> = slots
        .into_iter()
        .map(|JsonStr(s)| {
            (s.0.0 >= 0).then(|| {
                let options: SmallVec<[ItemOption; 4]> =
                    s.2.0.into_iter().map(|JsonStr((id, param))| ItemOption { id: id as u16, param }).collect();
                ItemInstance { template_id: s.0, quantity: s.1.max(0) as u32, options, create_time_ms: s.3 }
            })
        })
        .collect();
    if column == "items_body" && items.len() == 10 {
        items.push(None);
    } else if column == "items_daban" && items.len() > 20 {
        items.truncate(20);
    }
    Ok(items)
}

pub fn write_items_container(
    column: &'static str,
    items: &[Option<ItemInstance>],
    empty_ts: i64,
) -> DbResult<String> {
    let raw: Vec<JsonStr<RawItemSlot>> = items
        .iter()
        .map(|slot| match slot {
            Some(it) if it.template_id.0 >= 0 => {
                let opts = it.options.iter().map(|o| JsonStr((i32::from(o.id), o.param))).collect();
                JsonStr(RawItemSlot(it.template_id, it.quantity.min(i32::MAX as u32) as i32, JsonStr(opts), it.create_time_ms))
            }
            _ => JsonStr(RawItemSlot(TemplateId(-1), 0, JsonStr(Vec::new()), empty_ts)),
        })
        .collect();
    serde_json::to_string(&raw).map_err(|e| json_err(column, e))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialPeer {
    pub id: i32, pub name: String, pub head: i16, pub body: i16, pub leg: i16, pub bag: i8, pub power: i64,
}

pub fn parse_social_list(column: &'static str, raw: &str) -> DbResult<Vec<SocialPeer>> {
    Ok(parse_lenient_json_vec::<JsonStr<(i32, String, i16, i16, i16, i8, i64)>>(column, raw)?
        .into_iter()
        .map(|JsonStr(t)| SocialPeer {
            id: t.0, name: t.1, head: t.2, body: t.3, leg: t.4, bag: t.5, power: t.6,
        })
        .collect())
}

pub fn write_social_list(column: &'static str, peers: &[SocialPeer]) -> DbResult<String> {
    let v: Vec<JsonStr<(&i32, &str, &i16, &i16, &i16, &i8, &i64)>> = peers
        .iter()
        .map(|p| JsonStr((&p.id, p.name.as_str(), &p.head, &p.body, &p.leg, &p.bag, &p.power)))
        .collect();
    serde_json::to_string(&v).map_err(|e| json_err(column, e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct IntrinsicState {
    pub intrinsic_id: i8, pub param1: i16, pub param2: i16, pub count_open: i8,
    pub is_intrinsic: bool, pub skill_id: i32, pub cooldown: i32, pub last_time_use_skill_ms: i64,
}

pub fn parse_data_intrinsic(raw: &str) -> DbResult<IntrinsicState> {
    let a = PositionalArray::parse("data_intrinsic", raw)?;
    Ok(IntrinsicState {
        intrinsic_id: a.i32_or(0, 0) as i8, param1: a.i32_or(1, 0) as i16,
        param2: a.i32_or(2, 0) as i16, count_open: a.i32_or(3, 0) as i8,
        is_intrinsic: a.bool_or(4, false), skill_id: a.i32_or(5, 0),
        cooldown: a.i32_or(6, 0), last_time_use_skill_ms: a.i64_or(7, 0),
    })
}

pub fn write_data_intrinsic(s: &IntrinsicState) -> DbResult<String> {
    serde_json::to_string(&(
        s.intrinsic_id, s.param1, s.param2, s.count_open, s.is_intrinsic, s.skill_id, s.cooldown,
        s.last_time_use_skill_ms,
    ))
    .map_err(|e| json_err("data_intrinsic", e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ItemTimeSieuCap {
    pub bo_huyet2_ms: i64, pub bo_khi2_ms: i64, pub giap_xen2_ms: i64, pub cuong_no2_ms: i64,
    pub an_danh2_ms: i64, pub eat_meal2_ms: i64, pub icon_meal2: i32,
}

pub fn parse_data_item_time_sieu_cap(raw: &str) -> DbResult<ItemTimeSieuCap> {
    let a = PositionalArray::parse("data_item_time_sieu_cap", raw)?;
    Ok(ItemTimeSieuCap {
        bo_huyet2_ms: a.i64_or(0, 0), bo_khi2_ms: a.i64_or(1, 0), giap_xen2_ms: a.i64_or(2, 0),
        cuong_no2_ms: a.i64_or(3, 0), an_danh2_ms: a.i64_or(4, 0), eat_meal2_ms: a.i64_or(5, 0),
        icon_meal2: a.i32_or(6, 0),
    })
}

pub fn write_data_item_time_sieu_cap(sc: &ItemTimeSieuCap) -> DbResult<String> {
    serde_json::to_string(&[
        sc.bo_huyet2_ms, sc.bo_khi2_ms, sc.giap_xen2_ms, sc.cuong_no2_ms, sc.an_danh2_ms,
        sc.eat_meal2_ms, i64::from(sc.icon_meal2),
    ])
    .map_err(|e| json_err("data_item_time_sieu_cap", e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ItemTimeState {
    pub bo_huyet_ms: i64, pub bo_khi_ms: i64, pub giap_xen_ms: i64, pub cuong_no_ms: i64,
    pub an_danh_ms: i64, pub sieu_cap: ItemTimeSieuCap, pub open_power_ms: i64,
    pub may_do_ms: i64, pub co_bon_la_ms: i64, pub kho_bau_x2_ms: i64, pub bua_santa_ms: i64,
    pub eat_meal_ms: i64, pub icon_meal: i32, pub tdlt_minutes: i64, pub cms_ms: i64,
    pub gtpt_ms: i64, pub dk_ms: i64, pub rx_minutes: i64, pub ncd_ms: i64,
    pub nuoc_mia1_ms: i64, pub nuoc_mia2_ms: i64, pub nuoc_mia3_ms: i64, pub kilis_ms: i64,
}

pub fn parse_data_item_time(raw: &str, mode: ParityMode) -> DbResult<ItemTimeState> {
    let a = PositionalArray::parse("data_item_time", raw)?;
    if is_legacy_parity(mode) {
        Ok(ItemTimeState {
            bo_huyet_ms: a.i64_or(0, 0), bo_khi_ms: a.i64_or(2, 0), giap_xen_ms: a.i64_or(4, 0),
            cuong_no_ms: a.i64_or(6, 0), an_danh_ms: a.i64_or(8, 0),
            sieu_cap: ItemTimeSieuCap {
                bo_huyet2_ms: a.i64_or(1, 0), bo_khi2_ms: a.i64_or(3, 0), giap_xen2_ms: a.i64_or(5, 0),
                cuong_no2_ms: a.i64_or(7, 0), an_danh2_ms: a.i64_or(9, 0),
                eat_meal2_ms: a.i64_or(21, 0), icon_meal2: a.i32_or(22, 0),
            },
            open_power_ms: a.i64_or(10, 0), may_do_ms: a.i64_or(11, 0), co_bon_la_ms: 0,
            kho_bau_x2_ms: a.i64_or(12, 0), bua_santa_ms: a.i64_or(25, 0), eat_meal_ms: a.i64_or(14, 0),
            icon_meal: a.i32_or(15, 0), tdlt_minutes: a.i64_or(16, 0), cms_ms: a.i64_or(17, 0),
            gtpt_ms: a.i64_or(18, 0), dk_ms: a.i64_or(19, 0), rx_minutes: a.i64_or(20, 0),
            ncd_ms: a.i64_or(24, 0), kilis_ms: a.i64_or(26, 0), nuoc_mia1_ms: a.i64_or(27, 0),
            nuoc_mia2_ms: a.i64_or(28, 0),
            nuoc_mia3_ms: if a.len() > 29 { a.i64_or(28, 0) } else { 0 },
        })
    } else {
        Ok(ItemTimeState {
            bo_huyet_ms: a.i64_or(0, 0), bo_khi_ms: a.i64_or(2, 0), giap_xen_ms: a.i64_or(4, 0),
            cuong_no_ms: a.i64_or(6, 0), an_danh_ms: a.i64_or(8, 0),
            sieu_cap: ItemTimeSieuCap {
                bo_huyet2_ms: a.i64_or(1, 0), bo_khi2_ms: a.i64_or(3, 0), giap_xen2_ms: a.i64_or(5, 0),
                cuong_no2_ms: a.i64_or(7, 0), an_danh2_ms: a.i64_or(9, 0),
                eat_meal2_ms: a.i64_or(22, 0), icon_meal2: a.i32_or(23, 0),
            },
            open_power_ms: a.i64_or(10, 0), may_do_ms: a.i64_or(11, 0), co_bon_la_ms: a.i64_or(12, 0),
            kho_bau_x2_ms: a.i64_or(13, 0), bua_santa_ms: a.i64_or(14, 0), eat_meal_ms: a.i64_or(15, 0),
            icon_meal: a.i32_or(16, 0), tdlt_minutes: a.i64_or(17, 0), cms_ms: a.i64_or(18, 0),
            gtpt_ms: a.i64_or(19, 0), dk_ms: a.i64_or(20, 0), rx_minutes: a.i64_or(21, 0),
            ncd_ms: a.i64_or(25, 0), nuoc_mia1_ms: a.i64_or(26, 0), nuoc_mia2_ms: a.i64_or(27, 0),
            nuoc_mia3_ms: a.i64_or(28, 0), kilis_ms: a.i64_or(29, 0),
        })
    }
}

pub fn write_data_item_time(t: &ItemTimeState) -> DbResult<String> {
    let arr: [i64; 32] = [
        t.bo_huyet_ms, t.sieu_cap.bo_huyet2_ms, t.bo_khi_ms, t.sieu_cap.bo_khi2_ms,
        t.giap_xen_ms, t.sieu_cap.giap_xen2_ms, t.cuong_no_ms, t.sieu_cap.cuong_no2_ms,
        t.an_danh_ms, t.sieu_cap.an_danh2_ms, t.open_power_ms, t.may_do_ms, t.co_bon_la_ms,
        t.kho_bau_x2_ms, t.bua_santa_ms, t.eat_meal_ms, i64::from(t.icon_meal), t.tdlt_minutes,
        t.cms_ms, t.gtpt_ms, t.dk_ms, t.rx_minutes, t.sieu_cap.eat_meal2_ms,
        i64::from(t.sieu_cap.icon_meal2), 0, t.ncd_ms, t.nuoc_mia1_ms, t.nuoc_mia2_ms,
        t.nuoc_mia3_ms, t.kilis_ms, 0, 0,
    ];
    serde_json::to_string(arr.as_slice()).map_err(|e| json_err("data_item_time", e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MainTaskProgress {
    pub task_id: i8, pub sub_index: i8, pub count: i16, pub last_time_ms: i64,
}

pub fn parse_data_task(raw: &str) -> DbResult<MainTaskProgress> {
    let a = PositionalArray::parse("data_task", raw)?;
    if a.len() < 3 {
        return Err(DbError::InvalidColumnFormat { column: "data_task", reason: "len < 3".into() });
    }
    Ok(MainTaskProgress {
        task_id: a.i32_or(0, 0) as i8, sub_index: a.i32_or(1, 0) as i8,
        count: a.i32_or(2, 0) as i16, last_time_ms: a.i64_or(3, 0),
    })
}

pub fn write_data_task(t: &MainTaskProgress) -> DbResult<String> {
    serde_json::to_string(&(t.task_id, t.sub_index, t.count, t.last_time_ms))
        .map_err(|e| json_err("data_task", e))
}
