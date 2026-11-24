//! Shop skill learning (`learn_shop_skill`), skill book consumption (`apply_skill_book`), and capsule map labels.

use nro_core::{MapId, TemplateId};
use nro_db::{json_columns::SkillSlot, player_repo::PlayerSnapshot};
use nro_player::skills::SkillKind;
use nro_protocol::Packet;
use nro_world::s2c_packets;

use super::super::ServerSessionHandler;

impl ServerSessionHandler {
    pub(in crate::router) fn learn_shop_skill(
        &self,
        snap: &mut PlayerSnapshot,
        item_id: TemplateId,
    ) -> Option<Packet> {
        let g_cls = i8::from(snap.gender);
        let tpl = self
            .shared
            .templates
            .items
            .iter()
            .find(|r| r.id == i32::from(item_id.0));
        let st = tpl
            .and_then(|ir| {
                self.shared
                    .templates
                    .skills
                    .iter()
                    .find(|s| s.nclass_id == g_cls && ir.name.contains(&s.name))
            })
            .or_else(|| {
                self.shared
                    .templates
                    .skills
                    .iter()
                    .find(|s| s.nclass_id == g_cls && s.levels.iter().any(|l| l.id == item_id.0))
            })?;
        let cur_pt = snap
            .skills
            .iter()
            .find(|sk| sk.template_id == i32::from(st.id))
            .map_or(0, |sk| sk.point);
        let next_pt = (cur_pt + 1).min(st.max_point.max(1));
        if next_pt <= cur_pt {
            return None;
        }
        let lv = st
            .levels
            .iter()
            .find(|l| l.point == next_pt)
            .or_else(|| st.levels.first())?;
        let tn_cost = i64::from(lv.price.max(0)).saturating_mul(1_000);
        if snap.stats.power < lv.power_require || snap.stats.tiem_nang < tn_cost {
            return None;
        }
        snap.stats.tiem_nang -= tn_cost;
        if let Some(sk) = snap
            .skills
            .iter_mut()
            .find(|s| s.template_id == i32::from(st.id))
        {
            let was_zero = sk.point <= 0;
            sk.point = next_pt;
            Some(if was_zero {
                s2c_packets::me_add_skill(lv.id)
            } else {
                s2c_packets::me_update_skill(lv.id)
            })
        } else {
            snap.skills.push(SkillSlot {
                template_id: i32::from(st.id),
                point: next_pt,
                last_time_use_ms: 0,
                curr_level: 0,
            });
            Some(s2c_packets::me_add_skill(lv.id))
        }
    }

    pub(in crate::router) fn capsule_label_for(&self, map_id: MapId) -> (&str, &str) {
        self.shared
            .templates
            .maps
            .iter()
            .find(|m| m.id == map_id)
            .map_or(("Trạm tàu vũ trụ", "Trái Đất"), |m| {
                (
                    m.name.as_str(),
                    match m.planet_id {
                        1 => "Namếc",
                        2 => "Xayda",
                        _ => "Trái Đất",
                    },
                )
            })
    }

    pub(in crate::router) fn apply_skill_book(
        &self,
        snap: &mut PlayerSnapshot,
        book_tid: TemplateId,
    ) -> Option<Packet> {
        let g_cls = i8::from(snap.gender);
        let item_rec = self
            .shared
            .templates
            .items
            .iter()
            .find(|r| r.id == i32::from(book_tid.0));
        let st = u16::try_from(book_tid.0)
            .ok()
            .and_then(SkillKind::from_skill_book_item)
            .and_then(|kind| {
                self.shared
                    .templates
                    .skills
                    .iter()
                    .find(|s| s.id == kind as i8)
            })
            .or_else(|| {
                item_rec.and_then(|ir| {
                    self.shared
                        .templates
                        .skills
                        .iter()
                        .find(|s| s.nclass_id == g_cls && ir.name.contains(&s.name))
                })
            })?;
        let stid = st.id;
        let new_pt = item_rec.map_or(1_i8, |ir| {
            ir.level.clamp(1, i32::from(st.max_point.max(1))) as i8
        });
        if let Some(existing) = snap
            .skills
            .iter_mut()
            .find(|sk| sk.template_id == i32::from(stid))
        {
            let was_zero = existing.point <= 0;
            let final_pt = existing.point.max(new_pt);
            existing.point = final_pt;
            let lv_id = st
                .levels
                .iter()
                .find(|l| l.point == final_pt)
                .or_else(|| st.levels.first())
                .map_or(i16::from(stid), |l| l.id);
            Some(if was_zero {
                s2c_packets::me_add_skill(lv_id)
            } else {
                s2c_packets::me_update_skill(lv_id)
            })
        } else {
            let lv_id = st
                .levels
                .iter()
                .find(|l| l.point == new_pt)
                .or_else(|| st.levels.first())
                .map_or(i16::from(stid), |l| l.id);
            snap.skills.push(SkillSlot {
                template_id: i32::from(stid),
                point: new_pt,
                last_time_use_ms: 0,
                curr_level: 0,
            });
            Some(s2c_packets::me_add_skill(lv_id))
        }
    }
}
