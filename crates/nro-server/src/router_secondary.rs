//! Secondary gameplay systems: Pets (`-107`, `-108`, `125`) and Radar (`127`).

#[path = "router_secondary_fusion.rs"]
mod router_secondary_fusion;

use std::sync::OnceLock;

use dashmap::DashMap;
use nro_core::{ClanId, Gender, PlayerId};
use nro_db::player_repo::PlayerSnapshot;
use nro_protocol::{Packet, PacketWriter, ProtocolError, RadarAction, SessionHandle};
use nro_world::{ZoneCommand, s2c_packets};

use super::{
    ServerSessionHandler,
    router_secondary_clan::{build_clan_info_pkt, clear_pending_clan_invites},
    router_secondary_clan_ops::clear_clan_join_requests,
};
use crate::runtime::unix_now_ms;

pub(self) const PET_FUSION_COOLDOWN_MS: i64 = 5_000;
pub(self) const FUSION_CLIENT_ECHO_WINDOW_MS: i64 = 2_500;

static PET_UNFUSION_COOLDOWN: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
pub(self) fn pet_unfusion_cooldown() -> &'static DashMap<PlayerId, i64> {
    PET_UNFUSION_COOLDOWN.get_or_init(DashMap::new)
}

pub(super) fn is_pet_fusion_on_cooldown(pid: PlayerId, now_ms: i64) -> bool {
    pet_unfusion_cooldown()
        .get(&pid)
        .is_some_and(|t| now_ms.saturating_sub(*t) < PET_FUSION_COOLDOWN_MS)
}

pub(super) fn record_pet_unfusion_cooldown(pid: PlayerId, now_ms: i64) {
    pet_unfusion_cooldown().insert(pid, now_ms);
}

static LAST_FUSION_ENTER_MS: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
pub(super) fn last_fusion_enter_ms() -> &'static DashMap<PlayerId, i64> {
    LAST_FUSION_ENTER_MS.get_or_init(DashMap::new)
}

pub(super) const STARTER_PET_CLAIMED_FLAG: i32 = 1 << 30;

pub(super) fn clear_clan_invites(pid: PlayerId) {
    clear_pending_clan_invites(pid);
    clear_clan_join_requests(pid);
    pet_unfusion_cooldown().remove(&pid);
    last_fusion_enter_ms().remove(&pid);
}

impl ServerSessionHandler {
    pub(crate) fn on_pet_combat_progress(
        &self,
        session: &SessionHandle,
        sid: u64,
        killed_mob: Option<i16>,
    ) {
        let mut pkts = self.collect_task_on_power_pkts(sid);
        if let Some(mob_tid) = killed_mob {
            pkts.extend(self.collect_task_on_mob_kill_pkts(sid, mob_tid));
        }
        if session.sent_key() {
            for p in pkts {
                let _ = session.try_send(p);
            }
        }
    }

    pub(super) async fn send_login_secondary(
        &self,
        session: &SessionHandle,
        snap: &PlayerSnapshot,
    ) -> Result<(), ProtocolError> {
        if let Some(pet_pkt) = self.pkt_pet_detail(snap) {
            session.send(pet_pkt).await?;
        }
        if snap.clan_id.0 >= 0
            && let Ok(clan_opt) = self.shared.clans_repo.get_by_id(snap.clan_id).await
        {
            if let Some(clan) = clan_opt
                .as_ref()
                .filter(|c| c.members.iter().any(|m| m.id == snap.id.0 as i32))
            {
                session
                    .send(build_clan_info_pkt(Some(clan), snap.id)?)
                    .await?;
            } else {
                for mut e in self.shared.clients.authenticated.iter_mut() {
                    if e.snapshot.id == snap.id {
                        e.snapshot.clan_id = ClanId(-1);
                    }
                }
                session.send(build_clan_info_pkt(None, snap.id)?).await?;
            }
        }
        Ok(())
    }

    pub(super) async fn grant_starter_pet(
        &self,
        session: &SessionHandle,
        sid: u64,
    ) -> Result<(), ProtocolError> {
        let res = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .map(|mut entry| {
                if entry.snapshot.pet.is_some() {
                    return Ok(None);
                }
                if (entry.snapshot.data_event.0[5] & STARTER_PET_CLAIMED_FLAG) != 0 {
                    return Err("Bạn đã nhận Đệ Tử khởi đầu trước đó rồi!");
                }
                entry.snapshot.data_event.0[5] |= STARTER_PET_CLAIMED_FLAG;
                let pid = entry.snapshot.id;
                let g = entry.snapshot.gender;
                entry.snapshot.pet = Some(crate::router_pet::make_fresh_pet(0, g));
                crate::router_pet::recalculate_pet_in_snapshot(&self.shared, &mut entry.snapshot);
                super::recalculate_session_stats(
                    sid,
                    &mut entry.snapshot,
                    self.shared.config.parity_mode,
                );
                Ok(Some((
                    pid,
                    s2c_packets::have_pet(true),
                    self.pkt_pet_detail(&entry.snapshot),
                )))
            });
        match res {
            Some(Ok(Some((pid, hp_pkt, det_pkt)))) => {
                session.send(hp_pkt).await?;
                if let Some(dp) = det_pkt {
                    session.send(dp).await?;
                }
                crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
                session
                    .send(s2c_packets::thong_bao("Chúc mừng bạn đã nhận được Đệ Tử!")?)
                    .await?;
            }
            Some(Err(msg)) => session.send(s2c_packets::thong_bao(msg)?).await?,
            _ => {}
        }
        Ok(())
    }

