//! Wire decoder implementation for [`ClientPacket::decode`] and opcode-specific helpers.

use nro_core::{MapId, PlayerId, TemplateId, ZoneId};

use super::ClientPacket;
use crate::frame::PacketReader;
use crate::{
    ClanInfoAction, ClanInviteAction, ClanMessageAction, ClientTypeInfo, ConsignAction, FlagAction,
    ProtocolError, RadarAction, SkillSpecialArgs, SocialAction, TradeAction,
};

impl ClientPacket {
    /// Decodes a C2S `cmd` and payload `reader` according to `Controller.java` and `client_version`.
    pub fn decode(cmd: i8, reader: &mut PacketReader<'_>, client_version: i32) -> Result<Self, ProtocolError> {
        let r = reader;
        Ok(match cmd {
            -127 => Self::LuckyRoundOpen(r.read_i8().ok(), r.read_i8().ok()),
            -125 => Self::ClientInput(read_vec(r, PacketReader::read_i8_utf_fallback)?),
            -121 => Self::CheckMapPing,
            -120 => Self::CheckControllerPing,
            -118 => Self::SuperRankOrAdminBoss(r.read_i32()?),
            -114 => Self::RequestPeanNoop,
            -113 => Self::ChangeOnSkill(std::array::from_fn(|_| r.read_i8().unwrap_or(-1))),
            -111 => {
                if let Ok(n) = r.read_i16() {
                    for _ in 0..n.max(0) {
                        let _ = r.read_utf();
                    }
                }
                Self::GetImageSource2
            }
            -108 => Self::PetChangeStatus(r.read_i8()?),
            -107 => Self::PetShowInfo,
            -105 => Self::TransportComplete,
            -104 => Self::LockInventory(r.read_i32()?),
            -103 => Self::ChangeFlag(match r.read_i8()? {
                0 => FlagAction::OpenUi,
                1 => FlagAction::Choose(r.read_i8()?),
                2 => FlagAction::RequestIcon(r.read_i8()?),
                x => FlagAction::Unknown(x),
            }),
            -101 => Self::Login2GuestNotice,
            -100 => Self::ConsignShop(decode_consign(r, client_version)?),
            -99 => Self::EnemyList(decode_social(r)?),
            -96 => Self::TopQuery(r.read_utf()?, r.read_i8()?),
            -91 => Self::MapTransport(r.read_i8().ok()),
            -87 => Self::UpdateData,
            -86 => Self::Trade(match r.read_i8()? {
                a @ (0 | 1) => TradeAction::InviteOrAccept(a, r.read_i32().ok().map(PlayerId::from)),
                2 => TradeAction::AddItem(r.read_i8()?, r.read_i32()?),
                4 => TradeAction::RemoveItem(r.read_i8()?),
                a => TradeAction::StateChange(a),
            }),
            -81 => {
                let action = r.read_i8()?;
                let items = if action == 1 { read_vec(r, PacketReader::read_i8)? } else { Vec::new() };
                Self::CombinePreview(action, items)
            }
            -80 => Self::FriendList(decode_social(r)?),
            -79 => Self::PlayerMenu(PlayerId::from(r.read_i32()?)),
            -78 => Self::CheckMove(r.read_i32()?),
            -76 => Self::AchievementConfirm(r.read_i8()?),
            -74 => Self::GetImageSource(r.read_i8()?),
            -72 => Self::ChatPrivate(PlayerId::from(r.read_i32()?), r.read_utf()?),
            -71 => Self::ChatGlobal(r.read_utf()?),
            -67 => Self::RequestIcon(r.read_i32()?),
            -66 => Self::GetEffData(TemplateId(r.read_i16()?)),
            -63 => Self::GetBagEffect(r.read_i16()?),
            -62 => Self::ClanImageChoose(r.read_i8()?),
            -60 => decode_attack_players(r)?,
            -59 => Self::PvpChallenge(r.read_i8()?, r.read_i8()?, PlayerId::from(r.read_i32()?)),
            -57 => Self::ClanInvite(match r.read_i8()? {
                0 => ClanInviteAction::SendInvite(PlayerId::from(r.read_i32()?)),
                1 => ClanInviteAction::AcceptInvite(r.read_i32()?, r.read_i32()?),
                2 => ClanInviteAction::DeclineInvite(r.read_i32()?, r.read_i32()?),
                x => ClanInviteAction::Unknown(x),
            }),
            -56 => Self::ClanRemote(PlayerId::from(r.read_i32()?), r.read_i8()?),
            -55 => Self::ClanLeaveMenu,
            -54 => Self::ClanDonate(r.read_i32().ok()),
            -53 => Self::ClanInfo(r.read_i16()?),
            -51 => Self::ClanMessage(match r.read_i8()? { 0 => ClanMessageAction::Chat(r.read_utf()?), 1 => ClanMessageAction::AskForPea, 2 => ClanMessageAction::AskForJoin(r.read_i32()?), x => ClanMessageAction::Unknown(x) }),
            -50 => Self::ClanMemberList(r.read_i32()?),
            -49 => Self::ClanJoinDecision(r.read_i32()?, r.read_i8()?),
            -47 => Self::ClanSearch(r.read_utf()?),
            -46 => Self::ClanCreateInfo(match r.read_i8()? {
                a @ (1 | 3) => ClanInfoAction::RequestEmblems(a),
                2 => ClanInfoAction::Create(r.read_i16()?, r.read_utf()?),
                4 => ClanInfoAction::Update(r.read_i16()?, r.read_utf()?),
                x => ClanInfoAction::Unknown(x),
            }),
            -45 => { let s = r.read_i8()?; Self::SkillNotFocus(s, (s == 20).then(|| Ok::<_, ProtocolError>(SkillSpecialArgs { skill_id: r.read_i8()?, dx: r.read_i16()?, dy: r.read_i16()?, dir: r.read_i8()?, x: r.read_i16()?, y: r.read_i16()? })).transpose()?) }
            -43 => {
                let (t, w, idx) = (r.read_i8()?, r.read_i8()?, r.read_i8()?);
                Self::UseItem(t, w, idx, (idx == -1).then(|| r.read_i16().map(TemplateId)).transpose()?)
            }
            -41 => Self::UpdateCaption(r.read_i8()?),
            -40 => Self::GetItemMove(r.read_i8()?, r.read_i8()?),
            -39 => Self::FinishLoadMap,
            -38 => Self::FinishUpdate,
            -34 => Self::MagicTree(r.read_i8()?),
            -33 | -23 => Self::MapWaypointChange(cmd == -33),
            -32 => Self::BackgroundTemplate(TemplateId(r.read_i16()?)),
            -30 => match r.read_i8()? {
                16 => Self::PotentialUp(r.read_i8()?, r.read_i16()?),
                17 => Self::SkillUp { template_id: r.read_i16()?, point: r.read_i8()? },
                19 => Self::BoxSort,
                61 => Self::LoadRms(r.read_utf()?),
                63 => Self::GetPlayerMenu(PlayerId::from(r.read_i32()?)),
                64 => Self::PlayerMenuAction(PlayerId::from(r.read_i32()?), r.read_i16()?),
                s => Self::SubCommandUnknown(s),
            },
            -29 => decode_not_login(r)?,
            -28 => match r.read_i8()? {
                2 => Self::CreateChar(r.read_utf()?, r.read_i8()?, r.read_i8()?),
                6 => Self::UpdateMap,
                7 => Self::UpdateSkill,
                8 => Self::UpdateItem,
                10 => Self::RequestMapTemplate(MapId(i16::from(r.read_u8()?))),
                13 => Self::ClientOk,
                18 => Self::ChangeName { player_id: r.read_i32()?, name: r.read_utf()? },
                s => Self::NotMapUnknown(s),
            },
            -27 => Self::GetSessionId,
            -20 => Self::PickMapItem(r.read_i16()?),
            -16 => Self::ReviveMeLive,
            -15 => Self::ReturnHomeMeBack,
            -7 => Self::PlayerMove(r.read_i8()?, r.read_i16()?, r.read_i16().ok()),
            -4 | 67 => decode_attack_multi(r)?,
            6 => {
                let buy_type = r.read_i8()?;
                let item_id = TemplateId(r.read_i16()?);
                let quantity = if r.remaining() >= 2 { r.read_i16()? } else { 1 };
                Self::ShopBuy(buy_type, item_id, quantity)
            }
            7 => Self::ShopSell(r.read_i8()?, r.read_i8()?, r.read_i16()?),
            11 => Self::RequestMobTemplate(r.read_i16()?),
            18 => Self::GotoPlayer(PlayerId::from(r.read_i32()?)),
            21 => { let z = r.read_i8()?; Self::ZoneChange(z, u16::try_from(z).ok().map(ZoneId)) }
            22 => {
                let npc = r.read_i8()?;
                let select = r.read_i8()?;
                let option_id = if r.remaining() >= 1 { r.read_i8()? } else { 0 };
                Self::DauThanConfirm(npc, select, option_id)
            }
            27 => Self::OpenMenuIdNoop,
            29 => Self::OpenZoneUi,
            32 => Self::NpcMenuConfirm(r.read_i16()?, r.read_i8()?),
            33 => Self::NpcMenuOpen(r.read_i16()?),
            34 => Self::SkillSelect(TemplateId(r.read_i16()?)),
            40 => {
                let npc_id = r.read_i8()?;
                let menu_id = r.read_i8()?;
                let option_id = if r.remaining() >= 1 { r.read_i8()? } else { -1 };
                Self::TaskGet { npc_id, menu_id, option_id }
            }
            42 => Self::UserInfoDisabled,
            44 => Self::ChatMap(r.read_utf()?),
            54 => decode_attack_mobs(r)?,
            66 => Self::GetImgByName(r.read_utf()?),
            112 => Self::IntrinsicMenu(r.read_i8().unwrap_or(0)),
            125 => Self::Fusion(r.read_i8()?),
            126 => Self::AndroidPackNoop,
            127 => Self::Radar(match r.read_i8()? { 0 => RadarAction::OpenList, 1 => RadarAction::ToggleCard(TemplateId(r.read_i16()?)), x => RadarAction::Unknown(x) }),
            other => Self::Unhandled(other),
        })
    }
}

