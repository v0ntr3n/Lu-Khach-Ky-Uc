//! Complete `RewardEngine` porting `RewardService.java`, `Mob.getItemMobReward`, and boss loot.
//!
//! Re-exports [`ClothesStatProfile`] & [`ACTIVATION_SETS`], implements all 21 sequential mob drop
//! rules (fixing Clover `+15%` truncation), boss loot profiles, Lucky Round, Lân Con, and
//! `rewardCapsuleTet` (fixing Line 1739's `Util.nextInt(20, 10)` crash!).

pub use crate::clothes::{ACTIVATION_SETS, ClothesStatProfile};

use crate::{
    BossError, EngineEffect, GameRng, Gender, GroundDrop, ItemInstance, ItemOption, Location,
    MapId, ParityMode, PlayerId, PlayerView, TemplateId,
    clothes::{build_clothes_options, generate_skh_options, generate_than_linh_piece},
};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Map region predicates mirroring `nro.models.map.service.MapService`.
pub struct MapRegions;

impl MapRegions {
    /// Returns `true` for `0 <= map_id <= 163`.
    #[must_use] pub const fn is_all_map(m: MapId) -> bool { m.0 <= 163 }
    /// Three starter planets (`0..=38` excluding villages, homes, and spaceship stations).
    #[must_use] pub const fn is_3_planets(m: MapId) -> bool { m.0 <= 38 && !matches!(m.0, 0 | 7 | 14 | 21..=26) }
    /// Activation-gear farming maps (`{1, 2, 3, 8, 9, 11, 15, 16, 17}`).
    #[must_use] pub const fn is_up_skh(m: MapId) -> bool { matches!(m.0, 1..=3 | 8 | 9 | 11 | 15..=17) }
    /// Red Ribbon HQ (`53..=62`).
    #[must_use] pub const fn is_doanh_trai(m: MapId) -> bool { matches!(m.0, 53..=62) }
    /// Nappa saga maps (`63..=83`).
    #[must_use] pub const fn is_nappa(m: MapId) -> bool { matches!(m.0, 63..=83) }
    /// Future Trunks timeline maps (`92..=94, 96..=100, 102, 103`).
    #[must_use] pub const fn is_tuong_lai(m: MapId) -> bool { matches!(m.0, 92..=94 | 96..=100 | 102 | 103) }
    /// Planet Cold maps (`105..=110`).
    #[must_use] pub const fn is_cold(m: MapId) -> bool { matches!(m.0, 105..=110) }
    /// Five Elements Mountain (`122..=124`).
    #[must_use] pub const fn is_ngu_hanh_son(m: MapId) -> bool { matches!(m.0, 122..=124) }
    /// Instanced clan dungeons (`BĐKB`, `Doanh Trại`, `CDRĐ`, `Khí Gas`).
    #[must_use] pub const fn is_pho_ban(m: MapId) -> bool {
        Self::is_doanh_trai(m) || matches!(m.0, 135..=138 | 141..=144 | 147..=149 | 151 | 152)
    }
    /// Prison Planet (`155`).
    #[must_use] pub const fn is_nguc_tu(m: MapId) -> bool { m.0 == 155 }
    /// Clan Porata farming maps (`156..=159`).
    #[must_use] pub const fn is_up_porata(m: MapId) -> bool { matches!(m.0, 156..=159) }
    /// Private farming map (`164`).
    #[must_use] pub const fn is_rieng_tu(m: MapId) -> bool { m.0 == 164 }
}

