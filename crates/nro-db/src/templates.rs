//! Static game-data registry (`GameTemplates`) porting `Manager.loadDatabase` and `ShopDAO.java`
//! onto embedded `fjall` `postcard` snapshots.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDateTime, Utc};
use nro_core::{ItemOption, MapId, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use tracing::info;

use crate::codec::{decode_val, encode_val};
use crate::engine::{FjallEngine, TEMPLATES_SNAPSHOT_KEY};
use crate::world_repos::NumericOptionJson;
use crate::DbResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemTemplateRecord {
    pub id: i32, pub item_type: i32, pub gender: i16,
    pub name: String, pub description: Option<String>, pub level: i32,
    pub icon_id: i32, pub part: i32, pub is_up_to_up: bool, pub power_require: i32,
    pub gold: i32, pub gem: i32, pub head: i32, pub body: i32, pub leg: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MobTemplateRecord {
    pub id: i32, pub mob_type: i32, pub name: String,
    pub hp: i32, pub range_move: i16, pub speed: i16, pub dart_type: i16,
    pub percent_dame: i16, pub percent_tiem_nang: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcTemplateRecord {
    pub id: i32, pub name: String,
    pub head: i32, pub body: i32, pub leg: i32, pub avatar: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapWaypointRecord(
    pub String, pub i16, pub i16, pub i16, pub i16, pub u8, pub u8, pub MapId, pub i16, pub i16,
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapTemplateRecord {
    pub id: MapId, pub name: String, pub zones: u8, pub max_player: u8, pub map_type: i8,
    pub planet_id: i8, pub bg_type: i8, pub tile_id: i8, pub bg_id: i8,
    pub waypoints: Vec<MapWaypointRecord>, pub mobs: Vec<(i8, i8, i32, i16, i16)>,
    pub npcs: Vec<(i8, i16, i16)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillLevelRecord {
    pub id: i16, pub point: i8, pub power_require: i64, pub mana_use: i32, pub cool_down: i32,
    pub dx: i32, pub dy: i32, pub max_fight: i32, pub damage: i16, pub price: i16, pub info: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillTemplateRecord {
    pub nclass_id: i8, pub id: i8, pub name: String, pub max_point: i8, pub mana_use_type: i8,
    pub skill_type: i8, pub icon_id: i16, pub dam_info: String, pub slot: i32,
    pub levels: Vec<SkillLevelRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntrinsicRecord {
    pub id: i32, pub name: String, pub param_from_1: i32,
    pub param_to_1: i32, pub param_from_2: i32, pub param_to_2: i32, pub icon: i32, pub gender: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopItemRecord {
    pub id: i32, pub temp_id: TemplateId, pub is_new: bool, pub type_sell: i8,
    pub cost: i32, pub icon_spec: i32, pub options: SmallVec<[ItemOption; 4]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopTabRecord { pub id: i32, pub name: String, pub items: Vec<ShopItemRecord> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopRecord {
    pub id: i32, pub npc_id: i8, pub tag_name: Option<String>, pub type_shop: i8,
    pub tabs: Vec<ShopTabRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubTaskTemplateRecord {
    pub name: String, pub max_count: i16, pub notify: String, pub npc_id: i8, pub map_id: MapId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskMainTemplateRecord {
    pub id: i32, pub name: String, pub detail: String, pub sub_tasks: Vec<SubTaskTemplateRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RadarBodyParts { pub head: i16, pub body: i16, pub leg: i16, pub bag: i16 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadarOptionTemplate {
    pub id: i32, pub param: i16, #[serde(rename = "activeCard")] pub active_card: i8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadarTemplateRecord {
    pub id: i16, pub icon_id: i16, pub rank: i8, pub max: i8, pub card_type: i8, pub mob_id: i16,
    pub body: RadarBodyParts, pub name: String, pub info: String,
    pub options: Vec<RadarOptionTemplate>, pub require: i16, pub require_level: i16, pub aura_id: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalPaymentRow {
    pub id: i32, pub name: String, pub ref_no: String,
    pub date: NaiveDateTime, pub declared_amount: i32, pub final_credited_amount: Option<i32>,
    pub status_text: String, pub api_status_code: String, pub is_credited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalPostRow {
    pub id: i32, pub tieude: String, pub noidung: String, pub username: String,
    pub created_at: DateTime<Utc>, pub theloai: i32, pub ghimbai: i32, pub like: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameTemplates {
    pub items: Vec<ItemTemplateRecord>, pub item_options: Vec<(i32, String)>,
    pub mobs: Vec<MobTemplateRecord>, pub npcs: Vec<NpcTemplateRecord>,
    pub maps: Vec<MapTemplateRecord>, pub skills: Vec<SkillTemplateRecord>,
    pub intrinsics: Vec<IntrinsicRecord>, pub shops: Vec<ShopRecord>,
    pub tasks: Vec<TaskMainTemplateRecord>, pub side_tasks: Vec<(i32, String, [(i32, i32); 5])>,
    pub clan_tasks: Vec<(i32, String, [(i32, i32); 5])>,
    pub achievements: Vec<(i32, String, String, i32, i64)>,
    pub task_badges: Vec<(i32, String, i32, i32)>,
    pub badges: Vec<(i32, i32, i32, String, Vec<NumericOptionJson>)>,
    pub flag_bags: Vec<(i32, String, i32, i32, i16, Vec<i16>)>, pub head_avatars: Vec<(i16, i16)>,
    pub parts: Vec<(i16, i8, Vec<(i16, i8, i8)>)>, pub bg_items: Vec<(i32, i16, i8, i16, i16)>,
    pub arr_head_2_frames: Vec<Vec<i16>>, pub images_by_name: HashMap<String, u8>,
    pub mount_map: HashMap<TemplateId, i16>, pub radars: Vec<RadarTemplateRecord>,
    pub notices: Vec<String>,
}

pub(crate) fn parse_range_pair(s: &str) -> (i32, i32) {
    let mut sp = s.split('-');
    let lo = sp.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    let hi = sp.next().and_then(|v| v.trim().parse().ok()).unwrap_or(lo);
    (lo, hi)
}

#[allow(clippy::type_complexity)]
pub(crate) fn assemble_shops(
    mut shop_rows: Vec<(i32, i32, Option<String>, Option<i32>)>,
    mut tab_rows: Vec<(i32, i32, String)>,
    mut item_rows: Vec<(i32, i32, i32, bool, bool, Option<i32>, Option<i32>, Option<i32>, i64)>,
    opt_rows: Vec<(i32, i32, i32)>,
) -> Vec<ShopRecord> {
    shop_rows.sort_by_key(|r| r.1);
    tab_rows.sort_by_key(|r| r.0);
    item_rows.retain(|r| r.4);
    item_rows.sort_by_key(|b| std::cmp::Reverse(b.8));

    let mut opts_by_item: HashMap<i32, SmallVec<[ItemOption; 4]>> = HashMap::new();
    for (item_shop_id, id, param) in opt_rows {
        opts_by_item.entry(item_shop_id).or_default().push(ItemOption { id: id as u16, param });
    }
    let mut items_by_tab: HashMap<i32, Vec<ShopItemRecord>> = HashMap::new();
    for r in item_rows {
        items_by_tab.entry(r.1).or_default().push(ShopItemRecord {
            id: r.0, temp_id: TemplateId(r.2 as i16), is_new: r.3,
            type_sell: r.5.unwrap_or(1) as i8, cost: r.6.unwrap_or(0),
            icon_spec: r.7.unwrap_or(0), options: opts_by_item.remove(&r.0).unwrap_or_default(),
        });
    }
    let mut tabs_by_shop: HashMap<i32, Vec<ShopTabRecord>> = HashMap::new();
    for (tab_id, shop_id, raw_name) in tab_rows {
        let lookup_tab = if (41..=43).contains(&tab_id) { tab_id - 31 } else { tab_id };
        let items = items_by_tab.get(&lookup_tab).cloned().unwrap_or_default();
        tabs_by_shop.entry(shop_id).or_default().push(ShopTabRecord {
            id: tab_id, name: raw_name.replace("<>", "\n"), items,
        });
    }
    shop_rows.into_iter().map(|(id, npc, tag, tp)| ShopRecord {
        id, npc_id: npc as i8, tag_name: tag, type_shop: tp.unwrap_or(0) as i8,
        tabs: tabs_by_shop.remove(&id).unwrap_or_default(),
    }).collect()
}

pub(crate) fn assemble_tasks(
    mut mains: Vec<(i32, String, String)>,
    mut subs: Vec<(i32, String, i32, String, i32, i32, i32)>,
) -> Vec<TaskMainTemplateRecord> {
    mains.sort_by_key(|m| m.0);
    subs.sort_by_key(|s| (s.0, s.6));
    let mut by_main: HashMap<i32, Vec<SubTaskTemplateRecord>> = HashMap::new();
    for (main_id, name, max_count, notify, npc_id, map_id, _) in subs {
        by_main.entry(main_id).or_default().push(SubTaskTemplateRecord {
            name, max_count: max_count as i16, notify, npc_id: npc_id as i8, map_id: MapId(map_id as i16),
        });
    }
    mains.into_iter().filter_map(|(id, name, detail)| {
        let sub_tasks = by_main.remove(&id).unwrap_or_default();
        (!sub_tasks.is_empty()).then_some(TaskMainTemplateRecord { id, name, detail, sub_tasks })
    }).collect()
}

impl GameTemplates {
    pub fn save_to_engine(&self, pool: &FjallEngine) -> DbResult<()> {
        let encoded = encode_val("game_templates", self)?;
        pool.ks().templates.insert(TEMPLATES_SNAPSHOT_KEY, encoded)?;
        Ok(())
    }

    pub async fn load_all(pool: &FjallEngine) -> DbResult<Self> {
        let Some(bytes) = pool.ks().templates.get(TEMPLATES_SNAPSHOT_KEY)? else {
            return Ok(Self::default());
        };
        let out: Self = decode_val("game_templates", &bytes)?;
        info!(items = out.items.len(), maps = out.maps.len(), shops = out.shops.len(), "loaded static GameTemplates from fjall");
        Ok(out)
    }
}