trait ReadUtfHelper {
    fn read_i8_utf_fallback(&mut self) -> Result<String, ProtocolError>;
}

impl ReadUtfHelper for PacketReader<'_> {
    fn read_i8_utf_fallback(&mut self) -> Result<String, ProtocolError> {
        self.read_utf()
    }
}

fn decode_attack_mobs(r: &mut PacketReader<'_>) -> Result<ClientPacket, ProtocolError> {
    let mut mobs = Vec::new();
    let mut mob_mes = Vec::new();
    while r.remaining() > 1 {
        let m = r.read_i8()?;
        if m == -1 {
            mob_mes.push(PlayerId::from(r.read_i32()?));
        } else {
            mobs.push(m);
        }
    }
    if mobs.is_empty() && mob_mes.is_empty() && r.remaining() == 1 {
        let m = r.read_i8()?;
        if m != -1 {
            mobs.push(m);
        }
    }
    Ok(ClientPacket::AttackMobs(mobs, mob_mes))
}

fn decode_attack_players(r: &mut PacketReader<'_>) -> Result<ClientPacket, ProtocolError> {
    let mut players = Vec::new();
    while r.remaining() >= 4 {
        players.push(PlayerId::from(r.read_i32()?));
    }
    Ok(ClientPacket::AttackPlayers(players))
}