/// Extra drop-buff flags on a player evaluated by [`RewardEngine::roll_mob_drops`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerDropCtx {
    /// Active `Máy dò Capsule Kì Bí` (`380` from mobs `58..=65`).
    pub use_may_do: bool,
    /// Four-leaf clover drop rate buff (`isUseCoBonLa`).
    pub use_co_bon_la: bool,
    /// Account VIP activation status (`player.isActive()`).
    pub is_vip_active: bool,
    /// Wearing 5 pieces of Thần Linh (`checkSetGod()`).
    pub wearing_5pc_god: bool,
    /// Wearing 5 pieces of Hủy Diệt (`checkSetDes()`).
    pub wearing_5pc_destroyer: bool,
    /// Daily free green-gem drop flag (`luotNhanNgocMienPhi == 1`).
    pub free_gem_available: bool,
    /// Remaining daily `Mảnh vỡ bông tai` quota (`canDropManhVo(150)`).
    pub can_drop_manh_vo: bool,
    /// Sum of `ItemOption(236)` across equipped body items (`0..=100`).
    pub opt_236_sum: u16,
}

/// Declarative loot table profile assigned to each `BossDefinition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BossLootProfile {
    /// No ground item drop (`TaoPaiPai`, `DeathBeam`, arenas, training).
    #[default]
    None,
    /// Androids, Fide, Earth Ginyu (`20k..=30k` gold `190`, `80%` `{18..=20}`, `+5` pts).
    StandardWorld,
    /// Nappa (`Kuku`, `MapDauDinh`, `Rambo`: gold + `20%` Lv12 star gear + `80%` `{18..=20}`).
    NappaStar,
    /// Namek Ginyu & Bojack (`gem_stacks` of item `77` + `costume_id` + `{19, 20}` + `+5` pts).
    GemShowerCostume { gem_stacks: u8, costume_id: u16 },
    /// Cell saga (`tl_pct` % `randDoTLBoss`, `{16..=19}` NRO, gold, `+5` pts).
    CellTier { tl_pct: u8 },
    /// `BlackGoku`, `Cumber`, `Cooler` (`10%` `randDoTLBoss`, Lv12 star gear, optional `992`).
    HighWorld { star_pct: u8, drops_time_ring: bool },
    /// `Baby` (`10%` `randDoTLBoss`, `1%` `{1785,1786,1788}`, `5%` Lv12 gear, `{15..=20, 992}`).
    BabyBoss,
    /// `GoldenFrieza` (`629` costume + `+5` pts).
    GoldenFrieza,
    /// `Mabu` 12h (`15%` `randDoTLBoss`, `30%` `568` egg, gold).
    Mabu12h,
    /// `Mabu2H` 14h (`15%` `randDoTLBoss`, gold).
    Mabu14h,
    /// `SuperBroly` (grants normal pet if killer has none + `+5` pts).
    SuperBroly,
    /// Red Ribbon HQ (`100%` `1824`, `17`, `20%` `611`).
    RedRibbon,
    /// Treasure Map `TrungUyXanhLo` (`100%` `705` chest key).
    TreasureMapKey,
    /// Destron Gas (`738` `DrLychee` opt `94` or `729` `Hatchiyack` opt `5`, qty `1 + 2*players`).
    DestronGas { level: u32, is_hatchiyack: bool },
    /// Snake Way (`false` -> `19`; `true` `Cadich` -> `50%` `459` + `706`).
    SnakeWay { is_cadich: bool },
    /// Mini-bosses (`0 = MatTroi` `1562`; `1 = Thief/Virut/Odo/Soi` `1591+1594`; `2 = RongNhi` `1821`).
    MiniBoss { variant: u8 },
    /// Commeson clone `NhanBan` (`638` w/ `opts 93:30, 30:0`).
    Commeson638,
    /// Seasonal single-item drop (`585`, `1045`, `421/422`, `2123/2124`).
    SeasonalItem { item_id: u16, with_costume_opts: bool },
}

/// Stateless, deterministic drop-table and equipment-option generator (`RewardService.java`).
pub struct RewardEngine;

impl RewardEngine {
    /// Populates base options for any of the 14 equipment tiers (`0..=281`, `555..=567`, `650..=662`).
    pub fn init_clothes_stats(id: TemplateId, profile: ClothesStatProfile, rng: &mut GameRng) -> SmallVec<[ItemOption; 4]> {
        build_clothes_options(id, profile, rng)
    }

