//! Complete 14-tier x 5-slot equipment base-stat tables and SKH / Thần Linh generators.
//!
//! Reconciles `RewardService.SetClothes`, `RewardService.initBaseOptionClothes`, and
//! `ItemService.randDoTL` / `randDoTLBoss` across all 196 item IDs (`0..=281`, Thần Linh
//! `555..=567`, and Hủy Diệt `650..=662`).

use crate::{GameRng, Gender, ItemInstance, ItemOption, TemplateId};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Classic `RewardService.ACTIVATION_SET` (`[gender][0..3] -> (set_opt_id, effect_opt_id)`).
pub const ACTIVATION_SETS: [[(u16, u16); 3]; 3] = [
    [(129, 141), (127, 139), (128, 140)], // Earth: Songoku, Thien Xin Hang, Kirin
    [(131, 143), (132, 144), (130, 142)], // Namek: Oc Tieu, Pikkoro Daimao, Picolo
    [(135, 138), (133, 136), (134, 137)], // Saiyan: Kakarot, Cadic, Nappa
];

/// Stat variance & Thần Linh reconciliation profile for equipment generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ClothesStatProfile {
    /// `SetClothes`: `±10%` variance (fixed `opt 21`), includes Tier 14 Hủy Diệt (`650..=662`).
    #[default]
    SetClothes,
    /// `initBaseOptionClothes`: `+1%..+10%` upward variance across all options.
    ShopInitBase,
    /// Reconciled `randDoTL` / `randDoTLBoss` (`100..=115%` with `opt 206`/`207` tag).
    DropThanLinh {
        /// `true` for boss drops (`opt 207`), `false` for Cold mob drops (`opt 206`).
        from_boss: bool,
    },
}

/// Generates base options for any of the 14 equipment tiers (`0..=281`, `555..=567`, `650..=662`).
pub fn build_clothes_options(
    temp_id: TemplateId,
    profile: ClothesStatProfile,
    rng: &mut GameRng,
) -> SmallVec<[ItemOption; 4]> {
    let mut opts = SmallVec::new();
    let Some((opt_a, val_a, opt_b, req_pow, untradeable)) = lookup_clothes_base(temp_id.0 as u16, profile) else {
        return opts;
    };
    match profile {
        ClothesStatProfile::SetClothes => {
            opts.push(ItemOption::new(opt_a, jitter_pm10(val_a, rng)));
            if let Some((ob, vb)) = opt_b {
                opts.push(ItemOption::new(ob, jitter_pm10(vb, rng)));
            }
            if let Some(p) = req_pow {
                opts.push(ItemOption::new(21, p));
            }
            if untradeable {
                opts.push(ItemOption::new(30, 1));
            }
        }
        ClothesStatProfile::ShopInitBase => {
            let scale = |v: i32, r: &mut GameRng| (i64::from(v) * i64::from(r.next_int(101, 110)) / 100) as i32;
            opts.push(ItemOption::new(opt_a, scale(val_a, rng)));
            if let Some((ob, vb)) = opt_b {
                opts.push(ItemOption::new(ob, scale(vb, rng)));
            }
            if let Some(p) = req_pow {
                opts.push(ItemOption::new(21, scale(p, rng)));
            }
        }
        ClothesStatProfile::DropThanLinh { from_boss } => {
            let pct = rng.next_int(100, 115);
            opts.push(ItemOption::new(opt_a, val_a * pct / 100));
            if let Some((ob, vb)) = opt_b {
                opts.push(ItemOption::new(ob, vb * pct / 100));
            }
            if pct > 100 && temp_id.0 != 561 {
                opts.push(ItemOption::new(if from_boss { 207 } else { 206 }, pct - 100));
            }
            let bonus_proc = if from_boss {
                rng.is_true(30, 100)
            } else {
                rng.is_true(30, 100) && rng.is_true(70, 100)
            };
            if bonus_proc {
                opts.push(ItemOption::new(86 + rng.next_int(0, 1) as u16, 0));
            }
            opts.push(ItemOption::new(21, rng.next_int(15, 17)));
        }
    }
    opts
}

