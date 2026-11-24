//! World entry (`ClientOk`, `FinishLoadMap`) and map/global chat handlers.

#[path = "router_world_move.rs"]
mod router_world_move;

use std::sync::OnceLock;

use dashmap::{DashMap, DashSet, mapref::entry::Entry};
use nro_consts::const_player;
use nro_core::PlayerId;
use nro_protocol::{AdminCommand, ChatDirective, ProtocolError, SessionHandle, parse_chat_command};
use nro_world::{ZoneCommand, s2c_packets};
use tracing::info;

pub(super) use super::router_world_tree::{
    build_magic_tree_load, build_magic_tree_menu_pkt, tick_magic_tree,
};
use super::{
    ServerSessionHandler, effective_appearance, is_own_home_map, pkt_fusion, send_thong_bao,
};
use crate::data_game::DataGameAssets;
use crate::data_game_tpl;
use crate::runtime::unix_now_ms;

static CHAT_GLOBAL_COOLDOWN: OnceLock<DashMap<PlayerId, i64>> = OnceLock::new();
fn chat_global_cooldown() -> &'static DashMap<PlayerId, i64> {
    CHAT_GLOBAL_COOLDOWN.get_or_init(DashMap::new)
}
pub(super) fn clear_chat_global_cooldown(pid: PlayerId) {
    chat_global_cooldown().remove(&pid);
}

static INFLIGHT_MAP_CHANGES: OnceLock<DashSet<u64>> = OnceLock::new();
fn inflight_map_changes() -> &'static DashSet<u64> {
    INFLIGHT_MAP_CHANGES.get_or_init(DashSet::new)
}

pub(self) struct MapChangeGuard(u64);
impl MapChangeGuard {
    pub(self) fn acquire(sid: u64) -> Option<Self> {
        inflight_map_changes().insert(sid).then_some(Self(sid))
    }
}
impl Drop for MapChangeGuard {
    fn drop(&mut self) {
        inflight_map_changes().remove(&self.0);
    }
}

