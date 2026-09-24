// src/cff/sf1.rs

use super::container::Manifest;
use super::editor::EditorItem;
use super::formulas::{calculate_cascade_loot_chances, calculate_total_xp, calculate_weapon_dps};
use super::sf1_schema::*;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub struct Sf1ChunkInfo {
    pub id: u32,
    pub name: &'static str,
    pub stride: usize,
    pub default_c_type: i16,
    pub description: &'static str,
}

pub const SF1_CATEGORY_TABLE: &[Sf1ChunkInfo] = &[
    Sf1ChunkInfo {
        id: 0x07D1,
        name: "UnitBuildingReqs",
        stride: 5,
        default_c_type: 1,
        description: "Building requirements (Cat 2001)",
    },
    Sf1ChunkInfo {
        id: 0x07D2,
        name: "SpellsMaster",
        stride: 72,
        default_c_type: 1,
        description: "Combat spells parameters (Cat 2002)",
    },
    Sf1ChunkInfo {
        id: 0x07D3,
        name: "ItemsMaster",
        stride: 22,
        default_c_type: 1,
        description: "Master item catalog (Cat 2003)",
    },
    Sf1ChunkInfo {
        id: 0x07D4,
        name: "ItemStatsModifiers",
        stride: 36,
        default_c_type: 1,
        description: "Equipment attribute bonuses (Cat 2004)",
    },
    Sf1ChunkInfo {
        id: 0x07D5,
        name: "UnitStats",
        stride: 47,
        default_c_type: 1,
        description: "Creature stats and resistances (Cat 2005)",
    },
    Sf1ChunkInfo {
        id: 0x07D6,
        name: "UnitSkills",
        stride: 5,
        default_c_type: 1,
        description: "Skills assigned to units (Cat 2006)",
    },
    Sf1ChunkInfo {
        id: 0x07DC,
        name: "2dGfxItems",
        stride: 69,
        default_c_type: 1,
        description: "2D Icons and 3D meshes (Cat 2012)",
    },
    Sf1ChunkInfo {
        id: 0x07DD,
        name: "SpellScrolls",
        stride: 4,
        default_c_type: 1,
        description: "Scroll item mappings (Cat 2013)",
    },
    Sf1ChunkInfo {
        id: 0x07DE,
        name: "ItemEffects",
        stride: 5,
        default_c_type: 1,
        description: "Spell effects on items (Cat 2014)",
    },
    Sf1ChunkInfo {
        id: 0x07DF,
        name: "WeaponStats",
        stride: 16,
        default_c_type: 1,
        description: "Weapon damages and speeds (Cat 2015)",
    },
    Sf1ChunkInfo {
        id: 0x07E0,
        name: "LocalizedStrings",
        stride: 566,
        default_c_type: 1,
        description: "566-byte string tables (Cat 2016)",
    },
    Sf1ChunkInfo {
        id: 0x07E1,
        name: "ItemSkillReqs",
        stride: 6,
        default_c_type: 1,
        description: "Skill requirements (Cat 2017)",
    },
    Sf1ChunkInfo {
        id: 0x07E2,
        name: "SpellsBiMap",
        stride: 4,
        default_c_type: 1,
        description: "Spell to scroll map (Cat 2018)",
    },
    Sf1ChunkInfo {
        id: 0x07E6,
        name: "Races",
        stride: 27,
        default_c_type: 1,
        description: "Race visual ranges & AI (Cat 2022)",
    },
    Sf1ChunkInfo {
        id: 0x07E7,
        name: "DiplomacyMatrix",
        stride: 3,
        default_c_type: 1,
        description: "Faction relations (Cat 2023)",
    },
    Sf1ChunkInfo {
        id: 0x07E8,
        name: "UnitsMaster",
        stride: 23,
        default_c_type: 1,
        description: "Unit database and XP (Cat 2024)",
    },
    Sf1ChunkInfo {
        id: 0x07E9,
        name: "UnitEquipment",
        stride: 5,
        default_c_type: 1,
        description: "Equipped inventory slots (Cat 2025)",
    },
    Sf1ChunkInfo {
        id: 0x07EA,
        name: "UnitSpellbook",
        stride: 5,
        default_c_type: 1,
        description: "Castable spells (Cat 2026)",
    },
    Sf1ChunkInfo {
        id: 0x07EC,
        name: "UnitResources",
        stride: 4,
        default_c_type: 1,
        description: "Resource costs (Cat 2028)",
    },
    Sf1ChunkInfo {
        id: 0x07ED,
        name: "BuildingsMaster",
        stride: 24,
        default_c_type: 1,
        description: "Building health & tech (Cat 2029)",
    },
    Sf1ChunkInfo {
        id: 0x07EE,
        name: "BuildingCollision",
        stride: 8,
        default_c_type: 1,
        description: "Collision polygons (Cat 2030)",
    },
    Sf1ChunkInfo {
        id: 0x07EF,
        name: "BuildingResources",
        stride: 5,
        default_c_type: 1,
        description: "Building costs (Cat 2031)",
    },
    Sf1ChunkInfo {
        id: 0x07F0,
        name: "TerrainCultivation",
        stride: 4,
        default_c_type: 1,
        description: "Farming flags (Cat 2032)",
    },
    Sf1ChunkInfo {
        id: 0x07F4,
        name: "TechTreeUpgrades",
        stride: 34,
        default_c_type: 1,
        description: "Tech upgrades (Cat 2036)",
    },
    Sf1ChunkInfo {
        id: 0x07F7,
        name: "SkillsMaster",
        stride: 4,
        default_c_type: 1,
        description: "Skill classification (Cat 2039)",
    },
    Sf1ChunkInfo {
        id: 0x07F8,
        name: "UnitLootTables",
        stride: 12,
        default_c_type: 1,
        description: "Monster loot tables (Cat 2040)",
    },
    Sf1ChunkInfo {
        id: 0x07F9,
        name: "MerchantsMaster",
        stride: 4,
        default_c_type: 1,
        description: "Merchants catalog (Cat 2041)",
    },
    Sf1ChunkInfo {
        id: 0x07FA,
        name: "MerchantInventory",
        stride: 6,
        default_c_type: 1,
        description: "Merchant inventories (Cat 2042)",
    },
    Sf1ChunkInfo {
        id: 0x07FC,
        name: "Resources",
        stride: 3,
        default_c_type: 1,
        description: "Resource types (Cat 2044)",
    },
    Sf1ChunkInfo {
        id: 0x07FF,
        name: "MerchantPriceMultipliers",
        stride: 5,
        default_c_type: 1,
        description: "Price multipliers (Cat 2047)",
    },
    Sf1ChunkInfo {
        id: 0x0800,
        name: "ComplexProperties",
        stride: 15,
        default_c_type: 3,
        description: "Level progression (Cat 2048)",
    },
    Sf1ChunkInfo {
        id: 0x0801,
        name: "HeadMeshes",
        stride: 2,
        default_c_type: 1,
        description: "Head visual IDs (Cat 2049)",
    },
    Sf1ChunkInfo {
        id: 0x0802,
        name: "ObjectsMaster",
        stride: 22,
        default_c_type: 1,
        description: "Interactive map props (Cat 2050)",
    },
    Sf1ChunkInfo {
        id: 0x0803,
        name: "NpcMaster",
        stride: 6,
        default_c_type: 1,
        description: "NPC speech links (Cat 2051)",
    },
    Sf1ChunkInfo {
        id: 0x0804,
        name: "MapsCatalog",
        stride: 71,
        default_c_type: 1,
        description: "Map catalog (Cat 2052)",
    },
    Sf1ChunkInfo {
        id: 0x0805,
        name: "Portals",
        stride: 9,
        default_c_type: 1,
        description: "World map portals (Cat 2053)",
    },
    Sf1ChunkInfo {
        id: 0x0806,
        name: "SpellLines",
        stride: 18,
        default_c_type: 1,
        description: "Magic schools lines (Cat 2054)",
    },
    Sf1ChunkInfo {
        id: 0x080A,
        name: "Descriptions",
        stride: 4,
        default_c_type: 1,
        description: "Tooltip descriptions (Cat 2058)",
    },
    Sf1ChunkInfo {
        id: 0x080B,
        name: "ExtendedDescriptions",
        stride: 6,
        default_c_type: 1,
        description: "Lore descriptions (Cat 2059)",
    },
    Sf1ChunkInfo {
        id: 0x080D,
        name: "Quests",
        stride: 11,
        default_c_type: 1,
        description: "Quests tree (Cat 2061)",
    },
    Sf1ChunkInfo {
        id: 0x080E,
        name: "SkillAttributeReqs",
        stride: 9,
        default_c_type: 1,
        description: "Skill attributes (Cat 2062)",
    },
    Sf1ChunkInfo {
        id: 0x080F,
        name: "WeaponTypes",
        stride: 5,
        default_c_type: 1,
        description: "Weapon types (Cat 2063)",
    },
    Sf1ChunkInfo {
        id: 0x0810,
        name: "WeaponMaterials",
        stride: 4,
        default_c_type: 1,
        description: "Weapon materials (Cat 2064)",
    },
    Sf1ChunkInfo {
        id: 0x0811,
        name: "ObjectLootTables",
        stride: 12,
        default_c_type: 1,
        description: "Chest loot tables (Cat 2065)",
    },
    Sf1ChunkInfo {
        id: 0x0813,
        name: "UnitSpells",
        stride: 5,
        default_c_type: 1,
        description: "Combat spell slots (Cat 2067)",
    },
    Sf1ChunkInfo {
        id: 0x0818,
        name: "ItemSets",
        stride: 4,
        default_c_type: 1,
        description: "Equipment set bonuses (Cat 2072)",
    },
];

