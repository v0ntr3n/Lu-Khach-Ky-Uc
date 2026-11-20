//! Inbound packet router implementing [`nro_protocol::SessionEventHandler`] across
//! `DataGameAssets`, `nro-db`, `nro-world`, `nro-economy`, `nro-player`, and `nro-boss`.

#[path = "router_admin.rs"] mod router_admin;
#[path = "router_auth.rs"] mod router_auth;
#[path = "router_combat.rs"] mod router_combat;
#[path = "router_economy.rs"] mod router_economy;
#[path = "router_npc.rs"] mod router_npc;
#[path = "router_secondary.rs"] mod router_secondary;
#[path = "router_shop.rs"] mod router_shop;
#[path = "router_skill.rs"] mod router_skill;
#[path = "router_task.rs"] mod router_task;
#[path = "router_world.rs"] mod router_world;

use std::sync::{Arc, OnceLock};

use chrono::Utc;
use dashmap::DashMap;
use nro_consts::cmd_message;
use nro_core::{Gender, ItemOption, MapId, ParityMode, PlayerId, TemplateId, ZoneId};
use nro_db::{json_columns::InventoryWallet, player_repo::PlayerSnapshot, templates::GameTemplates};
use nro_economy::{CombineOp, Inventory, ItemCatalog, ItemMeta};
use nro_player::{Player, combat::StatUpgradeKind, npoint::NPoint, pet::{FusionKind, Pet, PetKind, PetStatus}, progression::RadarCard, skills::EffectSkillState};
use nro_protocol::{ClientPacket, Packet, PacketWriter, ProtocolError, SessionEventHandler, SessionHandle, SocialAction};
use nro_world::{ZoneCommand, s2c_packets};
use tracing::{debug, info, warn};

use crate::runtime::{LeaderboardSlot, SharedServerState, unix_now_ms};

static NPC_MENU_INDEX: OnceLock<DashMap<PlayerId, i32>> = OnceLock::new();
static TRADE_PARTNERS: OnceLock<DashMap<PlayerId, PlayerId>> = OnceLock::new();
static TRADE_GOLD_OFFERS: OnceLock<DashMap<(PlayerId, PlayerId), (u64, u64)>> = OnceLock::new();
static OPEN_SHOP_STATE: OnceLock<DashMap<PlayerId, String>> = OnceLock::new();
static PENDING_COMBINE: OnceLock<DashMap<PlayerId, (i16, CombineOp, Vec<usize>)>> = OnceLock::new();
static PENDING_INPUT: OnceLock<DashMap<PlayerId, &'static str>> = OnceLock::new();
static CAPSULE_DESTS: OnceLock<DashMap<PlayerId, (bool, Vec<MapId>)>> = OnceLock::new();
static CON_MEO_STATE: OnceLock<DashMap<PlayerId, i32>> = OnceLock::new();

pub(super) fn npc_menu_index() -> &'static DashMap<PlayerId, i32> { NPC_MENU_INDEX.get_or_init(DashMap::new) }
pub(super) fn trade_partners() -> &'static DashMap<PlayerId, PlayerId> { TRADE_PARTNERS.get_or_init(DashMap::new) }
pub(super) fn trade_gold_offers() -> &'static DashMap<(PlayerId, PlayerId), (u64, u64)> { TRADE_GOLD_OFFERS.get_or_init(DashMap::new) }
pub(super) fn open_shop_state() -> &'static DashMap<PlayerId, String> { OPEN_SHOP_STATE.get_or_init(DashMap::new) }
pub(super) fn pending_combine() -> &'static DashMap<PlayerId, (i16, CombineOp, Vec<usize>)> { PENDING_COMBINE.get_or_init(DashMap::new) }
pub(super) fn pending_input() -> &'static DashMap<PlayerId, &'static str> { PENDING_INPUT.get_or_init(DashMap::new) }
pub(super) fn capsule_dests() -> &'static DashMap<PlayerId, (bool, Vec<MapId>)> { CAPSULE_DESTS.get_or_init(DashMap::new) }
pub(super) fn con_meo_state() -> &'static DashMap<PlayerId, i32> { CON_MEO_STATE.get_or_init(DashMap::new) }

