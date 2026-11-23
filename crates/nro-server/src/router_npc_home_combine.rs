//! Bà Hạt Mít / Whis item combination (`MENU_START_COMBINE`) execution.

use nro_core::{GameRng, Gender};
use nro_db::{
    StrBool, StrNum,
    json_columns::{PlayerBadgeRecord, SkillSlot},
    player_repo::PlayerSnapshot,
};
use nro_economy::{CombineContext, CombineOp, CombineService};
use nro_protocol::{Packet, ProtocolError, SessionHandle};
use nro_world::s2c_packets;

use super::super::{
    DbItemCatalog, ServerSessionHandler, npc_menu_index, pending_combine,
    recalculate_session_stats, router_npc::is_wearing_full_destroyer, snapshot_to_economy_inv,
    sync_economy_inv_to_snapshot, wallet_money_pkt,
};
use crate::runtime::unix_now_ms;

const fn special_skill_template_id(gender: Gender) -> i32 {
    match gender {
        Gender::TraiDat => 24,
        Gender::Namec => 26,
        Gender::Xayda => 25,
    }
}

fn special_skill_stats(snap: &PlayerSnapshot) -> (u8, u16) {
    let tid = special_skill_template_id(snap.gender);
    snap.skills
        .iter()
        .find(|sk| sk.template_id == tid)
        .map_or((0, 0), |sk| {
            (sk.point.max(0) as u8, sk.curr_level.max(0) as u16)
        })
}

impl ServerSessionHandler {
    pub(in crate::router) async fn execute_pending_combine(
        &self,
        session: &SessionHandle,
        sid: u64,
        npc_id: i16,
        select: u8,
    ) -> Result<(), ProtocolError> {
        let Some(pid) = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .map(|e| e.snapshot.id)
        else {
            return Ok(());
        };
        npc_menu_index().remove(&pid);
        if super::super::router_shop::is_in_trade(pid) {
            return session
                .send(s2c_packets::thong_bao(
                    "Không thể thao tác khi đang giao dịch!",
                )?)
                .await;
        }
        let Some((comb_npc, op, slots)) = pending_combine().get(&pid).map(|v| v.value().clone())
        else {
            if select != 0 {
                if let Some(mp) = self.money_pkt_for(sid) {
                    session.send(mp).await?;
                }
                return Ok(());
            }
            return session
                .send(s2c_packets::npc_open_say(
                    npc_id,
                    "Chưa chọn vật phẩm nâng cấp!",
                    None,
                )?)
                .await;
        };
        let batch_rolls: Option<u8> = match (op, select) {
            (_, 0) => None,
            (CombineOp::PhaLeHoaTrangBi, 1) => Some(10),
            (CombineOp::PhaLeHoaTrangBi, 2) => Some(100),
            _ => {
                if let Some(mp) = self.money_pkt_for(sid) {
                    session.send(mp).await?;
                }
                return Ok(());
            }
        };
        let catalog = DbItemCatalog(&self.shared.templates);
        let outcome = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .map(|mut entry| {
                if super::super::router_shop::is_in_trade(entry.snapshot.id) {
                    return Err("Không thể thao tác khi đang giao dịch!".to_owned());
                }
                let mut inv = snapshot_to_economy_inv(&entry.snapshot);
                let (skill_point, skill_mastery) = special_skill_stats(&entry.snapshot);
                let ctx = CombineContext {
                    gender: entry.snapshot.gender,
                    power: entry.snapshot.stats.power,
                    wearing_full_destroyer: is_wearing_full_destroyer(&entry.snapshot),
                    skill_point,
                    skill_mastery,
                    parity: self.shared.config.parity_mode,
                };
                let mut rng = GameRng::from_seed(unix_now_ms().cast_unsigned() ^ sid);
                let exec_res = match batch_rolls {
                    Some(rolls) => {
                        let Some(&slot) = slots.first() else {
                            return Err("Chưa chọn trang bị cần pha lê hóa!".to_owned());
                        };
                        CombineService::execute_vip_batch(
                            &mut inv, slot, rolls, &ctx, &catalog, &mut rng,
                        )
                    }
                    None => CombineService::execute(&mut inv, op, &slots, &ctx, &catalog, &mut rng),
                };
                match exec_res {
                    Ok(out) => {
                        sync_economy_inv_to_snapshot(&inv, &mut entry.snapshot);
                        let mut extra_pkts: Vec<Packet> = Vec::new();
                        if let Some(new_pt) = out.new_skill_point {
                            let tid = special_skill_template_id(entry.snapshot.gender);
                            let new_pt_i8 = new_pt.min(9) as i8;
                            let was_new = if let Some(sk) = entry
                                .snapshot
                                .skills
                                .iter_mut()
                                .find(|s| s.template_id == tid)
                            {
                                let prev = sk.point;
                                sk.point = new_pt_i8;
                                sk.curr_level = 0;
                                prev <= 0
                            } else {
                                entry.snapshot.skills.push(SkillSlot {
                                    template_id: tid,
                                    point: new_pt_i8,
                                    last_time_use_ms: 0,
                                    curr_level: 0,
                                });
                                true
                            };
                            if let Some(st) = self
                                .shared
                                .templates
                                .skills
                                .iter()
                                .find(|s| i32::from(s.id) == tid)
                                && let Some(lv) = st
                                    .levels
                                    .iter()
                                    .find(|l| l.point == new_pt_i8)
                                    .or_else(|| st.levels.first())
                            {
                                extra_pkts.push(if was_new {
                                    s2c_packets::me_add_skill(lv.id)
                                } else {
                                    s2c_packets::me_update_skill(lv.id)
                                });
                            }
                        }
                        if let Some(badge_id) = out.badge_awarded {
                            let bid = i32::from(badge_id);
                            if !entry.snapshot.badges.iter().any(|b| b.id_badges.0 == bid) {
                                entry.snapshot.badges.push(PlayerBadgeRecord {
                                    id_badges: StrNum(bid),
                                    time_of_use_badges_ms: StrNum(unix_now_ms()),
                                    is_use: StrBool(false),
                                });
                            }
                            if let Ok(p) = s2c_packets::thong_bao(&format!(
                                "Chúc mừng! Bạn vừa mở khóa danh hiệu #{bid}!"
                            )) {
                                extra_pkts.push(p);
                            }
                        }
                        recalculate_session_stats(
                            sid,
                            &mut entry.snapshot,
                            self.shared.config.parity_mode,
                        );
                        let kept: Vec<i8> = slots
                            .iter()
                            .copied()
                            .filter(|&s| {
                                entry
                                    .snapshot
                                    .items_bag
                                    .get(s)
                                    .and_then(Option::as_ref)
                                    .is_some()
                            })
                            .filter_map(|s| i8::try_from(s).ok())
                            .collect();
                        let mut pkts = vec![
                            s2c_packets::combine_result(if out.success { 2 } else { 3 }),
                            s2c_packets::inventory_bag(&entry.snapshot.items_bag),
                            wallet_money_pkt(&entry.snapshot.wallet),
                            s2c_packets::combine_items(&kept),
                        ];
                        pkts.extend(extra_pkts);
                        Ok(pkts)
                    }
                    Err(err) => Err(format!("Không thể kết hợp: {err}")),
                }
            });
        match outcome {
            Some(Ok(pkts)) => {
                for p in pkts {
                    session.send(p).await?;
                }
            }
            Some(Err(msg)) => {
                session
                    .send(s2c_packets::npc_open_say(comb_npc, &msg, None)?)
                    .await?;
            }
            None => {}
        }
        Ok(())
    }
}
