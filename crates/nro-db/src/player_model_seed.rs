//! Character creation seed (`NewPlayerSeed`) and [`PlayerSnapshot::from_seed`] starter builder.

use chrono::{DateTime, Utc};
use nro_core::{
    AccountId, ClanId, CoreStats, Gender, ItemInstance, ItemOption, Location, MapId, PlayerId,
    TemplateId, ZoneId,
};
use smallvec::smallvec;

use super::PlayerSnapshot;
use crate::json_columns::*;
use crate::{UnquotedDungeonRecord, gender_to_sql};

impl PlayerSnapshot {
    /// Constructs a brand-new character directly in memory (`PlayerDAO.createNewPlayer`).
    #[must_use]
    pub fn from_seed(id: PlayerId, seed: &NewPlayerSeed) -> Self {
        let g = gender_to_sql(seed.gender);
        let now = seed.now_ms;
        let head = if seed.hair == -1 {
            match g {
                0 => 64,
                1 => 9,
                _ => 6,
            }
        } else {
            seed.hair
        };
        let (hp0, mp0, dame0) = (
            if g == 0 { 200_i64 } else { 100 },
            if g == 1 { 200_i64 } else { 100 },
            if g == 2 { 15_i64 } else { 10 },
        );
        let starter_hp = hp0 + if g == 0 { 30 } else { 20 };
        let mut items_body = vec![None; 11];
        items_body[0] = Some(ItemInstance {
            template_id: TemplateId(i16::from(g)),
            quantity: 1,
            options: smallvec![ItemOption {
                id: 47,
                param: if g == 2 { 3 } else { 2 }
            }],
            create_time_ms: now,
        });
        items_body[1] = Some(ItemInstance {
            template_id: TemplateId(6 + i16::from(g)),
            quantity: 1,
            options: smallvec![ItemOption {
                id: 6,
                param: if g == 0 { 30 } else { 20 }
            }],
            create_time_ms: now,
        });
        let mut items_bag = vec![None; 30];
        items_bag[0] = Some(ItemInstance {
            template_id: TemplateId(63),
            quantity: 10,
            options: smallvec![ItemOption { id: 2, param: 8 }],
            create_time_ms: now,
        });
        let mut items_box = vec![None; 30];
        items_box[0] = Some(ItemInstance {
            template_id: TemplateId(12),
            quantity: 1,
            options: smallvec![ItemOption { id: 14, param: 1 }],
            create_time_ms: now,
        });
        let skill_ids = match g {
            0 => [0, 1, 6, 9, 10, 20, 22, 19],
            1 => [2, 3, 7, 11, 12, 17, 18, 19],
            _ => [4, 5, 8, 13, 14, 21, 23, 19],
        };
        let skills = skill_ids
            .iter()
            .enumerate()
            .map(|(i, &tid)| SkillSlot {
                template_id: tid,
                point: i8::from(i == 0),
                last_time_use_ms: 0,
                curr_level: 0,
            })
            .collect();
        let first_sk = if g == 0 {
            0_i8
        } else if g == 1 {
            2
        } else {
            4
        };
        let mut skills_shortcut = [-1_i8; 10];
        skills_shortcut[0] = first_sk;

        Self {
            id,
            account_id: Some(seed.account_id),
            name: seed.name.clone(),
            head,
            gender: seed.gender,
            have_tennis_space_ship: false,
            clan_id: ClanId(-1),
            wallet: InventoryWallet {
                gold: 2_000,
                gem: if seed.test_mode {
                    1_000_000_000
                } else {
                    10_000
                },
                ..Default::default()
            },
            location: Location {
                map_id: MapId(39 + i16::from(g)),
                zone_id: ZoneId(0),
                x: 350,
                y: 336,
            },
            stats: CoreStats {
                limit_power: 0,
                power: 2_000,
                tiem_nang: 2_000,
                stamina: 1_000,
                max_stamina: 1_000,
                hpg: hp0,
                mpg: mp0,
                dameg: dame0,
                defg: 0,
                critg: 0,
                critdragon: 0,
                hp: starter_hp,
                mp: mp0,
                hp_max: starter_hp,
                mp_max: mp0,
                dame: dame0,
                speed: 5,
                ..CoreStats::default()
            },
            magic_tree: MagicTreeState {
                level: 1,
                curr_peas: 5,
                is_upgrade: false,
                last_time_harvest_ms: now,
                last_time_upgrade_ms: now,
            },
            items_body,
            items_bag,
            items_box,
            items_box_lucky_round: vec![None; 110],
            items_daban: vec![None; 20],
            friends: Vec::new(),
            enemies: Vec::new(),
            intrinsic: IntrinsicState::default(),
            item_time: ItemTimeState::default(),
            main_task: MainTaskProgress {
                task_id: if seed.test_mode { 28 } else { 0 },
                sub_index: 0,
                count: 0,
                last_time_ms: now,
            },
            side_task: DailyTaskProgress::default(),
            clan_task: DailyTaskProgress::default(),
            mabu_egg: None,
            dua_hau_egg: None,
            charms: CharmTimers {
                td_tri_tue_ms: now,
                td_manh_me_ms: now,
                td_da_trau_ms: now,
                td_oai_hung_ms: now,
                td_bat_tu_ms: now,
                td_deo_dai_ms: now,
                td_thu_hut_ms: now,
                td_de_tu_ms: now,
                td_tri_tue3_ms: now,
                td_tri_tue4_ms: now,
            },
            skills,
            skills_shortcut,
            pet: None,
            black_ball_rewards: [BlackBallSlot::default(); 7],
            notify: None,
            mbv_pin: (0, false, 0),
            radar_cards: Vec::new(),
            last_pk_commeson_ms: 0,
            bdkb: (0, 0),
            last_join_doanh_trai_ms: 0,
            cdrd: (false, 0, false, false),
            master_does_not_attack: false,
            nhan_thoi_vang: (false, 0),
            ruong_go: (0, 50_000_000, 100, 0, 0),
            sieu_than_thuy: (false, 0, false),
            vo_dai_sinh_tu: (false, 0, 0, 0),
            last_shenron_ms: 0,
            item_event_counters: [0; 16],
            training: TrainingState {
                map_id_dang_tap: -1,
                ..Default::default()
            },
            vip_state: (0, 0, false, false, false, 0, 0, 0),
            rank: 0,
            achievements: Vec::new(),
            redeemed_giftcodes: Vec::new(),
            event_point: 0,
            data_event: ([0; 6], [false; 4]),
            badges: Vec::new(),
            badge_tasks: Vec::new(),
            first_time_login: DateTime::from_timestamp_millis(now).unwrap_or_else(Utc::now),
            bought_skills: vec![i32::from(first_sk)],
            learn_skill: (-1, -1, 0),
            daily_gifts: Vec::new(),
            point_sukien: 0,
            thachdauwhis: 0,
            point_sukien1: 0,
            point_sukien2: 0,
            point_maydam: 0,
            total_damage_maydam: 0,
            check_nhan_qua: (1, 1, None),
            thanh_tich_bang: UnquotedDungeonRecord::default(),
            thanh_tich_khi_gas: UnquotedDungeonRecord::default(),
            thanh_tich_cdrd: UnquotedDungeonRecord::default(),
            kol_progress: KolProgressData::default(),
        }
    }
}

/// Parameters for creating a brand-new character via [`crate::player_repo::PlayerRepo::create_player`].
#[derive(Debug, Clone)]
pub struct NewPlayerSeed {
    pub account_id: AccountId,
    pub name: String,
    pub gender: Gender,
    pub hair: i16,
    pub test_mode: bool,
    pub now_ms: i64,
}
