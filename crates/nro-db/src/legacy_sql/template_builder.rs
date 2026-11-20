//! Accumulates all 26 static template SQL tables during dump streaming and compiles [`GameTemplates`].

use std::collections::HashMap;

use chrono::Utc;
use nro_core::{MapId, TemplateId};

use crate::legacy_sql::dump_parser::SqlRowMap;
use crate::templates::{
    GameTemplates, IntrinsicRecord, ItemTemplateRecord, MapTemplateRecord, MapWaypointRecord,
    MobTemplateRecord, NpcTemplateRecord, RadarBodyParts, RadarOptionTemplate, RadarTemplateRecord,
    SkillLevelRecord, SkillTemplateRecord, assemble_shops, assemble_tasks, parse_range_pair,
};
use crate::world_repos::NumericOptionJson;
use crate::{DbResult, JsonStr, parse_lenient_json_vec};

#[derive(Default)]
#[allow(clippy::type_complexity)]
pub(crate) struct TemplateCollector {
    items: Vec<ItemTemplateRecord>,
    item_options: Vec<(i32, String)>,
    mobs: Vec<MobTemplateRecord>,
    npcs: Vec<NpcTemplateRecord>,
    intrinsics: Vec<IntrinsicRecord>,
    maps: Vec<(i32, MapTemplateRecord)>,
    skills: Vec<SkillTemplateRecord>,
    shop_rows: Vec<(i32, i32, Option<String>, Option<i32>)>,
    tab_rows: Vec<(i32, i32, String)>,
    item_shop_rows: Vec<(i32, i32, i32, bool, bool, Option<i32>, Option<i32>, Option<i32>, i64)>,
    item_shop_opts: Vec<(i32, i32, i32)>,
    task_mains: Vec<(i32, String, String)>,
    task_subs: Vec<(i32, String, i32, String, i32, i32, i32)>,
    side_tasks: Vec<(i32, String, [(i32, i32); 5])>,
    clan_tasks: Vec<(i32, String, [(i32, i32); 5])>,
    achievements: Vec<(i32, String, String, i32, i64)>,
    task_badges: Vec<(i32, String, i32, i32)>,
    badges: Vec<(i32, i32, i32, String, Vec<NumericOptionJson>)>,
    flag_bags: Vec<(i32, String, i32, i32, i16, Vec<i16>)>,
    head_avatars: Vec<(i16, i16)>,
    parts: Vec<(i16, i8, Vec<(i16, i8, i8)>)>,
    bg_items: Vec<(i32, i16, i8, i16, i16)>,
    arr_head_2_frames: Vec<(i32, Vec<i16>)>,
    images_by_name: HashMap<String, u8>,
    radars: Vec<RadarTemplateRecord>,
    notices: Vec<(i32, String)>,
}

