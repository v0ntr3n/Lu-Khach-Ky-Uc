//! Subtask matching predicates and reward/milestone transitions for [`super`].

use nro_core::{MapId, TemplateId};
use nro_db::templates::{GameTemplates, SubTaskTemplateRecord};
use nro_player::progression::TaskPlayer;
use nro_protocol::Packet;
use nro_world::{ZoneCommand, s2c_packets};

use super::super::{ServerSessionHandler, is_own_home_map};
use crate::data_game_tpl::{
    pkt_task_get, resolve_task_map_id, resolve_task_npc_id, resolve_task_text,
};
use crate::runtime::{LeaderboardSlot, unix_now_ms};
use crate::state::ActivePlayerEntry;

#[inline]
pub(super) const fn home_elder_npc(gender_idx: usize) -> i16 {
    match gender_idx {
        0 => 0,
        1 => 2,
        _ => 1,
    }
}

#[inline]
pub(super) const fn power_milestone_for(task_id: i8, sub_idx: usize) -> Option<i64> {
    match (task_id, sub_idx) {
        (7, 0) => Some(16_000),
        (8, 0) => Some(40_000),
        (10, 0) => Some(200_000),
        (11, 0) => Some(500_000),
        (11, 1) => Some(550_000),
        (11, 2) => Some(600_000),
        _ => None,
    }
}

fn next_main_task_id(tpl: &GameTemplates, done_id: i8, gender_idx: usize) -> i8 {
    let g = (gender_idx.min(2)) as i8;
    let preferred = match done_id {
        3 => 4 + g,
        4..=6 => 7,
        other => other.saturating_add(1),
    };
    if tpl.tasks.iter().any(|t| t.id == i32::from(preferred)) {
        preferred
    } else {
        done_id.saturating_add(1)
    }
}