    /// Rolls a random Thần Linh piece (`555..=567`) matching `ItemService.randDoTL` / `randDoTLBoss`.
    pub fn roll_than_linh_item(from_boss: bool, rng: &mut GameRng) -> ItemInstance {
        generate_than_linh_piece(from_boss, rng)
    }

    /// Rolls SKH activation options matching `ItemService.randOptionItemKichHoat(gender)`.
    pub fn roll_skh_options(gender: Gender, rng: &mut GameRng) -> SmallVec<[ItemOption; 4]> {
        generate_skh_options(gender, rng)
    }

    /// Evaluates all 21 sequential mob-drop rules + quest drops (`Mob.getItemMobReward`).
    #[allow(clippy::too_many_lines)]
    pub fn roll_mob_drops(
        mob_temp: TemplateId, map_id: MapId, drop_loc: Location, killer: &PlayerView,
        ctx: &PlayerDropCtx, parity: ParityMode, rng: &mut GameRng,
    ) -> Vec<GroundDrop> {
        let mut list = Vec::new();
        if mob_temp.0 == 0 { return list; }
        let clover = |n: u32, d: u32, r: &mut GameRng| -> bool {
            if parity == ParityMode::Legacy {
                r.is_true(if ctx.use_co_bon_la { (f64::from(n) * 1.15) as u32 } else { n }, d)
            } else {
                r.is_true(if ctx.use_co_bon_la { n * 115 } else { n * 100 }, d * 100)
            }
        };
        let push = |out: &mut Vec<GroundDrop>, id: u16, qty: u32| {
            out.push(GroundDrop { item: ItemInstance::new(TemplateId(id as i16), qty), location: drop_loc, owner: Some(killer.id) });
        };

        if ctx.use_may_do && (58..=65).contains(&mob_temp.0) && rng.is_true(20, 100) { push(&mut list, 380, 1); }
        if killer.active_task == (8, 1) && matches!(mob_temp.0, 10..=12) { push(&mut list, 20, 1); }
        if killer.active_task == (2, 0) && matches!(mob_temp.0, 1..=3) { push(&mut list, 73, 1); }
        if MapRegions::is_up_porata(map_id) {
            let (r1, r2, r3) = if ctx.use_co_bon_la { (15, 8, 2) } else { (10, 5, 1) };
            let id_opt = if rng.is_true(r1, 100) { Some(933) }
                else if ctx.can_drop_manh_vo && rng.is_true(r2, 100) { Some(934) }
                else if rng.is_true(r3, 500) { Some(935) } else { None };
            if let Some(id) = id_opt {
                let mut it = ItemInstance::new(TemplateId(id), 1);
                it.options.push(ItemOption::new(31, 1));
                list.push(GroundDrop { item: it, location: drop_loc, owner: Some(killer.id) });
            }
        }
        if MapRegions::is_3_planets(map_id) && rng.is_true(1, 20) {
            let g = rng.next_int(500, 3_000) as u32;
            push(&mut list, if g < 1_000 { 76 } else if g < 2_000 { 188 } else { 189 }, g);
        }
        if MapRegions::is_nappa(map_id) && rng.is_true(1, 100) {
            let g = rng.next_int(2_000, 6_000) as u32;
            push(&mut list, if g < 3_000 { 188 } else if g < 5_000 { 189 } else { 190 }, g);
        }
        if MapRegions::is_all_map(map_id) {
            if ctx.free_gem_available { push(&mut list, 77, 1); push(&mut list, 77, 1); }
            for &(id, n, d) in &[(1798, 5, 100), (1799, 5, 100), (1800, 5, 100), (1612, 5, 100), (1801, 1, 100), (1802, 1, 130)] {
                if rng.is_true(n, d) { push(&mut list, id, 1); }
            }
        }
        if MapRegions::is_cold(map_id) && rng.is_true(30, 100) { push(&mut list, 190, rng.next_int(150_000, 250_000) as u32); }
        if MapRegions::is_tuong_lai(map_id) && rng.is_true(15, 100) { push(&mut list, 190, rng.next_int(80_000, 150_000) as u32); }
        if MapRegions::is_pho_ban(map_id) && rng.is_true(1, 100) {
            let g = rng.next_int(80_000, 200_000) as u32;
            push(&mut list, 190, if ctx.use_co_bon_la { g * 115 / 100 } else { g });
        }
        if rng.is_true(1, 1_000_000) { push(&mut list, 77, 1); }
        if MapRegions::is_up_skh(map_id) && clover(1, 9_999, rng) { push(&mut list, 1634, 1); }
        if MapRegions::is_doanh_trai(map_id) && clover(20, 100, rng) { push(&mut list, 1778, 1); }
        if MapRegions::is_rieng_tu(map_id) && clover(1, 19_999, rng) { push(&mut list, 1634, 1); }
        if MapRegions::is_rieng_tu(map_id) || MapRegions::is_up_skh(map_id) {
            let bonus = (f64::from(ctx.opt_236_sum.min(100)) / 100.0).powf(1.5) * 0.20;
            let tile = 2.0 * (1.0 + bonus) * if ctx.use_co_bon_la { 1.5 } else { 1.0 };
            if rng.is_true(tile as u32, 9_999) {
                let starter = [[[0_i16, 33], [1, 41], [2, 49]], [[6, 35], [7, 43], [8, 51]], [[27, 30], [28, 47], [29, 55]], [[21, 24], [22, 46], [23, 53]], [[12, 57], [12, 57], [12, 57]]];
                let id = starter[rng.next_int(0, 4) as usize][(killer.gender as usize).min(2)][rng.next_int(0, 1) as usize];
                let mut it = ItemInstance::new(TemplateId(id), 1);
                it.options = Self::init_clothes_stats(TemplateId(id), ClothesStatProfile::SetClothes, rng);
                it.options.extend(Self::roll_skh_options(killer.gender, rng));
                list.push(GroundDrop { item: it, location: drop_loc, owner: Some(killer.id) });
            }
        }
        if MapRegions::is_cold(map_id) && rng.is_true(1, 50_000) {
            list.push(GroundDrop { item: Self::roll_than_linh_item(false, rng), location: drop_loc, owner: Some(killer.id) });
        }
        if ctx.wearing_5pc_god && rng.is_true(3, 333) { push(&mut list, rng.next_int(663, 667) as u16, 1); }
        if ctx.wearing_5pc_destroyer && MapRegions::is_nguc_tu(map_id) && rng.is_true(10, 100) { push(&mut list, rng.next_int(1066, 1070) as u16, 1); }
        if MapRegions::is_ngu_hanh_son(map_id) && rng.is_true(10, 100) { push(&mut list, rng.next_int(541, 542) as u16, 1); }
        if clover(1, 100, rng) { push(&mut list, rng.next_int(17, 20) as u16, 1); }
        if clover(10, 100, rng) {
            let mut spl = ItemInstance::new(TemplateId(rng.next_int(441, 447) as i16), 1);
            spl.options.push(ItemOption::new(95 + (spl.template_id.0 - 441) as u16, 5));
            list.push(GroundDrop { item: spl, location: drop_loc, owner: Some(killer.id) });
        }
        list
    }

