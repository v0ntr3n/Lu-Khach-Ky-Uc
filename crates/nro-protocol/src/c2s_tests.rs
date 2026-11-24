use super::*;
use crate::frame::{PacketReader, PacketWriter};

#[test]
fn decodes_ping_and_multi_attack_with_trailing_cdir() {
    let mut empty = PacketReader::new(&[]);
    assert_eq!(
        ClientPacket::decode(-120, &mut empty, 231).expect("decode -120"),
        ClientPacket::CheckControllerPing
    );
    assert_eq!(
        ClientPacket::decode(-121, &mut empty, 231).expect("decode -121"),
        ClientPacket::CheckMapPing
    );

    let mut pw = PacketWriter::new();
    pw.write_u8(2);
    pw.write_u8(3);
    pw.write_u8(200);
    pw.write_i32(1001);
    pw.write_i8(1); // trailing cdir written by Service.sendPlayerAttack
    let pkt = pw.finish(-4);
    let decoded = ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("decode -4");
    assert_eq!(
        decoded,
        ClientPacket::AttackMulti(vec![3_u8, 200_u8], vec![PlayerId::from(1001)])
    );
}

#[test]
fn decodes_updated_c2s_opcodes() {
    // 11: RequestMobTemplate(i16)
    let mut pw = PacketWriter::new();
    pw.write_i16(300);
    let pkt = pw.finish(11);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("11"),
        ClientPacket::RequestMobTemplate(300)
    );

    // -63: GetBagEffect(i16)
    let mut pw = PacketWriter::new();
    pw.write_i16(512);
    let pkt = pw.finish(-63);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("-63"),
        ClientPacket::GetBagEffect(512)
    );

    // -46: ClanCreateInfo with i16 img_id
    let mut pw = PacketWriter::new();
    pw.write_i8(2);
    pw.write_i16(400);
    pw.write_utf("Dragon").expect("utf");
    let pkt = pw.finish(-46);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("-46"),
        ClientPacket::ClanCreateInfo(ClanInfoAction::Create(400, "Dragon".to_owned()))
    );

    // -81: CombinePreview only reads vec when action == 1
    let mut pw = PacketWriter::new();
    pw.write_i8(0);
    let pkt = pw.finish(-81);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("-81"),
        ClientPacket::CombinePreview(0, Vec::new())
    );

    // 54: AttackMobs multiple u8 mobs + 0xFF MobMe + trailing cdir
    let mut pw = PacketWriter::new();
    pw.write_u8(2);
    pw.write_u8(0xFF);
    pw.write_i32(77);
    pw.write_u8(180);
    pw.write_i8(1); // trailing cdir
    let pkt = pw.finish(54);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("54"),
        ClientPacket::AttackMobs(vec![2_u8, 180_u8], vec![PlayerId::from(77)])
    );

    // -60: AttackPlayers multiple players
    let mut pw = PacketWriter::new();
    pw.write_i32(10);
    pw.write_i32(20);
    pw.write_i8(1); // trailing cdir
    let pkt = pw.finish(-60);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("-60"),
        ClientPacket::AttackPlayers(vec![PlayerId::from(10), PlayerId::from(20)])
    );

    // -57: ClanInvite accept/decline with (clan_id, code)
    let mut pw = PacketWriter::new();
    pw.write_i8(1);
    pw.write_i32(99);
    pw.write_i32(12345);
    let pkt = pw.finish(-57);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("-57"),
        ClientPacket::ClanInvite(ClanInviteAction::AcceptInvite(99, 12345))
    );

    // -86: Trade RemoveItem(index)
    let mut pw = PacketWriter::new();
    pw.write_i8(4);
    pw.write_i8(3);
    let pkt = pw.finish(-86);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("-86"),
        ClientPacket::Trade(TradeAction::RemoveItem(3))
    );

    // -43: UseItem reads template_id whenever idx == -1 regardless of type
    let mut pw = PacketWriter::new();
    pw.write_i8(2);
    pw.write_i8(1);
    pw.write_i8(-1);
    pw.write_i16(457);
    let pkt = pw.finish(-43);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("-43"),
        ClientPacket::UseItem(2, 1, -1, Some(TemplateId(457)))
    );

    // 6: ShopBuy default quantity 1 and explicit quantity
    let mut pw = PacketWriter::new();
    pw.write_i8(0);
    pw.write_i16(12);
    let pkt = pw.finish(6);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("6 default"),
        ClientPacket::ShopBuy(0, TemplateId(12), 1)
    );
    let mut pw = PacketWriter::new();
    pw.write_i8(0);
    pw.write_i16(12);
    pw.write_i16(5);
    let pkt = pw.finish(6);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("6 qty"),
        ClientPacket::ShopBuy(0, TemplateId(12), 5)
    );

    // 22: DauThanConfirm optional 3rd byte
    let mut pw = PacketWriter::new();
    pw.write_i8(4);
    pw.write_i8(1);
    let pkt = pw.finish(22);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("22 default"),
        ClientPacket::DauThanConfirm(4, 1, 0)
    );

    // -29 sub 0: Login with 2 fields and 4 fields
    let mut pw = PacketWriter::new();
    pw.write_i8(0);
    pw.write_utf("goku").expect("utf");
    pw.write_utf("pass").expect("utf");
    let pkt = pw.finish(-29);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("-29 login 2"),
        ClientPacket::Login {
            username: "goku".to_owned(),
            password: "pass".to_owned(),
            version: String::new(),
            login_type: 0
        }
    );

    // -29 sub 1: Register
    let mut pw = PacketWriter::new();
    pw.write_i8(1);
    pw.write_utf("vegeta").expect("utf");
    pw.write_utf("secret").expect("utf");
    pw.write_utf("guest1").expect("utf");
    let pkt = pw.finish(-29);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("-29 register"),
        ClientPacket::Register {
            username: "vegeta".to_owned(),
            password: "secret".to_owned(),
            guest_user: Some("guest1".to_owned())
        }
    );

    // -30 sub 17: SkillUp
    let mut pw = PacketWriter::new();
    pw.write_i8(17);
    pw.write_i16(3);
    pw.write_i8(1);
    let pkt = pw.finish(-30);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("-30 skillup"),
        ClientPacket::SkillUp {
            template_id: 3,
            point: 1
        }
    );

    // 40: TaskGet default option_id -1
    let mut pw = PacketWriter::new();
    pw.write_i8(1);
    pw.write_i8(2);
    let pkt = pw.finish(40);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("40"),
        ClientPacket::TaskGet {
            npc_id: 1,
            menu_id: 2,
            option_id: -1
        }
    );

    // -53: ClanInfo
    let mut pw = PacketWriter::new();
    pw.write_i16(7);
    let pkt = pw.finish(-53);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("-53"),
        ClientPacket::ClanInfo(7)
    );
}