impl TemplateCollector {
    pub fn try_ingest(&mut self, table: &str, r: &SqlRowMap<'_>) -> DbResult<bool> {
        match table {
            "item_template" => self.items.push(ItemTemplateRecord {
                id: r.get("id").i32_or(0), item_type: r.get("type").i32_or(0),
                gender: r.get("gender").i32_or(0) as i16, name: r.get("name").as_str_or_empty().to_owned(),
                description: r.get("description").opt_string(), level: r.get("level").i32_or(0),
                icon_id: r.get("icon_id").i32_or(0), part: r.get("part").i32_or(-1),
                is_up_to_up: r.get("is_up_to_up").bool_or(false), power_require: r.get("power_require").i32_or(0),
                gold: r.get("gold").i32_or(0), gem: r.get("gem").i32_or(0),
                head: r.get("head").i32_or(-1), body: r.get("body").i32_or(-1), leg: r.get("leg").i32_or(-1),
            }),
            "item_option_template" => self.item_options.push((
                r.get("id").i32_or(0), r.get("name").as_str_or_empty().to_owned(),
            )),
            "mob_template" => self.mobs.push(MobTemplateRecord {
                id: r.get("id").i32_or(0), mob_type: r.get("type").i32_or(0),
                name: r.get("name").as_str_or_empty().to_owned(), hp: r.get("hp").i32_or(100),
                range_move: r.get("range_move").i32_or(0) as i16, speed: r.get("speed").i32_or(0) as i16,
                dart_type: r.get("dart_type").i32_or(0) as i16,
                percent_dame: r.get("percent_dame").i32_or(0) as i16,
                percent_tiem_nang: r.get("percent_tiem_nang").i32_or(0) as i16,
            }),
            "npc_template" => self.npcs.push(NpcTemplateRecord {
                id: r.get("id").i32_or(0), name: r.get("name").as_str_or_empty().to_owned(),
                head: r.get("head").i32_or(0), body: r.get("body").i32_or(0),
                leg: r.get("leg").i32_or(0), avatar: r.get("avatar").i32_opt(),
            }),
            "intrinsic" => self.intrinsics.push(IntrinsicRecord {
                id: r.get("id").i32_or(0), name: r.get("name").as_str_or_empty().to_owned(),
                param_from_1: r.get("param_from_1").i32_or(0), param_to_1: r.get("param_to_1").i32_or(0),
                param_from_2: r.get("param_from_2").i32_or(0), param_to_2: r.get("param_to_2").i32_or(0),
                icon: r.get("icon").i32_or(0), gender: r.get("gender").i32_or(0) as i16,
            }),
            "map_template" => {
                let id = r.get("id").i32_or(0);
                let waypoints = parse_map_waypoints(r.get("waypoints").as_str_or_empty())?;
                let mobs = parse_map_mobs(r.get("mobs").as_str_or_empty())?;
                let npcs = parse_map_npcs(r.get("npcs").as_str_or_empty())?;
                self.maps.push((id, MapTemplateRecord {
                    id: MapId(id as i16), name: r.get("name").as_str_or_empty().to_owned(),
                    zones: r.get("zones").i32_or(1) as u8, max_player: r.get("max_player").i32_or(15) as u8,
                    map_type: r.get("type").i32_or(0) as i8, planet_id: r.get("planet_id").i32_or(0) as i8,
                    bg_type: r.get("bg_type").i32_or(0) as i8, tile_id: r.get("tile_id").i32_or(0) as i8,
                    bg_id: r.get("bg_id").i32_or(0) as i8, waypoints, mobs, npcs,
                }));
            }
            "skill_template" => {
                let levels = parse_skill_levels(r.get("skills").as_str_or_empty())?;
                self.skills.push(SkillTemplateRecord {
                    nclass_id: r.get("nclass_id").i32_or(0) as i8, id: r.get("id").i32_or(0) as i8,
                    name: r.get("name").as_str_or_empty().to_owned(), max_point: r.get("max_point").i32_or(1) as i8,
                    mana_use_type: r.get("mana_use_type").i32_or(0) as i8, skill_type: r.get("type").i32_or(0) as i8,
                    icon_id: r.get("icon_id").i32_or(0) as i16, dam_info: r.get("dam_info").as_str_or_empty().to_owned(),
                    slot: r.get("slot").i32_or(0), levels,
                });
            }
            "shop" => self.shop_rows.push((
                r.get("id").i32_or(0), r.get("npc_id").i32_or(0),
                r.get("tag_name").opt_string(), r.get("type_shop").i32_opt(),
            )),
            "tab_shop" => self.tab_rows.push((
                r.get("id").i32_or(0), r.get("shop_id").i32_or(0), r.get("name").as_str_or_empty().to_owned(),
            )),
            "item_shop" => {
                let created_ms = r.get("create_time").datetime_utc(Utc::now()).timestamp_millis();
                self.item_shop_rows.push((
                    r.get("id").i32_or(0), r.get("tab_id").i32_or(0), r.get("temp_id").i32_or(0),
                    r.get("is_new").bool_or(false), r.get("is_sell").bool_or(true),
                    r.get("type_sell").i32_opt(), r.get("cost").i32_opt(),
                    r.get("icon_spec").i32_opt(), created_ms,
                ));
            }
            "item_shop_option" => self.item_shop_opts.push((
                r.get("item_shop_id").i32_or(0), r.get("option_id").i32_or(0), r.get("param").i32_or(0),
            )),
            "task_main_template" => self.task_mains.push((
                r.get("id").i32_or(0), r.get("name").as_str_or_empty().to_owned(),
                r.get("detail").as_str_or_empty().to_owned(),
            )),
            "task_sub_template" => self.task_subs.push((
                r.get("task_main_id").i32_or(0), r.get("name").as_str_or_empty().to_owned(),
                r.get("max_count").i32_or(0), r.get("notify").as_str_or_empty().to_owned(),
                r.get("npc_id").i32_or(0), r.get("map").i32_or(0), r.get("ducvupro").i32_or(0),
            )),
            "side_task_template" | "clan_task_template" => {
                let rec = (
                    r.get("id").i32_or(0),
                    r.get("name").as_str_or_empty().to_owned(),
                    [
                        parse_range_pair(r.get("max_count_lv1").as_str_or_empty()),
                        parse_range_pair(r.get("max_count_lv2").as_str_or_empty()),
                        parse_range_pair(r.get("max_count_lv3").as_str_or_empty()),
                        parse_range_pair(r.get("max_count_lv4").as_str_or_empty()),
                        parse_range_pair(r.get("max_count_lv5").as_str_or_empty()),
                    ],
                );
                if table == "clan_task_template" { self.clan_tasks.push(rec); } else { self.side_tasks.push(rec); }
            }
            "achievement_template" => self.achievements.push((
                r.get("id").i32_or(0), r.get("info1").as_str_or_empty().to_owned(),
                r.get("info2").as_str_or_empty().to_owned(), r.get("money").i32_or(0),
                r.get("max_count").i64_or(0),
            )),
            "task_badges_template" => self.task_badges.push((
                r.get("id").i32_or(0), r.get("name").as_str_or_empty().to_owned(),
                r.get("maxcount").i32_or(0), r.get("idbadgesreward").i32_or(0),
            )),
            "data_badges" => self.badges.push((
                r.get("id").i32_or(0), r.get("ideffect").i32_or(0), r.get("iditem").i32_or(0),
                r.get("name").as_str_or_empty().to_owned(),
                parse_lenient_json_vec("data_badges.Options", r.get("options").as_str_or_empty())?,
            )),
            "flag_bag" => {
                let eff = r.get("icon_data").as_str_or_empty().split(',')
                    .filter_map(|x| x.trim().parse::<i16>().ok()).collect();
                self.flag_bags.push((
                    r.get("id").i32_or(0), r.get("name").as_str_or_empty().to_owned(),
                    r.get("gold").i32_or(0), r.get("gem").i32_or(0),
                    r.get("icon_id").i32_or(0) as i16, eff,
                ));
            }
            "head_avatar" => self.head_avatars.push((
                r.get("head_id").i32_or(0) as i16, r.get("avatar_id").i32_or(0) as i16,
            )),
            "part" => {
                let details = parse_part_details(r.get("data").as_str_or_empty())?;
                self.parts.push((r.get("id").i32_or(0) as i16, r.get("type").i32_or(0) as i8, details));
            }
            "bg_item_template" => self.bg_items.push((
                r.get("id").i32_or(0), r.get("image_id").i32_or(0) as i16,
                r.get("layer").i32_or(0) as i8, r.get("dx").i32_or(0) as i16, r.get("dy").i32_or(0) as i16,
            )),
            "array_head_2_frames" => self.arr_head_2_frames.push((
                r.get("id").i32_or(0),
                parse_lenient_json_vec("array_head_2_frames.data", r.get("data").as_str_or_empty())?,
            )),
            "img_by_name" => {
                self.images_by_name.insert(
                    r.get("name").as_str_or_empty().to_owned(),
                    r.get("n_frame").i32_or(0) as u8,
                );
            }
            "radar" => {
                let body_raw = r.get("body").opt_string().unwrap_or_else(|| "[]".to_owned());
                let opt_raw = r.get("options").opt_string().unwrap_or_else(|| "[]".to_owned());
                let body = parse_lenient_json_vec::<RadarBodyParts>("radar.body", &body_raw)?
                    .into_iter().next().unwrap_or_default();
                let options = parse_lenient_json_vec::<RadarOptionTemplate>("radar.options", &opt_raw)?;
                self.radars.push(RadarTemplateRecord {
                    id: r.get("id").i32_or(0) as i16, icon_id: r.get("iconid").i32_or(0) as i16,
                    rank: r.get("rank").i32_or(0) as i8, max: r.get("max").i32_or(60) as i8,
                    card_type: r.get("type").i32_or(0) as i8, mob_id: r.get("mob_id").i32_or(1) as i16,
                    body, name: r.get("name").as_str_or_empty().to_owned(),
                    info: r.get("info").as_str_or_empty().to_owned(), options,
                    require: r.get("require").i32_or(-1) as i16,
                    require_level: r.get("require_level").i32_or(0) as i16,
                    aura_id: r.get("aura_id").i32_or(-1) as i16,
                });
            }
            "notify" => self.notices.push((
                r.get("id").i32_or(0),
                format!("{}<>{}", r.get("name").as_str_or_empty(), r.get("text").as_str_or_empty()),
            )),
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub fn build(mut self) -> GameTemplates {
        self.items.sort_by_key(|x| x.id);
        self.item_options.sort_by_key(|x| x.0);
        self.mobs.sort_by_key(|x| x.id);
        self.npcs.sort_by_key(|x| x.id);
        self.intrinsics.sort_by_key(|x| x.id);
        self.maps.sort_by_key(|x| x.0);
        self.skills.sort_by_key(|s| (s.nclass_id, s.slot));
        self.side_tasks.sort_by_key(|x| x.0);
        self.clan_tasks.sort_by_key(|x| x.0);
        self.achievements.sort_by_key(|x| x.0);
        self.task_badges.sort_by_key(|x| x.0);
        self.badges.sort_by_key(|x| x.0);
        self.flag_bags.sort_by_key(|x| x.0);
        self.arr_head_2_frames.sort_by_key(|x| x.0);
        self.notices.sort_by_key(|b| std::cmp::Reverse(b.0));

        let mut mount_map = HashMap::new();
        for it in &self.items {
            if it.item_type == 23
                && self.images_by_name.get(&format!("mount_{}_0", it.part)).copied().unwrap_or(0) != 0
            {
                mount_map.insert(TemplateId(it.id as i16), (it.part + 30_000) as i16);
            }
        }

        GameTemplates {
            items: self.items, item_options: self.item_options, mobs: self.mobs, npcs: self.npcs,
            maps: self.maps.into_iter().map(|(_, m)| m).collect(), skills: self.skills,
            intrinsics: self.intrinsics,
            shops: assemble_shops(self.shop_rows, self.tab_rows, self.item_shop_rows, self.item_shop_opts),
            tasks: assemble_tasks(self.task_mains, self.task_subs),
            side_tasks: self.side_tasks, clan_tasks: self.clan_tasks, achievements: self.achievements,
            task_badges: self.task_badges, badges: self.badges, flag_bags: self.flag_bags,
            head_avatars: self.head_avatars, parts: self.parts, bg_items: self.bg_items,
            arr_head_2_frames: self.arr_head_2_frames.into_iter().map(|(_, d)| d).collect(),
            images_by_name: self.images_by_name, mount_map, radars: self.radars,
            notices: self.notices.into_iter().map(|(_, s)| s).collect(),
        }
    }
}

fn sanitize_json_ctrl(raw: &str) -> String {
    raw.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

/// Normalizes legacy `Manager.java:816-819` unescaped `waypoints` strings (`["["Name",...]]`).
fn parse_map_waypoints(raw: &str) -> DbResult<Vec<MapWaypointRecord>> {
    let clean = sanitize_json_ctrl(raw);
    if let Ok(list) = parse_lenient_json_vec::<JsonStr<MapWaypointRecord>>("waypoints", &clean) {
        return Ok(list.into_iter().map(|w| w.0).collect());
    }
    let repaired = clean
        .replace("[\"[", "[[")
        .replace("]\"]", "]]")
        .replace("\",\"", ",");
    Ok(parse_lenient_json_vec::<JsonStr<MapWaypointRecord>>("waypoints", &repaired)?
        .into_iter()
        .map(|w| w.0)
        .collect())
}

/// Normalizes `Manager.java:839` `mobs` arrays (including embedded newlines/unquoted pairs on map `136`).
fn parse_map_mobs(raw: &str) -> DbResult<Vec<(i8, i8, i32, i16, i16)>> {
    let clean: String = sanitize_json_ctrl(raw).chars().filter(|&c| c != '"').collect();
    parse_lenient_json_vec::<(i8, i8, i32, i16, i16)>("mobs", &clean)
}

/// Normalizes `Manager.java:856` `npcs` arrays (including 4-element `[id, x, y, 0]` on maps `122..=124`).
fn parse_map_npcs(raw: &str) -> DbResult<Vec<(i8, i16, i16)>> {
    let clean: String = sanitize_json_ctrl(raw).chars().filter(|&c| c != '"').collect();
    let rows = parse_lenient_json_vec::<Vec<i32>>("npcs", &clean)?;
    Ok(rows
        .into_iter()
        .filter(|v| v.len() >= 3)
        .map(|v| (v[0] as i8, v[1] as i16, v[2] as i16))
        .collect())
}

/// Normalizes `Manager.java:259` `part.data` integer triples.
fn parse_part_details(raw: &str) -> DbResult<Vec<(i16, i8, i8)>> {
    let clean: String = sanitize_json_ctrl(raw).chars().filter(|&c| c != '"').collect();
    parse_lenient_json_vec::<(i16, i8, i8)>("part.DATA", &clean)
}

/// Normalizes `Manager.java:437-443` unescaped `skill_template.skills` JSON object strings.
fn parse_skill_levels(raw: &str) -> DbResult<Vec<SkillLevelRecord>> {
    let clean = sanitize_json_ctrl(raw);
    if let Ok(list) = parse_lenient_json_vec::<JsonStr<SkillLevelRecord>>("skill_template.skills", &clean) {
        return Ok(list.into_iter().map(|l| l.0).collect());
    }
    let repaired = clean
        .replace("[\"{", "[{")
        .replace("\"{", "{")
        .replace("}\"]", "}]")
        .replace("}\",\"{", "},{")
        .replace("}\"", "}");
    Ok(parse_lenient_json_vec::<JsonStr<SkillLevelRecord>>("skill_template.skills", &repaired)?
        .into_iter()
        .map(|l| l.0)
        .collect())
}
