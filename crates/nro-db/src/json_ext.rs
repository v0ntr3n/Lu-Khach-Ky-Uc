//! Codecs for `player` eggs, charms, skills, `pet`, black-ball rewards, custom `toString()`
//! quoted-primitive objects (`data_card`, `dataBadges`, `dataTaskBadges`, `dailyGift`),
//! Gson `nhiem_vu_kol` (`kol_progress`), and auxiliary state tuples.

use nro_core::{CoreStats, Gender, ItemInstance, ItemOption, ParityMode, PlayerId};
use serde::{Deserialize, Serialize};

use crate::json_columns::{json_err, parse_items_container, write_items_container};
use crate::{
    DbResult, JsonStr, PositionalArray, StrBool, StrNum, gender_from_sql, gender_to_sql,
    is_legacy_parity, parse_lenient_json_vec,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EggTimer {
    pub last_time_create_ms: i64,
    pub time_done_ms: i64,
}

pub fn parse_egg_timer(column: &'static str, raw: &str) -> DbResult<Option<EggTimer>> {
    let a = PositionalArray::parse(column, raw)?;
    Ok((a.len() >= 2).then(|| EggTimer {
        last_time_create_ms: a.i64_or(0, 0),
        time_done_ms: a.i64_or(1, 0),
    }))
}

pub fn write_egg_timer(column: &'static str, egg: Option<&EggTimer>) -> DbResult<String> {
    match egg {
        None => Ok("[]".to_owned()),
        Some(e) => serde_json::to_string(&[e.last_time_create_ms, e.time_done_ms])
            .map_err(|err| json_err(column, err)),
    }
}

pub fn parse_data_mabu_egg(raw: &str) -> DbResult<Option<EggTimer>> { parse_egg_timer("data_mabu_egg", raw) }
pub fn write_data_mabu_egg(e: Option<&EggTimer>) -> DbResult<String> { write_egg_timer("data_mabu_egg", e) }
pub fn parse_data_dua_hau_egg(raw: &str) -> DbResult<Option<EggTimer>> { parse_egg_timer("data_duahau_egg", raw) }
pub fn write_data_dua_hau_egg(e: Option<&EggTimer>) -> DbResult<String> { write_egg_timer("data_duahau_egg", e) }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CharmTimers {
    pub td_tri_tue_ms: i64, pub td_manh_me_ms: i64, pub td_da_trau_ms: i64, pub td_oai_hung_ms: i64,
    pub td_bat_tu_ms: i64, pub td_deo_dai_ms: i64, pub td_thu_hut_ms: i64, pub td_de_tu_ms: i64,
    pub td_tri_tue3_ms: i64, pub td_tri_tue4_ms: i64,
}

impl CharmTimers {
    /// Returns a mutable reference to the expiry timestamp (`ms`) for charm template ID `item_id`.
    pub fn slot_mut(&mut self, item_id: i16) -> Option<&mut i64> {
        match item_id {
            213 => Some(&mut self.td_tri_tue_ms),
            214 => Some(&mut self.td_manh_me_ms),
            215 => Some(&mut self.td_da_trau_ms),
            216 => Some(&mut self.td_oai_hung_ms),
            217 => Some(&mut self.td_bat_tu_ms),
            218 => Some(&mut self.td_deo_dai_ms),
            219 => Some(&mut self.td_thu_hut_ms),
            522 => Some(&mut self.td_de_tu_ms),
            671 => Some(&mut self.td_tri_tue3_ms),
            672 => Some(&mut self.td_tri_tue4_ms),
            _ => None,
        }
    }

    /// Extends charm `item_id` by `minutes` from `max(current_expiry, now_ms)`.
    pub fn extend_minutes(&mut self, item_id: i16, minutes: i64, now_ms: i64) -> bool {
        let Some(slot) = self.slot_mut(item_id) else { return false; };
        *slot = (*slot).max(now_ms).saturating_add(minutes.saturating_mul(60_000));
        true
    }

    /// Formats the remaining duration of charm `item_id` as an `ItemOption` (`63`/`64`/`65`/`66`).
    #[must_use]
    pub fn remaining_option(&self, item_id: i16, now_ms: i64) -> ItemOption {
        let expiry = match item_id {
            213 => self.td_tri_tue_ms, 214 => self.td_manh_me_ms, 215 => self.td_da_trau_ms,
            216 => self.td_oai_hung_ms, 217 => self.td_bat_tu_ms, 218 => self.td_deo_dai_ms,
            219 => self.td_thu_hut_ms, 522 => self.td_de_tu_ms, 671 => self.td_tri_tue3_ms,
            672 => self.td_tri_tue4_ms, _ => 0,
        };
        let rem_ms = expiry.saturating_sub(now_ms);
        if rem_ms <= 0 { return ItemOption::new(66, 0); }
        let mins = rem_ms / 60_000;
        if mins >= 1_440 {
            ItemOption::new(63, i32::try_from(mins / 1_440).unwrap_or(i32::MAX))
        } else if mins >= 60 {
            ItemOption::new(64, i32::try_from(mins / 60).unwrap_or(i32::MAX))
        } else {
            ItemOption::new(65, i32::try_from(mins.max(1)).unwrap_or(i32::MAX))
        }
    }

    /// Returns the active TNSM multiplier (`4`, `3`, `2`, or `1`) from Trí Tuệ charms.
    #[must_use]
    pub const fn tnsm_multiplier(&self, now_ms: i64) -> i64 {
        if self.td_tri_tue4_ms > now_ms { 4 }
        else if self.td_tri_tue3_ms > now_ms { 3 }
        else if self.td_tri_tue_ms > now_ms { 2 }
        else { 1 }
    }

    /// Returns the active damage multiplier percentage (`150` with Bùa Mạnh Mẽ, else `100`).
    #[must_use]
    pub const fn damage_multiplier_pct(&self, now_ms: i64) -> i64 {
        if self.td_manh_me_ms > now_ms { 150 } else { 100 }
    }
}

pub fn parse_data_charm(raw: &str) -> DbResult<CharmTimers> {
    let a = PositionalArray::parse("data_charm", raw)?;
    Ok(CharmTimers {
        td_tri_tue_ms: a.i64_or(0, 0), td_manh_me_ms: a.i64_or(1, 0), td_da_trau_ms: a.i64_or(2, 0), td_oai_hung_ms: a.i64_or(3, 0),
        td_bat_tu_ms: a.i64_or(4, 0), td_deo_dai_ms: a.i64_or(5, 0), td_thu_hut_ms: a.i64_or(6, 0), td_de_tu_ms: a.i64_or(7, 0),
        td_tri_tue3_ms: a.i64_or(8, 0), td_tri_tue4_ms: a.i64_or(9, 0),
    })
}

pub fn write_data_charm(c: &CharmTimers) -> DbResult<String> {
    serde_json::to_string(&[
        c.td_tri_tue_ms, c.td_manh_me_ms, c.td_da_trau_ms, c.td_oai_hung_ms, c.td_bat_tu_ms,
        c.td_deo_dai_ms, c.td_thu_hut_ms, c.td_de_tu_ms, c.td_tri_tue3_ms, c.td_tri_tue4_ms,
    ])
    .map_err(|e| json_err("data_charm", e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSlot {
    pub template_id: i32,
    pub point: i8,
    pub last_time_use_ms: i64,
    pub curr_level: i16,
}

pub fn parse_skills(raw: &str) -> DbResult<Vec<SkillSlot>> {
    parse_lenient_json_vec::<String>("skills", raw)?
        .into_iter()
        .map(|s| {
            let a = PositionalArray::parse("skills", &s)?;
            Ok(SkillSlot {
                template_id: a.i32_or(0, -1), point: a.i32_or(1, 0) as i8,
                last_time_use_ms: a.i64_or(2, 0), curr_level: a.i32_or(3, 0) as i16,
            })
        })
        .collect()
}

pub fn write_skills(skills: &[SkillSlot]) -> DbResult<String> {
    let v: Vec<JsonStr<(i32, i8, i64, i16)>> = skills
        .iter()
        .map(|s| JsonStr((s.template_id, s.point, s.last_time_use_ms, s.curr_level)))
        .collect();
    serde_json::to_string(&v).map_err(|e| json_err("skills", e))
}

pub fn parse_skills_shortcut(raw: &str) -> DbResult<[i8; 10]> {
    let v: Vec<i8> = parse_lenient_json_vec("skills_shortcut", raw)?;
    let mut out = [-1_i8; 10];
    for (d, s) in out.iter_mut().zip(v) { *d = s; }
    Ok(out)
}

pub fn write_skills_shortcut(sc: &[i8; 10]) -> DbResult<String> {
    serde_json::to_string(sc).map_err(|e| json_err("skills_shortcut", e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BlackBallSlot {
    pub time_out_of_date_ms: i64,
    pub last_time_get_reward_ms: i64,
    pub quantity: i32,
}

pub fn parse_data_black_ball(raw: &str) -> DbResult<[BlackBallSlot; 7]> {
    let strs: Vec<String> = parse_lenient_json_vec("data_black_ball", raw)?;
    let mut out = [BlackBallSlot::default(); 7];
    for (dst, s) in out.iter_mut().zip(strs) {
        let a = PositionalArray::parse("data_black_ball", &s)?;
        let exp = a.i64_or(0, 0);
        *dst = BlackBallSlot {
            time_out_of_date_ms: exp,
            last_time_get_reward_ms: a.i64_or(1, 0),
            quantity: if a.len() > 2 { a.i32_or(2, 0) } else { i32::from(exp != 0) },
        };
    }
    Ok(out)
}

pub fn write_data_black_ball(slots: &[BlackBallSlot; 7]) -> DbResult<String> {
    let v: Vec<JsonStr<(i64, i64, i32)>> = slots
        .iter()
        .map(|s| JsonStr((s.time_out_of_date_ms, s.last_time_get_reward_ms, s.quantity)))
        .collect();
    serde_json::to_string(&v).map_err(|e| json_err("data_black_ball", e))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PetSnapshot {
    pub type_pet: i8, pub gender: Gender, pub name: String, pub fusion_type: i8,
    pub fusion_time_left_ms: i32, pub status: i8, pub stats: CoreStats,
    pub items_body: Vec<Option<ItemInstance>>, pub skills: Vec<SkillSlot>,
}

impl PetSnapshot {
    #[must_use]
    pub const fn entity_id(owner_id: PlayerId) -> PlayerId { PlayerId(owner_id.0.saturating_neg()) }
}

pub fn parse_pet(raw: &str) -> DbResult<Option<PetSnapshot>> {
    let sec: Vec<String> = parse_lenient_json_vec("pet", raw)?;
    if sec.len() < 4 { return Ok(None); }
    let info: (i8, i32, String, i8, i32, i8) = serde_json::from_str(&sec[0]).map_err(|e| json_err("pet", e))?;
    let p = PositionalArray::parse("pet", &sec[1])?;
    let mut items_body = parse_items_container("pet", &sec[2])?;
    while items_body.len() < if matches!(info.0, 2..=4) { 9 } else { 7 } { items_body.push(None); }
    let mut skills = parse_skills(&sec[3])?;
    while skills.len() < if matches!(info.0, 3 | 4) { 5 } else { 4 } {
        skills.push(SkillSlot { template_id: -1, point: 0, last_time_use_ms: 0, curr_level: 0 });
    }
    Ok(Some(PetSnapshot {
        type_pet: info.0, gender: gender_from_sql(info.1)?, name: info.2, fusion_type: info.3,
        fusion_time_left_ms: info.4, status: info.5,
        stats: CoreStats {
            limit_power: p.i32_or(0, 0).clamp(0, 9) as u8, power: p.i64_or(1, 0), tiem_nang: p.i64_or(2, 0),
            stamina: p.i32_or(3, 0).clamp(0, 65_535) as u16, max_stamina: p.i32_or(4, 0).clamp(0, 65_535) as u16, hpg: p.i64_or(5, 100),
            mpg: p.i64_or(6, 100), dameg: p.i64_or(7, 10), defg: p.i32_or(8, 0),
            critg: p.i32_or(9, 0).clamp(0, 255) as u8, critdragon: 0, hp: p.i64_or(10, 100), mp: p.i64_or(11, 100),
            ..CoreStats::default()
        },
        items_body, skills,
    }))
}

pub fn write_pet(pet: Option<&PetSnapshot>, empty_ts: i64) -> DbResult<String> {
    let Some(p) = pet else { return Ok("[]".to_owned()); };
    let info = JsonStr((p.type_pet, gender_to_sql(p.gender), p.name.as_str(), p.fusion_type, p.fusion_time_left_ms.max(0), p.status));
    let s = &p.stats;
    let pt = JsonStr((s.limit_power, s.power, s.tiem_nang, s.stamina, s.max_stamina, s.hpg, s.mpg, s.dameg, s.defg, s.critg, s.hp, s.mp));
    serde_json::to_string(&(info, pt, write_items_container("pet", &p.items_body, empty_ts)?, write_skills(&p.skills)?))
        .map_err(|e| json_err("pet", e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadarCardOption { pub id: StrNum<i32>, pub active: StrNum<i8>, pub param: StrNum<i32> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadarCardRecord {
    pub id: StrNum<i16>, pub amount: StrNum<i8>, pub max: StrNum<i8>,
    pub option: Vec<RadarCardOption>, pub level: StrNum<i8>, pub used: StrNum<i8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerBadgeRecord {
    #[serde(rename = "idBadGes")] pub id_badges: StrNum<i32>,
    #[serde(rename = "timeofUseBadges")] pub time_of_use_badges_ms: StrNum<i64>,
    #[serde(rename = "isUse")] pub is_use: StrBool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BadgeTaskRecord {
    pub id: StrNum<i32>, pub count: StrNum<i32>,
    #[serde(rename = "countMax")] pub count_max: StrNum<i32>,
    #[serde(rename = "idBadgesReward")] pub id_badges_reward: StrNum<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyGiftRecord { pub id: StrNum<i8>, #[serde(rename = "daNhan")] pub da_nhan: StrBool }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KolProgressData {
    #[serde(rename = "kolQuestStage")] pub kol_quest_stage: i32,
    #[serde(rename = "kolVIPQuestStage")] pub kol_vip_quest_stage: i32,
    #[serde(rename = "destronGas70CompletionCount")] pub destron_gas_70_completion_count: i32,
    #[serde(rename = "martialArtsTournamentWins")] pub martial_arts_tournament_wins: i32,
    #[serde(rename = "dailySuperHardQuestCompletionCount")] pub daily_super_hard_quest_completion_count: i32,
    #[serde(rename = "bossBabyDefeatParticipationCount")] pub boss_baby_defeat_participation_count: i32,
    #[serde(rename = "monsterKillCountAutoTrain")] pub monster_kill_count_auto_train: i64,
}

impl Default for KolProgressData {
    fn default() -> Self {
        Self {
            kol_quest_stage: 1, kol_vip_quest_stage: 1, destron_gas_70_completion_count: 0,
            martial_arts_tournament_wins: 0, daily_super_hard_quest_completion_count: 0,
            boss_baby_defeat_participation_count: 0, monster_kill_count_auto_train: 0,
        }
    }
}

pub fn parse_data_card(raw: &str) -> DbResult<Vec<RadarCardRecord>> { parse_lenient_json_vec("data_card", raw) }
pub fn write_data_card(cards: &[RadarCardRecord]) -> DbResult<String> { serde_json::to_string(cards).map_err(|e| json_err("data_card", e)) }
pub fn parse_data_badges(raw: Option<&str>) -> DbResult<Vec<PlayerBadgeRecord>> { parse_lenient_json_vec("dataBadges", raw.unwrap_or("[]")) }
pub fn write_data_badges(b: &[PlayerBadgeRecord]) -> DbResult<String> { serde_json::to_string(b).map_err(|e| json_err("dataBadges", e)) }
pub fn parse_data_task_badges(raw: Option<&str>) -> DbResult<Vec<BadgeTaskRecord>> { parse_lenient_json_vec("dataTaskBadges", raw.unwrap_or("[]")) }
pub fn write_data_task_badges(t: &[BadgeTaskRecord]) -> DbResult<String> { serde_json::to_string(t).map_err(|e| json_err("dataTaskBadges", e)) }
pub fn parse_daily_gift(raw: &str) -> DbResult<Vec<DailyGiftRecord>> { parse_lenient_json_vec("dailyGift", raw) }
pub fn write_daily_gift(d: &[DailyGiftRecord]) -> DbResult<String> { serde_json::to_string(d).map_err(|e| json_err("dailyGift", e)) }

pub fn parse_kol_progress(raw: &str) -> DbResult<KolProgressData> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("null") { return Ok(KolProgressData::default()); }
    serde_json::from_str(t).map_err(|e| json_err("nhiem_vu_kol", e))
}

pub fn write_kol_progress(k: &KolProgressData) -> DbResult<String> {
    serde_json::to_string(k).map_err(|e| json_err("nhiem_vu_kol", e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TrainingState {
    pub level_luyen_tap: i32, pub dang_ky_tap_tu_dong: bool, pub map_id_dang_tap: i32,
    pub tnsm_luyen_tap: i32, pub last_time_offline_ms: i64, pub top: i32, pub time: i32,
    pub last_time_ms: i64, pub last_top: i32, pub last_reward_time_ms: i64,
}

#[must_use]
pub fn parse_data_luyentap(raw: &str) -> TrainingState {
    let Ok(a) = PositionalArray::parse("data_luyentap", raw) else {
        return TrainingState { map_id_dang_tap: -1, ..Default::default() };
    };
    if a.len() < 5 { return TrainingState { map_id_dang_tap: -1, ..Default::default() }; }
    TrainingState {
        level_luyen_tap: a.i32_or(0, 0), dang_ky_tap_tu_dong: a.bool_or(1, false),
        map_id_dang_tap: a.i32_or(2, -1), tnsm_luyen_tap: a.i32_or(3, 0),
        last_time_offline_ms: a.i64_or(4, 0), top: a.i32_or(5, 0), time: a.i32_or(6, 0),
        last_time_ms: a.i64_or(7, 0), last_top: a.i32_or(8, 0), last_reward_time_ms: a.i64_or(9, 0),
    }
}

pub fn write_data_luyentap(t: &TrainingState) -> DbResult<String> {
    serde_json::to_string(&(
        t.level_luyen_tap, t.dang_ky_tap_tu_dong, t.map_id_dang_tap, t.tnsm_luyen_tap,
        t.last_time_offline_ms, t.top, t.time, t.last_time_ms, t.last_top, t.last_reward_time_ms,
    )).map_err(|e| json_err("data_luyentap", e))
}

#[must_use]
pub fn parse_con_duong_ran_doc(raw: &str, mode: ParityMode) -> (bool, i64, bool, bool) {
    let Ok(a) = PositionalArray::parse("conduongrandoc", raw) else { return (false, 0, false, false); };
    if a.len() < 3 { return (false, 0, false, false); }
    let td = a.bool_or(2, false);
    let tm = if is_legacy_parity(mode) { td } else { a.bool_or(3, td) };
    (a.bool_or(0, false), a.i64_or(1, 0), td, tm)
}

#[must_use]
pub fn parse_data_item_event(raw: &str, mode: ParityMode) -> [i64; 16] {
    let Ok(v) = parse_lenient_json_vec::<i64>("data_item_event", raw) else { return [0; 16]; };
    if is_legacy_parity(mode) && v.len() < 16 { return [0; 16]; }
    let mut out = [0_i64; 16];
    for (d, s) in out.iter_mut().zip(v) { *d = s; }
    out
}

pub fn write_data_item_event(vals: &[i64; 16], mode: ParityMode) -> DbResult<String> {
    let slice = if is_legacy_parity(mode) { &vals[..6] } else { vals.as_slice() };
    serde_json::to_string(slice).map_err(|e| json_err("data_item_event", e))
}

pub fn parse_data_achievement(raw: &str) -> DbResult<Vec<(i64, bool)>> {
    Ok(parse_lenient_json_vec::<JsonStr<(i64, StrBool)>>("data_achievement", raw)?
        .into_iter().map(|JsonStr((c, r))| (c, r.0)).collect())
}

pub fn write_data_achievement(list: &[(i64, bool)]) -> DbResult<String> {
    let v: Vec<JsonStr<(i64, bool)>> = list.iter().copied().map(JsonStr).collect();
    serde_json::to_string(&v).map_err(|e| json_err("data_achievement", e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyTaskProgress {
    pub template_id: i32, pub received_time_ms: i64, pub count: i32,
    pub max_count: i32, pub left_task: i32, pub level: i32,
}

impl Default for DailyTaskProgress {
    fn default() -> Self {
        Self { template_id: -1, received_time_ms: 0, count: 0, max_count: 0, left_task: 20, level: 0 }
    }
}

pub fn parse_daily_task(column: &'static str, raw: &str) -> DbResult<DailyTaskProgress> {
    let a = PositionalArray::parse(column, raw)?;
    if a.len() < 6 { return Ok(DailyTaskProgress::default()); }
    Ok(DailyTaskProgress {
        template_id: a.i32_or(0, -1), received_time_ms: a.i64_or(1, 0), count: a.i32_or(2, 0),
        max_count: a.i32_or(3, 0), left_task: a.i32_or(4, 20), level: a.i32_or(5, 0),
    })
}

pub fn write_daily_task(column: &'static str, d: &DailyTaskProgress) -> DbResult<String> {
    serde_json::to_string(&(d.template_id, d.received_time_ms, d.count, d.max_count, d.left_task, d.level))
        .map_err(|e| json_err(column, e))
}

pub fn parse_data_side_task(raw: &str) -> DbResult<DailyTaskProgress> { parse_daily_task("data_side_task", raw) }
pub fn write_data_side_task(d: &DailyTaskProgress) -> DbResult<String> { write_daily_task("data_side_task", d) }