pub(super) fn is_map_visit_subtask(sub: &SubTaskTemplateRecord, gender_idx: usize) -> bool {
    if resolve_task_npc_id(sub.npc_id, gender_idx) >= 0 || sub.max_count > 1 {
        return false;
    }
    let s = sub.name.trim().to_lowercase();
    [
        "đi ",
        "hãy đi",
        "đến ",
        "hãy đến",
        "vào ",
        "di chuyển",
        "khám phá",
        "lên đường",
    ]
    .iter()
    .any(|p| s.starts_with(p))
        && !["hạ ", "đánh ", "tiêu diệt", "nhặt"]
            .iter()
            .any(|k| s.contains(k))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn mob_matches_subtask(
    tpl: &GameTemplates,
    task_id: i8,
    sub_idx: usize,
    sub: &SubTaskTemplateRecord,
    detail: &str,
    gender_idx: usize,
    cur_map: MapId,
    mob_temp_id: i16,
) -> bool {
    let g = gender_idx.min(2);
    if task_id == 1 && sub_idx == 0 {
        return mob_temp_id == 0;
    }
    if task_id == 2 && sub_idx == 0 {
        return mob_temp_id == [1, 2, 3][g];
    }
    let (sub_text, det_text) = (
        resolve_task_text(&sub.name, g).to_lowercase(),
        resolve_task_text(detail, g).to_lowercase(),
    );
    if let Some(mob_tpl) = tpl.mobs.iter().find(|m| m.id == i32::from(mob_temp_id)) {
        let mname = mob_tpl.name.trim().to_lowercase();
        if !mname.is_empty() {
            let (mother, king) = (format!("{mname} mẹ"), format!("{mname} chúa"));
            let in_sub = sub_text.contains(&mname)
                && !sub_text.contains(&mother)
                && !sub_text.contains(&king);
            let in_det = det_text.contains(&mname)
                && !det_text.contains(&mother)
                && !det_text.contains(&king);
            if in_sub || (sub_text.contains("đùi gà") && in_det) {
                return true;
            }
            if tpl.mobs.iter().any(|o| {
                !o.name.trim().is_empty() && sub_text.contains(&o.name.trim().to_lowercase())
            }) {
                return false;
            }
        }
    }
    let req_map = resolve_task_map_id(sub.map_id.0, g);
    req_map >= 0 && cur_map.0 == req_map
}

pub(super) fn item_matches_subtask(
    tpl: &GameTemplates,
    task_id: i8,
    sub_idx: usize,
    sub: &SubTaskTemplateRecord,
    detail: &str,
    gender_idx: usize,
    template_id: TemplateId,
) -> bool {
    let g = gender_idx.min(2);
    if task_id == 2 && sub_idx == 0 && matches!(template_id.0, 73 | 74) {
        return true;
    }
    if task_id == 3 && sub_idx == 2 && template_id.0 == 78 {
        return true;
    }
    let sub_text = resolve_task_text(&sub.name, g).to_lowercase();
    let det_text = resolve_task_text(detail, g).to_lowercase();
    if !sub_text.contains("nhặt") && !sub_text.contains("tìm ") {
        return false;
    }
    if let Some(it) = tpl.items.iter().find(|i| i.id == i32::from(template_id.0)) {
        let iname = it.name.trim().to_lowercase();
        if !iname.is_empty() && (sub_text.contains(&iname) || det_text.contains(&iname)) {
            return true;
        }
    }
    false
}

impl ServerSessionHandler {
    pub(super) fn check_map_visit_subtask(
        &self,
        e: &mut ActivePlayerEntry,
        map_id: MapId,
        gender_idx: usize,
        out_pkts: &mut Vec<Packet>,
    ) {
        let tid = i32::from(e.snapshot.main_task.task_id);
        let sub_idx = usize::from(e.snapshot.main_task.sub_index.max(0).cast_unsigned());
        if let Some(t_tpl) = self.shared.templates.tasks.iter().find(|t| t.id == tid)
            && let Some(sub) = t_tpl.sub_tasks.get(sub_idx)
        {
            let task11_home = tid == 11
                && sub_idx == 3
                && (sub.npc_id == 105 || sub.map_id.0 == 42)
                && is_own_home_map(map_id, e.snapshot.gender);
            if task11_home
                || (is_map_visit_subtask(sub, gender_idx)
                    && resolve_task_map_id(sub.map_id.0, gender_idx) == map_id.0)
            {
                let (sub_len, req_max) = (t_tpl.sub_tasks.len().min(127), sub.max_count.max(1));
                let delta = if task11_home { req_max } else { 1 };
                out_pkts.extend(
                    self.apply_subtask_delta(e, sub_idx, sub_len, req_max, delta, gender_idx),
                );
            }
        }
    }

    pub(super) fn advance_power_milestones(
        &self,
        e: &mut ActivePlayerEntry,
        gender_idx: usize,
        out_pkts: &mut Vec<Packet>,
    ) {
        for _ in 0..4 {
            let tid = e.snapshot.main_task.task_id;
            let sub_idx = usize::from(e.snapshot.main_task.sub_index.max(0).cast_unsigned());
            let Some(req_pwr) = power_milestone_for(tid, sub_idx) else {
                break;
            };
            if e.snapshot.stats.power < req_pwr {
                break;
            }
            let Some(t_tpl) = self
                .shared
                .templates
                .tasks
                .iter()
                .find(|t| t.id == i32::from(tid))
            else {
                break;
            };
            let Some(sub) = t_tpl.sub_tasks.get(sub_idx) else {
                break;
            };
            let (sub_len, req_max) = (t_tpl.sub_tasks.len().min(127), sub.max_count.max(1));
            out_pkts.extend(
                self.apply_subtask_delta(e, sub_idx, sub_len, req_max, req_max, gender_idx),
            );
        }
    }

    pub(super) fn apply_subtask_delta(
        &self,
        e: &mut ActivePlayerEntry,
        sub_idx: usize,
        sub_len: usize,
        req_max: i16,
        delta: i16,
        gender_idx: usize,
    ) -> Vec<Packet> {
        e.snapshot.main_task.count = e.snapshot.main_task.count.saturating_add(delta);
        e.snapshot.main_task.last_time_ms = unix_now_ms();
        if e.snapshot.main_task.count < req_max {
            return vec![s2c_packets::task_update(e.snapshot.main_task.count)];
        }
        if sub_idx + 1 < sub_len {
            e.snapshot.main_task.sub_index = (sub_idx + 1) as i8;
            e.snapshot.main_task.count = 0;
            self.shared.leaderboards.mark_dirty(LeaderboardSlot::Task);
            return vec![s2c_packets::task_next()];
        }
        let done_id = e.snapshot.main_task.task_id;
        self.complete_main_task(e, done_id, gender_idx)
    }

    pub(super) fn complete_main_task(
        &self,
        e: &mut ActivePlayerEntry,
        done_id: i8,
        gender_idx: usize,
    ) -> Vec<Packet> {
        let done_u16 = u16::from(done_id.max(0).cast_unsigned());
        let (early, _, both_smtn, gold) = TaskPlayer::main_task_reward(done_u16);
        let smtn_gain = i64::try_from(early.saturating_add(both_smtn).max(500)).unwrap_or(500);
        e.snapshot.stats.power = e.snapshot.stats.power.saturating_add(smtn_gain);
        e.snapshot.stats.tiem_nang = e.snapshot.stats.tiem_nang.saturating_add(smtn_gain);
        if gold > 0 {
            e.snapshot.wallet.gold = e
                .snapshot
                .wallet
                .gold
                .saturating_add(i64::try_from(gold).unwrap_or(0))
                .min(nro_db::player_repo::LIMIT_GOLD);
        }
        e.snapshot.main_task.task_id =
            next_main_task_id(&self.shared.templates, done_id, gender_idx);
        e.snapshot.main_task.sub_index = 0;
        e.snapshot.main_task.count = 0;
        e.snapshot.main_task.last_time_ms = unix_now_ms();
        self.shared.leaderboards.mark_dirty(LeaderboardSlot::Task);
        self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
        if let (Some(ek), Ok(tx)) = (
            e.entity_key,
            self.shared
                .world_router
                .resolve_sender(e.snapshot.location.map_id, e.snapshot.location.zone_id),
        ) {
            let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, e.snapshot.stats));
        }
        let mut pkts = vec![
            s2c_packets::player_up_exp(2, i32::try_from(smtn_gain).unwrap_or(i32::MAX)),
            s2c_packets::me_load_point(&e.snapshot.stats),
        ];
        if gold > 0 {
            pkts.push(s2c_packets::send_money(
                e.snapshot.wallet.gold.max(0).cast_unsigned(),
                u64::from(e.snapshot.wallet.gem.max(0).cast_unsigned()),
                u64::from(e.snapshot.wallet.ruby.max(0).cast_unsigned()),
            ));
        }
        pkts.push(pkt_task_get(&self.shared.templates, &e.snapshot));
        self.advance_power_milestones(e, gender_idx, &mut pkts);
        pkts
    }
}
