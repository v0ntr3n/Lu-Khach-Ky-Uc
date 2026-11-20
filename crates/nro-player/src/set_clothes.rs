//! Equipment set activation (`SetClothes.java`), seal sets, tier sets, and aura resolvers.
//!
//! Scans equipped body slots `0..=4` (Áo, Quần, Găng, Giày, Rada) and disguise slot `5`
//! (Cải trang) to compute Earth, Namek, and Saiyan 5-piece SKH (`Set Kích Hoạt`) multipliers,
//! 5-piece Seal sets (`Tinh Ấn`, `Nguyệt Ấn`, `Nhật Ấn`, `Linh Thủy Đánh Bạc`), equipment tier
//! sets (`Thần Linh`, `Hủy Diệt`, `Thiên Sứ`), WorldCup 2-piece bonus, and Pirate proximity scaling.

use bitflags::bitflags;
use nro_consts as consts;
use nro_core::{ItemInstance, ParityMode, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::tpl_raw;

/// Number of core armor/radar slots scanned for 5-piece activation sets (`itemsBody[0..5]`).
pub const CORE_ARMOR_SLOTS: usize = 5;

/// Disguise (`Cải trang`) body slot index (`itemsBody[5]`).
pub const DISGUISE_SLOT: usize = 5;

/// Training armor (`Giáp tập luyện`) body slot index (`itemsBody[6]`).
pub const TRAIN_ARMOR_SLOT: usize = 6;

bitflags! {
    /// High-tier full-armor set flags (`Thần Linh`, `Hủy Diệt`, `Thiên Sứ`, and fusion outfits).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct TierSetFlags: u16 {
        /// Full 5-piece Thần Linh (`555..=567`) — unlocks Cold/Future drop tables.
        const THAN_LINH = 1 << 0;
        /// Full 5-piece Hủy Diệt (`650..=662`) — unlocks Whis angel crafting & Ngục Tù bonuses.
        const HUY_DIET = 1 << 1;
        /// Full 5-piece Thiên Sứ (`1048..=1062`).
        const THIEN_SU = 1 << 2;
        /// Cross-paired Gogeta disguises (`2133` + `2134`) on master & pet slot `5` (`+10%` stats).
        const GOGETA_PAIR = 1 << 3;
        /// Cross-paired Supreme Fusion disguises (`1693` + `1553`) triggering outfit `1578/1581/1582`.
        const SUPREME_FUSION_PAIR = 1 << 4;
        /// Thỏ Bulma sexy disguise (`584`) in body slot `5`.
        const THO_BULMA = 1 << 5;
    }
}

/// Five-piece Seal (`Ấn`) counters accumulated across Radar cards, Porata earrings, and body gear.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealSetCounts {
    /// Option `34` (`Tinh Ấn`): `>= 5` grants `+15%` attack damage (`dame`).
    pub tinh_an: u8,
    /// Option `35` (`Nguyệt Ấn`): `>= 5` grants `+15%` maximum KI (`mp_max`).
    pub nguyet_an: u8,
    /// Option `36` (`Nhật Ấn`): `>= 5` grants `+15%` maximum HP (`hp_max`).
    pub nhat_an: u8,
    /// Option `211` (`Linh Thủy Đánh Bạc`): `>= 5` grants immunity to Pet Hakai instant-kill.
    pub linh_thuy_danh_bac: u8,
}

impl SealSetCounts {
    /// Increments the matching seal counter if `option_id` is a seal option (`34, 35, 36, 211`).
    #[inline]
    pub fn record_option(&mut self, option_id: u16) {
        match option_id {
            34 => self.tinh_an = self.tinh_an.saturating_add(1),
            35 => self.nguyet_an = self.nguyet_an.saturating_add(1),
            36 => self.nhat_an = self.nhat_an.saturating_add(1),
            211 => self.linh_thuy_danh_bac = self.linh_thuy_danh_bac.saturating_add(1),
            _ => {}
        }
    }

    /// Returns `true` when `Tinh Ấn` (`+15%` damage) is active (`>= 5` pieces).
    #[inline]
    pub const fn has_tinh_an(self) -> bool {
        self.tinh_an >= 5
    }

