//! Safe Rust wire protocol, rolling XOR cipher, asymmetric packet framing, C2S opcode
//! router, `MenuController` NPC resolver, admin chat parser, and split Tokio TCP session
//! driver for Teamobi2026 (Ngọc Rồng Online).
//!
//! # Module Overview
//! - [`cipher`]: Rolling XOR cipher (`XorSessionCipher`, `XorReadHalf`, `XorWriteHalf`),
//!   `-27` delta-XOR handshake codec, and out-of-place broadcast-safe slice encryption.
//! - [`frame`]: `Packet`, `PacketReader`, `PacketWriter` (Java big-endian + Modified UTF-8),
//!   and asymmetric S2C 24-bit LE `- 128` extended framing (`-32, -66, -74, 11, -67, -87, 66, 12`).
//! - [`c2s`]: Exhaustive `ClientPacket` AST & `ClientPacket::decode` covering all `Controller.java`
//!   opcodes, `-28`/`-29`/`-30` sub-routers, and `MenuController.java`.
//! - [`admin_cmd`]: Longest-prefix-first chat command parser (`Command.java`) preventing `"up"`
//!   from shadowing `"upp"`.
//! - [`session`]: Non-blocking split Tokio TCP reader/writer tasks with bounded backpressure.

#![forbid(unsafe_code)]

use nro_consts as _;
use nro_core::{MapId, PlayerId, TemplateId};

pub mod admin_cmd;
pub mod c2s;
pub mod cipher;
pub mod frame;
pub mod session;

pub use admin_cmd::{
    AdminCommand, AdminCommandError, AdminItemOption, ChatDirective, PetChatCommand,
    parse_chat_command,
};
pub use c2s::ClientPacket;
pub use cipher::{
    DEFAULT_SESSION_KEY, HANDSHAKE_CMD, XorReadHalf, XorSessionCipher, XorWriteHalf,
    build_handshake_payload, parse_handshake_payload,
};
pub use frame::{
    EXTENDED_S2C_OPCODES, MAX_EXTENDED_PAYLOAD, MAX_STANDARD_PAYLOAD, MAX_UTF_BYTES, Packet,
    PacketReader, PacketWriter, decode_c2s_frame, decode_s2c_frame, encode_c2s_frame,
    encode_s2c_frame, is_extended_s2c_cmd,
};
pub use session::{
    SessionConfig, SessionEventHandler, SessionHandle, drive_session_stream, spawn_tcp_session,
};

/// Alias of [`ProtocolError`] used by downstream packet builders.
pub type PacketError = ProtocolError;

/// Wire-level protocol, framing, cipher, and session transport errors.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    /// Underlying socket or stream I/O failure.
    #[error("transport I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Packet buffer ended before reading the required field width.
    #[error("unexpected EOF reading packet field: needed {needed} bytes, {remaining} remaining")]
    UnexpectedEof {
        /// Minimum bytes required by the primitive or UTF slice.
        needed: usize,
        /// Unread bytes remaining in the packet slice.
        remaining: usize,
    },
    /// Malformed Java Modified UTF-8 / CESU-8 byte sequence.
    #[error("invalid Java Modified UTF-8 sequence in packet string")]
    InvalidModifiedUtf8,
    /// Outbound string exceeds C# `myReader.readUTF()`'s 32,767-byte (`i16::MAX`) ceiling.
    #[error("UTF string encoded length {byte_len} exceeds 32767-byte wire limit")]
    Utf8StringTooLong {
        /// Encoded Modified UTF-8 length in bytes.
        byte_len: usize,
    },
    /// Outbound frame payload exceeds the maximum representable length for its header class.
    #[error("payload size {size} for cmd {cmd} exceeds maximum frame limit {max}")]
    PayloadTooLarge {
        /// Packet command opcode (`i8`).
        cmd: i8,
        /// Actual payload byte length.
        size: usize,
        /// Maximum supported payload byte length (`65_535` or `16_777_215`).
        max: usize,
    },
    /// Session XOR key slice was empty (`KEYS.is_empty()`).
    #[error("XOR session key must contain at least 1 byte")]
    EmptySessionKey,
    /// Key length exceeded the 1-byte `-27` handshake length prefix (`255`).
    #[error("XOR session key length {len} exceeds 255-byte handshake limit")]
    SessionKeyTooLong {
        /// Provided key length in bytes.
        len: usize,
    },
    /// Expected `-27` (`GET_SESSION_ID`) handshake frame, got another opcode.
    #[error("expected handshake opcode -27, received {actual}")]
    InvalidHandshakeOpcode {
        /// Actual `cmd` byte decoded from the frame.
        actual: i8,
    },
    /// Outbound bounded channel was full when attempting non-blocking broadcast delivery.
    #[error("session outbound queue is full (bounded backpressure active)")]
    OutboundQueueFull,
    /// Session read/write loop has already terminated.
    #[error("session connection is closed")]
    SessionClosed,
}