/// `GameTemplates`-backed [`ItemCatalog`] falling back to [`ItemMeta::classify_builtin`].
pub(super) struct DbItemCatalog<'a>(pub &'a GameTemplates);

impl ItemCatalog for DbItemCatalog<'_> {
    fn meta_of(&self, id: TemplateId) -> ItemMeta {
        let Some(rec) = self.0.items.iter().find(|r| r.id == i32::from(id.0)) else {
            return ItemMeta::classify_builtin(id);
        };
        let gender = match rec.gender { 0 => Some(Gender::TraiDat), 1 => Some(Gender::Namec), 2 => Some(Gender::Xayda), _ => None };
        let base_gold = if id.0 == 457 { 148_000_000 } else if rec.gold > 0 { u64::try_from(rec.gold).unwrap_or(4_000) } else { 4_000 };
        ItemMeta {
            template_id: id, item_type: u8::try_from(rec.item_type.max(0)).unwrap_or(27), gender,
            power_require: i64::from(rec.power_require), base_gold_value: base_gold,
            base_gem_value: u32::try_from(rec.gem.max(0)).unwrap_or(0), stackable: rec.is_up_to_up,
        }
    }
}

/// Computes effective `(head, body, leg, bag, is_disguised)` sprite parts from worn equipment.
#[must_use]
pub(super) fn effective_appearance(tpl: &GameTemplates, snap: &PlayerSnapshot) -> (i16, i16, i16, i16, bool) {
    let g = match snap.gender { Gender::TraiDat => 0_usize, Gender::Namec => 1, Gender::Xayda => 2 };
    let part_of = |slot: usize| -> Option<i16> {
        let tid = snap.items_body.get(slot)?.as_ref()?.template_id.0;
        let p = tpl.items.iter().find(|r| r.id == i32::from(tid))?.part;
        (p != -1).then_some(p as i16)
    };
    let (mut head, mut body, mut leg) = (snap.head, part_of(0).unwrap_or([57, 59, 57][g]), part_of(1).unwrap_or([58, 60, 58][g]));
    let (bag, mut is_disguised) = (part_of(8).unwrap_or(-1), false);
    if let Some(Some(disg)) = snap.items_body.get(5)
        && let Some(rec) = tpl.items.iter().find(|r| r.id == i32::from(disg.template_id.0))
        && (rec.head != -1 || rec.body != -1 || rec.leg != -1)
    {
        if rec.head != -1 { head = rec.head as i16; }
        if rec.body != -1 { body = rec.body as i16; }
        if rec.leg != -1 { leg = rec.leg as i16; }
        is_disguised = true;
    }
    (head, body, leg, bag, is_disguised)
}

/// Stateless [`SessionEventHandler`] bridging [`SessionHandle`] I/O tasks to authoritative subsystems.
pub struct ServerSessionHandler { shared: Arc<SharedServerState> }

impl ServerSessionHandler {
    #[must_use]
    pub const fn new(shared: Arc<SharedServerState>) -> Self { Self { shared } }