    pub(super) async fn grant_boss_normal_pet(
        &self,
        session: &SessionHandle,
        sid: u64,
    ) -> Result<(), ProtocolError> {
        let now_ms = unix_now_ms().cast_unsigned();
        let res = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .and_then(|mut entry| {
                if entry.snapshot.pet.is_some() {
                    return None;
                }
                let pid = entry.snapshot.id;
                let g = Gender::from_i8_lossy(((now_ms ^ sid) % 3) as i8);
                entry.snapshot.pet = Some(crate::router_pet::make_fresh_pet(0, g));
                crate::router_pet::recalculate_pet_in_snapshot(&self.shared, &mut entry.snapshot);
                super::recalculate_session_stats(
                    sid,
                    &mut entry.snapshot,
                    self.shared.config.parity_mode,
                );
                Some((
                    pid,
                    s2c_packets::have_pet(true),
                    self.pkt_pet_detail(&entry.snapshot),
                ))
            });
        if let Some((pid, hp_pkt, det_pkt)) = res {
            session.send(hp_pkt).await?;
            if let Some(dp) = det_pkt {
                session.send(dp).await?;
            }
            crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
            session
                .send(s2c_packets::thong_bao(
                    "Chúc mừng bạn đã thu phục được Đệ Tử mới!",
                )?)
                .await?;
        }
        Ok(())
    }

    #[must_use]
    pub(super) fn pkt_pet_detail(&self, snap: &PlayerSnapshot) -> Option<Packet> {
        crate::router_pet::build_pet_detail_pkt(&self.shared, snap)
    }

    pub(super) async fn handle_pet_info(
        &self,
        session: &SessionHandle,
        sid: u64,
    ) -> Result<(), ProtocolError> {
        if let Some(p) = self.shared.clients.authenticated.get(&sid).map(|e| {
            self.pkt_pet_detail(&e.snapshot)
                .unwrap_or_else(|| s2c_packets::have_pet(false))
        }) {
            session.send(p).await?;
        }
        Ok(())
    }

    pub(super) async fn handle_radar(
        &self,
        session: &SessionHandle,
        sid: u64,
        action: RadarAction,
    ) -> Result<(), ProtocolError> {
        match action {
            RadarAction::OpenList => {
                let owned = self
                    .shared
                    .clients
                    .authenticated
                    .get(&sid)
                    .map_or_default(|e| e.snapshot.radar_cards.clone());
                let cnt = self.shared.templates.radars.len().min(i16::MAX as usize);
                let mut w = PacketWriter::with_capacity(8 + cnt * 48);
                w.write_i8(0);
                w.write_i16(cnt as i16);
                for r in self.shared.templates.radars.iter().take(cnt) {
                    let rec = owned.iter().find(|c| c.id.0 == r.id);
                    for v in [r.id, r.icon_id] {
                        w.write_i16(v);
                    }
                    for v in [r.rank, rec.map_or(0, |c| c.amount.0), r.max, r.card_type] {
                        w.write_i8(v);
                    }
                    if r.card_type == 0 {
                        w.write_i16(r.mob_id);
                    } else {
                        for p in [r.body.head, r.body.body, r.body.leg, r.body.bag] {
                            w.write_i16(p);
                        }
                    }
                    w.write_utf(&r.name)?;
                    w.write_utf(&r.info)?;
                    w.write_i8(rec.map_or(0, |c| c.level.0));
                    w.write_i8(rec.map_or(0, |c| c.used.0));
                    w.write_i8(r.options.len().min(8) as i8);
                    for o in r.options.iter().take(8) {
                        w.write_i16(o.id as i16);
                        w.write_i32(i32::from(o.param));
                        w.write_i8(o.active_card);
                    }
                }
                session.send(w.finish(127)).await?;
            }
            RadarAction::ToggleCard(tid) => {
                let res = self
                    .shared
                    .clients
                    .authenticated
                    .get_mut(&sid)
                    .map(|mut e| {
                        if !e
                            .snapshot
                            .radar_cards
                            .iter()
                            .any(|c| c.id.0 == tid.0 && c.level.0 >= 0)
                        {
                            return None;
                        }
                        let mut new_use = 0_i8;
                        for c in &mut e.snapshot.radar_cards {
                            if c.id.0 == tid.0 {
                                c.used.0 = i8::from(c.used.0 == 0);
                                new_use = c.used.0;
                            } else {
                                c.used.0 = 0;
                            }
                        }
                        super::recalculate_session_stats(
                            sid,
                            &mut e.snapshot,
                            self.shared.config.parity_mode,
                        );
                        if let (Some(ek), Ok(tx)) = (
                            e.entity_key,
                            self.shared.world_router.resolve_sender(
                                e.snapshot.location.map_id,
                                e.snapshot.location.zone_id,
                            ),
                        ) {
                            let _ =
                                tx.try_send(ZoneCommand::UpdatePlayerStats(ek, e.snapshot.stats));
                        }
                        Some((new_use, s2c_packets::me_load_point(&e.snapshot.stats)))
                    });
                match res {
                    Some(Some((used, pt_pkt))) => {
                        let mut w = PacketWriter::with_capacity(4);
                        w.write_i8(1);
                        w.write_i16(tid.0);
                        w.write_i8(used);
                        session.send(w.finish(127)).await?;
                        session.send(pt_pkt).await?;
                    }
                    Some(None) => {
                        session
                            .send(s2c_packets::thong_bao("Bạn chưa mở khóa thẻ sưu tầm này!")?)
                            .await?;
                    }
                    None => {}
                }
            }
            RadarAction::Unknown(_) => {}
        }
        Ok(())
    }
}