impl PartialEq for ProtocolError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Io(a), Self::Io(b)) => a.kind() == b.kind(),
            (Self::UnexpectedEof { needed: n1, remaining: r1 }, Self::UnexpectedEof { needed: n2, remaining: r2 }) => n1 == n2 && r1 == r2,
            (Self::InvalidModifiedUtf8, Self::InvalidModifiedUtf8)
            | (Self::EmptySessionKey, Self::EmptySessionKey)
            | (Self::OutboundQueueFull, Self::OutboundQueueFull)
            | (Self::SessionClosed, Self::SessionClosed) => true,
            (Self::Utf8StringTooLong { byte_len: a }, Self::Utf8StringTooLong { byte_len: b })
            | (Self::SessionKeyTooLong { len: a }, Self::SessionKeyTooLong { len: b }) => a == b,
            (Self::PayloadTooLarge { cmd: c1, size: s1, max: m1 }, Self::PayloadTooLarge { cmd: c2, size: s2, max: m2 }) => c1 == c2 && s1 == s2 && m1 == m2,
            (Self::InvalidHandshakeOpcode { actual: a }, Self::InvalidHandshakeOpcode { actual: b }) => a == b,
            _ => false,
        }
    }
}

impl Eq for ProtocolError {}

/// Client platform, display, zoom, and version metadata (`-29` sub `2`, `Service.setClientType`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientTypeInfo {
    /// Client platform ID (`byte`).
    pub type_client: i8,
    /// Sprite asset zoom level (`1..=4`).
    pub zoom_level: u8,
    /// GPRS/mobile network indicator (`boolean`).
    pub is_gprs: bool,
    /// Viewport width in pixels (`int`).
    pub width: i32,
    /// Viewport height in pixels (`int`).
    pub height: i32,
    /// Physical QWERTY keyboard indicator (`boolean`).
    pub is_qwerty: bool,
    /// Touchscreen input indicator (`boolean`).
    pub is_touch: bool,
    /// Raw `"OS|X.Y.Z|..."` platform descriptor (`UTF`).
    pub platform: String,
    /// Numeric client version stripped of dots (e.g. `"2.3.1"` -> `231`).
    pub parsed_version: i32,
}

/// Extended AoE vector payload read on `cmd -45` when `status == 20` (`SkillService.java:66-76`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillSpecialArgs {
    /// Skill template ID (`byte`).
    pub skill_id: i8,
    /// Origin X (`short`).
    pub dx: i16,
    /// Origin Y (`short`).
    pub dy: i16,
    /// Facing direction (`byte`).
    pub dir: i8,
    /// Target X (`short`).
    pub x: i16,
    /// Target Y (`short`).
    pub y: i16,
}

