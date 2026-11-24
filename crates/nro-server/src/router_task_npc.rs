//! NPC dialogue quest progression (`advance_task_on_npc`).

use nro_protocol::{ProtocolError, SessionHandle};
use nro_world::s2c_packets;

use super::super::ServerSessionHandler;
use super::router_task_helpers::{home_elder_npc, power_milestone_for};
use crate::data_game_tpl::{resolve_task_map_id, resolve_task_npc_id, resolve_task_text};
use crate::runtime::unix_now_ms;

impl ServerSessionHandler {
    pub(in crate::router) async fn advance_task_on_npc(
        &self,
        session: &SessionHandle,
        sid: u64,
        npc_id: i16,
    ) -> Result<Option<String>, ProtocolError> {
        let (mut out_pkts, mut say_reply) = (Vec::new(), None);
        if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) {
            let map_id = e.snapshot.location.map_id;
            let on_map = npc_id >= 0
                && self
                    .shared
                    .templates
                    .maps
                    .iter()
                    .find(|m| m.id == map_id)
                    .is_some_and(|m| {
                        m.npcs.iter().any(|&(tid, _, _)| {
                            i16::from(tid) == npc_id || i16::from(tid.cast_unsigned()) == npc_id
                        })
                    });
            if !on_map {
                return Ok(None);
            }
            let g = usize::from(u8::from(e.snapshot.gender)).min(2);
            let elder = home_elder_npc(g);
            if e.snapshot.main_task.task_id == 0 {
                match (e.snapshot.main_task.sub_index, npc_id) {
                    (2, id) if id == elder => {
                        e.snapshot.main_task.sub_index = 3;
                        e.snapshot.main_task.count = 0;
                        e.snapshot.main_task.last_time_ms = unix_now_ms();
                        out_pkts.push(s2c_packets::task_next());
                        say_reply = Some(
                            "Con hãy mở rương đồ trong nhà để kiểm tra hành trang nhé!".to_owned(),
                        );
                    }
                    (3, 3) => {
                        e.snapshot.main_task.sub_index = 4;
                        e.snapshot.main_task.count = 0;
                        e.snapshot.main_task.last_time_ms = unix_now_ms();
                        if e.snapshot.magic_tree.curr_peas <= 0 {
                            e.snapshot.magic_tree.curr_peas = 1;
                            out_pkts.push(super::super::router_world::build_magic_tree_load(
                                &e.snapshot,
                            )?);
                        }
                        out_pkts.push(s2c_packets::task_next());
                    }
                    (5, id) if id == elder => {
                        out_pkts = self.complete_main_task(&mut e, 0, g);
                        say_reply = Some(
                            "Tốt lắm! Bây giờ con hãy ra ngoài làng đánh ngã 5 Mộc nhân để rèn luyện sức mạnh nhé!"
                                .to_owned(),
                        );
                    }
                    _ => {}
                }
            } else {
                let tid = i32::from(e.snapshot.main_task.task_id);
                let sub_idx = usize::from(e.snapshot.main_task.sub_index.max(0).cast_unsigned());
                if let Some(t_tpl) = self.shared.templates.tasks.iter().find(|t| t.id == tid)
                    && let Some(sub) = t_tpl.sub_tasks.get(sub_idx)
                    && power_milestone_for(tid as i8, sub_idx).is_none()
                    && (!(tid == 13 && sub_idx == 0) || e.snapshot.clan_id.0 >= 0)
                {
                    let (req_npc, req_map) =
                        if tid == 11 && sub_idx == 3 && (sub.npc_id == 105 || sub.map_id.0 == 42) {
                            (elder, -1_i16)
                        } else {
                            (
                                i16::from(resolve_task_npc_id(sub.npc_id, g)),
                                resolve_task_map_id(sub.map_id.0, g),
                            )
                        };
                    if req_npc >= 0
                        && req_npc == npc_id
                        && (req_map < 0 || e.snapshot.location.map_id.0 == req_map)
                    {
                        let (req_max, sub_len) =
                            (sub.max_count.max(1), t_tpl.sub_tasks.len().min(127));
                        let note = resolve_task_text(sub.notify.trim_start_matches('#'), g);
                        say_reply = Some(if note.trim().is_empty() {
                            format!("Đã hoàn thành: {}!", resolve_task_text(&sub.name, g))
                        } else {
                            note
                        });
                        out_pkts =
                            self.apply_subtask_delta(&mut e, sub_idx, sub_len, req_max, req_max, g);
                    }
                }
            }
        }
        for p in out_pkts {
            session.send(p).await?;
        }
        Ok(say_reply)
    }
}