    async fn dispatch_in_world(&self, session: &SessionHandle, packet: ClientPacket) -> Result<(), ProtocolError> {
        let sid = session.id();
        if matches!(
            &packet,
            ClientPacket::ClientInput(_) | ClientPacket::CombinePreview(_, _) | ClientPacket::MapTransport(_) | ClientPacket::TransportComplete
                | ClientPacket::NpcMenuOpen(_) | ClientPacket::NpcMenuConfirm(_, _) | ClientPacket::DauThanConfirm(_, _, _)
                | ClientPacket::LuckyRoundOpen(_, _) | ClientPacket::AchievementConfirm(_) | ClientPacket::MagicTree(_)
                | ClientPacket::PickMapItem(_) | ClientPacket::ReviveMeLive | ClientPacket::BoxSort | ClientPacket::ClanCreateInfo(_)
                | ClientPacket::ZoneChange(_, _) | ClientPacket::MapWaypointChange(_) | ClientPacket::GotoPlayer(_) | ClientPacket::ReturnHomeMeBack
                | ClientPacket::EnemyList(SocialAction::PrimaryTarget(_)) | ClientPacket::TaskGet { .. } | ClientPacket::PetChangeStatus(_) | ClientPacket::Fusion(_)
        ) && self.shared.clients.authenticated.get(&sid).is_some_and(|e| router_shop::is_in_trade(e.snapshot.id)) {
            return send_thong_bao(session, "Không thể thao tác khi đang giao dịch!");
        }
        match packet {
            ClientPacket::ClientOk => self.handle_client_ok(session, sid).await?,
            ClientPacket::MagicTree(action) => self.handle_magic_tree(session, sid, action).await?,
            ClientPacket::FinishLoadMap => self.handle_finish_load_map(session, sid).await?,
            ClientPacket::LoadRms(key) => {
                if let Some(p) = self.shared.clients.authenticated.get(&sid).and_then(|e| {
                    let (sc, cs) = (e.snapshot.skills_shortcut.map(|s| s as u8), [e.selected_skill as u8]);
                    let bytes: &[u8] = match key.as_str() { "KSkill" | "OSkill" => &sc, "CSkill" => &cs, _ => &[] };
                    s2c_packets::key_skill_rms(&key, bytes).ok()
                }) { session.send(p).await?; }
            }
            ClientPacket::PlayerMove(_t, x, maybe_y) => {
                router_skill::clear_ttnl(session.player_id());
                if let Some(loc) = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
                    e.snapshot.location.x = x;
                    if let Some(y) = maybe_y { e.snapshot.location.y = y; }
                    let loc = e.snapshot.location;
                    if let (Some(ek), Ok(tx)) = (e.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                        let _ = tx.try_send(ZoneCommand::MovePlayer(ek, loc));
                    }
                    loc
                }) { self.advance_task_on_move(session, sid, loc.map_id, x).await?; }
            }
            ClientPacket::OpenZoneUi => self.handle_open_zone_ui(session, sid).await?,
            ClientPacket::ZoneChange(_raw, opt_zone) => {
                if let Some(new_zone) = opt_zone {
                    self.switch_player_zone(session, sid, None, new_zone, None).await?;
                } else if let Some(loc) = self.shared.clients.authenticated.get(&sid).map(|e| e.snapshot.location) {
                    let max_z = self.shared.templates.maps.iter().find(|m| m.id == loc.map_id).map_or(1, |m| u16::from(m.zones.max(1)));
                    if max_z > 1 { self.switch_player_zone(session, sid, None, ZoneId((loc.zone_id.0 + 1) % max_z), None).await?; }
                    else { session.send(s2c_packets::reset_point(loc)).await?; send_thong_bao(session, "Không thể đổi khu vực tại bản đồ này!")?; }
                }
            }
            ClientPacket::MapWaypointChange(is_off) => self.handle_waypoint_change(session, sid, is_off).await?,
            ClientPacket::AttackMobs(mobs, _) => self.handle_attack_mobs(session, sid, &mobs).await?,
            ClientPacket::AttackPlayers(targets) => self.handle_attack_players(sid, &targets),
            ClientPacket::AttackMulti(mobs, players) => self.handle_attack_multi(session, sid, &mobs, &players).await?,
            ClientPacket::SkillNotFocus(status, special) => self.handle_skill_not_focus(session, sid, status, special).await?,
            ClientPacket::PotentialUp(stat_idx, points) => {
                if let Some((pt_pkt, err_pkt, upgraded)) = self.shared.clients.authenticated.get_mut(&sid).map(|mut entry| {
                    let (mut err_pkt, mut upgraded) = (None, false);
                    if let Some(kind) = u8::try_from(stat_idx).ok().and_then(StatUpgradeKind::from_repr)
                        && let Ok(pts) = u16::try_from(points)
                    {
                        let mut np = NPoint::new(entry.snapshot.stats);
                        if np.increase_point(kind, pts, self.shared.config.parity_mode).is_ok() {
                            entry.snapshot.stats = np.core;
                            recalculate_session_stats(sid, &mut entry.snapshot, self.shared.config.parity_mode);
                            self.shared.leaderboards.mark_dirty(LeaderboardSlot::Power);
                            upgraded = true;
                        } else { err_pkt = s2c_packets::thong_bao("Không đủ tiềm năng hoặc chưa mở giới hạn sức mạnh!").ok(); }
                    } else { err_pkt = s2c_packets::thong_bao("Điểm nâng không hợp lệ!").ok(); }
                    let loc = entry.snapshot.location;
                    if let (Some(ek), Ok(tx)) = (entry.entity_key, self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id)) {
                        let _ = tx.try_send(ZoneCommand::UpdatePlayerStats(ek, entry.snapshot.stats));
                    }
                    (s2c_packets::me_load_point(&entry.snapshot.stats), err_pkt, upgraded)
                }) {
                    session.send(pt_pkt).await?;
                    if let Some(ep) = err_pkt { session.send(ep).await?; }
                    if upgraded { self.advance_task_on_potential_up(session, sid).await?; }
                }
            }
            ClientPacket::SkillUp { template_id, point } => self.handle_skill_up(session, sid, template_id, point).await?,
            ClientPacket::SkillSelect(tid) => {
                if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid)
                    && e.snapshot.skills.iter().any(|sk| sk.template_id == i32::from(tid.0) && sk.point > 0) { e.selected_skill = tid.0; }
            }
            ClientPacket::ChangeOnSkill(sc) => { if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) { e.snapshot.skills_shortcut = sc; } }
            ClientPacket::BoxSort => if let Some(p) = self.shared.clients.authenticated.get_mut(&sid).map(|mut e| {
                let cap = e.snapshot.items_box.len();
                let mut packed: Vec<_> = e.snapshot.items_box.drain(..).flatten().map(Some).collect();
                packed.resize(cap, None); e.snapshot.items_box = packed;
                s2c_packets::inventory_box(&e.snapshot.items_box)
            }) { session.send(p).await?; },
            ClientPacket::ChangeName { name, .. } => {
                let trimmed = name.trim();
                if (3..=12).contains(&trimmed.chars().count()) {
                    if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) { trimmed.clone_into(&mut e.snapshot.name); }
                    self.shared.leaderboards.mark_all_dirty();
                    send_thong_bao(session, "Đổi tên nhân vật thành công!")?;
                } else { send_thong_bao(session, "Tên nhân vật phải từ 3 đến 12 ký tự!")?; }
            }
            ClientPacket::PickMapItem(id) => self.handle_pick_map_item(session, sid, id).await?,
            ClientPacket::ReturnHomeMeBack => self.handle_revive(session, sid, true).await?,
            ClientPacket::ReviveMeLive => self.handle_revive(session, sid, false).await?,
            ClientPacket::ChatMap(raw) => self.handle_chat_map(session, sid, &raw).await?,
            ClientPacket::ChatGlobal(text) => if let Some(pkt) = self.shared.clients.authenticated.get(&sid).and_then(|e| {
                let (h, b, l, bg, _) = effective_appearance(&self.shared.templates, &e.snapshot);
                s2c_packets::chat_global(e.snapshot.id, &e.snapshot.name, &text, (h, b, bg, l)).ok()
            }) { for rec in &self.shared.clients.authenticated { let _ = rec.value().handle.try_send(pkt.clone()); } },
            ClientPacket::ChatPrivate(target, text) => self.handle_whisper(session, sid, target, &text).await?,
            ClientPacket::NpcMenuOpen(npc_id) => self.handle_npc_menu(session, sid, npc_id, None).await?,
            ClientPacket::NpcMenuConfirm(npc_id, sel) => self.handle_npc_menu(session, sid, npc_id, Some(sel.max(0).cast_unsigned())).await?,
            ClientPacket::DauThanConfirm(npc_id, menu_id, _) => {
                if npc_id == 4 { self.handle_magic_tree_confirm(session, sid, menu_id).await?; }
                else { self.handle_npc_menu(session, sid, i16::from(npc_id), Some(menu_id.max(0).cast_unsigned())).await?; }
            }
            ClientPacket::TaskGet { npc_id, .. } => if let Some(say) = self.advance_task_on_npc(session, sid, i16::from(npc_id)).await? {
                session.send(s2c_packets::npc_open_say(i16::from(npc_id), &say, None)?).await?;
            },
            ClientPacket::ShopBuy(bt, id, qty) => self.handle_shop_buy(session, sid, bt, id, qty).await?,
            ClientPacket::ShopSell(act, wh, idx) => self.handle_shop_sell(session, sid, act, wh, idx).await?,
            ClientPacket::ConsignShop(act) => self.handle_consign_action(session, sid, act).await?,
            ClientPacket::ClientInput(fields) => self.handle_client_input(session, sid, &fields).await?,
            ClientPacket::GetItemMove(mt, idx) => self.handle_get_item_move(session, sid, mt, idx).await?,
            ClientPacket::UseItem(at, wh, idx, tid) => self.handle_use_item(session, sid, at, wh, idx, tid).await?,
            ClientPacket::MapTransport(idx) => self.handle_map_transport(session, sid, idx).await?,
            ClientPacket::TransportComplete => self.handle_transport_complete(session, sid).await?,
            ClientPacket::GotoPlayer(pid) => self.handle_goto_player(session, sid, pid).await?,
            ClientPacket::CombinePreview(action, bag_indices) => match action {
                1 => { session.send(s2c_packets::combine_items(&bag_indices)).await?; self.handle_combine_preview(session, sid, &bag_indices).await?; }
                2 | 3 | 8 => session.send(s2c_packets::combine_result(action)).await?,
                4 | 5 | 7 => session.send(s2c_packets::combine_result_with_icon(action, 0)).await?,
                _ => {}
            },
            ClientPacket::Trade(action) => self.handle_trade(session, sid, action).await?,
            ClientPacket::ChangeFlag(act) => self.handle_change_flag(session, sid, act).await?,
            ClientPacket::PetShowInfo => self.handle_pet_info(session, sid).await?,
            ClientPacket::PetChangeStatus(st) => self.handle_pet_status(session, sid, st, false).await?,
            ClientPacket::Fusion(ft) => self.handle_pet_status(session, sid, ft, true).await?,
            ClientPacket::IntrinsicMenu(sub) => self.handle_intrinsic_open(session, sid, sub).await?,
            ClientPacket::PlayerMenu(pid) => self.handle_player_menu(session, sid, pid, false).await?,
            ClientPacket::GetPlayerMenu(pid) => self.handle_player_menu(session, sid, pid, true).await?,
            ClientPacket::PlayerMenuAction(pid, mid) => self.handle_player_menu_action(session, sid, pid, mid).await?,
            ClientPacket::PvpChallenge(act, pty, pid) => self.handle_pvp_challenge(session, sid, act, pty, pid).await?,
            ClientPacket::FriendList(act) => self.handle_social(session, sid, act, true).await?,
            ClientPacket::EnemyList(act) => self.handle_social(session, sid, act, false).await?,
            ClientPacket::ClanCreateInfo(act) => self.handle_clan_create_info(session, sid, &act).await?,
            ClientPacket::ClanSearch(q) => self.handle_clan_search(session, sid, &q).await?,
            ClientPacket::ClanMemberList(cid) => self.handle_clan_member_list(session, sid, cid).await?,
            ClientPacket::ClanMessage(act) => self.handle_clan_message(session, sid, &act).await?,
            ClientPacket::ClanInfo(cid) => self.handle_clan_info_req(session, sid, cid).await?,
            ClientPacket::ClanLeaveMenu => self.handle_clan_leave_menu(session, sid).await?,
            ClientPacket::ClanInvite(act) => self.handle_clan_invite(session, sid, act).await?,
            ClientPacket::ClanRemote(pid, role) => self.handle_clan_remote(session, sid, pid, role).await?,
            ClientPacket::ClanDonate(msg_id) => self.handle_clan_donate(session, sid, msg_id).await?,
            ClientPacket::ClanJoinDecision(msg_id, decision) => self.handle_clan_join_decision(session, sid, msg_id, decision).await?,
            ClientPacket::SuperRankOrAdminBoss(_) => {
                session.send(s2c_packets::thong_bao("Tính năng đang được phát triển!")?).await?;
            }
            ClientPacket::Radar(act) => self.handle_radar(session, sid, act).await?,
            ClientPacket::TopQuery(name, page) => self.handle_top_query(session, sid, &name, page).await?,
            ClientPacket::LuckyRoundOpen(ty, cnt) => self.handle_lucky_round(session, sid, ty, cnt).await?,
            ClientPacket::AchievementConfirm(idx) => self.handle_achievement(session, sid, Some(idx)).await?,
            ClientPacket::LockInventory(pin) => {
                if let Some(mut e) = self.shared.clients.authenticated.get_mut(&sid) { e.snapshot.mbv_pin = (pin, e.snapshot.mbv_pin.0 != pin, unix_now_ms()); }
                send_thong_bao(session, "Đã cập nhật khóa bảo vệ hành trang!")?;
            }
            _ => debug!(sid, op = <&'static str>::from(&packet), "processed auxiliary world packet"),
        }
        Ok(())
    }
}