/// Rolls a random Thần Linh piece (`555..=567`) matching `ItemService.randDoTL` / `randDoTLBoss`.
pub fn generate_than_linh_piece(from_boss: bool, rng: &mut GameRng) -> ItemInstance {
    let id = if rng.is_true(10, 100) {
        561
    } else if rng.is_true(25, 100) {
        [562, 564, 566][rng.next_int(0, 2) as usize]
    } else if rng.is_true(45, 100) {
        [556, 558, 560][rng.next_int(0, 2) as usize]
    } else if rng.is_true(75, 100) {
        [555, 557, 559][rng.next_int(0, 2) as usize]
    } else {
        [563, 565, 567][rng.next_int(0, 2) as usize]
    };
    let mut it = ItemInstance::new(TemplateId(id), 1);
    it.options = build_clothes_options(TemplateId(id), ClothesStatProfile::DropThanLinh { from_boss }, rng);
    it
}

/// Rolls SKH activation options matching `ItemService.randOptionItemKichHoat(gender)`.
pub fn generate_skh_options(gender: Gender, rng: &mut GameRng) -> SmallVec<[ItemOption; 4]> {
    let g = (gender as usize).min(2);
    let mut out = SmallVec::new();
    if rng.is_true(30, 100) {
        let base = match g {
            0 => 245_u16,
            1 => 237,
            _ => 241,
        };
        for off in 0..4 {
            out.push(ItemOption::new(base + off, 0));
        }
    } else {
        let (a, b) = if rng.is_true(50, 100) {
            [(128, 140), (130, 142), (134, 137)][g]
        } else if rng.is_true(50, 100) {
            [(127, 139), (131, 143), (135, 138)][g]
        } else if rng.is_true(50, 100) {
            (233, 234)
        } else {
            [(129, 141), (132, 144), (133, 136)][g]
        };
        out.push(ItemOption::new(a, 0));
        out.push(ItemOption::new(b, 0));
    }
    out.push(ItemOption::new(30, 0));
    out
}

fn jitter_pm10(val: i32, rng: &mut GameRng) -> i32 {
    let span = val * 10 / 100;
    val + rng.next_int(-span, span)
}