pub fn get_sf1_chunk_info(id: u32) -> Option<Sf1ChunkInfo> {
    SF1_CATEGORY_TABLE.iter().find(|i| i.id == id).copied()
}

pub fn describe_category(id: u32) -> Option<&'static str> {
    get_sf1_chunk_info(id).map(|i| i.description)
}

fn parse_numeric_key_u16(source: &str, key: &str) -> Option<u16> {
    if let Some(pos) = source.find(key) {
        let tail = &source[pos + key.len()..];
        let num_str: String = tail
            .chars()
            .skip_while(|c| c.is_whitespace() || *c == ':')
            .take_while(|c| c.is_ascii_digit())
            .collect();
        num_str.parse::<u16>().ok()
    } else {
        source.trim().parse::<u16>().ok()
    }
}

fn parse_numeric_key_u32(source: &str, key: &str) -> Option<u32> {
    if let Some(pos) = source.find(key) {
        let tail = &source[pos + key.len()..];
        let num_str: String = tail
            .chars()
            .skip_while(|c| c.is_whitespace() || *c == ':')
            .take_while(|c| c.is_ascii_digit())
            .collect();
        num_str.parse::<u32>().ok()
    } else {
        source.trim().parse::<u32>().ok()
    }
}

pub fn load_sf1_items(cff_dir: &Path, category: &str, filter: &str) -> Vec<EditorItem> {
    let mut items = Vec::new();
    let filter_lower = filter.to_lowercase();
    let manifest_path = cff_dir.join("manifest.json");

    let Ok(m_str) = fs::read_to_string(manifest_path) else {
        return items;
    };
    let Ok(manifest) = serde_json::from_str::<Manifest>(&m_str) else {
        return items;
    };

    // 1. Spells Master (0x07D2)
    if (category.contains("0x07D2") || category.contains("2002"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07D2)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ SpellEntry::STRIDE }>().0 {
            if let Ok(e) = SpellEntry::decode(chunk_slice) {
                let display = format!(
                    "Spell #{:<5} [Line: {:<3}] | Mana: {:<3} | Power: {:<3} | Target: {} ({}) | Range: {}-{}",
                    e.spell_id,
                    e.spell_line_id,
                    e.mana_cost,
                    e.effect_power,
                    e.format_target_faction(),
                    e.format_target_mode(),
                    e.min_range,
                    e.max_range
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: e.spell_id.to_string(),
                        val1: format!("Mana: {}, Power: {}", e.mana_cost, e.effect_power),
                        val2: format!(
                            "Line: {} | Target: {} ({}) | Range: {}-{} | Cast: {}ms | Recast: {}ms",
                            e.spell_line_id,
                            e.format_target_faction(),
                            e.format_target_mode(),
                            e.min_range,
                            e.max_range,
                            e.cast_time_ms,
                            e.recast_time_ms
                        ),
                        display,
                    });
                }
            }
        }
    }
    // 2. Items Master (0x07D3)
    else if (category.contains("0x07D3") || category.contains("2003"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07D3)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ ItemMasterEntry::STRIDE }>().0 {
            if let Ok(e) = ItemMasterEntry::decode(chunk_slice) {
                let display = format!(
                    "Item #{:<5} | Buy: {:<6}c | Sell: {:<6}c | Type: ({}, {}) | Set: {}",
                    e.item_id, e.buy_value, e.sell_value, e.item_type1, e.item_type2, e.item_set_id
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: e.item_id.to_string(),
                        val1: format!("Buy: {}, Sell: {}", e.buy_value, e.sell_value),
                        val2: format!("NameID: {} | StatsID: {} | UnitID: {} | BuildingID: {} | Flags: 0x{:02X} | Set: {}", e.name_id, e.unit_stats_id, e.army_unit_id, e.building_id, e.option_flags, e.item_set_id),
                        display,
                    });
                }
            }
        }
    }
    // 3. Item Stats Modifiers (0x07D4)
    else if (category.contains("0x07D4") || category.contains("2004"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07D4)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ ItemStatsModifierEntry::STRIDE }>().0 {
            if let Ok(m) = ItemStatsModifierEntry::decode(chunk_slice) {
                let display = format!(
                    "Item Mod #{:<5} | Str: {:>+3} Sta: {:>+3} Agi: {:>+3} Dex: {:>+3} | Armor: {:>+3}",
                    m.item_id, m.strength, m.stamina, m.agility, m.dexterity, m.armor
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: m.item_id.to_string(),
                        val1: format!(
                            "Str: {}, Sta: {}, Agi: {}, Dex: {}, Armor: {}",
                            m.strength, m.stamina, m.agility, m.dexterity, m.armor
                        ),
                        val2: format!(
                            "HP: {} | Mana: {} | Res(F/I/B/M): {}/{}/{}/{} | Spd(W/F/C): {}/{}/{}",
                            m.health,
                            m.mana,
                            m.resist_fire,
                            m.resist_ice,
                            m.resist_black,
                            m.resist_mind,
                            m.speed_walk,
                            m.speed_fight,
                            m.speed_cast
                        ),
                        display,
                    });
                }
            }
        }
    }
    // 4. Unit Stats (0x07D5)
    else if (category.contains("0x07D5") || category.contains("2005"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07D5)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ UnitStatsEntry::STRIDE }>().0 {
            if let Ok(u) = UnitStatsEntry::decode(chunk_slice) {
                let (hp, mana) = u.calculate_effective_hp_and_mana(100, 100);
                let flags_desc = if u.is_unkillable() {
                    " [Invulnerable]"
                } else if u.is_female() {
                    " [Female]"
                } else {
                    ""
                };
                let display = format!(
                    "UnitStats #{:<5} [Lvl: {:<2}, Race: {}]{} | HP: {}/MP: {} | Spd: {}/{}/{}",
                    u.stats_id,
                    u.unit_level,
                    u.unit_race,
                    flags_desc,
                    hp,
                    mana,
                    u.speed_walk,
                    u.speed_fight,
                    u.speed_cast
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: u.stats_id.to_string(),
                        val1: format!("Level: {}, Race: {}", u.unit_level, u.unit_race),
                        val2: format!(
                            "HP: {} | MP: {} | Str: {} | Sta: {} | Agi: {} | Dex: {} | Head: {}",
                            hp, mana, u.strength, u.stamina, u.agility, u.dexterity, u.head_id
                        ),
                        display,
                    });
                }
            }
        }
    }
    // 5. 2D Gfx Items (0x07DC)
    else if (category.contains("0x07DC") || category.contains("2012"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DC)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ Gfx2dItemEntry::STRIDE }>().0 {
            if let Ok(g) = Gfx2dItemEntry::decode(chunk_slice) {
                let display = format!(
                    "ID: {:<5} [Flag: {}] | Mesh: {}",
                    g.item_id, g.flag, g.mesh_name
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: g.item_id.to_string(),
                        val1: g.mesh_name,
                        val2: format!("Flag: {}, Extra: {}", g.flag, g.extra),
                        display,
                    });
                }
            }
        }
    }
    // 6. Weapon Stats (0x07DF)
    else if (category.contains("0x07DF") || category.contains("2015"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DF)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ WeaponStatsEntry::STRIDE }>().0 {
            if let Ok(w) = WeaponStatsEntry::decode(chunk_slice) {
                let dps = calculate_weapon_dps(w.min_damage, w.max_damage, w.speed);
                let display = format!(
                    "Weapon #{:<5} | Dmg: {}-{} [DPS: {:.1}] | Spd: {}% | Type: {}",
                    w.item_id, w.min_damage, w.max_damage, dps, w.speed, w.weapon_type
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: w.item_id.to_string(),
                        val1: format!("{}-{}", w.min_damage, w.max_damage),
                        val2: format!(
                            "Speed: {} | Range: {}-{} | Type: {} | Material: {} | DPS: {:.2}",
                            w.speed, w.min_range, w.max_range, w.weapon_type, w.material, dps
                        ),
                        display,
                    });
                }
            }
        }
    }
    // 7. Spells BiMap (0x07E2)
    else if (category.contains("0x07E2") || category.contains("2018"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E2)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ SpellsBiMapEntry::STRIDE }>().0 {
            if let Ok(b) = SpellsBiMapEntry::decode(chunk_slice) {
                let display = format!(
                    "Spell ID: {:<5} -> Scroll Item ID: {}",
                    b.spell_id, b.scroll_item_id
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: b.spell_id.to_string(),
                        val1: b.scroll_item_id.to_string(),
                        val2: "BiMap Entry".into(),
                        display,
                    });
                }
            }
        }
    }
    // 8. Races (0x07E6)
    else if (category.contains("0x07E6") || category.contains("2022"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E6)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ RaceEntry::STRIDE }>().0 {
            if let Ok(r) = RaceEntry::decode(chunk_slice) {
                let clan = get_clan_name(r.faction_id as u8);
                let display = format!(
                    "Race #{:<3} [{}] | Vis: Day {}/Night {} | Aggro: {} | Moral: {}",
                    r.race_id, clan, r.vis_day, r.vis_night, r.aggro_factor, r.moral
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: r.race_id.to_string(),
                        val1: format!("Aggro: {}, Moral: {}", r.aggro_factor, r.moral),
                        val2: format!(
                            "Faction: {} ({}) | Hear: {} | Retreat: {}% | Flee: {}",
                            r.faction_id, clan, r.hear_range, r.retreat_on_dmg, r.flee
                        ),
                        display,
                    });
                }
            }
        }
    }
    // 9. Units Master (0x07E8)
    else if (category.contains("0x07E8") || category.contains("2024"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E8)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ UnitMasterEntry::STRIDE }>().0 {
            if let Ok(u) = UnitMasterEntry::decode(chunk_slice) {
                let max_farm_xp = calculate_total_xp(u.xp_gain, u.xp_falloff, 500);
                let display = format!(
                    "Unit #{:<5} [Stats: {}] | Armor: {} | Base XP: {} | Loot: {}c",
                    u.unit_id, u.stats_id, u.armor, u.xp_gain, u.copper
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: u.unit_id.to_string(),
                        val1: u.xp_gain.to_string(),
                        val2: format!(
                            "Armor: {} | Falloff: {} | Copper: {} | NameID: {} | Max XP: {}",
                            u.armor, u.xp_falloff, u.copper, u.name_id, max_farm_xp
                        ),
                        display,
                    });
                }
            }
        }
    }
    // 10. Buildings Master (0x07ED)
    else if (category.contains("0x07ED") || category.contains("2029"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07ED)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ BuildingMasterEntry::STRIDE }>().0 {
            if let Ok(b) = BuildingMasterEntry::decode(chunk_slice) {
                let display = format!(
                    "Building #{:<5} [Race {}] | HP: {} | Workers: {}ms | Slots: {}",
                    b.building_id, b.race_id, b.health, b.worker_cycle_time, b.slots
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: b.building_id.to_string(),
                        val1: format!("Health: {}, Slots: {}", b.health, b.slots),
                        val2: format!("ReqBuilding: {} | WorkerCycle: {}ms | RotCenter: ({}, {}) | Polygons: {}", b.building_req_id, b.worker_cycle_time, b.rot_center_x, b.rot_center_y, b.num_of_polygons),
                        display,
                    });
                }
            }
        }
    }
    // 11. Unit Loot Tables (0x07F8)
    else if (category.contains("0x07F8") || category.contains("2040"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07F8)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ UnitLootTableEntry::STRIDE }>().0 {
            if let Ok(l) = UnitLootTableEntry::decode(chunk_slice) {
                let (eff1, eff2, eff3) = calculate_cascade_loot_chances(l.chance1, l.chance2);
                let display = format!(
                    "Loot Unit #{:<5} [Slot {}] | Items: {}, {}, {}",
                    l.unit_id, l.slot, l.item1, l.item2, l.item3
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: format!("{}:{}", l.unit_id, l.slot),
                        val1: format!("{}, {}, {}", l.item1, l.item2, l.item3),
                        val2: format!(
                            "Chances: {}%, {}% | Cascading: [{:.1}%, {:.1}%, {:.1}%]",
                            l.chance1, l.chance2, eff1, eff2, eff3
                        ),
                        display,
                    });
                }
            }
        }
    }
    // 12. Level Progression (0x0800)
    else if (category.contains("0x0800") || category.contains("2048"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x0800)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ ComplexPropertyEntry::STRIDE }>().0 {
            if let Ok(cp) = ComplexPropertyEntry::decode(chunk_slice) {
                let display = format!(
                    "Progression Level #{:<2} | XP Req: {:<8} | HP Factor: {}% | MP Factor: {}%",
                    cp.level, cp.experience_required, cp.health_factor, cp.mana_factor
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: cp.level.to_string(),
                        val1: cp.experience_required.to_string(),
                        val2: format!("HP Factor: {}% | MP Factor: {}% | Dmg Factor: {}% | Armor Factor: {}% | AttrLimit: {} | SkillLimit: {}", cp.health_factor, cp.mana_factor, cp.damage_factor, cp.armor_class_factor, cp.attribute_point_limit, cp.skill_point_limit),
                        display,
                    });
                }
            }
        }
    }
    // 13. Objects Master (0x0802)
    else if (category.contains("0x0802") || category.contains("2050"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x0802)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ ObjectMasterEntry::STRIDE }>().0 {
            if let Ok(o) = ObjectMasterEntry::decode(chunk_slice) {
                let loot_tag = if o.contains_loot() {
                    " [Loot Chest]"
                } else {
                    ""
                };
                let block_tag = if o.blocks_terrain() {
                    " [Blocks Path]"
                } else {
                    ""
                };
                let height_tag = if o.adjusts_height() {
                    " [Adjusts Height]"
                } else {
                    ""
                };
                let place_tag = if o.is_placeable() { " [Placeable]" } else { "" };
                let display = format!(
                    "Object #{:<5}{}{} | Size: {}x{} | Res: {}{}{}",
                    o.object_id,
                    loot_tag,
                    place_tag,
                    o.width,
                    o.height,
                    o.resource_amount,
                    block_tag,
                    height_tag
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: o.object_id.to_string(),
                        val1: o.resource_amount.to_string(),
                        val2: format!("NameID: {} | Dims: {}x{} | Flatten: {} | Polygons: {} | Flags: 0x{:02X}{}{}", o.name_id, o.width, o.height, o.flatten_mode, o.polygon_num, o.flags, height_tag, place_tag),
                        display,
                    });
                }
            }
        }
    }
    // 14. Unit Equipment (0x07E9)
    else if (category.contains("0x07E9") || category.contains("2025"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E9)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ UnitEquipmentEntry::STRIDE }>().0 {
            if let Ok(e) = UnitEquipmentEntry::decode(chunk_slice) {
                let slot_name = get_equipment_slot_name(e.equipment_slot);
                let display = format!(
                    "Unit #{:<5} [{}] -> Item #{}",
                    e.unit_id, slot_name, e.item_id
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: format!("{}:{}", e.unit_id, e.equipment_slot),
                        val1: e.item_id.to_string(),
                        val2: slot_name.to_string(),
                        display,
                    });
                }
            }
        }
    }
    // 15. Merchant Inventory (0x07FA)
    else if (category.contains("0x07FA") || category.contains("2042"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07FA)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ MerchantInventoryEntry::STRIDE }>().0 {
            if let Ok(m) = MerchantInventoryEntry::decode(chunk_slice) {
                let display = format!(
                    "Merchant #{:<5} sells Item #{:<5} (Qty: {})",
                    m.merchant_id, m.item_id, m.stock
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: format!("{}:{}", m.merchant_id, m.item_id),
                        val1: m.stock.to_string(),
                        val2: format!("Merchant: {}, Item: {}", m.merchant_id, m.item_id),
                        display,
                    });
                }
            }
        }
    }
    // 16. Quests (0x080D)
    else if (category.contains("0x080D") || category.contains("2061"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x080D)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ QuestEntry::STRIDE }>().0 {
            if let Ok(q) = QuestEntry::decode(chunk_slice) {
                let main_tag = if q.is_main_quest != 0 {
                    "[Main]"
                } else {
                    "[Side]"
                };
                let display = format!(
                    "Quest #{:<4} {} | Parent: #{:<4} | Order: {}",
                    q.quest_id, main_tag, q.parent_quest_id, q.order_index
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: q.quest_id.to_string(),
                        val1: format!("Parent: {}, Main: {}", q.parent_quest_id, q.is_main_quest),
                        val2: format!(
                            "NameID: {}, DescID: {}, Order: {}",
                            q.name_id, q.description_id, q.order_index
                        ),
                        display,
                    });
                }
            }
        }
    }
    // 17. Weapon Types (0x080F)
    else if (category.contains("0x080F") || category.contains("2063"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x080F)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ WeaponTypeEntry::STRIDE }>().0 {
            if let Ok(wt) = WeaponTypeEntry::decode(chunk_slice) {
                let display = format!(
                    "Weapon Type #{:<3} | NameID: {:<5} | Sharpness: {}%",
                    wt.type_id, wt.name_id, wt.sharpness
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: wt.type_id.to_string(),
                        val1: wt.sharpness.to_string(),
                        val2: format!("NameID: {}", wt.name_id),
                        display,
                    });
                }
            }
        }
    }
    // 18. Weapon Materials (0x0810)
    else if (category.contains("0x0810") || category.contains("2064"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x0810)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ WeaponMaterialEntry::STRIDE }>().0 {
            if let Ok(wm) = WeaponMaterialEntry::decode(chunk_slice) {
                let display = format!(
                    "Weapon Material #{:<3} | NameID: {:<5}",
                    wm.material_id, wm.name_id
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: wm.material_id.to_string(),
                        val1: wm.name_id.to_string(),
                        val2: format!("Material ID: {}", wm.material_id),
                        display,
                    });
                }
            }
        }
    }
    // 19. Item Sets (0x0818)
    else if (category.contains("0x0818") || category.contains("2072"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x0818)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ ItemSetEntry::STRIDE }>().0 {
            if let Ok(s) = ItemSetEntry::decode(chunk_slice) {
                let display = format!(
                    "Item Set #{:<3} [Type: {}] | DescID: {}",
                    s.set_id, s.set_type, s.description_id
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: s.set_id.to_string(),
                        val1: s.description_id.to_string(),
                        val2: format!("Set Type: {}", s.set_type),
                        display,
                    });
                }
            }
        }
    }
    // 20. Terrain Cultivation (0x07F0)
    else if (category.contains("0x07F0") || category.contains("2032"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07F0)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ TerrainCultivationEntry::STRIDE }>().0 {
            if let Ok(tc) = TerrainCultivationEntry::decode(chunk_slice) {
                let display = format!(
                    "Terrain #{:<3} | Block: {} | Cultivation: 0x{:02X}",
                    tc.terrain_id, tc.block_value, tc.cultivation_flags
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: tc.terrain_id.to_string(),
                        val1: tc.block_value.to_string(),
                        val2: format!("Flags: 0x{:02X}", tc.cultivation_flags),
                        display,
                    });
                }
            }
        }
    }
    // 21. Portals (0x0805)
    else if (category.contains("0x0805") || category.contains("2053"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x0805)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ PortalEntry::STRIDE }>().0 {
            if let Ok(p) = PortalEntry::decode(chunk_slice) {
                let def_tag = if p.is_default != 0 { " [Default]" } else { "" };
                let display = format!(
                    "Portal #{:<3} [Map: {}]{} | Pos: ({}, {})",
                    p.portal_id, p.map_id, def_tag, p.pos_x, p.pos_y
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: p.portal_id.to_string(),
                        val1: p.map_id.to_string(),
                        val2: format!("X: {}, Y: {}, Default: {}", p.pos_x, p.pos_y, p.is_default),
                        display,
                    });
                }
            }
        }
    }
    // 22. Descriptions (0x080A)
    else if (category.contains("0x080A") || category.contains("2058"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x080A)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for chunk_slice in bytes.as_chunks::<{ DescriptionEntry::STRIDE }>().0 {
            if let Ok(d) = DescriptionEntry::decode(chunk_slice) {
                let display = format!(
                    "Description #{:<5} -> TextID: {:<5}",
                    d.description_id, d.text_id
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: d.description_id.to_string(),
                        val1: d.text_id.to_string(),
                        val2: format!("Description link #{}", d.description_id),
                        display,
                    });
                }
            }
        }
    }

    items
}

