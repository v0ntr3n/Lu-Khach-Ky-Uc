use super::*;
use crate::frame::PacketWriter;

#[test]
fn decodes_unsigned_clan_image_flag_id_above_127() {
    let mut pw = PacketWriter::new();
    pw.write_u8(200);
    let pkt = pw.finish(-62);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("-62"),
        ClientPacket::ClanImageChoose(200)
    );
}

#[test]
fn attack_decoders_keep_final_mob_and_cap_targets() {
    let mut pw = PacketWriter::new();
    pw.write_u8(1);
    pw.write_u8(200);
    pw.write_u8(3);
    pw.write_i8(1);
    let pkt = pw.finish(54);
    assert_eq!(
        ClientPacket::decode(pkt.cmd, &mut pkt.reader(), 231).expect("54"),
        ClientPacket::AttackMobs(vec![1, 200, 3], vec![])
    );

    let mut pw_cap = PacketWriter::new();
    for i in 0..50_u8 {
        pw_cap.write_u8(i);
    }
    pw_cap.write_i8(1);
    let pkt_cap = pw_cap.finish(54);
    if let ClientPacket::AttackMobs(mobs, _) =
        ClientPacket::decode(pkt_cap.cmd, &mut pkt_cap.reader(), 231).expect("54 cap")
    {
        assert_eq!(mobs.len(), MAX_ATTACK_TARGETS);
    } else {
        panic!("expected AttackMobs");
    }
}