#[test]
fn decodes_flag_icon_rms_player_menu_box_sort_rename_fusion_and_top() {
    // -103: FlagAction OpenUi, Choose, RequestIcon, Unknown
    let mut pw = PacketWriter::new();
    pw.write_i8(0);
    let pkt = pw.finish(-103);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 250).expect("-103 0"),
        ClientPacket::ChangeFlag(FlagAction::OpenUi)
    );

    let mut pw = PacketWriter::new();
    pw.write_i8(1);
    pw.write_i8(5);
    let pkt = pw.finish(-103);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 250).expect("-103 1"),
        ClientPacket::ChangeFlag(FlagAction::Choose(5))
    );

    let mut pw = PacketWriter::new();
    pw.write_i8(2);
    pw.write_i8(8);
    let pkt = pw.finish(-103);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 250).expect("-103 2"),
        ClientPacket::ChangeFlag(FlagAction::RequestIcon(8))
    );

    // -111: GetImageSource2 drains string list cleanly
    let mut pw = PacketWriter::new();
    pw.write_i16(2);
    pw.write_utf("res_v1").expect("utf");
    pw.write_utf("res_v2").expect("utf");
    let pkt = pw.finish(-111);
    let mut reader = pkt.reader();
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut reader, 250).expect("-111"),
        ClientPacket::GetImageSource2
    );
    assert_eq!(reader.remaining(), 0);

    // -30 subs 19, 61, 63
    let mut pw = PacketWriter::new();
    pw.write_i8(19);
    let pkt = pw.finish(-30);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 250).expect("-30 19"),
        ClientPacket::BoxSort
    );

    let mut pw = PacketWriter::new();
    pw.write_i8(61);
    pw.write_utf("shortcut").expect("utf");
    let pkt = pw.finish(-30);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 250).expect("-30 61"),
        ClientPacket::LoadRms("shortcut".to_owned())
    );

    let mut pw = PacketWriter::new();
    pw.write_i8(63);
    pw.write_i32(4242);
    let pkt = pw.finish(-30);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 250).expect("-30 63"),
        ClientPacket::GetPlayerMenu(PlayerId::from(4242))
    );

    // -28 sub 18: ChangeName
    let mut pw = PacketWriter::new();
    pw.write_i8(18);
    pw.write_i32(77);
    pw.write_utf("kakarot").expect("utf");
    let pkt = pw.finish(-28);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 250).expect("-28 18"),
        ClientPacket::ChangeName {
            player_id: 77,
            name: "kakarot".to_owned()
        }
    );

    // 125: Fusion
    let mut pw = PacketWriter::new();
    pw.write_i8(4);
    let pkt = pw.finish(125);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 250).expect("125"),
        ClientPacket::Fusion(4)
    );

    // -96: TopQuery
    let mut pw = PacketWriter::new();
    pw.write_utf("power").expect("utf");
    pw.write_i8(0);
    let pkt = pw.finish(-96);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 250).expect("-96"),
        ClientPacket::TopQuery("power".to_owned(), 0)
    );
}