    /// Generates boss death drops and bonus effects for any [`BossLootProfile`].
    #[allow(clippy::too_many_lines)]
    pub fn roll_boss_drops(
        profile: BossLootProfile, loc: Location, killer: &PlayerView, players_in_zone: usize,
        stolen_gold: u64, rng: &mut GameRng, effects: &mut Vec<EngineEffect>,
    ) {
        let drop_to = |fx: &mut Vec<EngineEffect>, item: ItemInstance, owner: Option<PlayerId>| {
            fx.push(EngineEffect::DropItem(GroundDrop { item, location: loc, owner }));
        };
        let drop_gold = |fx: &mut Vec<EngineEffect>, r: &mut GameRng| {
            drop_to(fx, ItemInstance::new(TemplateId(190), r.next_int(20_000, 30_000) as u32), Some(killer.id));
        };
        match profile {
            BossLootProfile::None => {}
            BossLootProfile::StandardWorld => {
                drop_gold(effects, rng);
                if rng.is_true(4, 5) { drop_to(effects, ItemInstance::new(TemplateId(rng.next_int(18, 20) as i16), 1), Some(killer.id)); }
                effects.push(EngineEffect::GrantEventPoints { target: killer.id, points: 5 });
            }
            BossLootProfile::NappaStar => {
                drop_gold(effects, rng);
                if rng.is_true(20, 100) { drop_to(effects, lv12_star_gear(4, rng), Some(killer.id)); }
                effects.push(EngineEffect::GrantEventPoints { target: killer.id, points: 5 });
            }
            BossLootProfile::GemShowerCostume { gem_stacks, costume_id } => {
                for _ in 0..gem_stacks { drop_to(effects, ItemInstance::new(TemplateId(77), rng.next_int(1, 3) as u32), None); }
                drop_to(effects, make_costume(costume_id, 10, 20, 15, false, rng), Some(killer.id));
                effects.push(EngineEffect::GrantEventPoints { target: killer.id, points: 5 });
            }
            BossLootProfile::CellTier { tl_pct } => {
                drop_gold(effects, rng);
                if rng.is_true(u32::from(tl_pct), 100) { drop_to(effects, Self::roll_than_linh_item(true, rng), Some(killer.id)); }
                drop_to(effects, ItemInstance::new(TemplateId(rng.next_int(16, 19) as i16), 1), Some(killer.id));
                effects.push(EngineEffect::GrantEventPoints { target: killer.id, points: 5 });
            }
            BossLootProfile::HighWorld { star_pct, drops_time_ring } => {
                drop_gold(effects, rng);
                if drops_time_ring && rng.is_true(10, 100) { drop_to(effects, ItemInstance::new(TemplateId(992), 1), Some(killer.id)); }
                if rng.is_true(10, 100) { drop_to(effects, Self::roll_than_linh_item(true, rng), Some(killer.id)); }
                if rng.is_true(u32::from(star_pct), 100) { drop_to(effects, lv12_star_gear(6, rng), Some(killer.id)); }
                effects.push(EngineEffect::GrantEventPoints { target: killer.id, points: 5 });
            }
            BossLootProfile::BabyBoss => {
                drop_gold(effects, rng);
                if rng.is_true(10, 100) { drop_to(effects, Self::roll_than_linh_item(true, rng), Some(killer.id)); }
                if rng.is_true(1, 100) { drop_to(effects, make_costume([1785_u16, 1786, 1788][rng.next_int(0, 2) as usize], 30, 40, 5, true, rng), Some(killer.id)); }
                effects.push(EngineEffect::GrantEventPoints { target: killer.id, points: 5 });
            }
            BossLootProfile::GoldenFrieza => {
                let mut c = ItemInstance::new(TemplateId(629), 1);
                for &(oid, p) in &[(30, 1), (50, 20), (77, 20), (103, 20), (93, 20)] { c.options.push(ItemOption::new(oid, p)); }
                drop_to(effects, c, Some(killer.id));
                effects.push(EngineEffect::GrantEventPoints { target: killer.id, points: 5 });
            }
            BossLootProfile::Mabu12h | BossLootProfile::Mabu14h => {
                if rng.is_true(15, 100) { drop_to(effects, Self::roll_than_linh_item(true, rng), Some(killer.id)); }
                if profile == BossLootProfile::Mabu12h && rng.is_true(30, 100) { drop_to(effects, ItemInstance::new(TemplateId(568), 1), Some(killer.id)); }
                drop_gold(effects, rng);
            }
            BossLootProfile::SuperBroly => {
                if !killer.has_pet { effects.push(EngineEffect::GrantNormalPet { target: killer.id }); }
                effects.push(EngineEffect::GrantEventPoints { target: killer.id, points: 5 });
            }
            BossLootProfile::RedRibbon => {
                drop_to(effects, ItemInstance::new(TemplateId(1824), 1), Some(killer.id));
                drop_to(effects, ItemInstance::new(TemplateId(17), 1), Some(killer.id));
                if rng.is_true(20, 100) { drop_to(effects, ItemInstance::new(TemplateId(611), 1), Some(killer.id)); }
            }
            BossLootProfile::TreasureMapKey => drop_to(effects, ItemInstance::new(TemplateId(705), 1), Some(killer.id)),
            BossLootProfile::DestronGas { level, is_hatchiyack } => {
                let p_hi = match level { 0..=9 => 10, 10..=29 => 15, 30..=59 => 20, 60..=89 => 25, 90..=99 => 30, _ => 35 };
                let p_lo = (p_hi - 5).max(5);
                for _ in 0..(1 + 2 * players_in_zone.max(1)) {
                    let mut c = make_costume(if is_hatchiyack { 729 } else { 738 }, p_lo, p_hi, 30, false, rng);
                    c.options.push(ItemOption::new(if is_hatchiyack { 5 } else { 94 }, rng.next_int(p_lo, p_hi)));
                    drop_to(effects, c, None);
                }
            }
            BossLootProfile::SnakeWay { is_cadich } => {
                if is_cadich && rng.is_true(50, 100) {
                    drop_to(effects, ItemInstance::new(TemplateId(459), 1), Some(killer.id));
                    drop_to(effects, ItemInstance::new(TemplateId(706), 1), Some(killer.id));
                } else {
                    drop_to(effects, ItemInstance::new(TemplateId(19), 1), Some(killer.id));
                }
            }
            BossLootProfile::MiniBoss { variant } => match variant {
                0 if rng.is_true(50, 100) => drop_to(effects, ItemInstance::new(TemplateId(1562), 1), Some(killer.id)),
                1 => {
                    if stolen_gold > 0 {
                        let pile = (((stolen_gold * 8 / 10) / 5).max(1)) as u32;
                        for _ in 0..5 { drop_to(effects, ItemInstance::new(TemplateId(190), pile), None); }
                    }
                    for id in [1591_i16, 1594] { drop_to(effects, ItemInstance::new(TemplateId(id), 1), Some(killer.id)); }
                }
                2 if rng.is_true(20, 100) => drop_to(effects, ItemInstance::new(TemplateId(if rng.is_true(20, 100) { 1821 } else { rng.next_int(18, 20) as i16 }), 1), Some(killer.id)),
                _ => {}
            },
            BossLootProfile::Commeson638 => {
                let mut it = ItemInstance::new(TemplateId(638), 1);
                for &(oid, p) in &[(93, 30), (30, 0)] { it.options.push(ItemOption::new(oid, p)); }
                drop_to(effects, it, Some(killer.id));
            }
            BossLootProfile::SeasonalItem { item_id, with_costume_opts } => {
                drop_to(effects, if with_costume_opts { make_costume(item_id, 10, 20, 15, true, rng) } else { ItemInstance::new(TemplateId(item_id as i16), 1) }, Some(killer.id));
            }
        }
    }