/// Consignment shop (`cmd -100`, `KIGUI`) sub-actions (`Controller.java:84-136`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::IntoStaticStr)]
pub enum ConsignAction {
    /// `0`: Consign `(item_id, money_type, money, quantity)`; `quantity` is `i32` if `v >= 220` else `i8`.
    Create(TemplateId, i8, i32, i32),
    /// `1 | 2`: Cancel (`1`) or claim (`2`) `(action, item_id)`.
    ClaimOrCancel(i8, TemplateId),
    /// `3`: Purchase `(item_id, unused_money_type, unused_money)`.
    Buy(TemplateId, i8, i32),
    /// `4`: Open consign shop `(tab_type, page_index)`.
    OpenTab(i8, i8),
    /// `5`: Bump `item_id` to top of consign tab.
    UpToTop(TemplateId),
    /// Unrecognized sub-action.
    Unknown(i8),
}

/// Radar card collection (`cmd 127`, `RADA_CARD`) sub-actions (`Controller.java:138-167`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::IntoStaticStr)]
pub enum RadarAction {
    /// `0`: Open Radar card book.
    OpenList,
    /// `1`: Toggle equipped state of `TemplateId`.
    ToggleCard(TemplateId),
    /// Unhandled sub-action.
    Unknown(i8),
}

/// PK flag (`cmd -103`, `CHANGE_FLAG`) sub-actions (`Controller.java:353-363`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::IntoStaticStr)]
pub enum FlagAction {
    /// `0`: Open PK flag UI.
    OpenUi,
    /// `1`: Equip flag index `i8`.
    Choose(i8),
    /// `2`: Request flag icon image for `c_flag`.
    RequestIcon(i8),
    /// Unhandled sub-action.
    Unknown(i8),
}

/// Friend (`cmd -80`) and Enemy (`cmd -99`) sub-actions (`FriendAndEnemyService.java:44-96`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::IntoStaticStr)]
pub enum SocialAction {
    /// `0`: Open friend or enemy list.
    OpenList,
    /// `1`: Add friend (`-80`) or initiate revenge (`-99`) against `PlayerId`.
    PrimaryTarget(PlayerId),
    /// `2`: Remove friend (`-80`) or enemy (`-99`) `PlayerId`.
    RemoveTarget(PlayerId),
    /// Unhandled sub-action.
    Unknown(i8),
}

/// Trade (`cmd -86`, `GIAO_DICH`) sub-actions (`TransactionService.java:53-175`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::IntoStaticStr)]
pub enum TradeAction {
    /// `0 | 1`: Invite (`0`) or accept (`1`) `(action, partner_player_id)`.
    InviteOrAccept(i8, Option<PlayerId>),
    /// `2`: Add `(bag_index, quantity)` (`bag_index == -1` denotes gold).
    AddItem(i8, i32),
    /// `4`: Remove item at `index` from trade offer.
    RemoveItem(i8),
    /// `3 | 5 | 7` or other: Cancel (`3`), lock (`5`), or confirm (`7`).
    StateChange(i8),
}

/// Clan create/edit (`cmd -46`, `CLAN_CREATE_INFO`) sub-actions (`ClanService.java:126-151`).
#[derive(Debug, Clone, PartialEq, Eq, strum::IntoStaticStr)]
pub enum ClanInfoAction {
    /// `1 | 3`: Request flag icons for create (`1`) or edit (`3`).
    RequestEmblems(i8),
    /// `2`: Submit new clan `(img_id, clan_name)`.
    Create(i16, String),
    /// `4`: Submit updated clan `(img_id, slogan)`.
    Update(i16, String),
    /// Unhandled sub-action.
    Unknown(i8),
}

/// Clan chat/board (`cmd -51`, `CLAN_MESSAGE`) sub-actions (`ClanService.java:153-170`).
#[derive(Debug, Clone, PartialEq, Eq, strum::IntoStaticStr)]
pub enum ClanMessageAction {
    /// `0`: Clan chat message text.
    Chat(String),
    /// `1`: Request Senzu beans (`ASK_FOR_PEA`).
    AskForPea,
    /// `2`: Apply to join `clan_id`.
    AskForJoin(i32),
    /// Unhandled sub-action.
    Unknown(i8),
}