fn decode_attack_multi(r: &mut PacketReader<'_>) -> Result<ClientPacket, ProtocolError> {
    let mob_count = usize::try_from(r.read_i8()?).unwrap_or(0);
    let mut mobs = Vec::with_capacity(mob_count.min(32));
    for _ in 0..mob_count {
        mobs.push(r.read_i8()?);
    }
    let mut players = Vec::new();
    while r.remaining() >= 4 {
        players.push(PlayerId::from(r.read_i32()?));
    }
    Ok(ClientPacket::AttackMulti(mobs, players))
}

fn read_vec<'a, T>(r: &mut PacketReader<'a>, f: fn(&mut PacketReader<'a>) -> Result<T, ProtocolError>) -> Result<Vec<T>, ProtocolError> {
    let n = usize::try_from(r.read_i8()?).unwrap_or(0);
    (0..n).map(|_| f(r)).collect()
}

fn decode_consign(r: &mut PacketReader<'_>, v: i32) -> Result<ConsignAction, ProtocolError> {
    Ok(match r.read_i8()? {
        0 => ConsignAction::Create(TemplateId(r.read_i16()?), r.read_i8()?, r.read_i32()?, if v >= 220 || r.remaining() >= 4 { r.read_i32()? } else { i32::from(r.read_i8()?) }),
        a @ (1 | 2) => ConsignAction::ClaimOrCancel(a, TemplateId(r.read_i16()?)),
        3 => ConsignAction::Buy(TemplateId(r.read_i16()?), r.read_i8()?, r.read_i32()?),
        4 => ConsignAction::OpenTab(r.read_i8()?, r.read_i8()?),
        5 => ConsignAction::UpToTop(TemplateId(r.read_i16()?)),
        x => ConsignAction::Unknown(x),
    })
}