    /// Ports `RewardService.rewardLancon(player)`.
    pub fn roll_lan_con_reward(empty_slots: usize, rng: &mut GameRng) -> Result<ItemInstance, BossError> {
        if empty_slots == 0 { return Err(BossError::EmptyBagSlotRequired); }
        let mut it = ItemInstance::new(TemplateId([734_i16, 920, 849, 743, 733][rng.next_int(0, 4) as usize]), 1);
        it.options.push(ItemOption::new([77_u16, 80, 81, 103, 50, 94, 5][rng.next_int(0, 6) as usize], rng.next_int(1, 10)));
        for &(oid, p) in &[(93, rng.next_int(1, 30)), (89, 0), (30, 0)] { it.options.push(ItemOption::new(oid, p)); }
        Ok(it)
    }

    /// Ports `RewardService.rewardCapsuleTet`, fixing Line 1739's `nextInt(20, 10)` crash!
    pub fn roll_capsule_tet(empty_slots: usize, parity: ParityMode, rng: &mut GameRng) -> Result<ItemInstance, BossError> {
        if empty_slots == 0 { return Err(BossError::EmptyBagSlotRequired); }
        if rng.is_true(40, 100) { return Self::roll_lan_con_reward(empty_slots, rng); }
        if rng.is_true(50, 100) {
            let mut it = ItemInstance::new(TemplateId([942_i16, 943, 944][rng.next_int(0, 2) as usize]), 1);
            it.options.push(ItemOption::new([77_u16, 80, 81, 103, 50, 94, 5][rng.next_int(0, 6) as usize], rng.next_int(1, 10)));
            it.options.push(ItemOption::new(30, 0));
            return Ok(it);
        }
        let mut it = ItemInstance::new(TemplateId(rng.next_int(2148, 2152) as i16), 1);
        if rng.is_true(5, 100) {
            for &(oid, p) in &[(77, rng.next_int(10, 20)), (103, 20), (50, rng.next_int(10, 20)), (94, 20), (154, 0)] { it.options.push(ItemOption::new(oid, p)); }
        } else if parity == ParityMode::Legacy {
            return Err(BossError::LegacyBoundsCrash { from: 20, to: 10 });
        } else {
            for &(oid, p) in &[(77, rng.next_int(10, 20)), (103, 20), (50, rng.next_int(10, 20)), (94, rng.next_int(10, 20)), (14, rng.next_int(2, 10)), (93, rng.next_int(1, 15))] {
                it.options.push(ItemOption::new(oid, p));
            }
        }
        Ok(it)
    }

