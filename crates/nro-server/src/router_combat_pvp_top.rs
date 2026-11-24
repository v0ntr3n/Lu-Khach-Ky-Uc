//! Player context menus, private whispers (`ChatPrivate`), and leaderboard (`TOP`, `-96`) queries.

use nro_consts::cmd_message;
use nro_core::PlayerId;
use nro_protocol::{PacketWriter, ProtocolError, SessionHandle, SocialAction, TradeAction};
use nro_world::s2c_packets;

use super::super::super::{ServerSessionHandler, effective_appearance};
use crate::runtime::LeaderboardSlot;

impl ServerSessionHandler {
    pub(in crate::router) async fn handle_player_menu(
        &self,
        session: &SessionHandle,
        _sid: u64,
        target_pid: PlayerId,
        with_submenu: bool,
    ) -> Result<(), ProtocolError> {
        let pwr = self
            .shared
            .clients
            .players
            .get(&target_pid)
            .and_then(|s| {
                self.shared
                    .clients
                    .authenticated
                    .get(s.value())
                    .map(|e| e.snapshot.stats.power)
            })
            .unwrap_or(2_000);
        let mut w = PacketWriter::with_capacity(28);
        w.write_i32(target_pid.0 as i32);
        w.write_i64(pwr);
        w.write_utf("Chiến binh")?;
        session.send(w.finish(cmd_message::PLAYER_MENU)).await?;
        if with_submenu {
            let mut sub = s2c_packets::sub_command_writer(cmd_message::GET_PLAYER_MENU, 48);
            sub.write_i8(2);
            for (c1, c2, mid) in [
                ("Kết bạn", "Thêm bạn bè", 0_i16),
                ("Giao dịch", "Mời giao dịch", 1_i16),
            ] {
                sub.write_utf(c1)?;
                sub.write_utf(c2)?;
                sub.write_i16(mid);
            }
            session.send(sub.finish(cmd_message::SUB_COMMAND)).await?;
        }
        Ok(())
    }

    pub(in crate::router) async fn handle_player_menu_action(
        &self,
        session: &SessionHandle,
        sid: u64,
        target_pid: PlayerId,
        menu_id: i16,
    ) -> Result<(), ProtocolError> {
        match menu_id {
            0 => {
                self.handle_social(session, sid, SocialAction::PrimaryTarget(target_pid), true)
                    .await
            }
            1 => {
                self.handle_trade(
                    session,
                    sid,
                    TradeAction::InviteOrAccept(0, Some(target_pid)),
                )
                .await
            }
            _ => Ok(()),
        }
    }

    pub(in crate::router) async fn handle_whisper(
        &self,
        session: &SessionHandle,
        sid: u64,
        target: PlayerId,
        text: &str,
    ) -> Result<(), ProtocolError> {
        let Some(pkt) = self.shared.clients.authenticated.get(&sid).and_then(|e| {
            let (h, b, l, bg, _) = effective_appearance(&self.shared.templates, &e.snapshot);
            s2c_packets::chat_global(
                e.snapshot.id,
                &e.snapshot.name,
                &format!("[Mật] {text}"),
                (h, b, bg, l),
            )
            .ok()
        }) else {
            return Ok(());
        };
        if let Some(th) = self.handle_for_pid(target) {
            let _ = th.try_send(pkt);
        } else {
            session
                .send(s2c_packets::thong_bao("Người chơi hiện không trực tuyến!")?)
                .await?;
        }
        Ok(())
    }

    pub(in crate::router) async fn handle_top_query(
        &self,
        session: &SessionHandle,
        _sid: u64,
        name: &str,
        _page: i8,
    ) -> Result<(), ProtocolError> {
        let slot = if name.contains("NV") || name.contains("Nhiệm") {
            LeaderboardSlot::Task
        } else {
            LeaderboardSlot::Power
        };
        let rows = self.shared.leaderboards.snapshot(slot).await;
        let mut w = PacketWriter::with_capacity(16 + rows.len() * 36);
        w.write_i8(0);
        w.write_utf(if name.is_empty() {
            "Bảng Xếp Hạng Sức Mạnh"
        } else {
            name
        })?;
        w.write_i8(rows.len().min(50) as i8);
        for r in rows.iter().take(50) {
            w.write_i32(r.rank as i32);
            w.write_i32(r.rank as i32);
            for v in [64_i16, -1, 57, 58] {
                w.write_i16(v);
            }
            w.write_utf(&r.player_name)?;
            w.write_utf(&format!("Sức mạnh: {}", r.score))?;
            w.write_utf("")?;
        }
        session.send(w.finish(cmd_message::TOP)).await
    }
}
