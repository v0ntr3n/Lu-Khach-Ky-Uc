//! Pre-login and in-world `DataGame` C2S packet dispatch and handshake/resource-pack builders.

use bytes::Bytes;
use nro_consts::cmd_message;
use nro_db::templates::GameTemplates;
use nro_protocol::{ClientPacket, Packet, PacketWriter, ProtocolError, SessionHandle};
use tracing::{error, info, warn};

use crate::data_game_tpl;

use super::{DataGameAssets, VS_DATA, VS_ITEM, VS_MAP, VS_RES, VS_SKILL, put_utf};

const SM_TIEU_CHUAN: [i64; 22] = [
    1_000, 3_000, 15_000, 40_000, 90_000, 170_000, 340_000, 700_000, 1_500_000, 15_000_000,
    150_000_000, 1_500_000_000, 5_000_000_000, 10_000_000_000, 40_000_000_000, 50_010_000_000,
    60_010_000_000, 70_010_000_000, 80_010_000_000, 100_010_000_000, 1_000_010_000_000,
    10_000_010_000_000,
];

const LEVEL_CAPTIONS_TD: [&str; 22] = [
    "Tân binh", "Tập sự", "Chiến binh", "Đội trưởng", "Siêu cấp 1", "Siêu cấp 2", "Siêu cấp 3",
    "Siêu cấp 4", "Siêu cấp 5", "Siêu cấp 6", "Siêu cấp 7", "Thần cấp 1", "Thần cấp 2",
    "Thần cấp 3", "Thần cấp 4", "Thần cấp 5", "Thần cấp 6", "Thần cấp 7", "Thần cấp 8",
    "Thần cấp 9", "Thần cấp 10", "Thần cấp 11",
];

const LEVEL_CAPTIONS_NM: [&str; 22] = [
    "Tân binh", "Tập sự", "Chiến binh", "Đội trưởng", "Siêu Namec 1", "Siêu Namec 2", "Siêu Namec 3",
    "Siêu Namec 4", "Siêu Namec 5", "Siêu Namec 6", "Siêu Namec 7", "Thần Namec 1", "Thần Namec 2",
    "Thần Namec 3", "Thần Namec 4", "Thần Namec 5", "Thần Namec 6", "Thần Namec 7", "Thần Namec 8",
    "Thần Namec 9", "Thần Namec 10", "Thần Namec 11",
];

const LEVEL_CAPTIONS_XD: [&str; 22] = [
    "Tân binh", "Tập sự", "Chiến binh", "Đội trưởng", "Siêu Xayda 1", "Siêu Xayda 2", "Siêu Xayda 3",
    "Siêu Xayda 4", "Siêu Xayda 5", "Siêu Xayda 6", "Siêu Xayda 7", "Thần Xayda 1", "Thần Xayda 2",
    "Thần Xayda 3", "Thần Xayda 4", "Thần Xayda 5", "Thần Xayda 6", "Thần Xayda 7", "Thần Xayda 8",
    "Thần Xayda 9", "Thần Xayda 10", "Thần Xayda 11",
];

