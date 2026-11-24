//! Lenient JSON array normalizers for `map_template`, `part`, and `skill_template` SQL columns.

use crate::templates::{MapWaypointRecord, SkillLevelRecord};
use crate::{DbResult, JsonStr, parse_lenient_json_vec};

fn sanitize_json_ctrl(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Normalizes legacy `Manager.java:816-819` unescaped `waypoints` strings (`["["Name",...]]`).
pub(super) fn parse_map_waypoints(raw: &str) -> DbResult<Vec<MapWaypointRecord>> {
    let clean = sanitize_json_ctrl(raw);
    if let Ok(list) = parse_lenient_json_vec::<JsonStr<MapWaypointRecord>>("waypoints", &clean) {
        return Ok(list.into_iter().map(|w| w.0).collect());
    }
    let repaired = clean
        .replace("[\"[", "[[")
        .replace("]\"]", "]]")
        .replace("\",\"", ",");
    Ok(
        parse_lenient_json_vec::<JsonStr<MapWaypointRecord>>("waypoints", &repaired)?
            .into_iter()
            .map(|w| w.0)
            .collect(),
    )
}

/// Normalizes `Manager.java:839` `mobs` arrays (including embedded newlines/unquoted pairs on map `136`).
pub(super) fn parse_map_mobs(raw: &str) -> DbResult<Vec<(i8, i8, i32, i16, i16)>> {
    let clean: String = sanitize_json_ctrl(raw)
        .chars()
        .filter(|&c| c != '"')
        .collect();
    parse_lenient_json_vec::<(i8, i8, i32, i16, i16)>("mobs", &clean)
}

/// Normalizes `Manager.java:856` `npcs` arrays (including 4-element `[id, x, y, 0]` on maps `122..=124`).
pub(super) fn parse_map_npcs(raw: &str) -> DbResult<Vec<(i8, i16, i16)>> {
    let clean: String = sanitize_json_ctrl(raw)
        .chars()
        .filter(|&c| c != '"')
        .collect();
    let rows = parse_lenient_json_vec::<Vec<i32>>("npcs", &clean)?;
    Ok(rows
        .into_iter()
        .filter(|v| v.len() >= 3)
        .map(|v| (v[0] as i8, v[1] as i16, v[2] as i16))
        .collect())
}

/// Normalizes `Manager.java:259` `part.data` integer triples.
pub(super) fn parse_part_details(raw: &str) -> DbResult<Vec<(i16, i8, i8)>> {
    let clean: String = sanitize_json_ctrl(raw)
        .chars()
        .filter(|&c| c != '"')
        .collect();
    parse_lenient_json_vec::<(i16, i8, i8)>("part.DATA", &clean)
}

/// Normalizes `Manager.java:437-443` unescaped `skill_template.skills` JSON object strings.
pub(super) fn parse_skill_levels(raw: &str) -> DbResult<Vec<SkillLevelRecord>> {
    let clean = sanitize_json_ctrl(raw);
    if let Ok(list) =
        parse_lenient_json_vec::<JsonStr<SkillLevelRecord>>("skill_template.skills", &clean)
    {
        return Ok(list.into_iter().map(|l| l.0).collect());
    }
    let repaired = clean
        .replace("[\"{", "[{")
        .replace("\"{", "{")
        .replace("}\"]", "}]")
        .replace("}\",\"{", "},{")
        .replace("}\"", "}");
    Ok(
        parse_lenient_json_vec::<JsonStr<SkillLevelRecord>>("skill_template.skills", &repaired)?
            .into_iter()
            .map(|l| l.0)
            .collect(),
    )
}