impl SessionEventHandler for ServerSessionHandler {
    async fn on_packet(&self, session: &SessionHandle, packet: ClientPacket) -> Result<(), ProtocolError> {
        if self.shared.assets.try_dispatch_packet(session, &packet, &self.shared.templates, &self.shared.config.server_name, &self.shared.config.advertised_host(), self.shared.config.port).await? { return Ok(()); }
        match packet {
            ClientPacket::CheckControllerPing => session.send(Packet::empty(-120)).await,
            ClientPacket::CheckMapPing => session.send(Packet::empty(-121)).await,
            ClientPacket::Login { username, password, version: _, login_type } => self.handle_login(session, &username, &password, login_type).await,
            ClientPacket::Register { username, password, guest_user } => self.handle_register(session, &username, &password, guest_user.as_deref()).await,
            ClientPacket::CreateChar(name, gender, hair) => if session.player_id().0 != 0 { send_dialog(session, "Tài khoản đã có nhân vật!") } else { self.handle_create_char(session, &name, gender, hair).await },
            ClientPacket::Login2GuestNotice => self.handle_guest_login(session).await,
            ClientPacket::UserInfoDisabled => send_dialog(session, "Vui lòng đăng ký tài khoản chính thức tại trang chủ!"),
            ClientPacket::LoadRms(key) if session.player_id().0 == 0 => session.send(s2c_packets::key_skill_rms(&key, &[])?).await,
            ClientPacket::ClientOk if session.account_id().0 > 0 && session.player_id().0 == 0 => {
                info!(sid = session.id(), account_id = session.account_id().0, "ClientOk before character creation; sending CREATE_PLAYER");
                session.send(Packet::empty(cmd_message::CREATE_PLAYER)).await
            }
            other if session.player_id().0 > 0 => self.dispatch_in_world(session, other).await,
            _ => Ok(()),
        }
    }