/// Clan invite (`cmd -57`, `CLAN_INVITE`) sub-actions (`ClanService.java:259-273`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::IntoStaticStr)]
pub enum ClanInviteAction {
    /// `0`: Send invite to `PlayerId`.
    SendInvite(PlayerId),
    /// `1`: Accept invite `(clan_id, code)`.
    AcceptInvite(i32, i32),
    /// `2`: Decline invite `(clan_id, code)`.
    DeclineInvite(i32, i32),
    /// Unhandled sub-action.
    Unknown(i8),
}

/// Global NPC IDs special-cased in `Controller.java` and `MenuController.java`.
pub mod npc_id {
    /// Magic Senzu Tree (`ConstNpc.DAU_THAN = 4`, `Controller.java:460`).
    pub const DAU_THAN: i16 = 4;
    /// Virtual system dialog cat (`ConstNpc.CON_MEO = 5`, `MenuController.java:50`).
    pub const CON_MEO: i16 = 5;
    /// Earth Shenron dialog (`ConstNpc.RONG_THIENG = 24`, `MenuController.java:50`).
    pub const RONG_THIENG: i16 = 24;
    /// Future Trunks / Calick (`ConstNpc.CALICK = 38`, global when `map_id != 102`).
    pub const CALICK: i16 = 38;
    /// Event NPC Lý Tiểu Nương (`ConstNpc.LY_TIEU_NUONG = 54`, always global).
    pub const LY_TIEU_NUONG: i16 = 54;
    /// Admin dashboard menu index (`ConstNpc.MENU_ADMIN = 512`, `Command.java:61`).
    pub const MENU_ADMIN: i16 = 512;
    /// Future Bunker map where `CALICK` is resolved map-locally (`MenuController.java:33`).
    pub const FUTURE_BUNKER_MAP: super::MapId = super::MapId(102);
}

/// Resolved NPC lookup target implementing `MenuController.java` (lines 30–69).
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::IntoStaticStr)]
pub enum MenuNpcTarget {
    /// Global singleton NPC resolved via `NpcManager.getNpc(npc_id)` regardless of zone.
    GlobalNpc(i16),
    /// Map-bound NPC resolved via `player.zone.map.getNpc(player, npc_id)`.
    ZoneMapNpc(i16),
    /// Player has no active zone (`player.zone == null`); server sends `hideWaitDialog`.
    HideWaitDialog,
}

/// Resolves `MenuController.openMenuNPC` (`cmd 33`, `MenuController.java:30-45`).
#[must_use]
pub const fn resolve_menu_open_target(npc_id: i16, current_map: Option<MapId>) -> MenuNpcTarget {
    match (npc_id, current_map) {
        (npc_id::CALICK, Some(map_id)) if map_id.0 != npc_id::FUTURE_BUNKER_MAP.0 => {
            MenuNpcTarget::GlobalNpc(npc_id::CALICK)
        }
        (npc_id::LY_TIEU_NUONG, _) => MenuNpcTarget::GlobalNpc(npc_id::LY_TIEU_NUONG),
        (_, Some(_)) => MenuNpcTarget::ZoneMapNpc(npc_id),
        (_, None) => MenuNpcTarget::HideWaitDialog,
    }
}

/// Resolves `MenuController.doSelectMenu` (`cmd 32`, `MenuController.java:47-69`).
#[must_use]
pub const fn resolve_menu_confirm_target(npc_id: i16, current_map: Option<MapId>) -> MenuNpcTarget {
    match npc_id {
        npc_id::RONG_THIENG | npc_id::CON_MEO => MenuNpcTarget::GlobalNpc(npc_id),
        _ => resolve_menu_open_target(npc_id, current_map),
    }
}
