// src/cff/formulas.rs

/// Расчёт чистого урона в секунду (DPS) для оружия (Категория 2015 / 0x07DF)
pub fn calculate_weapon_dps(min_damage: u16, max_damage: u16, weapon_speed: u16) -> f32 {
    let mean = (min_damage as f32 + max_damage as f32) / 2.0;
    let ratio = weapon_speed as f32 / 100.0;
    mean * ratio
}

/// Расчёт суммарного опыта с учётом кривой затухания (Категория 2024 / 0x07E8)
pub fn calculate_total_xp(xp_gain: u32, xp_falloff: u16, max_kills: usize) -> u64 {
    if xp_gain == 0 || xp_falloff == 0 {
        return 0;
    }
    let mut total_xp = 0u64;
    let falloff_f = xp_falloff as f64;
    for i in 0..max_kills {
        let factor = falloff_f / (falloff_f + i as f64);
        total_xp += (xp_gain as f64 * factor).floor() as u64;
    }
    total_xp
}

/// Расчёт каскадных эффективных шансов выпадения лута (Категории 2040 и 2065)
pub fn calculate_cascade_loot_chances(chance1: u8, chance2: u8) -> (f32, f32, f32) {
    let c1 = chance1 as f32;
    let c2 = (chance2 as f32) * (1.0 - c1 / 100.0);
    let c3 = (100.0 - c1 - c2).max(0.0);
    (c1, c2, c3)
}

/// Расчёт реального запаса здоровья и маны юнита с учётом уровня (Категории 2005 и 2048)
pub fn calculate_health_and_mana(
    stamina: u16,
    wisdom: u16,
    health_factor: u16,
    mana_factor: u16,
) -> (u32, u32) {
    let hp = ((stamina as u32) * (health_factor as u32)) / 100;
    let mana = ((wisdom as u32) * (mana_factor as u32)) / 100;
    (hp, mana)
}

// -----------------------------------------------------------------------------
// ДЕКОДЕРЫ И ФОРМАТТЕРЫ БИТОВЫХ МАСОК
// -----------------------------------------------------------------------------

pub const RACE_FLAGS: [&str; 8] = [
    "Undead",
    "Breathing",
    "Huntable",
    "Animal",
    "Has Soul",
    "Attacks Buildings",
    "Bleeds",
    "Unused",
];

pub const RACE_AI_FLAGS: [&str; 8] = [
    "Default",
    "Idle",
    "Stroll Along",
    "Nomadic",
    "Aggressive",
    "Defensive",
    "Scripted",
    "Unused",
];

pub fn decode_race_flags(flags: u8) -> Vec<&'static str> {
    (0..8)
        .filter(|&i| (flags & (1 << i)) != 0)
        .map(|i| RACE_FLAGS[i])
        .collect()
}

pub fn format_race_flags(flags: u8) -> String {
    let active = decode_race_flags(flags);
    if active.is_empty() {
        "None".into()
    } else {
        active.join(", ")
    }
}

pub fn decode_ai_flags(ai_flags: u16) -> Vec<&'static str> {
    (0..8)
        .filter(|&i| (ai_flags & (1 << i)) != 0)
        .map(|i| RACE_AI_FLAGS[i])
        .collect()
}

pub fn decode_item_options(options: u8) -> Vec<&'static str> {
    let mut tags = Vec::new();
    if (options & 0b0000_0001) != 0 {
        tags.push("Stackable");
    }
    if (options & 0b0000_0010) != 0 {
        tags.push("Lore Item");
    }
    if (options & 0b0000_0100) != 0 {
        tags.push("Quest Item (No Sell)");
    }
    if (options & 0b0000_1000) != 0 {
        tags.push("Quest Item (Can Sell)");
    }
    if (options & 0b0001_0000) != 0 {
        tags.push("Strict Reqs Check");
    }
    tags
}

pub fn format_item_options(options: u8) -> String {
    let active = decode_item_options(options);
    if active.is_empty() {
        "Standard".into()
    } else {
        active.join(", ")
    }
}

pub fn decode_cultivation_flags(flags: u8) -> Vec<&'static str> {
    let mut tags = Vec::new();
    if (flags & 0x01) != 0 {
        tags.push("Grain");
    }
    if (flags & 0x02) != 0 {
        tags.push("Mushroom");
    }
    if (flags & 0x04) != 0 {
        tags.push("Trees");
    }
    tags
}