impl ServerSessionHandler {
    pub(super) async fn handle_client_ok(
        &self,
        session: &SessionHandle,
        sid: u64,
    ) -> Result<(), ProtocolError> {
        if !session.try_mark_client_ok_sent() {
            tracing::debug!(
                sid,
                "ignoring duplicate ClientOk on already-admitted session"
            );
            return Ok(());
        }
        self.sanitize_login_main_task(sid);
        let now_ms = unix_now_ms();
        let Some((snap, activated, ek)) =
            self.shared
                .clients
                .authenticated
                .get_mut(&sid)
                .map(|mut e| {
                    if is_own_home_map(e.snapshot.location.map_id, e.snapshot.gender) {
                        tick_magic_tree(&mut e.snapshot, now_ms);
                    }
                    (e.snapshot.clone(), e.activated, e.entity_key)
                })
        else {
            return Ok(());
        };
        info!(
            sid,
            player_id = snap.id.0,
            map_id = snap.location.map_id.0,
            "ClientOk received; sending player & mapInfo"
        );
        let (eff_head, eff_body, eff_leg, eff_bag, is_disguised) =
            effective_appearance(&self.shared.templates, &snap);
        let [p_all, p_flag, p_task, p_map] =
            data_game_tpl::pkts_client_ok(&self.shared.assets, &self.shared.templates, &snap);
        let skill_bytes: [u8; 10] = snap.skills_shortcut.map(|s| s as u8);
        let first_skill_u8 = snap
            .skills_shortcut
            .iter()
            .copied()
            .find(|&s| s >= 0)
            .or_else(|| {
                snap.skills
                    .iter()
                    .find(|sk| sk.point > 0)
                    .map(|sk| sk.template_id as i8)
            })
            .unwrap_or(0) as u8;
        session.send(p_all).await?;
        if let Some(ft) = snap.pet.as_ref().map(|p| p.fusion_type).filter(|&f| f != 0) {
            let wire_mode = match ft {
                const_player::HOP_THE_PORATA
                | const_player::HOP_THE_PORATA2
                | const_player::HOP_THE_PORATA3 => 6_i8,
                other => other,
            };
            session.send(pkt_fusion(snap.id, wire_mode)).await?;
        }
        if is_disguised {
            session
                .send(s2c_packets::update_body(
                    snap.id,
                    Some((eff_head, eff_body, eff_leg, 0)),
                ))
                .await?;
        }
        session
            .send(if eff_bag >= 0 {
                s2c_packets::send_flag_bag(snap.id, eff_bag)
            } else {
                p_flag
            })
            .await?;
        session
            .send(s2c_packets::key_skill_rms("KSkill", &skill_bytes)?)
            .await?;
        session
            .send(s2c_packets::key_skill_rms("OSkill", &skill_bytes)?)
            .await?;
        session
            .send(s2c_packets::key_skill_rms("CSkill", &[first_skill_u8])?)
            .await?;
        session
            .send(s2c_packets::me_load_point(&snap.stats))
            .await?;
        session
            .send(s2c_packets::max_stamina(
                snap.stats.max_stamina.min(i16::MAX as u16) as i16,
            ))
            .await?;
        session
            .send(s2c_packets::stamina(
                snap.stats.stamina.min(i16::MAX as u16) as i16,
            ))
            .await?;
        session.send(s2c_packets::active_point(0)).await?;
        session
            .send(DataGameAssets::pkt_update_caption(i8::from(snap.gender)))
            .await?;
        session
            .send(s2c_packets::have_pet(snap.pet.is_some()))
            .await?;
        let (intr_icon, intr_name) = if snap.intrinsic.intrinsic_id > 0
            && let Some(rec) = self
                .shared
                .templates
                .intrinsics
                .iter()
                .find(|r| r.id == i32::from(snap.intrinsic.intrinsic_id))
        {
            let name = rec
                .name
                .replacen('#', &snap.intrinsic.param1.to_string(), 1)
                .replacen('#', &snap.intrinsic.param2.to_string(), 1);
            (rec.icon as i16, name)
        } else {
            (0_i16, "Chưa mở nội tại".to_owned())
        };
        session
            .send(s2c_packets::intrinsic_info(intr_icon, &intr_name)?)
            .await?;
        session.send(s2c_packets::type_active(activated)).await?;
        self.send_login_secondary(session, &snap).await?;
        session.send(p_task).await?;
        session.send(p_map).await?;
        if is_own_home_map(snap.location.map_id, snap.gender) {
            session.send(build_magic_tree_load(&snap)?).await?;
            for egg_pkt in crate::router_pet::build_home_egg_pkts(&snap, now_ms) {
                session.send(egg_pkt).await?;
            }
        }
        if let (Some(ek), Ok(tx)) = (
            ek,
            self.shared
                .world_router
                .resolve_sender(snap.location.map_id, snap.location.zone_id),
        ) {
            let _ = tx.send(ZoneCommand::SyncZoneState(ek)).await;
        }
        crate::router_pet::sync_online_pet_map_state(&self.shared, snap.id).await;
        Ok(())
    }

    pub(super) async fn handle_finish_load_map(
        &self,
        session: &SessionHandle,
        sid: u64,
    ) -> Result<(), ProtocolError> {
        let now_ms = unix_now_ms();
        let Some((snap, ek)) = self
            .shared
            .clients
            .authenticated
            .get_mut(&sid)
            .map(|mut e| {
                if is_own_home_map(e.snapshot.location.map_id, e.snapshot.gender) {
                    tick_magic_tree(&mut e.snapshot, now_ms);
                }
                (e.snapshot.clone(), e.entity_key)
            })
        else {
            return Ok(());
        };
        session
            .send(s2c_packets::me_load_point(&snap.stats))
            .await?;
        if is_own_home_map(snap.location.map_id, snap.gender) {
            session.send(build_magic_tree_load(&snap)?).await?;
            for egg_pkt in crate::router_pet::build_home_egg_pkts(&snap, now_ms) {
                session.send(egg_pkt).await?;
            }
        }
        if let (Some(ek), Ok(tx)) = (
            ek,
            self.shared
                .world_router
                .resolve_sender(snap.location.map_id, snap.location.zone_id),
        ) {
            let _ = tx.send(ZoneCommand::SyncZoneState(ek)).await;
        }
        crate::router_pet::sync_online_pet_map_state(&self.shared, snap.id).await;
        Ok(())
    }

    pub(crate) async fn grant_starter_pet_for_effect(
        &self,
        session: &SessionHandle,
        sid: u64,
    ) -> Result<(), ProtocolError> {
        self.grant_boss_normal_pet(session, sid).await
    }

