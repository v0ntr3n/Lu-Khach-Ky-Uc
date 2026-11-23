#![forbid(unsafe_code)]
//! Library surface of `nro-server`, exposing `ServerRuntime`, `ServerConfig`, and `AdminControl`
//! for both the headless `nro-server` CLI binary and the embedded `nro-admin-ui` desktop panel.

pub mod admin_control;
pub mod config;
pub mod data_game;
pub mod data_game_tpl;
pub mod pgo_bench;
pub mod router;
pub(crate) mod router_pet;
pub mod runtime;
pub mod state;

#[cfg(test)]
mod tests {
    use crate::config::ServerConfig;
    use crate::data_game::DataGameAssets;
    use crate::data_game_tpl;
    use nro_core::{AccountId, Gender, MapId, PlayerId};
    use nro_db::player_repo::{NewPlayerSeed, PlayerSnapshot};
    use nro_db::templates::{
        GameTemplates, ItemTemplateRecord, MapTemplateRecord, SkillTemplateRecord,
    };
    use nro_protocol::is_extended_s2c_cmd;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn data_game_streams_all_asset_and_template_opcodes_cleanly() -> anyhow::Result<()> {
        let tmp = std::env::temp_dir().join(format!("nro_assets_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        for sub in [
            "update_data",
            "res/x2",
            "icon/x2",
            "effdata",
            "effect/x2",
            "img_by_name/x2",
            "item_bg_temp/x2",
            "mob/x2",
            "map/tile_map_data",
            "map/item_bg_map_data",
            "map/eff_map",
        ] {
            fs::create_dir_all(tmp.join(sub))?;
        }
        for part in ["dart", "arrow", "effect", "image", "part", "skill"] {
            fs::write(tmp.join("update_data").join(part), b"\x01\x02\x03")?;
        }
        fs::write(tmp.join("res/x2/0"), b"RES0")?;
        fs::write(tmp.join("icon/x2/4028.png"), b"\x89PNG_ICON")?;
        fs::write(tmp.join("effdata/DataEffect_60"), b"EFF60")?;
        fs::write(tmp.join("effect/x2/ImgEffect_60.png"), b"PNG60")?;
        fs::write(tmp.join("img_by_name/x2/mount_12_0.png"), b"MOUNT")?;
        fs::write(tmp.join("item_bg_temp/x2/5.png"), b"BG5")?;
        fs::write(tmp.join("mob/x2/1"), b"MOB1")?;
        fs::write(tmp.join("map/tile_set_info"), b"TILESET")?;
        fs::write(tmp.join("map/tile_map_data/21"), b"TILEMAP21")?;
        fs::write(tmp.join("map/item_bg_map_data/21"), b"\x00\x00")?;
        fs::write(tmp.join("map/eff_map/21"), b"\x00\x00")?;

        let assets = DataGameAssets::open(&tmp);
        let mut tpl = GameTemplates::default();
        tpl.images_by_name.insert("mount_12_0".to_owned(), 4);
        tpl.flag_bags
            .push((1, "Cờ Đỏ".to_owned(), 1000, 0, 2001, vec![2002, 2003]));
        tpl.skills.push(SkillTemplateRecord {
            nclass_id: 0,
            id: 0,
            name: "Đấm Dragon".to_owned(),
            max_point: 7,
            mana_use_type: 0,
            skill_type: 1,
            icon_id: 539,
            dam_info: "100%".to_owned(),
            slot: 0,
            levels: Vec::new(),
        });
        tpl.maps.push(MapTemplateRecord {
            id: MapId(21),
            name: "Nhà Gôhan".to_owned(),
            zones: 1,
            max_player: 15,
            map_type: 0,
            planet_id: 0,
            bg_type: 0,
            tile_id: 1,
            bg_id: 0,
            waypoints: Vec::new(),
            mobs: Vec::new(),
            npcs: Vec::new(),
        });
        for idx in 0..900_i32 {
            tpl.items.push(ItemTemplateRecord {
                id: idx,
                item_type: 0,
                gender: 0,
                name: format!("Item {idx}"),
                description: Some("Mô tả".to_owned()),
                level: 1,
                icon_id: 390,
                part: 1,
                is_up_to_up: false,
                power_require: 100,
                gold: 0,
                gem: 0,
                head: -1,
                body: -1,
                leg: -1,
            });
        }

        assert_eq!(assets.pkt_size_res(2).cmd, -74);
        let res_pack = assets.pkts_res_pack(2);
        assert_eq!(res_pack.len(), 2);
        assert!(is_extended_s2c_cmd(res_pack[0].cmd));
        assert_eq!(assets.pkt_update_data().expect("-87").cmd, -87);
        assert_eq!(assets.pkt_icon(2, 4028).expect("-67").cmd, -67);
        assert_eq!(
            assets.pkt_effect_template(2, 231, 60, 60).expect("-66").cmd,
            -66
        );
        assert_eq!(assets.pkt_img_by_name(&tpl, 2, "mount_12_0").cmd, 66);
        assert_eq!(
            assets
                .pkt_img_by_name(&tpl, 2, "../etc/passwd")
                .reader()
                .remaining(),
            2 + 13 + 1 + 4
        );
        assert_eq!(assets.pkt_item_bg_template(2, 5).expect("-32").cmd, -32);
        assert_eq!(assets.pkt_mob_template(2, 1).expect("11").cmd, 11);
        assert_eq!(assets.pkt_tile_set_info().expect("-82").cmd, -82);
        assert_eq!(assets.pkt_map_temp(21).expect("-28:10").cmd, -28);
        assert_eq!(
            DataGameAssets::pkt_link_ip("NRO", "127.0.0.1", 14445).cmd,
            -29
        );
        let item_pkts = data_game_tpl::pkts_update_item(&tpl);
        assert_eq!(item_pkts.len(), 3);
        assert_eq!(item_pkts[2].cmd, 12);
        assert!(is_extended_s2c_cmd(item_pkts[2].cmd));
        assert_eq!(
            data_game_tpl::pkt_flag_bag(&tpl, 1, true).expect("-62").cmd,
            -62
        );
        assert_eq!(
            data_game_tpl::pkt_flag_bag(&tpl, 1, false)
                .expect("-63")
                .cmd,
            -63
        );

        let snap = PlayerSnapshot::from_seed(
            PlayerId(1),
            &NewPlayerSeed {
                account_id: AccountId(1),
                name: "goku01".to_owned(),
                gender: Gender::TraiDat,
                hair: 0,
                test_mode: false,
                now_ms: 0,
            },
        );
        let world_pkts = data_game_tpl::pkts_client_ok(&assets, &tpl, &snap);
        assert_eq!(
            [
                world_pkts[0].cmd,
                world_pkts[1].cmd,
                world_pkts[2].cmd,
                world_pkts[3].cmd
            ],
            [-30, -64, 40, -24]
        );

        let _ = fs::remove_dir_all(&tmp);
        Ok(())
    }

    #[tokio::test]
    async fn bootstrap_discovers_nested_sql_and_formats_clear_missing_hint() -> anyhow::Result<()> {
        let tmp = std::env::temp_dir().join(format!("nro_boot_sql_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp)?;

        let mut cfg = ServerConfig::default();
        cfg.storage.path = tmp.join("data/nro.fjall");
        cfg.storage.bootstrap_sql_path = Some(PathBuf::from("database team2026.sql"));

        let err = crate::runtime::open_or_bootstrap_store_in(&cfg, &tmp)
            .await
            .expect_err("empty store without SQL should return clear hint");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("is empty and no SQL seed dump was found"),
            "unexpected error: {msg}"
        );
        assert!(
            !msg.contains("achievement_template"),
            "raw table list leaked: {msg}"
        );

        let nested_zip = tmp.join("teamobi2026_databases (1)");
        fs::create_dir_all(&nested_zip)?;
        let sql_file = nested_zip.join("database team2026.sql");
        fs::write(&sql_file, b"-- sql stub")?;
        let discovered = crate::runtime::discover_bootstrap_sql_in(
            &tmp,
            cfg.storage.bootstrap_sql_path.as_deref(),
        );
        assert_eq!(discovered, Some(sql_file));

        let _ = fs::remove_dir_all(&tmp);
        Ok(())
    }
}