    async fn on_disconnect(&self, session: &SessionHandle) {
        let sid = session.id();
        router_economy::clear_session_buffs(sid);
        self.shared.rate_limiter.release(session.ip());
        let detached = {
            let guard = router_shop::trade_lifecycle_lock().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let active_info = self.shared.clients.authenticated.get(&sid).map(|e| (e.snapshot.id, e.snapshot.location));
            let still_active = active_info.is_some_and(|(pid, _)| self.shared.clients.players.get(&pid).is_some_and(|s| *s == sid));
            if let Some((pid, _)) = active_info {
                if still_active { self.cleanup_trade_on_disconnect_unlocked(pid); }
                self.shared.clients.players.remove_if(&pid, |_, s| *s == sid);
            }
            let removed = self.shared.clients.authenticated.remove(&sid).map(|(_, entry)| (entry, still_active));
            drop(guard);
            removed
        };
        if let Some((entry, still_active)) = detached {
            let (pid, loc) = (entry.snapshot.id, entry.snapshot.location);
            if still_active {
                npc_menu_index().remove(&pid); open_shop_state().remove(&pid); pending_combine().remove(&pid);
                pending_input().remove(&pid); capsule_dests().remove(&pid); con_meo_state().remove(&pid);
                self.cleanup_combat_on_disconnect(pid, loc, sid);
                router_skill::clear_skill_state(pid); router_secondary::clear_clan_invites(pid);
                if let Some(acc) = entry.snapshot.account_id { let _ = self.shared.accounts_repo.update_logout_time(acc, Utc::now()).await; }
                if let Ok(zone_tx) = self.shared.world_router.resolve_sender(loc.map_id, loc.zone_id) {
                    let _ = zone_tx.send(ZoneCommand::LeavePlayer { player_id: pid }).await;
                }
                if let Err(err) = self.shared.players_repo.save_player(&entry.snapshot, None, self.shared.config.parity_mode, unix_now_ms()).await {
                    warn!(player_id = pid.0, error = %err, "failed persisting player on disconnect");
                }
            }
            if let Some(acc) = entry.snapshot.account_id { self.shared.clients.accounts.remove_if(&acc, |_, s| *s == sid); }
            else { self.shared.clients.accounts.retain(|_, s| *s != sid); }
        } else {
            self.shared.clients.accounts.retain(|_, s| *s != sid);
        }
        self.shared.clients.sessions.remove(&sid);
    }
}