    pub(super) async fn handle_chat_map(
        &self,
        session: &SessionHandle,
        sid: u64,
        raw_text: &str,
    ) -> Result<(), ProtocolError> {
        let Some((pid, is_admin)) = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .map(|e| (e.snapshot.id, e.is_admin))
        else {
            return Ok(());
        };
        if is_admin && raw_text.trim().eq_ignore_ascii_case("pet") {
            if super::router_shop::is_in_trade(pid) {
                return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
            }
            return self.grant_boss_normal_pet(session, sid).await;
        }
        match parse_chat_command(raw_text, is_admin) {
            ChatDirective::EmptyIgnore => {}
            ChatDirective::Admin(Err(syntax_err)) => {
                send_thong_bao(session, &syntax_err.to_string())?;
            }
            ChatDirective::Admin(Ok(cmd)) => {
                if matches!(
                    cmd,
                    AdminCommand::SpawnItems { .. } | AdminCommand::TeleportMap { .. }
                ) && super::router_shop::is_in_trade(pid)
                {
                    return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
                }
                self.exec_admin_chat_cmd(session, sid, cmd).await?;
            }
            ChatDirective::ZoneChat {
                cleaned_text,
                pet_side_effect,
            } => {
                let outcome = self
                    .shared
                    .clients
                    .authenticated
                    .get_mut(&sid)
                    .map(|mut e| {
                        let loc = e.snapshot.location;
                        if let (Some(ek), Ok(tx)) = (
                            e.entity_key,
                            self.shared
                                .world_router
                                .resolve_sender(loc.map_id, loc.zone_id),
                        ) {
                            let _ = tx.try_send(ZoneCommand::ChatMap(ek, cleaned_text));
                        }
                        let applied = pet_side_effect.as_ref().and_then(|cmd| {
                            crate::router_pet::apply_pet_chat_command(
                                &self.shared,
                                &mut e.snapshot,
                                cmd,
                            )
                        });
                        let pet_visible = crate::router_pet::build_pet_companion(
                            &self.shared.templates,
                            &e.snapshot,
                        )
                        .is_some_and(|c| c.is_visible_on(loc.map_id));
                        (loc, applied, pet_visible)
                    });
                if let Some((loc, Some((reply, maybe_det)), pet_visible)) = outcome {
                    if let Some(dp) = maybe_det {
                        session.send(dp).await?;
                    }
                    crate::router_pet::sync_online_pet_map_state(&self.shared, pid).await;
                    if pet_visible
                        && let Ok(chat_pkt) =
                            s2c_packets::chat_map(PlayerId(pid.0.saturating_neg()), reply)
                    {
                        self.broadcast_zone_or_self(loc, sid, &chat_pkt);
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn handle_chat_global(
        &self,
        session: &SessionHandle,
        sid: u64,
        raw_text: &str,
    ) -> Result<(), ProtocolError> {
        let text = raw_text.trim();
        if text.is_empty() {
            return Ok(());
        }
        if text.chars().count() > 200 || text.len() > 512 {
            return send_thong_bao(session, "Tin nhắn thế giới tối đa 200 ký tự!");
        }
        let Some(pid) = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .map(|e| e.snapshot.id)
        else {
            return Ok(());
        };
        let now_ms = unix_now_ms();
        let on_cooldown = match chat_global_cooldown().entry(pid) {
            Entry::Occupied(occ) if now_ms.saturating_sub(*occ.get()) < 5_000 => true,
            Entry::Occupied(mut occ) => {
                occ.insert(now_ms);
                false
            }
            Entry::Vacant(vac) => {
                vac.insert(now_ms);
                false
            }
        };
        if on_cooldown {
            return send_thong_bao(session, "Vui lòng đợi 5 giây trước khi chat thế giới tiếp!");
        }
        if let Some(pkt) = self.shared.clients.authenticated.get(&sid).and_then(|e| {
            let (h, b, l, bg, _) = effective_appearance(&self.shared.templates, &e.snapshot);
            s2c_packets::chat_global(e.snapshot.id, &e.snapshot.name, text, (h, b, bg, l)).ok()
        }) {
            for rec in &self.shared.clients.authenticated {
                let _ = rec.value().handle.try_send(pkt.clone());
            }
        }
        Ok(())
    }
}