/// Compact 14-tier x 5-slot clothes lookup `(opt_a, val_a, opt_b, req_pow, untradeable)`.
#[allow(clippy::too_many_lines)]
pub const fn lookup_clothes_base(
    id: u16,
    profile: ClothesStatProfile,
) -> Option<(u16, i32, Option<(u16, i32)>, Option<i32>, bool)> {
    let is_drop = matches!(profile, ClothesStatProfile::DropThanLinh { .. });
    let is_shop = matches!(profile, ClothesStatProfile::ShopInitBase);
    match id {
        // Áo (opt 47): Tiers 1..=12
        0 | 1 => Some((47, 2, None, None, false)),
        2 => Some((47, 3, None, None, false)),
        33 | 41 => Some((47, 4, None, None, false)),
        49 => Some((47, 5, None, None, false)),
        3 | 4 => Some((47, 8, None, None, false)),
        5 => Some((47, 10, None, None, false)),
        34 | 42 => Some((47, 16, None, None, false)),
        50 => Some((47, 20, None, None, false)),
        136 | 152 => Some((47, 24, None, None, false)),
        168 => Some((47, 30, None, None, false)),
        137 | 153 => Some((47, 40, None, None, false)),
        169 => Some((47, 50, None, None, false)),
        138 | 154 => Some((47, 60, None, None, false)),
        170 => Some((47, 70, None, None, false)),
        139 | 155 => Some((47, 90, None, None, false)),
        171 => Some((47, 100, None, None, false)),
        230..=233 => Some((47, [200, 250, 300, 400][(id - 230) as usize], None, None, false)),
        234..=237 => Some((47, [200, 250, 300, 400][(id - 234) as usize], None, None, false)),
        238..=241 => Some((47, [230, 280, 330, 450][(id - 238) as usize], None, None, false)),
        // Quần (opt 6, opt 27): Tiers 1..=12
        6 => Some((6, 30, None, None, false)),
        7 | 8 => Some((6, 20, None, None, false)),
        35 => Some((6, 150, Some((27, 12)), None, false)),
        43 => Some((6, 25, Some((27, 10)), None, false)),
        51 => Some((6, 20, Some((27, 8)), None, false)),
        9 => Some((6, 300, Some((27, 40)), None, false)),
        10 => Some((6, 120, Some((27, 28)), None, false)),
        11 => Some((6, 100, Some((27, 20)), None, false)),
        36 => Some((6, 600, Some((27, 120)), None, false)),
        44 => Some((6, 250, Some((27, 100)), None, false)),
        52 => Some((6, 200, Some((27, 80)), None, false)),
        140..=143 => {
            let k = (id - 140) as usize;
            Some((6, [1400, 3000, 6000, 10000][k], Some((27, [280, 600, 1200, 2000][k])), None, false))
        }
        156..=159 => {
            let k = (id - 156) as usize;
            Some((6, [600, 1200, 2400, 4800][k], Some((27, [240, 480, 960, 1800][k])), None, false))
        }
        172..=175 => {
            let k = (id - 172) as usize;
            Some((6, [500, 1000, 2000, 4000][k], Some((27, [200, 400, 800, 1600][k])), None, false))
        }
        242..=245 => {
            let k = (id - 242) as usize;
            Some((6, 14000 + (k as i32) * 4000, Some((27, 2500 + (k as i32) * 500)), None, false))
        }
        246..=249 => {
            let k = (id - 246) as usize;
            Some((6, 13000 + (k as i32) * 4000, Some((27, 2200 + (k as i32) * 500)), None, false))
        }
        250..=253 => {
            let k = (id - 250) as usize;
            Some((6, 12000 + (k as i32) * 4000, Some((27, 2100 + (k as i32) * 500)), None, false))
        }
        // Găng (opt 0): Tiers 1..=12
        21..=26 => Some((0, [4, 3, 5, 7, 12, 16][(id - 21) as usize], None, None, false)),
        37 | 38 => Some((0, if id == 37 { 14 } else { 28 }, None, None, false)),
        45 | 46 => Some((0, if id == 45 { 24 } else { 6 }, None, None, false)),
        53 | 54 => Some((0, if id == 53 { 8 } else { 32 }, None, None, false)),
        144..=147 => Some((0, [55, 110, 220, 530][(id - 144) as usize], None, None, false)),
        160..=163 => Some((0, [50, 100, 200, 500][(id - 160) as usize], None, None, false)),
        176..=179 => Some((0, [60, 120, 240, 560][(id - 176) as usize], None, None, false)),
        254..=257 => Some((0, [680, 1000, 1500, 2200][(id - 254) as usize], None, None, false)),
        258..=261 => Some((0, [630, 950, 1450, 2150][(id - 258) as usize], None, None, false)),
        262..=265 => Some((0, [700, 1050, 1550, 2250][(id - 262) as usize], None, None, false)),
        // Giày (opt 7, opt 28): Tiers 1..=12
        27 | 29 => Some((7, 10, None, None, false)),
        28 => Some((7, 15, None, None, false)),
        30 => Some((7, 25, Some((28, 5)), None, false)),
        47 => Some((7, 30, Some((28, 6)), None, false)),
        55 => Some((7, 20, Some((28, 4)), None, false)),
        31 | 32 => Some((7, if id == 31 { 150 } else { 100 }, Some((28, if id == 31 { 30 } else { 20 })), None, false)),
        39 | 40 => Some((7, if id == 39 { 120 } else { 250 }, Some((28, if id == 39 { 24 } else { 50 })), None, false)),
        48 | 56 => Some((7, if id == 48 { 300 } else { 200 }, Some((28, if id == 48 { 60 } else { 40 })), None, false)),
        148..=151 => {
            let k = (id - 148) as usize;
            Some((7, [500, 1200, 2400, 5000][k], Some((28, [100, 240, 480, 1000][k])), None, false))
        }
        164..=167 => {
            let k = (id - 164) as usize;
            Some((7, [600, 1500, 3000, 6000][k], Some((28, [120, 300, 600, 1200][k])), None, false))
        }
        180..=183 => {
            let k = (id - 180) as usize;
            Some((7, [400, 1000, 2000, 4000][k], Some((28, [80, 200, 400, 800][k])), None, false))
        }
        266..=269 => {
            let k = (id - 266) as i32;
            Some((7, 9000 + k * 5000, Some((28, 1500 + k * 500)), None, false))
        }
        270..=273 => {
            let k = (id - 270) as i32;
            Some((7, 10000 + k * 5000, Some((28, 1700 + k * 500)), None, false))
        }
        274..=277 => {
            let k = (id - 274) as i32;
            Some((7, 8000 + k * 5000, Some((28, 1300 + k * 500)), None, false))
        }
        // Rada (opt 14): Tiers 1..=12
        12 => Some((14, 1, None, None, false)),
        57..=59 => Some((14, (id - 55) as i32, None, None, false)),
        184..=187 => Some((14, (id - 179) as i32, None, None, false)),
        278..=281 => Some((14, (id - 269) as i32, None, None, false)),
        // Tier 13 Thần Linh (555..=567) reconciled
        555 => Some((47, if is_drop || is_shop { 800 } else { 700 }, None, Some(15), false)),
        557 => Some((47, if is_drop { 850 } else if is_shop { 800 } else { 600 }, None, Some(15), false)),
        559 => Some((47, if is_drop { 900 } else { 800 }, None, Some(15), false)),
        556 => Some((22, if is_drop || is_shop { 52 } else { 65 }, Some((27, if is_drop { 2600 } else { 10000 })), Some(15), false)),
        558 => Some((22, if is_drop || is_shop { 50 } else { 55 }, Some((27, if is_drop { 2500 } else { 10000 })), Some(15), false)),
        560 => Some((22, if is_drop || is_shop { 48 } else { 60 }, Some((27, if is_drop { 2400 } else { 10000 })), Some(15), false)),
        562 => Some((0, if is_drop { 4400 } else if is_shop { 3700 } else { 3200 }, None, Some(17), false)),
        564 => Some((0, if is_drop { 4300 } else if is_shop { 3500 } else { 3000 }, None, Some(17), false)),
        566 => Some((0, if is_drop { 4500 } else if is_shop { 3800 } else { 3500 }, None, Some(17), false)),
        563 => Some((23, if is_drop || is_shop { 48 } else { 40 }, Some((28, if is_drop { 2400 } else { 10000 })), Some(14), false)),
        565 => Some((23, if is_drop { 50 } else if is_shop { 48 } else { 60 }, Some((28, if is_drop { 2500 } else { 10000 })), Some(14), false)),
        567 => Some((23, if is_drop || is_shop { 46 } else { 50 }, Some((28, if is_drop { 6900 } else { 10000 })), Some(14), false)),
        561 => Some((14, if is_drop { 14 } else if is_shop { 15 } else { 16 }, None, Some(18), false)),
        // Tier 14 Hủy Diệt (650..=662)
        650 if !is_shop => Some((47, 1100, None, Some(5), true)),
        652 if !is_shop => Some((47, 1000, None, Some(5), true)),
        654 if !is_shop => Some((47, 1300, None, Some(5), true)),
        651 if !is_shop => Some((22, 100, Some((27, 25000)), Some(5), true)),
        653 if !is_shop => Some((22, 90, Some((27, 22000)), Some(5), true)),
        655 if !is_shop => Some((22, 80, Some((27, 24000)), Some(5), true)),
        657 if !is_shop => Some((0, 7000, None, Some(5), true)),
        659 if !is_shop => Some((0, 6500, None, Some(5), true)),
        661 if !is_shop => Some((0, 7500, None, Some(5), true)),
        658 if !is_shop => Some((23, 80, Some((28, 22000)), Some(5), true)),
        660 if !is_shop => Some((23, 90, Some((28, 25000)), Some(5), true)),
        662 if !is_shop => Some((23, 85, Some((28, 20000)), Some(5), true)),
        656 if !is_shop => Some((14, 18, None, Some(5), true)),
        _ => None,
    }
}