    /// Returns `true` when `Nguyệt Ấn` (`+15%` MP) is active (`>= 5` pieces).
    #[inline]
    pub const fn has_nguyet_an(self) -> bool {
        self.nguyet_an >= 5
    }

    /// Returns `true` when `Nhật Ấn` (`+15%` HP) is active (`>= 5` pieces).
    #[inline]
    pub const fn has_nhat_an(self) -> bool {
        self.nhat_an >= 5
    }

    /// Returns `true` when `Linh Thủy Đánh Bạc` (Hakai immunity) is active (`>= 5` pieces).
    #[inline]
    pub const fn has_linh_thuy_danh_bac(self) -> bool {
        self.linh_thuy_danh_bac >= 5
    }
}

/// Complete port of `nro.models.player.SetClothes`, tracking planetary SKH and tier sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetClothes {
    /// Earth Songoku set count (options `129, 141`): `5` pieces => `+100%` Kamejoko damage.
    pub songoku: u8,
    /// Earth Thiên Xin Hăng set count (options `127, 139`): `5` pieces => `2x` TDHS stun time.
    pub thien_xin_hang: u8,
    /// Earth Kirin set count (options `128, 140`): `5` pieces => `2x` Quả Cầu Kênh Khi damage.
    pub kirin: u8,
    /// Earth Kaioken visual set count (option `253`): `5` pieces => aura `1200`.
    pub kaioken: u8,
    /// Earth Thần Vũ Trụ Kaio set count (options `245..=248`): crit, Kaioken HP cost & `+30%` dame.
    pub than_vu_tru_kaio: u8,
    /// Namek Ốc Tiêu set count (options `131, 143`): `5` pieces => `+100%` Liên Hoàn damage.
    pub oc_tieu: u8,
    /// Namek Pikkoro Daimao set count (options `132, 144`): `5` pieces => `4x` permanent `MobMe`.
    pub pikkoro_daimao: u8,
    /// Namek Picolo set count (options `130, 142`): `5` pieces => `1.5x` Makankosappo (`ParityMode`).
    pub picolo: u8,
    /// Namek Liên Hoàn set count (option `250`).
    pub lien_hoan: u8,
    /// Namek Nail set count (options `237..=240`): crit dame, Masenko CDR, and `+80%` Masenko dame.
    pub nail: u8,
    /// Saiyan Kakarot set count (options `133, 136`): `5` pieces => `+100%` Galick damage.
    pub kakarot: u8,
    /// Saiyan Cadic set count (options `134, 137`): `5` pieces => `5x` Biến Khỉ duration.
    pub cadic: u8,
    /// Saiyan Nappa set count (options `135, 138`): `5` pieces => `+80%` `hp_max`.
    pub nappa: u8,
    /// Common damage-reduction SKH set count (options `252, 255`).
    pub giam_sat_thuong: u8,
    /// Saiyan Cadic-M set count (options `241..=244`): `+20%` HP, Tự Sát range & `+20%/+50%` damage.
    pub cadic_m: u8,
    /// WorldCup item count (`966, 982, 983, 883, 904`): `== 2` grants `+10%` HP, MP, and Dame.
    pub worldcup: u8,
    /// Hủy Diệt 80B-power requirement piece count (option `21` with `param == 80`).
    pub set_dhd: u8,
    /// Equipped Pirate disguise template ID (`618..=624, 626, 627`), or `None`.
    pub ct_hai_tac: Option<u16>,
    /// Active full-tier and fusion-pair flags (`Thần Linh`, `Hủy Diệt`, `Thiên Sứ`, `Gogeta`).
    pub tiers: TierSetFlags,
}