fn recalculate_snapshot_stats_unclamped(snap: &mut PlayerSnapshot, parity: ParityMode) {
    let mut player = Player::new(snap.id, snap.account_id.unwrap_or_default(), snap.gender, snap.stats, snap.location);
    player.inventory.items_body.clear();
    player.inventory.items_body.extend(snap.items_body.iter().cloned());
    player.inventory.items_bag.clone_from(&snap.items_bag);
    player.inventory.items_box.clone_from(&snap.items_box);
    player.inventory.gold = snap.wallet.gold.max(0).cast_unsigned();
    player.inventory.gem = snap.wallet.gem.max(0).cast_unsigned();
    player.inventory.ruby = snap.wallet.ruby.max(0).cast_unsigned();
    for rc in &snap.radar_cards {
        player.radar_cards.push(RadarCard {
            id: rc.id.0.max(0).cast_unsigned(), level: rc.level.0,
            progress: (rc.amount.0.max(0).cast_unsigned(), rc.max.0.max(0).cast_unsigned()),
            used: rc.used.0 != 0, aura_id: -1,
            options: rc.option.iter().map(|o| (ItemOption { id: o.id.0.max(0) as u16, param: o.param.0 }, o.active.0)).collect(),
        });
    }
    if let Some(ps) = &snap.pet && ps.fusion_type != 0 {
        player.fusion.kind = FusionKind::from_repr(u8::try_from(ps.fusion_type).unwrap_or(0)).unwrap_or(FusionKind::LuongLongNhatThe);
        player.pet = Some(Pet {
            id: PlayerId(snap.id.0.saturating_neg()),
            kind: PetKind::from_repr(u8::try_from(ps.type_pet).unwrap_or(0)).unwrap_or_default(),
            gender: ps.gender, status: PetStatus::Fusion, is_transform: false, location: snap.location,
            npoint: NPoint::new(ps.stats), items_body: ps.items_body.iter().cloned().collect(),
            skills: Default::default(), effects: EffectSkillState::default(), timers_ms: (0, 0, 0), home_dir: -1,
        });
    }
    player.recalculate_stats(false, false, unix_now_ms().cast_unsigned(), parity);
    snap.stats.hp_max = i64::from(player.npoint.hp_max); snap.stats.mp_max = i64::from(player.npoint.mp_max);
    snap.stats.dame = i64::from(player.npoint.dame); snap.stats.def = player.npoint.def;
    snap.stats.crit = u8::try_from(player.npoint.crit.clamp(0, i32::from(u8::MAX))).unwrap_or(0); snap.stats.speed = player.npoint.speed;
}

