//! Main quest progression engine (`Task 0` + `Task 1..`).

#[path = "router_task_helpers.rs"]
mod router_task_helpers;
#[path = "router_task_npc.rs"]
mod router_task_npc;

use nro_core::{MapId, TemplateId};
use nro_player::progression::TaskPlayer;
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::s2c_packets;

use super::{ServerSessionHandler, is_own_home_map};
use crate::data_game_tpl::{pkt_task_get, resolve_task_npc_id};
use crate::runtime::unix_now_ms;
use router_task_helpers::{is_map_visit_subtask, item_matches_subtask, mob_matches_subtask};

impl ServerSessionHandler {
    pub(super) fn collect_task_on_power_pkts(&self, sid: u64) -> Vec<Packet> {
        let mut out_pkts = Vec::new();
        if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) {
            let g = usize::from(u8::from(e.snapshot.gender)).min(2);
            self.advance_power_milestones(&mut e, g, &mut out_pkts);
        }
        out_pkts
    }

    pub(super) fn collect_task_on_mob_kill_pkts(&self, sid: u64, mob_temp_id: i16) -> Vec<Packet> {
        let coop_info = self.shared.clients.authenticated.get(&sid).map(|e| {
            (
                e.snapshot.main_task.task_id,
                e.snapshot.clan_id,
                e.snapshot.location.map_id,
                e.snapshot.location.zone_id,
            )
        });
        let coop_delta: Option<i16> = match coop_info {
            Some((14 | 15, clan_id, map_id, zone_id)) => {
                let in_clan = clan_id.0 >= 0;
                let same_clan_in_zone = if in_clan {
                    self.shared
                        .clients
                        .authenticated
                        .iter()
                        .filter(|rec| {
                            let s = &rec.value().snapshot;
                            s.clan_id == clan_id
                                && s.location.map_id == map_id
                                && s.location.zone_id == zone_id
                        })
                        .count()
                } else {
                    0
                };
                Some(
                    i16::try_from(TaskPlayer::clan_coop_kill_credit(
                        in_clan,
                        same_clan_in_zone,
                    ))
                    .unwrap_or(0),
                )
            }
            _ => None,
        };
        let mut out_pkts = Vec::new();
        if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) {
            let task_id = e.snapshot.main_task.task_id;
            if task_id >= 1 {
                let g = usize::from(u8::from(e.snapshot.gender)).min(2);
                let sub_idx = usize::from(e.snapshot.main_task.sub_index.max(0).cast_unsigned());
                let cur_map = e.snapshot.location.map_id;
                if let Some(t_tpl) = self
                    .shared
                    .templates
                    .tasks
                    .iter()
                    .find(|t| t.id == i32::from(task_id))
                    && let Some(sub) = t_tpl.sub_tasks.get(sub_idx)
                    && resolve_task_npc_id(sub.npc_id, g) < 0
                    && !is_map_visit_subtask(sub, g)
                    && mob_matches_subtask(
                        &self.shared.templates,
                        task_id,
                        sub_idx,
                        sub,
                        &t_tpl.detail,
                        g,
                        cur_map,
                        mob_temp_id,
                    )
                {
                    let delta = if matches!(task_id, 14 | 15) {
                        coop_delta.unwrap_or(0)
                    } else {
                        1
                    };
                    if delta > 0 {
                        let sub_len = t_tpl.sub_tasks.len().min(127);
                        out_pkts = self.apply_subtask_delta(
                            &mut e,
                            sub_idx,
                            sub_len,
                            sub.max_count.max(1),
                            delta,
                            g,
                        );
                    }
                }
            }
        }
        out_pkts
    }

    pub(super) async fn advance_task_on_move(
        &self,
        session: &SessionHandle,
        sid: u64,
        map_id: MapId,
        x: i16,
    ) -> Result<(), ProtocolError> {
        let mut out_pkts: Vec<Packet> = Vec::new();
        if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) {
            if e.snapshot.main_task.task_id == 0 {
                let gender = e.snapshot.gender;
                let mt = &mut e.snapshot.main_task;
                if mt.sub_index == 0 && matches!(map_id.0, 39..=41) && (x - 350).abs() >= 12 {
                    mt.sub_index = 1;
                    mt.count = 0;
                    mt.last_time_ms = unix_now_ms();
                    out_pkts.push(s2c_packets::task_next());
                } else if mt.sub_index == 1 && is_own_home_map(map_id, gender) {
                    mt.sub_index = 2;
                    mt.count = 0;
                    mt.last_time_ms = unix_now_ms();
                    out_pkts.push(s2c_packets::task_next());
                }
            } else if e.snapshot.main_task.task_id >= 1 {
                let g = usize::from(u8::from(e.snapshot.gender)).min(2);
                self.check_map_visit_subtask(&mut e, map_id, g, &mut out_pkts);
            }
        }
        for p in out_pkts {
            session.send(p).await?;
        }
        Ok(())
    }

    pub(super) async fn advance_task_on_enter_map(
        &self,
        session: &SessionHandle,
        sid: u64,
        map_id: MapId,
    ) -> Result<(), ProtocolError> {
        let mut out_pkts: Vec<Packet> = Vec::new();
        if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) {
            let g = usize::from(u8::from(e.snapshot.gender)).min(2);
            if e.snapshot.main_task.task_id == 0 {
                if e.snapshot.main_task.sub_index <= 1 && is_own_home_map(map_id, e.snapshot.gender)
                {
                    let jumped = e.snapshot.main_task.sub_index == 0;
                    e.snapshot.main_task.sub_index = 2;
                    e.snapshot.main_task.count = 0;
                    e.snapshot.main_task.last_time_ms = unix_now_ms();
                    out_pkts.push(if jumped {
                        pkt_task_get(&self.shared.templates, &e.snapshot)
                    } else {
                        s2c_packets::task_next()
                    });
                }
            } else {
                self.check_map_visit_subtask(&mut e, map_id, g, &mut out_pkts);
            }
        }
        for p in out_pkts {
            session.send(p).await?;
        }
        Ok(())
    }

    pub(super) async fn advance_task_on_potential_up(
        &self,
        session: &SessionHandle,
        sid: u64,
    ) -> Result<(), ProtocolError> {
        let mut out_pkts: Vec<Packet> = Vec::new();
        if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) {
            let task_id = e.snapshot.main_task.task_id;
            if task_id >= 1 {
                let g = usize::from(u8::from(e.snapshot.gender)).min(2);
                let sub_idx = usize::from(e.snapshot.main_task.sub_index.max(0).cast_unsigned());
                if let Some(t_tpl) = self
                    .shared
                    .templates
                    .tasks
                    .iter()
                    .find(|t| t.id == i32::from(task_id))
                    && let Some(sub) = t_tpl.sub_tasks.get(sub_idx)
                    && ((task_id == 3 && sub_idx == 0)
                        || (resolve_task_npc_id(sub.npc_id, g) < 0
                            && sub.name.to_lowercase().contains("tiềm năng")))
                {
                    let sub_len = t_tpl.sub_tasks.len().min(127);
                    out_pkts = self.apply_subtask_delta(
                        &mut e,
                        sub_idx,
                        sub_len,
                        sub.max_count.max(1),
                        1,
                        g,
                    );
                    let cur_map = e.snapshot.location.map_id;
                    self.check_map_visit_subtask(&mut e, cur_map, g, &mut out_pkts);
                }
            }
        }
        for p in out_pkts {
            session.send(p).await?;
        }
        Ok(())
    }

    pub(super) async fn advance_task_on_power(
        &self,
        session: &SessionHandle,
        sid: u64,
    ) -> Result<(), ProtocolError> {
        for p in self.collect_task_on_power_pkts(sid) {
            session.send(p).await?;
        }
        Ok(())
    }

    pub(super) async fn advance_task_on_pick_item(
        &self,
        session: &SessionHandle,
        sid: u64,
        template_id: TemplateId,
    ) -> Result<(), ProtocolError> {
        let mut out_pkts: Vec<Packet> = Vec::new();
        if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) {
            let task_id = e.snapshot.main_task.task_id;
            if task_id >= 1 {
                let g = usize::from(u8::from(e.snapshot.gender)).min(2);
                let sub_idx = usize::from(e.snapshot.main_task.sub_index.max(0).cast_unsigned());
                if let Some(t_tpl) = self
                    .shared
                    .templates
                    .tasks
                    .iter()
                    .find(|t| t.id == i32::from(task_id))
                    && let Some(sub) = t_tpl.sub_tasks.get(sub_idx)
                    && resolve_task_npc_id(sub.npc_id, g) < 0
                    && item_matches_subtask(
                        &self.shared.templates,
                        task_id,
                        sub_idx,
                        sub,
                        &t_tpl.detail,
                        g,
                        template_id,
                    )
                {
                    let sub_len = t_tpl.sub_tasks.len().min(127);
                    out_pkts = self.apply_subtask_delta(
                        &mut e,
                        sub_idx,
                        sub_len,
                        sub.max_count.max(1),
                        1,
                        g,
                    );
                }
            }
        }
        for p in out_pkts {
            session.send(p).await?;
        }
        Ok(())
    }

    pub(super) async fn advance_task_on_harvest(
        &self,
        session: &SessionHandle,
        sid: u64,
    ) -> Result<(), ProtocolError> {
        let pkt = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut e| {
                (e.snapshot.main_task.task_id == 0 && e.snapshot.main_task.sub_index == 4).then(
                    || {
                        e.snapshot.main_task.sub_index = 5;
                        e.snapshot.main_task.count = 0;
                        e.snapshot.main_task.last_time_ms = unix_now_ms();
                        s2c_packets::task_next()
                    },
                )
            });
        if let Some(p) = pkt {
            session.send(p).await?;
        }
        Ok(())
    }

    pub(super) async fn advance_task_on_mob_kill(
        &self,
        session: &SessionHandle,
        sid: u64,
        mob_temp_id: i16,
    ) -> Result<(), ProtocolError> {
        for p in self.collect_task_on_mob_kill_pkts(sid, mob_temp_id) {
            session.send(p).await?;
        }
        Ok(())
    }
}