fn decode_social(r: &mut PacketReader<'_>) -> Result<SocialAction, ProtocolError> {
    let read_pid = |r: &mut PacketReader<'_>| -> Result<PlayerId, ProtocolError> {
        Ok(PlayerId::from(if r.remaining() >= 4 { r.read_i32()? } else { -1 }))
    };
    Ok(match r.read_i8()? {
        0 => SocialAction::OpenList,
        1 => SocialAction::PrimaryTarget(read_pid(r)?),
        2 => SocialAction::RemoveTarget(read_pid(r)?),
        x => SocialAction::Unknown(x),
    })
}

fn decode_not_login(r: &mut PacketReader<'_>) -> Result<ClientPacket, ProtocolError> {
    Ok(match r.read_i8()? {
        0 => {
            let username = r.read_utf()?;
            let password = r.read_utf()?;
            let version = if r.remaining() >= 2 { r.read_utf()? } else { String::new() };
            let login_type = if r.remaining() >= 1 { r.read_i8()? } else { 0 };
            ClientPacket::Login { username, password, version, login_type }
        }
        1 => {
            let username = r.read_utf()?;
            let password = r.read_utf()?;
            let guest_user = if r.remaining() >= 2 {
                let guest = r.read_utf()?;
                if guest.is_empty() { None } else { Some(guest) }
            } else {
                None
            };
            ClientPacket::Register { username, password, guest_user }
        }
        2 => {
            let (type_client, zoom_level, is_gprs) = (r.read_i8()?, r.read_u8()?, r.read_bool()?);
            let (width, height) = (r.read_i32()?, r.read_i32()?);
            let (is_qwerty, is_touch, platform) = (r.read_bool()?, r.read_bool()?, r.read_utf()?);
            let parsed_version = platform.split('|').nth(1).and_then(|s| s.replace('.', "").parse().ok()).unwrap_or(0);
            ClientPacket::SetClientType(ClientTypeInfo { type_client, zoom_level, is_gprs, width, height, is_qwerty, is_touch, platform, parsed_version })
        }
        s => ClientPacket::NotLoginUnknown(s),
    })
}