pub fn save_sf1_item(
    cff_dir: &Path,
    category: &str,
    index: usize,
    _id_str: &str,
    val1: &str,
    val2: &str,
) -> std::io::Result<()> {
    let manifest_path = cff_dir.join("manifest.json");
    let m_str = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&m_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    macro_rules! update_record {
        ($chunk_id:expr, $entry_type:ident, $modify_block:expr) => {
            if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == $chunk_id) {
                let chunk_path = cff_dir.join(&chunk.file);
                let mut bytes = fs::read(&chunk_path)?;
                let offset = index * $entry_type::STRIDE;
                if offset + $entry_type::STRIDE <= bytes.len()
                    && let Ok(mut entry) =
                        $entry_type::decode(&bytes[offset..offset + $entry_type::STRIDE])
                {
                    $modify_block(&mut entry);
                    entry.encode(&mut bytes[offset..offset + $entry_type::STRIDE])?;
                    crate::tools::atomic_write(&chunk_path, &bytes)?;
                }
            }
        };
    }

    if category.contains("0x07D2") || category.contains("2002") {
        update_record!(0x07D2, SpellEntry, |e: &mut SpellEntry| {
            if let Some(m) = parse_numeric_key_u16(val1, "Mana") {
                e.mana_cost = m;
            }
            if let Some(p) = parse_numeric_key_u16(val1, "Power") {
                e.effect_power = p;
            }
        });
    } else if category.contains("0x07D3") || category.contains("2003") {
        update_record!(0x07D3, ItemMasterEntry, |e: &mut ItemMasterEntry| {
            if let Some(buy) = parse_numeric_key_u32(val1, "Buy") {
                e.buy_value = buy;
            }
            if let Some(sell) = parse_numeric_key_u32(val1, "Sell") {
                e.sell_value = sell;
            }
        });
    } else if category.contains("0x07D4") || category.contains("2004") {
        update_record!(
            0x07D4,
            ItemStatsModifierEntry,
            |e: &mut ItemStatsModifierEntry| {
                if let Some(s) = parse_numeric_key_u16(val1, "Str") {
                    e.strength = s as i16;
                }
                if let Some(a) = parse_numeric_key_u16(val1, "Armor") {
                    e.armor = a as i16;
                }
            }
        );
    } else if category.contains("0x07D5") || category.contains("2005") {
        update_record!(0x07D5, UnitStatsEntry, |e: &mut UnitStatsEntry| {
            if let Some(lvl) = parse_numeric_key_u16(val1, "Level") {
                e.unit_level = lvl;
            }
            if let Some(r) = parse_numeric_key_u16(val1, "Race") {
                e.unit_race = r as u8;
            }
        });
    } else if category.contains("0x07DC") || category.contains("2012") {
        update_record!(0x07DC, Gfx2dItemEntry, |e: &mut Gfx2dItemEntry| {
            e.mesh_name = val1.trim().to_string();
        });
    } else if category.contains("0x07DF") || category.contains("2015") {
        update_record!(0x07DF, WeaponStatsEntry, |e: &mut WeaponStatsEntry| {
            if val1.contains('-') {
                let parts: Vec<&str> = val1.split('-').collect();
                e.min_damage = parts
                    .first()
                    .and_then(|s| s.trim().parse::<u16>().ok())
                    .unwrap_or(e.min_damage);
                e.max_damage = parts
                    .get(1)
                    .and_then(|s| s.trim().parse::<u16>().ok())
                    .unwrap_or(e.min_damage);
            } else if let Ok(d) = val1.trim().parse::<u16>() {
                e.min_damage = d;
                e.max_damage = d;
            }
            if let Some(spd) = parse_numeric_key_u16(val2, "Speed") {
                e.speed = spd;
            }
        });
    } else if category.contains("0x07E2") || category.contains("2018") {
        update_record!(0x07E2, SpellsBiMapEntry, |e: &mut SpellsBiMapEntry| {
            if let Ok(rel) = val1.trim().parse::<u16>() {
                e.scroll_item_id = rel;
            }
        });
    } else if category.contains("0x07E6") || category.contains("2022") {
        update_record!(0x07E6, RaceEntry, |e: &mut RaceEntry| {
            if let Some(aggro) = parse_numeric_key_u16(val1, "Aggro") {
                e.aggro_factor = aggro as u8;
            }
            if let Some(moral) = parse_numeric_key_u16(val1, "Moral") {
                e.moral = moral as u8;
            }
        });
    } else if category.contains("0x07E8") || category.contains("2024") {
        update_record!(0x07E8, UnitMasterEntry, |e: &mut UnitMasterEntry| {
            if let Ok(xp) = val1.trim().parse::<u32>() {
                e.xp_gain = xp;
            }
            if let Some(arm) = parse_numeric_key_u16(val2, "Armor") {
                e.armor = arm;
            }
        });
    } else if category.contains("0x07ED") || category.contains("2029") {
        update_record!(
            0x07ED,
            BuildingMasterEntry,
            |e: &mut BuildingMasterEntry| {
                if let Some(hp) = parse_numeric_key_u16(val1, "Health") {
                    e.health = hp;
                }
                if let Some(slots) = parse_numeric_key_u16(val1, "Slots") {
                    e.slots = slots as u8;
                }
            }
        );
    } else if category.contains("0x07F8") || category.contains("2040") {
        update_record!(0x07F8, UnitLootTableEntry, |e: &mut UnitLootTableEntry| {
            let items: Vec<u16> = val1
                .split(',')
                .filter_map(|s| s.trim().parse::<u16>().ok())
                .collect();
            if let Some(&i1) = items.first() {
                e.item1 = i1;
            }
            if let Some(&i2) = items.get(1) {
                e.item2 = i2;
            }
            if let Some(&i3) = items.get(2) {
                e.item3 = i3;
            }
        });
    } else if category.contains("0x0800") || category.contains("2048") {
        update_record!(
            0x0800,
            ComplexPropertyEntry,
            |e: &mut ComplexPropertyEntry| {
                if let Ok(xp) = val1.trim().parse::<u32>() {
                    e.experience_required = xp;
                }
            }
        );
    } else if category.contains("0x0802") || category.contains("2050") {
        update_record!(0x0802, ObjectMasterEntry, |e: &mut ObjectMasterEntry| {
            if let Ok(res) = val1.trim().parse::<u16>() {
                e.resource_amount = res;
            }
        });
    } else if category.contains("0x07E9") || category.contains("2025") {
        update_record!(0x07E9, UnitEquipmentEntry, |e: &mut UnitEquipmentEntry| {
            if let Ok(item) = val1.trim().parse::<u16>() {
                e.item_id = item;
            }
        });
    } else if category.contains("0x07FA") || category.contains("2042") {
        update_record!(
            0x07FA,
            MerchantInventoryEntry,
            |e: &mut MerchantInventoryEntry| {
                if let Ok(stk) = val1.trim().parse::<u16>() {
                    e.stock = stk;
                }
            }
        );
    } else if category.contains("0x080D") || category.contains("2061") {
        update_record!(0x080D, QuestEntry, |e: &mut QuestEntry| {
            if let Some(p) = parse_numeric_key_u16(val1, "Parent") {
                e.parent_quest_id = p;
            }
        });
    } else if category.contains("0x080F") || category.contains("2063") {
        update_record!(0x080F, WeaponTypeEntry, |e: &mut WeaponTypeEntry| {
            if let Ok(sh) = val1.trim().parse::<u8>() {
                e.sharpness = sh;
            }
        });
    } else if category.contains("0x0810") || category.contains("2064") {
        update_record!(
            0x0810,
            WeaponMaterialEntry,
            |e: &mut WeaponMaterialEntry| {
                if let Ok(nid) = val1.trim().parse::<u16>() {
                    e.name_id = nid;
                }
            }
        );
    } else if category.contains("0x0818") || category.contains("2072") {
        update_record!(0x0818, ItemSetEntry, |e: &mut ItemSetEntry| {
            if let Ok(desc) = val1.trim().parse::<u16>() {
                e.description_id = desc;
            }
        });
    } else if category.contains("0x07F0") || category.contains("2032") {
        update_record!(
            0x07F0,
            TerrainCultivationEntry,
            |e: &mut TerrainCultivationEntry| {
                if let Ok(b) = val1.trim().parse::<u8>() {
                    e.block_value = b;
                }
            }
        );
    } else if category.contains("0x0805") || category.contains("2053") {
        update_record!(0x0805, PortalEntry, |e: &mut PortalEntry| {
            if let Ok(m) = val1.trim().parse::<u32>() {
                e.map_id = m as u16;
            }
        });
    } else if category.contains("0x080A") || category.contains("2058") {
        update_record!(0x080A, DescriptionEntry, |e: &mut DescriptionEntry| {
            if let Ok(t) = val1.trim().parse::<u16>() {
                e.text_id = t;
            }
        });
    }

    Ok(())
}