impl DataGameAssets {
    async fn run_blocking<T: Send + 'static>(&self, f: impl FnOnce(Self) -> T + Send + 'static) -> Option<T> {
        let assets = self.clone();
        tokio::task::spawn_blocking(move || f(assets)).await.ok()
    }

    /// Handles any pre-login or in-world asset/template C2S packet, returning `Ok(true)` if consumed.
    pub async fn try_dispatch_packet(
        &self,
        session: &SessionHandle,
        packet: &ClientPacket,
        tpl: &GameTemplates,
        realm_name: &str,
        advertised_host: &str,
        port: u16,
    ) -> Result<bool, ProtocolError> {
        let zoom = session.zoom_level().clamp(1, 4);
        match packet {
            ClientPacket::SetClientType(_) => {
                session.send(Self::pkt_link_ip(realm_name, advertised_host, port)).await?;
            }
            ClientPacket::GetImageSource(0) => {
                let mut pw = PacketWriter::with_capacity(5);
                pw.write_i8(0);
                pw.write_i32(VS_RES);
                session.send(pw.finish(-74)).await?;
            }
            ClientPacket::GetImageSource(1) => {
                let effective_zoom = self.resolve_res_pack_zoom(zoom);
                let count = self.res_pack_count(effective_zoom);
                if count == 0 {
                    error!(sid = session.id(), zoom, assets_dir = %self.root.display(), "missing res/x1..x4 resource packs (-74 mode 1)");
                    send_missing_res_message(session);
                } else {
                    info!(sid = session.id(), zoom = effective_zoom, count, "client requested resource pack size (-74 mode 1)");
                    session.send(self.pkt_size_res(effective_zoom)).await?;
                }
            }
            ClientPacket::GetImageSource(2) => {
                if !self.asset_limiter.check_asset_burst(session.ip()) {
                    return Ok(true);
                }
                if !session.try_mark_res_streamed() {
                    warn!(sid = session.id(), "ignoring duplicate -74 mode 2 resource pack request");
                    return Ok(true);
                }
                let effective_zoom = self.resolve_res_pack_zoom(zoom);
                let count = self.res_pack_count(effective_zoom);
                if count == 0 {
                    error!(sid = session.id(), zoom, assets_dir = %self.root.display(), "missing res/x1..x4 resource packs (-74 mode 2); refusing to send -74 mode 3");
                    send_missing_res_message(session);
                } else {
                    info!(sid = session.id(), zoom = effective_zoom, count, "streaming -74 resource pack to client (-74 mode 2)");
                    self.stream_res_pack(session, effective_zoom).await?;
                }
            }
            ClientPacket::GetImageSource2 => {
                let mut pw = PacketWriter::with_capacity(2);
                pw.write_i16(0);
                session.send(pw.finish(cmd_message::GET_IMAGE_SOURCE2)).await?;
            }
            ClientPacket::GetImageSource(_) => {}
            ClientPacket::UpdateData
            | ClientPacket::UpdateMap
            | ClientPacket::UpdateSkill
            | ClientPacket::UpdateItem => {
                if self.asset_limiter.check_asset_burst(session.ip()) {
                    self.dispatch_bulk_template(session, packet, tpl).await?;
                }
            }
            ClientPacket::UpdateCaption(gender) => {
                session.send(Self::pkt_update_caption(*gender)).await?;
            }
            ClientPacket::RequestMapTemplate(_)
            | ClientPacket::RequestIcon(_)
            | ClientPacket::GetImgByName(_)
            | ClientPacket::GetEffData(_)
            | ClientPacket::BackgroundTemplate(_)
            | ClientPacket::RequestMobTemplate(_)
            | ClientPacket::ClanImageChoose(_)
            | ClientPacket::GetBagEffect(_) => {
                if session.account_id().0 > 0 && self.asset_limiter.check_asset_burst(session.ip()) {
                    self.dispatch_in_world_asset(session, packet, tpl, zoom).await?;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    async fn dispatch_bulk_template(
        &self,
        session: &SessionHandle,
        packet: &ClientPacket,
        tpl: &GameTemplates,
    ) -> Result<(), ProtocolError> {
        match packet {
            ClientPacket::UpdateData => {
                let pkt = match self.tpl_cache.update_data.get() {
                    Some(cached) => Some(cached.clone()),
                    None => self.run_blocking(|a| a.pkt_update_data()).await.flatten(),
                };
                if let Some(pkt) = pkt {
                    session.send(pkt).await?;
                } else {
                    warn!(sid = session.id(), assets_dir = %self.root.display(), "missing update_data/* assets; client cannot finish -87 UpdateData");
                    send_dialog_message(
                        session,
                        "Thiếu thư mục SRC/data/update_data trên server! Hãy copy SRC/data vào thư mục server rồi khởi động lại.",
                    );
                }
            }
            ClientPacket::UpdateMap => {
                let pkt = self.tpl_cache.update_map.get_or_init(|| data_game_tpl::pkt_update_map(tpl)).clone();
                session.send(pkt).await?;
            }
            ClientPacket::UpdateSkill => {
                let pkt = self.tpl_cache.update_skill.get_or_init(|| data_game_tpl::pkt_update_skill(tpl)).clone();
                session.send(pkt).await?;
            }
            ClientPacket::UpdateItem => {
                let pkts = self.tpl_cache.update_item.get_or_init(|| data_game_tpl::pkts_update_item(tpl)).clone();
                for pkt in pkts {
                    session.send(pkt).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn dispatch_in_world_asset(
        &self,
        session: &SessionHandle,
        packet: &ClientPacket,
        tpl: &GameTemplates,
        zoom: u8,
    ) -> Result<(), ProtocolError> {
        match packet {
            ClientPacket::RequestMapTemplate(map_id) => {
                let pkt = match u8::try_from(map_id.0) {
                    Ok(mid) => self.run_blocking(move |a| a.pkt_map_temp(mid)).await.flatten(),
                    Err(_) => None,
                };
                if let Some(pkt) = pkt {
                    session.send(pkt).await?;
                } else {
                    warn!(sid = session.id(), map_id = map_id.0, assets_dir = %self.root.display(), "missing map/tile_map_data asset; dismissing client loading dialog");
                    send_dialog_message(session, "Thiếu dữ liệu bản đồ trên server, vui lòng thử lại sau!");
                }
            }
            ClientPacket::RequestIcon(icon_id) => {
                let id = *icon_id;
                if let Some(pkt) = self.run_blocking(move |a| a.pkt_icon(zoom, id)).await.flatten() {
                    session.send(pkt).await?;
                }
            }
            ClientPacket::GetImgByName(name) => {
                let n_frame = tpl.images_by_name.get(name.as_str()).copied().unwrap_or(0);
                let img_name = name.clone();
                if let Some(pkt) = self.run_blocking(move |a| a.pkt_img_by_name_with_frame(zoom, &img_name, n_frame)).await {
                    session.send(pkt).await?;
                }
            }
            ClientPacket::GetEffData(eff_id) => {
                let (cv, id) = (session.client_version(), eff_id.0);
                if let Some(pkt) = self.run_blocking(move |a| a.pkt_effect_template(zoom, cv, id, id)).await.flatten() {
                    session.send(pkt).await?;
                }
            }
            ClientPacket::BackgroundTemplate(bg_id) => {
                let id = bg_id.0;
                if let Some(pkt) = self.run_blocking(move |a| a.pkt_item_bg_template(zoom, id)).await.flatten() {
                    session.send(pkt).await?;
                }
            }
            ClientPacket::RequestMobTemplate(mob_id) => {
                let id = *mob_id;
                if let Some(pkt) = self.run_blocking(move |a| a.pkt_mob_template(zoom, id)).await.flatten() {
                    session.send(pkt).await?;
                }
            }
            ClientPacket::ClanImageChoose(flag_id) => {
                if let Some(pkt) = data_game_tpl::pkt_flag_bag(tpl, i32::from(*flag_id), true) {
                    session.send(pkt).await?;
                }
            }
            ClientPacket::GetBagEffect(flag_id) => {
                if let Some(pkt) = data_game_tpl::pkt_flag_bag(tpl, i32::from(*flag_id), false) {
                    session.send(pkt).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn resolve_res_pack_zoom(&self, zoom: u8) -> u8 {
        let z = zoom.clamp(1, 4);
        if z == 1 && self.res_pack_count(1) < 200 && self.res_pack_count(2) > 200 {
            return 2;
        }
        [z, 2, 1, 3, 4].into_iter().find(|&c| self.res_pack_count(c) > 0).unwrap_or(z)
    }

    async fn stream_res_pack(&self, session: &SessionHandle, zoom: u8) -> Result<(), ProtocolError> {
        let z = zoom.clamp(1, 4);
        let dir = self.root.join(format!("res/x{z}"));
        for name in &self.res_pack_names[Self::zoom_idx(z)] {
            let path = dir.join(name);
            let Ok(data) = tokio::fs::read(&path).await else {
                warn!(path = %path.display(), "failed reading res pack file during -74 mode 2 stream");
                continue;
            };
            let mut pw = PacketWriter::with_capacity(8 + name.len() + data.len());
            pw.write_i8(2);
            put_utf(&mut pw, name);
            pw.write_i32(data.len() as i32);
            pw.write_bytes(&data);
            session.send(pw.finish(-74)).await?;
        }
        let mut done = PacketWriter::with_capacity(5);
        done.write_i8(3);
        done.write_i32(VS_RES);
        session.send(done.finish(-74)).await
    }

    /// `-41`: `Service.updateCaption` sending power level captions for `gender` (`0` = TD, `1` = NM, `2` = XD).
    #[must_use]
    pub fn pkt_update_caption(gender: i8) -> Packet {
        let captions: &[&str] = match gender {
            1 => &LEVEL_CAPTIONS_NM,
            2 => &LEVEL_CAPTIONS_XD,
            _ => &LEVEL_CAPTIONS_TD,
        };
        let mut pw = PacketWriter::with_capacity(256);
        pw.write_i8(captions.len() as i8);
        for caption in captions {
            put_utf(&mut pw, caption);
        }
        pw.finish(cmd_message::UPDATE_CAPTION)
    }

    /// `-74` mode `1`: `DataGame.sendSizeRes` (`res/x{zoom}` file count).
    #[must_use]
    pub fn pkt_size_res(&self, zoom: u8) -> Packet {
        let count = self.res_pack_names[Self::zoom_idx(zoom)].len().min(i16::MAX as usize) as i16;
        let mut pw = PacketWriter::with_capacity(3);
        pw.write_i8(1);
        pw.write_i16(count);
        pw.finish(-74)
    }

    /// `-74` mode `2` + mode `3`: `DataGame.sendRes` (`res/x{zoom}/*` files + completion trailer).
    #[cfg(test)]
    #[must_use]
    pub fn pkts_res_pack(&self, zoom: u8) -> Vec<Packet> {
        let z = zoom.clamp(1, 4);
        let dir = self.root.join(format!("res/x{z}"));
        let names = &self.res_pack_names[Self::zoom_idx(z)];
        let mut out = Vec::with_capacity(names.len() + 1);
        for name in names {
            let Ok(data) = std::fs::read(dir.join(name)) else {
                continue;
            };
            let mut pw = PacketWriter::with_capacity(8 + name.len() + data.len());
            pw.write_i8(2);
            put_utf(&mut pw, name);
            pw.write_i32(data.len() as i32);
            pw.write_bytes(&data);
            out.push(pw.finish(-74));
        }
        let mut done = PacketWriter::with_capacity(5);
        done.write_i8(3);
        done.write_i32(VS_RES);
        out.push(done.finish(-74));
        out
    }

    /// `-29` subcommand `2`: `DataGame.sendLinkIP` (sanitizing `,` and `:` from `realm_name`).
    #[must_use]
    pub fn pkt_link_ip(realm_name: &str, host: &str, port: u16) -> Packet {
        let clean = realm_name.replace([',', ':'], " ");
        let label = if clean.trim().is_empty() { "NRO" } else { clean.trim() };
        let mut pw = PacketWriter::with_capacity(64);
        pw.write_i8(2);
        put_utf(&mut pw, &format!("{label}:{host}:{port}:0:0:0,0,0"));
        pw.write_i8(1);
        pw.finish(-29)
    }

    /// `-28` subcommand `4`: `DataGame.sendVersionGame`.
    #[must_use]
    pub fn pkt_version_game() -> Packet {
        let mut pw = PacketWriter::with_capacity(192);
        pw.write_i8(4);
        pw.write_i8(VS_DATA);
        pw.write_i8(VS_MAP);
        pw.write_i8(VS_SKILL);
        pw.write_i8(VS_ITEM);
        pw.write_i8(0);
        pw.write_u8(SM_TIEU_CHUAN.len() as u8);
        for &sm in &SM_TIEU_CHUAN {
            pw.write_i64(sm);
        }
        pw.finish(-28)
    }

    /// `-87`: `DataGame.updateData` (`dart`, `arrow`, `effect`, `image`, `part`, `skill`).
    #[must_use]
    pub fn pkt_update_data(&self) -> Option<Packet> {
        if let Some(cached) = self.tpl_cache.update_data.get() {
            return Some(cached.clone());
        }
        let parts = [
            self.read_cached("update_data/dart")?,
            self.read_cached("update_data/arrow")?,
            self.read_cached("update_data/effect")?,
            self.read_cached("update_data/image")?,
            self.read_cached("update_data/part")?,
            self.read_cached("update_data/skill")?,
        ];
        let total: usize = parts.iter().map(Bytes::len).sum();
        let mut pw = PacketWriter::with_capacity(25 + total);
        pw.write_i8(VS_DATA);
        for blob in &parts {
            pw.write_i32(blob.len() as i32);
            pw.write_bytes(blob);
        }
        let pkt = pw.finish(-87);
        let _ = self.tpl_cache.update_data.set(pkt.clone());
        Some(pkt)
    }
}

fn send_dialog_message(session: &SessionHandle, text: &str) {
    let mut pw = PacketWriter::with_capacity(text.len() + 4);
    put_utf(&mut pw, text);
    let _ = session.try_send(pw.finish(cmd_message::DIALOG_MESSAGE));
}

fn send_missing_res_message(session: &SessionHandle) {
    send_dialog_message(
        session,
        "Thiếu thư mục SRC/data/res trên server! Hãy copy SRC/data vào thư mục server rồi khởi động lại.",
    );
}