impl SetClothes {
    /// Recomputes all set counts and tier flags from `body` and optional `pet_body`.
    ///
    /// Mirrors `SetClothes.setup()`, `setupSKT()`, `NPoint.setOutfitFusion()`, and `setThoBulma()`.
    pub fn recompute(
        body: &[Option<ItemInstance>],
        pet_body: Option<&[Option<ItemInstance>]>,
        is_bot: bool,
    ) -> Self {
        let _ = consts::PER100;
        let mut out = Self::default();
        let mut all_god = !is_bot && body.len() >= CORE_ARMOR_SLOTS;
        let mut all_des = body.len() >= CORE_ARMOR_SLOTS;
        let mut all_angel = body.len() >= CORE_ARMOR_SLOTS;

        for slot in 0..CORE_ARMOR_SLOTS {
            if let Some(Some(item)) = body.get(slot) {
                let id = tpl_raw(item.template_id);
                all_god &= is_than_linh_id(id);
                all_des &= is_huy_diet_id(id);
                all_angel &= is_thien_su_id(id);
                out.scan_skh_slot(item);
            } else {
                all_god = false;
                all_des = false;
                all_angel = false;
            }
        }

        if all_god {
            out.tiers.insert(TierSetFlags::THAN_LINH);
        }
        if all_des {
            out.tiers.insert(TierSetFlags::HUY_DIET);
        }
        if all_angel {
            out.tiers.insert(TierSetFlags::THIEN_SU);
        }

        for item in body.iter().flatten() {
            if matches!(tpl_raw(item.template_id), 966 | 982 | 983 | 883 | 904) {
                out.worldcup = out.worldcup.saturating_add(1);
            }
        }

        if let Some(Some(ct)) = body.get(DISGUISE_SLOT) {
            let ct_id = tpl_raw(ct.template_id);
            if matches!(ct_id, 618..=624 | 626 | 627) {
                out.ct_hai_tac = Some(ct_id);
            }
            if ct_id == 584 {
                out.tiers.insert(TierSetFlags::THO_BULMA);
            }
            if let Some(p_body) = pet_body
                && let Some(Some(p_ct)) = p_body.get(DISGUISE_SLOT)
            {
                let p_id = tpl_raw(p_ct.template_id);
                if (ct_id == 2133 && p_id == 2134) || (ct_id == 2134 && p_id == 2133) {
                    out.tiers.insert(TierSetFlags::GOGETA_PAIR);
                }
                if matches!(ct_id, 1693 | 1553) && matches!(p_id, 1693 | 1553) && ct_id != p_id {
                    out.tiers.insert(TierSetFlags::SUPREME_FUSION_PAIR);
                }
            }
        }

        out
    }

    /// Scans a single armor piece (`0..=4`), breaking on the first matching SKH option (`setupSKT`).
    fn scan_skh_slot(&mut self, item: &ItemInstance) {
        for opt in &item.options {
            let mut matched_skh = true;
            match opt.id {
                129 | 141 => self.songoku += 1,
                127 | 139 => self.thien_xin_hang += 1,
                128 | 140 => self.kirin += 1,
                131 | 143 => self.oc_tieu += 1,
                132 | 144 => self.pikkoro_daimao += 1,
                130 | 142 => self.picolo += 1,
                135 | 138 => self.nappa += 1,
                133 | 136 => self.kakarot += 1,
                134 | 137 => self.cadic += 1,
                253 => self.kaioken += 1,
                250 => self.lien_hoan += 1,
                252 | 255 => self.giam_sat_thuong += 1,
                245..=248 => self.than_vu_tru_kaio += 1,
                237..=240 => self.nail += 1,
                241..=244 => self.cadic_m += 1,
                21 => {
                    matched_skh = false;
                    if opt.param == 80 {
                        self.set_dhd += 1;
                    }
                }
                _ => matched_skh = false,
            }
            if matched_skh {
                break;
            }
        }
    }

    /// Returns `true` if wearing full 5-piece Thần Linh (`SetClothes.checkSetGod`).
    #[inline]
    pub const fn check_set_god(&self) -> bool {
        self.tiers.contains(TierSetFlags::THAN_LINH)
    }

    /// Returns `true` if wearing full 5-piece Hủy Diệt (`SetClothes.checkSetDes`).
    #[inline]
    pub const fn check_set_des(&self) -> bool {
        self.tiers.contains(TierSetFlags::HUY_DIET)
    }

    /// Returns `true` if wearing full 5-piece Thiên Sứ (`Item.isDTS`).
    #[inline]
    pub const fn check_set_angel(&self) -> bool {
        self.tiers.contains(TierSetFlags::THIEN_SU)
    }

    /// Returns the Kaioken self-HP cost percentage (`10%` base, `5%` at 4pc, `3%` at 5pc Kaio).
    #[inline]
    pub const fn kaioken_hp_cost_pct(&self) -> i64 {
        match self.than_vu_tru_kaio {
            5.. => 3,
            4 => 5,
            _ => 10,
        }
    }