pub(super) fn recalculate_snapshot_stats(snap: &mut PlayerSnapshot, parity: ParityMode) {
    recalculate_snapshot_stats_unclamped(snap, parity);
    snap.stats.hp = snap.stats.hp.clamp(0, snap.stats.hp_max); snap.stats.mp = snap.stats.mp.clamp(0, snap.stats.mp_max);
}

pub(super) fn recalculate_session_stats(sid: u64, snap: &mut PlayerSnapshot, parity: ParityMode) {
    recalculate_snapshot_stats_unclamped(snap, parity);
    router_economy::apply_session_buffs_before_clamp(sid, &mut snap.stats);
    snap.stats.hp = snap.stats.hp.clamp(0, snap.stats.hp_max); snap.stats.mp = snap.stats.mp.clamp(0, snap.stats.mp_max);
}

pub(super) fn send_dialog(session: &SessionHandle, text: &str) -> Result<(), ProtocolError> {
    let mut pw = PacketWriter::with_capacity(text.len() + 4);
    pw.write_utf(text)?;
    let _ = session.try_send(pw.finish(cmd_message::DIALOG_MESSAGE));
    Ok(())
}

pub(super) fn send_thong_bao(session: &SessionHandle, text: &str) -> Result<(), ProtocolError> {
    let pkt = s2c_packets::thong_bao(text)?;
    let _ = session.try_send(pkt);
    Ok(())
}

