//! Exhaustive Client-to-Server (C2S) opcode AST and `ClientPacket::decode` wire parser
//! covering `Controller.java`, `messageNotLogin` (`-29`), `messageNotMap` (`-28`),
//! `messageSubCommand` (`-30`), `MenuController.java` (`32`, `33`), RTT ping probes
//! (`-120`, `-121`), multi-target AoE combat (`-4`, `67`), and v2.5.0 Unity sub-opcodes.

use nro_core::{MapId, PlayerId, TemplateId, ZoneId};

pub use crate::{
    ClanInfoAction, ClanInviteAction, ClanMessageAction, ClientTypeInfo, ConsignAction, FlagAction,
    MenuNpcTarget, RadarAction, SkillSpecialArgs, SocialAction, TradeAction, npc_id,
    resolve_menu_confirm_target, resolve_menu_open_target,
};

#[path = "c2s_decode.rs"]
mod c2s_decode;

#[cfg(test)]
#[path = "c2s_tests.rs"]
mod tests;

/// Exhaustive Client-to-Server decoded packet covering 100% of `Controller.java` and Unity client extensions.
#[derive(Debug, Clone, PartialEq, Eq, strum::IntoStaticStr)]
pub enum ClientPacket {
    /// `-27`: Session XOR key handshake (`GET_SESSION_ID`).
    GetSessionId,
    /// `-29, sub 0`: Account login (`username`, `password`, optional `version`, optional `login_type`).
    Login {
        /// Account username (`UTF`).
        username: String,
        /// Account password (`UTF`).
        password: String,
        /// Client version string (`UTF`, defaults to `""` when omitted).
        version: String,
        /// Login type (`byte`, defaults to `0` when omitted).
        login_type: i8,
    },
    /// `-29, sub 1`: Account registration.
    Register {
        /// Requested username (`UTF`).
        username: String,
        /// Requested password (`UTF`).
        password: String,
        /// Optional guest username being upgraded (`UTF`).
        guest_user: Option<String>,
    },
    /// `-29, sub 2`: Client device/version registration (`CLIENT_INFO`).
    SetClientType(ClientTypeInfo),
    /// `-29, other`: Unhandled pre-login sub-opcode.
    NotLoginUnknown(i8),
    /// `-28, sub 2`: Character creation `(name, gender, hair)`.
    CreateChar(String, i8, i8),
    /// `-28, sub 6`: Request map/NPC/mob templates (`UPDATE_MAP`).
    UpdateMap,
    /// `-28, sub 7`: Request class/skill templates (`UPDATE_SKILL`).
    UpdateSkill,
    /// `-28, sub 8`: Request 4-part item templates (`UPDATE_ITEM`).
    UpdateItem,
    /// `-28, sub 10`: Request tile map collision data (`u8` widened to `MapId`).
    RequestMapTemplate(MapId),
    /// `-28, sub 13`: Client ready to enter map (`CLIENT_OK`).
    ClientOk,
    /// `-28, sub 18`: Rename target character (`player_id`, `name`).
    ChangeName {
        /// Target player ID (`int`).
        player_id: i32,
        /// Requested new name (`UTF`).
        name: String,
    },
    /// `-28, other`: Unhandled not-map sub-opcode.
    NotMapUnknown(i8),
    /// `-30, sub 16`: Spend potential points `(stat_type, points)`.
    PotentialUp(i8, i16),
    /// `-30, sub 17`: Upgrade skill level (`template_id`, `point`).
    SkillUp {
        /// Skill template ID (`short`).
        template_id: i16,
        /// Skill points to spend (`byte`).
        point: i8,
    },
    /// `-30, sub 19`: Sort storage box (`BOX_SORT`).
    BoxSort,
    /// `-30, sub 61`: Request server-stored RMS blob (`key`).
    LoadRms(String),
    /// `-30, sub 63`: Request interactive context menu for `PlayerId`.
    GetPlayerMenu(PlayerId),
    /// `-30, sub 64`: Execute player context submenu `(target_id, menu_id)`.
    PlayerMenuAction(PlayerId, i16),
    /// `-30, other`: Unhandled sub-command opcode.
    SubCommandUnknown(i8),
    /// `-127`: Lucky Round crack-ball `(unused_type, count)`.
    LuckyRoundOpen(Option<i8>, Option<i8>),
    /// `-125`: Dynamic client input form fields (`CLIENT_INPUT`).
    ClientInput(Vec<String>),
    /// `-121`: Map latency RTT ping probe (`SEND_CHECK_MAP`).
    CheckMapPing,
    /// `-120`: Controller latency RTT ping probe (`SEND_CHECK_CONTROLLER`).
    CheckControllerPing,
    /// `-118`: Super Rank duel or admin boss teleport (`target_id`).
    SuperRankOrAdminBoss(i32),
    /// `-114`: Request Senzu bean no-op (`REQUEST_PEAN`).
    RequestPeanNoop,
    /// `-113`: 10-slot skill shortcut bar (`-1` padded on short payloads).
    ChangeOnSkill([i8; 10]),
    /// `-111`: Legacy image source version stub (`GET_IMAGE_SOURCE2`).
    GetImageSource2,
    /// `-108`: Pet stance change (`0..=5`, `PET_STATUS`).
    PetChangeStatus(i8),
    /// `-107`: Open pet info sheet (`PET_INFO`).
    PetShowInfo,
    /// `-105`: Finish timed spaceship/tunnel travel (`TRANSPORT`).
    TransportComplete,
    /// `-104`: Account security PIN submission (`LOCK_INVENTORY`).
    LockInventory(i32),
    /// `-103`: PK flag UI, selection, or icon query (`CHANGE_FLAG`).
    ChangeFlag(FlagAction),
    /// `-101`: Guest login redirect notice (`LOGIN2`).
    Login2GuestNotice,
    /// `-100`: Consignment shop action (`KIGUI`).
    ConsignShop(ConsignAction),
    /// `-99`: Enemy list action (`ENEMY_LIST`).
    EnemyList(SocialAction),
    /// `-96`: Leaderboard query `(top_name, page)`.
    TopQuery(String, i8),
    /// `-91`: Capsule or Black Ball map transport index (`MAP_TRASPORT`).
    MapTransport(Option<i8>),
    /// `-87`: Request `DataGame.updateData` archive (`UPDATE_DATA`).
    UpdateData,
    /// `-86`: Player-to-player trade (`GIAO_DICH`).
    Trade(TradeAction),
    /// `-81`: Item combine preview `(action, bag_indices)` (`bag_indices` populated only when `action == 1`).
    CombinePreview(i8, Vec<i8>),
    /// `-80`: Friend list action (`FRIEND`).
    FriendList(SocialAction),
    /// `-79`: Inspect target player context menu (`PLAYER_MENU`).
    PlayerMenu(PlayerId),
    /// `-78`: Movement heartbeat (`client_seconds`, `CHECK_MOVE`).
    CheckMove(i32),
    /// `-76`: Claim achievement reward at `index` (`ARCHIVEMENT`).
    AchievementConfirm(i8),
    /// `-74`: Resource bundle query (`1` = count, `2` = stream files).
    GetImageSource(i8),
    /// `-72`: Private whisper `(target_id, text)`.
    ChatPrivate(PlayerId, String),
    /// `-71`: Global world chat message (`CHAT_THEGIOI_CLIENT`).
    ChatGlobal(String),
    /// `-67`: Request PNG icon (`icon_id`, `REQUEST_ICON`).
    RequestIcon(i32),
    /// `-66`: Request effect data & sprite (`eff_id`, `GET_EFFDATA`).
    GetEffData(TemplateId),
    /// `-63`: Request clan flag bag effect (`fbid`, `short`).
    GetBagEffect(i16),
    /// `-62`: Request clan emblem icon (`flag_id` unsigned `0..=255`, `CLAN_IMAGE`).
    ClanImageChoose(u8),
    /// `-60`: Attack one or more target players (`PLAYER_ATTACK_PLAYER`).
    AttackPlayers(Vec<PlayerId>),
    /// `-59`: Gold duel challenge `(action, pvp_type, target_id)`.
    PvpChallenge(i8, i8, PlayerId),
    /// `-57`: Clan invite send/accept/decline (`CLAN_INVITE`).
    ClanInvite(ClanInviteAction),
    /// `-56`: Clan member promote/demote/kick `(member_id, role)`.
    ClanRemote(PlayerId, i8),
    /// `-55`: Open leave-clan confirmation dialog (`CLAN_LEAVE`).
    ClanLeaveMenu,
    /// `-54`: Donate Senzu bean (`clan_message_id`, `CLAN_DONATE`).
    ClanDonate(Option<i32>),
    /// `-53`: Request clan info (`CLAN_INFO`, `short`).
    ClanInfo(i16),
    /// `-51`: Clan chat, pea request, or join request (`CLAN_MESSAGE`).
    ClanMessage(ClanMessageAction),
    /// `-50`: Request clan member roster (`clan_id`, `CLAN_MEMBER`).
    ClanMemberList(i32),
    /// `-49`: Approve (`0`) or reject (`1`) clan join `(clan_message_id, action)`.
    ClanJoinDecision(i32, i8),
    /// `-47`: Search clans by name substring (`CLAN_SEARCH`).
    ClanSearch(String),
    /// `-46`: Clan creation or info update (`CLAN_CREATE_INFO`).
    ClanCreateInfo(ClanInfoAction),
    /// `-45`: Non-focus or directional AoE (`status == 20`) skill cast.
    SkillNotFocus(i8, Option<SkillSpecialArgs>),
    /// `-43`: Use or drop item `(action_type, where_slot, index, template_id_if_idx_neg1)`.
    UseItem(i8, i8, i8, Option<TemplateId>),
    /// `-41`: Request power captions for `gender` (`UPDATE_CAPTION`).
    UpdateCaption(i8),
    /// `-40`: Move item across containers `(move_type, index)`.
    GetItemMove(i8, i8),
    /// `-39`: Client finished loading map (`FINISH_LOADMAP`).
    FinishLoadMap,
    /// `-38`: Client finished initial asset update (`FINISH_UPDATE`).
    FinishUpdate,
    /// `-34`: Magic Senzu tree (`1` = menu, `2` = load).
    MagicTree(i8),
    /// `-33 | -23`: Trigger map waypoint transition (`true` when `cmd == -33`).
    MapWaypointChange(bool),
    /// `-32`: Request background prop sprite (`BACKGROUND_TEMPLATE`).
    BackgroundTemplate(TemplateId),
    /// `-20`: Pick up ground item (`item_map_id`, `ITEMMAP_MYPICK`).
    PickMapItem(i16),
    /// `-16`: Instant revive (`ME_LIVE`).
    ReviveMeLive,
    /// `-15`: Return home post-death (`ME_BACK`).
    ReturnHomeMeBack,
    /// `-7`: Player movement `(move_type, to_x, optional_to_y)`.
    PlayerMove(i8, i16, Option<i16>),
    /// `-4 | 67`: Simultaneous multi-target mob & player AoE attack (`SEND_PLAYER_ATTACK`).
    AttackMulti(Vec<u8>, Vec<PlayerId>),
    /// `6`: Buy item from shop `(buy_type, item_id, quantity)` (`quantity` defaults to `1`).
    ShopBuy(i8, TemplateId, i16),
    /// `7`: Confirm (`0`) or execute (`!= 0`) item sale `(action, where_slot, index)`.
    ShopSell(i8, i8, i16),
    /// `11`: Request mob sprite template (`mob_id`, `short`).
    RequestMobTemplate(i16),
    /// `18`: Yardrat teleport to `PlayerId` (`GOTO_PLAYER`).
    GotoPlayer(PlayerId),
    /// `21`: Switch zone `(raw_zone_id, validated_zone_id)`.
    ZoneChange(i8, Option<ZoneId>),
    /// `22`: Confirm Magic Tree (`DAU_THAN`) shortcut `(unused_npc, select, option_id)` (`option_id` defaults to `0`).
    DauThanConfirm(i8, i8, i8),
    /// `27`: Legacy open menu ID no-op (`OPEN_MENU_ID`).
    OpenMenuIdNoop,
    /// `29`: Open zone selection UI (`OPEN_UI_ZONE`).
    OpenZoneUi,
    /// `32`: Confirm NPC dialog `(npc_id, select)` (`MenuController.doSelectMenu`).
    NpcMenuConfirm(i16, i8),
    /// `33`: Open NPC dialog `npc_id` (`MenuController.openMenuNPC`).
    NpcMenuOpen(i16),
    /// `34`: Select active skill (`SKILL_SELECT`).
    SkillSelect(TemplateId),
    /// `40`: Task dialog request/advance (`GET_TASK`).
    TaskGet {
        /// Target NPC ID (`byte`).
        npc_id: i8,
        /// Menu index (`byte`).
        menu_id: i8,
        /// Optional dialog option index (`byte`, defaults to `-1`).
        option_id: i8,
    },
    /// `42`: Disabled fast-account registration (isolated from Java's `-127` fallthrough bug!).
    UserInfoDisabled,
    /// `44`: Local map chat or admin/pet command (`CHAT_MAP`).
    ChatMap(String),
    /// `54`: Attack one or more mobs (`mob_id != 0xFF`) and/or `MobMe` targets (`mob_id == 0xFF`, `master_id`).
    AttackMobs(Vec<u8>, Vec<PlayerId>),
    /// `66`: Request named image asset (`GET_IMG_BY_NAME`).
    GetImgByName(String),
    /// `112`: Show Intrinsic (`Nội tại`) menu (`SPEACIAL_SKILL`).
    IntrinsicMenu(i8),
    /// `125`: Fusion request (`type_fusion`).
    Fusion(i8),
    /// `126`: Android asset pack no-op (`ANDROID_PACK`).
    AndroidPackNoop,
    /// `127`: Radar card book action (`RADA_CARD`).
    Radar(RadarAction),
    /// Unrecognized top-level opcode ignored by `Controller.java:695`.
    Unhandled(i8),
}