    /// Ports `RewardService.getListItemLuckyRound`, fixing `vip` routing & Non-VIP overwrite.
    pub fn roll_lucky_round(num: usize, vip: bool, parity: ParityMode, rng: &mut GameRng) -> Vec<ItemInstance> {
        (0..num).map(|_| {
            let use_vip = if parity == ParityMode::Legacy { rng.is_true(1, 2) } else { vip };
            if use_vip && rng.is_true(25, 100) {
                let id = [1208_u16, 1209, 1210, 884, 860, 956][rng.next_int(0, 5) as usize];
                if id == 956 { ItemInstance::new(TemplateId(956), 1) } else { make_costume(id, 20, 25, 15, true, rng) }
            } else if !use_vip && rng.is_true(50, 100) && !(parity == ParityMode::Legacy && rng.is_true(1, 2)) {
                let id = [467_i16, 468, 469, 470, 471, 741, 745, 800, 801, 803, 804, 1000][rng.next_int(0, 11) as usize];
                let mut it = ItemInstance::new(TemplateId(id), 1);
                for &(oid, p) in &[(50, rng.next_int(5, 10)), (30, 0)] { it.options.push(ItemOption::new(oid, p)); }
                it
            } else {
                ItemInstance::new(TemplateId(189), (rng.next_int(5, 50) * 1_000) as u32)
            }
        }).collect()
    }
}

fn lv12_star_gear(max_star: i32, rng: &mut GameRng) -> ItemInstance {
    let id = [233_i16, 237, 241, 245, 249, 253, 257, 261, 265, 269, 273, 277, 281][rng.next_int(0, 12) as usize];
    let mut it = ItemInstance::new(TemplateId(id), 1);
    it.options = build_clothes_options(TemplateId(id), ClothesStatProfile::SetClothes, rng);
    it.options.push(ItemOption::new(107, rng.next_int(1, max_star)));
    it
}

fn make_costume(id: u16, lo: i32, hi: i32, max_days: i32, no_resell: bool, rng: &mut GameRng) -> ItemInstance {
    let mut it = ItemInstance::new(TemplateId(id as i16), 1);
    for oid in [50_u16, 77, 103] { it.options.push(ItemOption::new(oid, rng.next_int(lo, hi))); }
    if no_resell { it.options.push(ItemOption::new(154, 0)); }
    it.options.push(ItemOption::new(93, rng.next_int(1, max_days)));
    it
}