pub(super) fn wallet_money_pkt(w: &InventoryWallet) -> Packet {
    s2c_packets::send_money(w.gold.max(0).cast_unsigned(), u64::from(w.gem.max(0).cast_unsigned()), u64::from(w.ruby.max(0).cast_unsigned()))
}

pub(super) fn snapshot_to_economy_inv(snap: &PlayerSnapshot) -> Inventory {
    Inventory {
        gold: snap.wallet.gold.max(0).cast_unsigned(), gem: snap.wallet.gem.max(0).cast_unsigned(),
        ruby: snap.wallet.ruby.max(0).cast_unsigned(), coupon: snap.wallet.coupon.max(0).cast_unsigned(),
        body: snap.items_body.clone(), bag: snap.items_bag.clone(),
        box_items: snap.items_box.clone(), lucky_round_box: snap.items_box_lucky_round.clone(),
    }
}

pub(super) fn sync_economy_inv_to_snapshot(inv: &Inventory, snap: &mut PlayerSnapshot) {
    snap.wallet.gold = i64::try_from(inv.gold).unwrap_or(i64::MAX); snap.wallet.gem = i32::try_from(inv.gem).unwrap_or(i32::MAX);
    snap.wallet.ruby = i32::try_from(inv.ruby).unwrap_or(i32::MAX); snap.wallet.coupon = i32::try_from(inv.coupon).unwrap_or(i32::MAX);
    snap.items_body.clone_from(&inv.body); snap.items_bag.clone_from(&inv.bag);
    snap.items_box.clone_from(&inv.box_items); snap.items_box_lucky_round.clone_from(&inv.lucky_round_box);
}