    /// Returns extra `Tự Sát` explosion radius (`+200` at `cadicM == 2`, or `>= 2` in `Fixed` mode).
    #[inline]
    pub const fn tu_sat_bonus_range(&self, parity: ParityMode) -> i32 {
        match parity {
            ParityMode::Fixed if self.cadic_m >= 2 => 200,
            _ if self.cadic_m == 2 => 200,
            _ => 0,
        }
    }

    /// Returns `Tự Sát` bonus damage percentage of `hp_max` (`20%` at 4pc, `50%` at 5pc `cadicM`).
    #[inline]
    pub const fn tu_sat_bonus_hp_max_pct(&self) -> i64 {
        match self.cadic_m {
            5.. => 50,
            4 => 20,
            _ => 0,
        }
    }

    /// Returns visual effect IDs emitted by `Service.sendEffPlayer` for 5-piece SKH sets.
    pub fn visual_effect_ids(&self) -> SmallVec<[u16; 2]> {
        let mut effs = SmallVec::new();
        if self.songoku >= 5 || self.kaioken >= 5 || self.kirin >= 5 || self.thien_xin_hang >= 5 {
            effs.push(1200);
        } else if self.oc_tieu >= 5 || self.pikkoro_daimao >= 5 || self.picolo >= 5 {
            effs.push(10277);
        } else if self.than_vu_tru_kaio >= 5 {
            effs.push(10277);
            effs.push(5017);
        } else if self.kakarot >= 5 || self.nappa >= 5 || self.cadic >= 5 {
            effs.push(1202);
        }
        effs
    }
}

/// Checks if `id` is a Thần Linh equipment template (`555..=567`).
#[inline]
pub const fn is_than_linh_id(id: u16) -> bool {
    id >= 555 && id <= 567
}

/// Checks if `id` is a Hủy Diệt equipment template (`650..=662`).
#[inline]
pub const fn is_huy_diet_id(id: u16) -> bool {
    id >= 650 && id <= 662
}

/// Checks if `id` is a Thiên Sứ equipment template (`1048..=1062`).
#[inline]
pub const fn is_thien_su_id(id: u16) -> bool {
    id >= 1048 && id <= 1062
}

/// Computes the Pirate Crew (`Cải trang Hải Tặc`, `EffectSkin.updateCTHaiTac`) stat `%` bonus.
///
/// Counts unique Pirate disguise IDs (`618..=627`) worn by self and allies within `300` px,
/// returning `unique_count * 3` (`+3%..+30%` for options `147`, `77`, `103`).
pub fn pirate_crew_bonus_pct(self_ct: Option<u16>, nearby_pirate_cts: &[u16]) -> i32 {
    let Some(own) = self_ct else {
        return 0;
    };
    let mut seen = [false; 10];
    if (618..=627).contains(&own) {
        seen[usize::from(own - 618)] = true;
    }
    for &ct in nearby_pirate_cts {
        if (618..=627).contains(&ct) {
            seen[usize::from(ct - 618)] = true;
        }
    }
    (seen.iter().filter(|&&b| b).count() as i32) * 3
}

/// Computes the equipment upgrade front-aura tier (`Player.getEffFront`, option `72` across `0..5`).
pub fn equipment_upgrade_eff_front(body: &[Option<ItemInstance>]) -> Option<u8> {
    if body.len() < 10 {
        return None;
    }
    let mut min_level = u8::MAX;
    for slot in 0..CORE_ARMOR_SLOTS {
        let item = body.get(slot)?.as_ref()?;
        let lvl = item.options.iter().find(|o| o.id == 72).and_then(|o| u8::try_from(o.param).ok())?;
        min_level = min_level.min(lvl);
    }
    match min_level {
        8.. => Some(8),
        4..=7 => Some(min_level),
        _ => None,
    }
}

/// Returns the Training Armor (`Giáp tập luyện`) percentage modifier (`ItemService.getPercentTrainArmor`).
#[inline]
pub const fn train_armor_percent(template_id: TemplateId) -> i32 {
    match tpl_raw(template_id) {
        529 | 534 => 10,
        530 | 535 => 20,
        531 | 536 => 30,
        1716 => 40,
        _ => 0,
    }
}
