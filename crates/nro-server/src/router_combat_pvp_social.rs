//! Social/enemy revenge (`-80`, `-99`) handlers.

#[path = "router_combat_pvp_top.rs"]
mod router_combat_pvp_top;

use nro_consts::cmd_message;
use nro_core::PlayerId;
use nro_db::json_columns::SocialPeer;
use nro_protocol::{PacketWriter, ProtocolError, SessionHandle, SocialAction};
use nro_world::s2c_packets;

use super::super::{ServerSessionHandler, effective_appearance, wallet_money_pkt};
use super::lock_pvp;

impl ServerSessionHandler {
    pub(in crate::router) async fn handle_social(
        &self,
        session: &SessionHandle,
        sid: u64,
        action: SocialAction,
        is_friend: bool,
    ) -> Result<(), ProtocolError> {
        match action {
            SocialAction::PrimaryTarget(tpid) if is_friend => {
                let peer = self
                    .shared
                    .clients
                    .players
                    .get(&tpid)
                    .map(|s| *s.value())
                    .and_then(|tsid| {
                        let e = self.shared.clients.authenticated.get(&tsid)?;
                        let (h, b, l, bg, _) =
                            effective_appearance(&self.shared.templates, &e.snapshot);
                        Some(SocialPeer {
                            id: tpid.0 as i32,
                            name: e.snapshot.name.clone(),
                            head: h,
                            body: b,
                            leg: l,
                            bag: bg as i8,
                            power: e.snapshot.stats.power,
                        })
                    });
                let msg = match peer {
                    None => "Người chơi hiện không trực tuyến!",
                    Some(peer) => {
                        let added =
                            self.shared
                                .clients
                                .authenticated
                                .get_mut(&sid)
                                .map(|mut me| {
                                    if me.snapshot.friends.iter().any(|f| f.id == peer.id) {
                                        false
                                    } else {
                                        me.snapshot.friends.push(peer);
                                        true
                                    }
                                });
                        match added {
                            Some(true) => "Đã thêm vào danh sách bạn bè!",
                            Some(false) => "Người chơi đã có trong danh sách bạn bè!",
                            None => "Không thể thêm bạn bè lúc này!",
                        }
                    }
                };
                session.send(s2c_packets::thong_bao(msg)?).await?;
            }
            SocialAction::PrimaryTarget(tpid) => {
                let target_loc = self
                    .shared
                    .clients
                    .players
                    .get(&tpid)
                    .map(|s| *s.value())
                    .and_then(|tsid| {
                        self.shared
                            .clients
                            .authenticated
                            .get(&tsid)
                            .map(|e| e.snapshot.location)
                    });
                let revenge_prep = i32::try_from(tpid.0).ok().and_then(|id| {
                    self.shared
                        .clients
                        .authenticated
                        .get_mut(&sid)
                        .and_then(|mut me| {
                            if !me.snapshot.enemies.iter().any(|e| e.id == id) {
                                return None;
                            }
                            if me.snapshot.stats.hp <= 0
                                || super::super::router_shop::is_in_trade(me.snapshot.id)
                                || super::super::router_shop::is_in_trade(tpid)
                            {
                                return Some(Err("Không thể dịch chuyển lúc này!"));
                            }
                            let Some(tloc) = target_loc else {
                                return Some(Err("Kẻ thù hiện không trực tuyến!"));
                            };
                            let non_normal = self
                                .shared
                                .templates
                                .maps
                                .iter()
                                .find(|m| m.id == tloc.map_id)
                                .is_some_and(|m| m.map_type != nro_consts::const_map::MAP_NORMAL);
                            if non_normal
                                || matches!(
                                    tloc.map_id.0,
                                    21..=23
                                        | 39..=41
                                        | 45..=51
                                        | 53..=62
                                        | 85..=91
                                        | 112..=120
                                        | 127..=129
                                )
                                || tloc.map_id.0 >= 135
                            {
                                return Some(Err("Không thể dịch chuyển đến khu vực này!"));
                            }
                            let my_pid = me.snapshot.id;
                            let in_pvp = {
                                let pvp = lock_pvp();
                                pvp.contains_key(&my_pid) || pvp.contains_key(&tpid)
                            };
                            if in_pvp {
                                return Some(Err("Không thể trả thù khi đang trong trận PvP!"));
                            }
                            let used_ruby = if me.snapshot.wallet.ruby >= 1 {
                                me.snapshot.wallet.ruby -= 1;
                                true
                            } else if me.snapshot.wallet.gem >= 1 {
                                me.snapshot.wallet.gem -= 1;
                                false
                            } else {
                                return Some(Err("Bạn không đủ ngọc để trả thù!"));
                            };
                            Some(Ok((
                                my_pid,
                                tloc,
                                used_ruby,
                                wallet_money_pkt(&me.snapshot.wallet),
                            )))
                        })
                });
                match revenge_prep {
                    Some(Err(err_msg)) => {
                        session.send(s2c_packets::thong_bao(err_msg)?).await?;
                    }
                    Some(Ok((my_pid, tloc, used_ruby, money_pkt))) => {
                        let switched = self
                            .switch_player_zone_with_ship(
                                session,
                                sid,
                                Some(tloc.map_id),
                                tloc.zone_id,
                                Some((tloc.x, tloc.y)),
                                0,
                            )
                            .await;
                        if matches!(switched, Ok(true)) {
                            session.send(money_pkt).await?;
                            let arrived = self
                                .shared
                                .clients
                                .authenticated
                                .get(&sid)
                                .map(|e| (e.snapshot.location, e.snapshot.stats.hp));
                            let target_state = self
                                .shared
                                .clients
                                .players
                                .get(&tpid)
                                .map(|s| *s.value())
                                .and_then(|tsid| {
                                    self.shared
                                        .clients
                                        .authenticated
                                        .get(&tsid)
                                        .map(|e| (e.snapshot.location, e.snapshot.stats.hp))
                                });
                            if let (Some((mloc, mhp)), Some((tloc, thp))) = (arrived, target_state)
                                && mhp > 0
                                && thp > 0
                                && mloc.map_id == tloc.map_id
                                && mloc.zone_id == tloc.zone_id
                            {
                                let inserted = {
                                    let mut pvp = lock_pvp();
                                    if !pvp.contains_key(&my_pid)
                                        && !pvp.contains_key(&tpid)
                                        && !super::super::router_shop::is_in_trade(my_pid)
                                        && !super::super::router_shop::is_in_trade(tpid)
                                    {
                                        pvp.insert(my_pid, (tpid, 0));
                                        pvp.insert(tpid, (my_pid, 0));
                                        true
                                    } else {
                                        false
                                    }
                                };
                                if inserted {
                                    self.broadcast_pk_type(mloc, sid, my_pid, 3);
                                    self.broadcast_pk_type(tloc, sid, tpid, 3);
                                }
                            }
                        } else {
                            if let Some(mut me) = self.shared.clients.authenticated.get_mut(&sid) {
                                if used_ruby {
                                    me.snapshot.wallet.ruby =
                                        me.snapshot.wallet.ruby.saturating_add(1);
                                } else {
                                    me.snapshot.wallet.gem =
                                        me.snapshot.wallet.gem.saturating_add(1);
                                }
                            }
                            let _ = switched?;
                        }
                    }
                    None => {}
                }
            }
            SocialAction::RemoveTarget(tpid) => {
                if let Some(mut me) = self.shared.clients.authenticated.get_mut(&sid) {
                    let list = if is_friend {
                        &mut me.snapshot.friends
                    } else {
                        &mut me.snapshot.enemies
                    };
                    list.retain(|p| p.id != tpid.0 as i32);
                }
            }
            SocialAction::OpenList | SocialAction::Unknown(_) => {}
        }
        let peers = self
            .shared
            .clients
            .authenticated
            .get(&sid)
            .map_or_default(|e| {
                if is_friend {
                    e.snapshot.friends.clone()
                } else {
                    e.snapshot.enemies.clone()
                }
            });
        let mut w = PacketWriter::with_capacity(8 + peers.len() * 32);
        w.write_i8(0);
        w.write_u8(peers.len().min(100) as u8);
        for p in peers.iter().take(100) {
            let online = self
                .shared
                .clients
                .players
                .contains_key(&PlayerId(i64::from(p.id)));
            w.write_i32(p.id);
            for v in [p.head, -1, p.body, p.leg, i16::from(p.bag)] {
                w.write_i16(v);
            }
            w.write_utf(&p.name)?;
            if is_friend {
                w.write_bool(online);
                w.write_utf(&p.power.to_string())?;
            } else {
                w.write_utf(&p.power.to_string())?;
                w.write_bool(online);
            }
        }
        session
            .send(w.finish(if is_friend {
                cmd_message::FRIEND
            } else {
                cmd_message::ENEMY_LIST
            }))
            .await
    }
}
