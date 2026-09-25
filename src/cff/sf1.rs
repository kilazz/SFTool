// src/cff/sf1.rs

use super::container::Manifest;
use super::editor::EditorItem;
use super::formulas::{calculate_cascade_loot_chances, calculate_total_xp, calculate_weapon_dps};
use super::sf1_schema::*;
use byteorder::{LittleEndian, ReadBytesExt};
use std::fs;
use std::io::Cursor;
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
        stride: 76,
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
        stride: 64,
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
        stride: 23,
        default_c_type: 1,
        description: "Building health & tech (Cat 2029)",
    },
    Sf1ChunkInfo {
        id: 0x07EE,
        name: "BuildingCollision",
        stride: 0,
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
        stride: 90,
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
        stride: 11,
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
        stride: 54,
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
        stride: 13,
        default_c_type: 3,
        description: "World map portals (Cat 2053)",
    },
    Sf1ChunkInfo {
        id: 0x0806,
        name: "SpellLines",
        stride: 75,
        default_c_type: 1,
        description: "Magic schools lines (Cat 2054)",
    },
    Sf1ChunkInfo {
        id: 0x0807,
        name: "EngineConfigGlobal",
        stride: 6,
        default_c_type: 1,
        description: "Global Engine Config (Cat 2055)",
    },
    Sf1ChunkInfo {
        id: 0x0808,
        name: "SpellLineRequirements",
        stride: 6,
        default_c_type: 1,
        description: "Spell line tier prerequisites (Cat 2056)",
    },
    Sf1ChunkInfo {
        id: 0x0809,
        name: "ObjectCollision",
        stride: 0,
        default_c_type: 1,
        description: "Object Collision Polygons (Cat 2057)",
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
        stride: 17,
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
        stride: 11,
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

fn set_labels_12(arr: [&str; 12]) -> [String; 12] {
    std::array::from_fn(|i| arr[i].to_string())
}

fn parse_leading_num<T: std::str::FromStr>(s: &str) -> Option<T> {
    let num_str: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    num_str.parse::<T>().ok()
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

    // 1. Spells Master (0x07D2) - Verified 76-byte stride
    if (category.contains("0x07D2") || category.contains("2002"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07D2)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ SpellEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(e) = SpellEntry::decode(chunk_slice) {
                let req_str = e.format_skill_reqs();
                let faction = e.format_target_faction();
                let mode = e.format_target_mode();
                let display = format!(
                    "Spell #{:<5} [Line: {:<3}] | Req: {:<16} | Target: {} ({}) | Mana: {:<3} | CD: {:<5}ms | Range: {}-{}",
                    e.spell_id,
                    e.spell_line_id,
                    req_str,
                    faction,
                    mode,
                    e.mana_cost,
                    e.recast_time_ms,
                    e.min_range,
                    e.max_range,
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: e.spell_id.to_string(),
                        val1: e.mana_cost.to_string(),
                        val2: req_str.clone(),
                        p1: e.mana_cost.to_string(),
                        p2: e.cast_time_ms.to_string(),
                        p3: e.recast_time_ms.to_string(),
                        p4: e.min_range.to_string(),
                        p5: e.max_range.to_string(),
                        p6: format!("{} - {}", e.cast_target_faction, faction),
                        p7: format!("{} - {}", e.cast_target_mode, mode),
                        p8: e.effect_power.to_string(),
                        p9: e.effect_range.to_string(),
                        p10: e.spell_line_id.to_string(),
                        p11: e.params[0].to_string(),
                        p12: e.params[1].to_string(),
                        labels: set_labels_12([
                            "Mana Cost:",
                            "Cast Time (ms):",
                            "Cooldown (ms):",
                            "Min Range:",
                            "Max Range:",
                            "Target Faction (1=Enemy, 2=Ally):",
                            "Target Mode (1=Figure, 5=Area):",
                            "Power / Success Rate (%):",
                            "Aura / AoE Radius:",
                            "Spell Line ID Link:",
                            "Param 0 (Base Dmg / Summon ID):",
                            "Param 1 (Scaling / Duration):",
                        ]),
                        display,
                    });
                }
            }
        }
    }
    // 2. Spell Lines (0x0806 / Cat 2054) - Verified 75-byte stride
    else if (category.contains("0x0806") || category.contains("2054"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x0806)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ SpellLineEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(l) = SpellLineEntry::decode(chunk_slice) {
                let school = l.format_school();
                let aura_tag = if l.is_aura() { " [Aura]" } else { "" };
                let display = format!(
                    "SpellLine #{:<3} [{}] | School: {}{} | MaxLvl: {} | NameID: {}",
                    l.line_id, l.icon_name, school, aura_tag, l.max_level, l.name_id
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: l.line_id.to_string(),
                        val1: l.icon_name.clone(),
                        val2: school.to_string(),
                        p1: l.icon_name,
                        p2: school.to_string(),
                        p3: l.max_level.to_string(),
                        p4: format!("0x{:02X}", l.line_flags),
                        p5: l.name_id.to_string(),
                        p6: l.description_id.to_string(),
                        p7: l.ui_order.to_string(),
                        p8: l.sub_school_id.to_string(),
                        labels: set_labels_12([
                            "Icon Texture Name:",
                            "Magic School:",
                            "Max Spell Level (12/20):",
                            "Behavior Flags (Aura/Shield):",
                            "Name Text ID:",
                            "Description Text ID:",
                            "UI Sort Order:",
                            "Sub-School Index:",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 3. Tech Tree Upgrades (0x07F4 / Cat 2036) - Verified 90-byte stride
    else if (category.contains("0x07F4") || category.contains("2036"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07F4)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ TechTreeUpgradeEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(u) = TechTreeUpgradeEntry::decode(chunk_slice) {
                let display = format!(
                    "Upgrade #{:<3} [Bld: {:<3}] [{}] | Time: {}s | NameID: {}",
                    u.upgrade_id,
                    u.building_id,
                    u.icon_name,
                    u.research_time_ms / 1000,
                    u.name_id
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: u.upgrade_id.to_string(),
                        val1: u.icon_name.clone(),
                        val2: format!("Bld: {}", u.building_id),
                        p1: u.icon_name,
                        p2: u.building_id.to_string(),
                        p3: u.research_time_ms.to_string(),
                        p4: u.costs[0].to_string(),
                        p5: u.costs[1].to_string(),
                        p6: u.costs[2].to_string(),
                        p7: u.costs[3].to_string(),
                        p8: u.costs[4].to_string(),
                        p9: u.costs[5].to_string(),
                        p10: u.costs[6].to_string(),
                        p11: u.name_id.to_string(),
                        p12: u.description_id.to_string(),
                        labels: set_labels_12([
                            "Button Texture Asset:",
                            "Building ID (Monument/Forge):",
                            "Research Duration (ms):",
                            "Wood Resource Cost:",
                            "Stone Resource Cost:",
                            "Iron Resource Cost:",
                            "Lenya Resource Cost:",
                            "Aria Resource Cost:",
                            "Moonglass Resource Cost:",
                            "Food Resource Cost:",
                            "Button Name Text ID:",
                            "Description Text ID:",
                        ]),
                        display,
                    });
                }
            }
        }
    }
    // 4. Items Master (0x07D3 / Cat 2003)
    else if (category.contains("0x07D3") || category.contains("2003"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07D3)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ ItemMasterEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(e) = ItemMasterEntry::decode(chunk_slice) {
                let display = format!(
                    "Item #{:<5} | Buy: {:<6}c | Sell: {:<6}c | Type: ({}, {})",
                    e.item_id, e.buy_value, e.sell_value, e.item_type1, e.item_type2
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: e.item_id.to_string(),
                        val1: e.buy_value.to_string(),
                        val2: e.sell_value.to_string(),
                        p1: e.buy_value.to_string(),
                        p2: e.sell_value.to_string(),
                        p3: e.name_id.to_string(),
                        p4: e.unit_stats_id.to_string(),
                        p5: e.army_unit_id.to_string(),
                        p6: e.building_id.to_string(),
                        p7: format!("0x{:02X}", e.option_flags),
                        p8: e.item_set_id.to_string(),
                        labels: set_labels_12([
                            "Buy Price (Copper):",
                            "Sell Price (Copper):",
                            "Localized Name ID:",
                            "UnitStats ID Link:",
                            "Army Unit ID Link:",
                            "Building ID Link:",
                            "Option Flags:",
                            "Item Set ID:",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 5. Item Stats Modifiers (0x07D4 / Cat 2004)
    else if (category.contains("0x07D4") || category.contains("2004"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07D4)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ ItemStatsModifierEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(m) = ItemStatsModifierEntry::decode(chunk_slice) {
                let display = format!(
                    "Item Mod #{:<5} | Str: {:>+3} Sta: {:>+3} Agi: {:>+3} Dex: {:>+3} | Armor: {:>+3}",
                    m.item_id, m.strength, m.stamina, m.agility, m.dexterity, m.armor
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: m.item_id.to_string(),
                        p1: m.strength.to_string(),
                        p2: m.stamina.to_string(),
                        p3: m.agility.to_string(),
                        p4: m.dexterity.to_string(),
                        p5: m.armor.to_string(),
                        p6: m.health.to_string(),
                        p7: m.mana.to_string(),
                        p8: m.resist_fire.to_string(),
                        p9: m.resist_ice.to_string(),
                        p10: m.resist_black.to_string(),
                        p11: m.resist_mind.to_string(),
                        p12: format!("{}/{}/{}", m.speed_walk, m.speed_fight, m.speed_cast),
                        labels: set_labels_12([
                            "Strength:",
                            "Stamina:",
                            "Agility:",
                            "Dexterity:",
                            "Armor Class:",
                            "Health Bonus:",
                            "Mana Bonus:",
                            "Fire Resist:",
                            "Ice Resist:",
                            "Black Resist:",
                            "Mind Resist:",
                            "Speeds (W/F/C):",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 6. Unit Stats (0x07D5 / Cat 2005)
    else if (category.contains("0x07D5") || category.contains("2005"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07D5)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ UnitStatsEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
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
                        record_index: rec_idx,
                        id_str: u.stats_id.to_string(),
                        val1: u.unit_level.to_string(),
                        val2: u.unit_race.to_string(),
                        p1: u.unit_level.to_string(),
                        p2: u.unit_race.to_string(),
                        p3: u.strength.to_string(),
                        p4: u.stamina.to_string(),
                        p5: u.agility.to_string(),
                        p6: u.dexterity.to_string(),
                        p7: u.intelligence.to_string(),
                        p8: u.wisdom.to_string(),
                        p9: u.charisma.to_string(),
                        p10: u.res_fire.to_string(),
                        p11: format!("HP: {}, MP: {}", hp, mana),
                        p12: format!(
                            "Walk: {}, Fight: {}, Cast: {}",
                            u.speed_walk, u.speed_fight, u.speed_cast
                        ),
                        labels: set_labels_12([
                            "Unit Level:",
                            "Race ID:",
                            "Strength:",
                            "Stamina:",
                            "Agility:",
                            "Dexterity:",
                            "Intelligence:",
                            "Wisdom:",
                            "Charisma:",
                            "Fire Resistance:",
                            "Effective Health / Mana:",
                            "Movement & Combat Speeds:",
                        ]),
                        display,
                    });
                }
            }
        }
    }
    // 7. 2D Gfx Items (0x07DC / Cat 2012)
    else if (category.contains("0x07DC") || category.contains("2012"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DC)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ Gfx2dItemEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(g) = Gfx2dItemEntry::decode(chunk_slice) {
                let display = format!(
                    "ID: {:<5} [Flag: {}] | Mesh: {}",
                    g.item_id, g.flag, g.mesh_name
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: g.item_id.to_string(),
                        val1: g.mesh_name.clone(),
                        p1: g.mesh_name,
                        p2: g.flag.to_string(),
                        p3: g.extra.to_string(),
                        labels: set_labels_12([
                            "Mesh / Icon Texture Name:",
                            "Type Flag (1=Scroll, 2=Spell, 0=Item):",
                            "Extra Value:",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 8. Weapon Stats (0x07DF / Cat 2015)
    else if (category.contains("0x07DF") || category.contains("2015"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DF)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ WeaponStatsEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(w) = WeaponStatsEntry::decode(chunk_slice) {
                let dps = calculate_weapon_dps(w.min_damage, w.max_damage, w.speed);
                let display = format!(
                    "Weapon #{:<5} | Dmg: {}-{} [DPS: {:.1}] | Spd: {}% | Type: {}",
                    w.item_id, w.min_damage, w.max_damage, dps, w.speed, w.weapon_type
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: w.item_id.to_string(),
                        val1: w.min_damage.to_string(),
                        val2: w.max_damage.to_string(),
                        p1: w.min_damage.to_string(),
                        p2: w.max_damage.to_string(),
                        p3: w.speed.to_string(),
                        p4: format!("{:.2}", dps),
                        p5: w.min_range.to_string(),
                        p6: w.max_range.to_string(),
                        p7: w.weapon_type.to_string(),
                        p8: w.material.to_string(),
                        labels: set_labels_12([
                            "Min Damage:",
                            "Max Damage:",
                            "Attack Speed (%):",
                            "Calculated Live DPS:",
                            "Min Range:",
                            "Max Range:",
                            "Weapon Type ID:",
                            "Material ID:",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 9. Spells BiMap (0x07E2 / Cat 2018)
    else if (category.contains("0x07E2") || category.contains("2018"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E2)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ SpellsBiMapEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(b) = SpellsBiMapEntry::decode(chunk_slice) {
                let display = format!(
                    "Spell ID: {:<5} -> Scroll Item ID: {}",
                    b.spell_id, b.scroll_item_id
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: b.spell_id.to_string(),
                        val1: b.scroll_item_id.to_string(),
                        p1: b.scroll_item_id.to_string(),
                        labels: set_labels_12([
                            "Mapped Scroll Item ID:",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 10. Races (0x07E6 / Cat 2022)
    else if (category.contains("0x07E6") || category.contains("2022"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E6)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ RaceEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(r) = RaceEntry::decode(chunk_slice) {
                let clan = get_clan_name(r.faction_id as u8);
                let display = format!(
                    "Race #{:<3} [{}] | Vis: Day {}/Night {} | Aggro: {} | Moral: {}",
                    r.race_id, clan, r.vis_day, r.vis_night, r.aggro_factor, r.moral
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: r.race_id.to_string(),
                        p1: r.aggro_factor.to_string(),
                        p2: r.moral.to_string(),
                        p3: r.aggressiveness.to_string(),
                        p4: format!("{} ({})", r.faction_id, clan),
                        p5: r.vis_day.to_string(),
                        p6: r.vis_night.to_string(),
                        p7: r.hear_range.to_string(),
                        p8: format!("0x{:04X}", r.ai_flags),
                        labels: set_labels_12([
                            "Aggro Factor:",
                            "Morale:",
                            "Aggressiveness:",
                            "Faction / Clan:",
                            "Vision (Day):",
                            "Vision (Night):",
                            "Hearing Range:",
                            "AI Flags Bitmask:",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 11. Units Master (0x07E8 / Cat 2024)
    else if (category.contains("0x07E8") || category.contains("2024"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E8)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ UnitMasterEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(u) = UnitMasterEntry::decode(chunk_slice) {
                let max_farm_xp = calculate_total_xp(u.xp_gain, u.xp_falloff, 500);
                let display = format!(
                    "Unit #{:<5} [{}] | Stats: {} | Base XP: {} | Loot: {}c",
                    u.unit_id, u.internal_name, u.stats_id, u.xp_gain, u.copper
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: u.unit_id.to_string(),
                        p1: u.internal_name,
                        p2: u.xp_gain.to_string(),
                        p3: u.xp_falloff.to_string(),
                        p4: u.copper.to_string(),
                        p5: u.stats_id.to_string(),
                        p6: u.name_id.to_string(),
                        p7: u.spawn_flag.to_string(),
                        p8: max_farm_xp.to_string(),
                        labels: set_labels_12([
                            "Internal Developer Name:",
                            "Base XP Gain:",
                            "XP Falloff Curve:",
                            "Copper Drop:",
                            "UnitStats ID Link:",
                            "Localized Name ID:",
                            "Spawn Flag:",
                            "Max XP (500 Kills):",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 12. Buildings Master (0x07ED / Cat 2029)
    else if (category.contains("0x07ED") || category.contains("2029"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07ED)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ BuildingMasterEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(b) = BuildingMasterEntry::decode(chunk_slice) {
                let display = format!(
                    "Building #{:<5} [Race {}] | HP: {} | Workers: {}ms | Slots: {}",
                    b.building_id, b.race_id, b.health, b.worker_cycle_time, b.slots
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: b.building_id.to_string(),
                        p1: b.health.to_string(),
                        p2: b.slots.to_string(),
                        p3: b.worker_cycle_time.to_string(),
                        p4: b.building_req_id.to_string(),
                        p5: b.name_id.to_string(),
                        p6: b.race_id.to_string(),
                        p7: b.initial_angle.to_string(),
                        p8: format!("({},{})", b.rot_center_x, b.rot_center_y),
                        labels: set_labels_12([
                            "Health (HP):",
                            "Worker Slots:",
                            "Worker Cycle (ms):",
                            "Prerequisite Building ID:",
                            "Localized Name Text ID:",
                            "Faction Race ID:",
                            "Initial Placement Angle:",
                            "Center Offset (X,Y):",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 13. Building Collision Polygons (0x07EE / Cat 2030)
    else if (category.contains("0x07EE") || category.contains("2030"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07EE)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        let mut cur = Cursor::new(&bytes);
        let mut rec_idx = 0;
        while (cur.position() as usize) < bytes.len() {
            if cur.position() as usize + 5 > bytes.len() {
                break;
            }
            let bld_id = cur.read_u16::<LittleEndian>().unwrap_or(0);
            let poly_idx = cur.read_u8().unwrap_or(0);
            let flag = cur.read_u8().unwrap_or(0);
            let vertex_count = cur.read_u8().unwrap_or(0);

            let mut coords = Vec::new();
            for _ in 0..vertex_count {
                if cur.position() as usize + 4 > bytes.len() {
                    break;
                }
                let x = cur.read_i16::<LittleEndian>().unwrap_or(0);
                let y = cur.read_i16::<LittleEndian>().unwrap_or(0);
                coords.push(format!("({},{})", x, y));
            }

            let display = format!(
                "BuildingCollision #{:<4} [Poly #{}] | Vertices: {:<2} | Flag: {}",
                bld_id, poly_idx, vertex_count, flag
            );
            if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                items.push(EditorItem {
                    record_index: rec_idx,
                    id_str: format!("{}:{}", bld_id, poly_idx),
                    p1: vertex_count.to_string(),
                    p2: flag.to_string(),
                    p3: coords.join(" "),
                    labels: set_labels_12([
                        "Vertex Count:",
                        "Collision Pass Flag:",
                        "Vertices Vector List (X,Y):",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                    ]),
                    display,
                    ..Default::default()
                });
            }
            rec_idx += 1;
        }
    }
    // 14. Unit Loot Tables (0x07F8 / Cat 2040)
    else if (category.contains("0x07F8") || category.contains("2040"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07F8)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ UnitLootTableEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(l) = UnitLootTableEntry::decode(chunk_slice) {
                let (eff1, eff2, eff3) = calculate_cascade_loot_chances(l.chance1, l.chance2);
                let display = format!(
                    "Loot Unit #{:<5} [Slot {}] | Items: {}, {}, {}",
                    l.unit_id, l.slot, l.item1, l.item2, l.item3
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: format!("{}:{}", l.unit_id, l.slot),
                        val1: l.item1.to_string(),
                        val2: l.item2.to_string(),
                        p1: l.slot.to_string(),
                        p2: l.item1.to_string(),
                        p3: l.chance1.to_string(),
                        p4: l.item2.to_string(),
                        p5: l.chance2.to_string(),
                        p6: l.item3.to_string(),
                        p7: format!("{:.1}%", eff1),
                        p8: format!("{:.1}%", eff2),
                        p9: format!("{:.1}%", eff3),
                        labels: set_labels_12([
                            "Drop Slot Number:",
                            "Primary Item 1 ID:",
                            "Item 1 Drop Chance (%):",
                            "Secondary Item 2 ID:",
                            "Item 2 Drop Chance (%):",
                            "Fallback Item 3 ID:",
                            "Effective Item 1 Odds (%):",
                            "Effective Item 2 Odds (%):",
                            "Effective Empty Odds (%):",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 15. Chest Loot Tables (0x0811 / Cat 2065)
    else if (category.contains("0x0811") || category.contains("2065"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x0811)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ ObjectLootTableEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(l) = ObjectLootTableEntry::decode(chunk_slice) {
                let (eff1, eff2, eff3) = calculate_cascade_loot_chances(l.chance1, l.chance2);
                let display = format!(
                    "Chest Loot #{:<4} [Slot {}] | Items: {}, {}, {}",
                    l.object_id, l.slot, l.item1, l.item2, l.item3
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: format!("{}:{}", l.object_id, l.slot),
                        val1: l.item1.to_string(),
                        val2: l.item2.to_string(),
                        p1: l.slot.to_string(),
                        p2: l.item1.to_string(),
                        p3: l.chance1.to_string(),
                        p4: l.item2.to_string(),
                        p5: l.chance2.to_string(),
                        p6: l.item3.to_string(),
                        p7: format!("{:.1}%", eff1),
                        p8: format!("{:.1}%", eff2),
                        p9: format!("{:.1}%", eff3),
                        labels: set_labels_12([
                            "Drop Slot Number:",
                            "Primary Item 1 ID:",
                            "Item 1 Drop Chance (%):",
                            "Secondary Item 2 ID:",
                            "Item 2 Drop Chance (%):",
                            "Fallback Item 3 ID:",
                            "Effective Item 1 Odds (%):",
                            "Effective Item 2 Odds (%):",
                            "Effective Empty Odds (%):",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 16. Unit Equipment (0x07E9 / Cat 2025)
    else if (category.contains("0x07E9") || category.contains("2025"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E9)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ UnitEquipmentEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(e) = UnitEquipmentEntry::decode(chunk_slice) {
                let slot_name = get_equipment_slot_name(e.equipment_slot);
                let display = format!(
                    "Unit #{:<5} [{}] -> Item #{}",
                    e.unit_id, slot_name, e.item_id
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: format!("{}:{}", e.unit_id, e.equipment_slot),
                        p1: e.item_id.to_string(),
                        p2: e.equipment_slot.to_string(),
                        p3: slot_name.to_string(),
                        labels: set_labels_12([
                            "Equipped Item ID:",
                            "Equipment Slot Index:",
                            "Slot Name:",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 17. Merchant Inventory (0x07FA / Cat 2042)
    else if (category.contains("0x07FA") || category.contains("2042"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07FA)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ MerchantInventoryEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(m) = MerchantInventoryEntry::decode(chunk_slice) {
                let display = format!(
                    "Merchant #{:<5} sells Item #{:<5} (Stock: {})",
                    m.merchant_id, m.item_id, m.stock
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: format!("{}:{}", m.merchant_id, m.item_id),
                        p1: m.stock.to_string(),
                        p2: m.merchant_id.to_string(),
                        p3: m.item_id.to_string(),
                        labels: set_labels_12([
                            "Inventory Stock Quantity:",
                            "Merchant Unit ID:",
                            "Sold Item ID:",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 18. Complex Properties / Level Progression (0x0800 / Cat 2048)
    else if (category.contains("0x0800") || category.contains("2048"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x0800)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ ComplexPropertyEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(cp) = ComplexPropertyEntry::decode(chunk_slice) {
                let display = format!(
                    "Level #{:<2} | XP Req: {:<8} | HP Factor: {}% | MP Factor: {}%",
                    cp.level, cp.experience_required, cp.health_factor, cp.mana_factor
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: cp.level.to_string(),
                        p1: cp.experience_required.to_string(),
                        p2: cp.health_factor.to_string(),
                        p3: cp.mana_factor.to_string(),
                        p4: cp.damage_factor.to_string(),
                        p5: cp.armor_class_factor.to_string(),
                        p6: cp.attribute_point_limit.to_string(),
                        p7: cp.skill_point_limit.to_string(),
                        labels: set_labels_12([
                            "XP Required for Level:",
                            "HP Scaling Factor (%):",
                            "Mana Scaling Factor (%):",
                            "Damage Factor (%):",
                            "Armor Factor (%):",
                            "Attribute Point Limit:",
                            "Skill Point Limit:",
                            "",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 19. Objects Master (0x0802 / Cat 2050) - 54-byte verified stride
    else if (category.contains("0x0802") || category.contains("2050"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x0802)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ ObjectMasterEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(o) = ObjectMasterEntry::decode(chunk_slice) {
                let mut flags_tags = Vec::new();
                if o.contains_loot() {
                    flags_tags.push("Loot");
                }
                if o.blocks_terrain() {
                    flags_tags.push("Block");
                }
                if o.is_placeable() {
                    flags_tags.push("Placeable");
                }
                if o.adjusts_height() {
                    flags_tags.push("Height");
                }
                let tag_str = if flags_tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", flags_tags.join(","))
                };

                let display = format!(
                    "Object #{:<4} [{}] | Res: {}{}",
                    o.object_id, o.category_name, o.resource_amount, tag_str
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: o.object_id.to_string(),
                        p1: o.category_name,
                        p2: o.resource_amount.to_string(),
                        p3: o.width.to_string(),
                        p4: o.height.to_string(),
                        p5: o.name_id.to_string(),
                        p6: format!("0x{:02X}", o.flags),
                        p7: o.flatten_mode.to_string(),
                        p8: o.polygon_num.to_string(),
                        labels: set_labels_12([
                            "Internal Category Path:",
                            "Resource Harvest Amount (Wood):",
                            "Model Footprint Width:",
                            "Model Footprint Height:",
                            "Localized Name Text ID:",
                            "Object Behavior Flags Bitmask:",
                            "Terrain Flatten Mode:",
                            "Collision Polygon Count:",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 20. Quests (0x080D / Cat 2061)
    else if (category.contains("0x080D") || category.contains("2061"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x080D)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ QuestEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
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
                        record_index: rec_idx,
                        id_str: q.quest_id.to_string(),
                        p1: q.parent_quest_id.to_string(),
                        p2: q.is_main_quest.to_string(),
                        p3: q.name_id.to_string(),
                        p4: q.description_id.to_string(),
                        p5: q.order_index.to_string(),
                        labels: set_labels_12([
                            "Parent Quest ID Link:",
                            "Is Main Quest (1/0):",
                            "Quest Title Text ID:",
                            "Journal Text ID:",
                            "Order Index:",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 21. Weapon Types (0x080F / Cat 2063)
    else if (category.contains("0x080F") || category.contains("2063"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x080F)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ WeaponTypeEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(wt) = WeaponTypeEntry::decode(chunk_slice) {
                let display = format!(
                    "Weapon Type #{:<3} | NameID: {:<5} | Sharpness: {}%",
                    wt.type_id, wt.name_id, wt.sharpness
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: wt.type_id.to_string(),
                        p1: wt.sharpness.to_string(),
                        p2: wt.name_id.to_string(),
                        labels: set_labels_12([
                            "Weapon Sharpness (%):",
                            "Localized Name Text ID:",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 22. Weapon Materials (0x0810 / Cat 2064)
    else if (category.contains("0x0810") || category.contains("2064"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x0810)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ WeaponMaterialEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(wm) = WeaponMaterialEntry::decode(chunk_slice) {
                let display = format!(
                    "Weapon Material #{:<3} | NameID: {:<5}",
                    wm.material_id, wm.name_id
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: wm.material_id.to_string(),
                        p1: wm.name_id.to_string(),
                        labels: set_labels_12([
                            "Localized Name Text ID:",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 23. Item Sets (0x0818 / Cat 2072)
    else if (category.contains("0x0818") || category.contains("2072"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x0818)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ ItemSetEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(s) = ItemSetEntry::decode(chunk_slice) {
                let display = format!(
                    "Item Set #{:<3} [Type: {}] | DescID: {}",
                    s.set_id, s.set_type, s.description_id
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: s.set_id.to_string(),
                        p1: s.description_id.to_string(),
                        p2: s.set_type.to_string(),
                        labels: set_labels_12([
                            "Set Bonus Description ID:",
                            "Item Set Type ID:",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 24. Terrain Cultivation (0x07F0 / Cat 2032)
    else if (category.contains("0x07F0") || category.contains("2032"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07F0)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ TerrainCultivationEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(tc) = TerrainCultivationEntry::decode(chunk_slice) {
                let display = format!(
                    "Terrain #{:<3} | Block: {} | Cultivation: 0x{:02X}",
                    tc.terrain_id, tc.block_value, tc.cultivation_flags
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: tc.terrain_id.to_string(),
                        p1: tc.block_value.to_string(),
                        p2: format!("0x{:02X}", tc.cultivation_flags),
                        labels: set_labels_12([
                            "Block Value:",
                            "Cultivation Flags Bitmask:",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 25. Portals (0x0805 / Cat 2053)
    else if (category.contains("0x0805") || category.contains("2053"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x0805)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ PortalEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(p) = PortalEntry::decode(chunk_slice) {
                let def_tag = if p.is_default != 0 { " [Default]" } else { "" };
                let display = format!(
                    "Portal #{:<4} [Map: {:<2}]{} | Pos: ({}, {})",
                    p.portal_id, p.map_id, def_tag, p.pos_x, p.pos_y
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: p.portal_id.to_string(),
                        p1: p.map_id.to_string(),
                        p2: p.pos_x.to_string(),
                        p3: p.pos_y.to_string(),
                        p4: p.is_default.to_string(),
                        p5: p.name_id.to_string(),
                        labels: set_labels_12([
                            "Target Map ID:",
                            "Coordinate X:",
                            "Coordinate Y:",
                            "Default Bindstone (1/0):",
                            "Portal Name Text ID:",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 26. Descriptions (0x080A / Cat 2058)
    else if (category.contains("0x080A") || category.contains("2058"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x080A)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        for (rec_idx, chunk_slice) in bytes
            .as_chunks::<{ DescriptionEntry::STRIDE }>()
            .0
            .iter()
            .enumerate()
        {
            if let Ok(d) = DescriptionEntry::decode(chunk_slice) {
                let display = format!(
                    "Description #{:<5} -> TextID: {:<5}",
                    d.description_id, d.text_id
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: rec_idx,
                        id_str: d.description_id.to_string(),
                        p1: d.text_id.to_string(),
                        labels: set_labels_12([
                            "Target Text ID Link:",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                            "",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }
    // 27. Universal fallback for any other small table
    else if let Some(chunk) = manifest.chunks.iter().find(|c| {
        category.contains(&format!("0x{:04X}", c.id)) || category.contains(&c.id.to_string())
    }) && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
        && let Some(info) = get_sf1_chunk_info(chunk.id)
        && info.stride > 0
        && bytes.len() % info.stride == 0
    {
        let count = bytes.len() / info.stride;
        for i in 0..count {
            let offset = i * info.stride;
            let id = Cursor::new(&bytes[offset..offset + 2])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let display = format!("Record #{:<5} [Cat 0x{:04X}]", id, chunk.id);
            if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                items.push(EditorItem {
                    record_index: i,
                    id_str: id.to_string(),
                    p1: id.to_string(),
                    labels: set_labels_12([
                        "Record ID:",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                        "",
                    ]),
                    display,
                    ..Default::default()
                });
            }
        }
    }

    items
}

pub fn save_sf1_item(
    cff_dir: &Path,
    category: &str,
    index: usize,
    fields: &[String],
) -> std::io::Result<()> {
    let manifest_path = cff_dir.join("manifest.json");
    let m_str = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&m_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    // Dynamic-length collision chunks are skipped directly from flat UI rows
    if category.contains("0x0809")
        || category.contains("2057")
        || category.contains("0x07EE")
        || category.contains("2030")
    {
        return Ok(());
    }

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

    let p1 = fields.get(1).map(|s| s.trim()).unwrap_or("");
    let p2 = fields.get(2).map(|s| s.trim()).unwrap_or("");
    let p3 = fields.get(3).map(|s| s.trim()).unwrap_or("");
    let p4 = fields.get(4).map(|s| s.trim()).unwrap_or("");
    let p5 = fields.get(5).map(|s| s.trim()).unwrap_or("");
    let p6 = fields.get(6).map(|s| s.trim()).unwrap_or("");
    let p7 = fields.get(7).map(|s| s.trim()).unwrap_or("");
    let p8 = fields.get(8).map(|s| s.trim()).unwrap_or("");
    let p9 = fields.get(9).map(|s| s.trim()).unwrap_or("");
    let p10 = fields.get(10).map(|s| s.trim()).unwrap_or("");
    let p11 = fields.get(11).map(|s| s.trim()).unwrap_or("");
    let p12 = fields.get(12).map(|s| s.trim()).unwrap_or("");

    if category.contains("0x07D2") || category.contains("2002") {
        update_record!(0x07D2, SpellEntry, |e: &mut SpellEntry| {
            if let Ok(m) = p1.parse::<u16>() {
                e.mana_cost = m;
            }
            if let Ok(c) = p2.parse::<u32>() {
                e.cast_time_ms = c;
            }
            if let Ok(r) = p3.parse::<u32>() {
                e.recast_time_ms = r;
            }
            if let Ok(min) = p4.parse::<u16>() {
                e.min_range = min;
            }
            if let Ok(max) = p5.parse::<u16>() {
                e.max_range = max;
            }
            if let Some(f) = parse_leading_num::<u8>(p6) {
                e.cast_target_faction = f;
            }
            if let Some(m) = parse_leading_num::<u8>(p7) {
                e.cast_target_mode = m;
            }
            if let Ok(pow) = p8.parse::<u16>() {
                e.effect_power = pow;
            }
            if let Ok(rad) = p9.parse::<u16>() {
                e.effect_range = rad;
            }
            if let Ok(line) = p10.parse::<u16>() {
                e.spell_line_id = line;
            }
            if let Ok(p0) = p11.parse::<u32>() {
                e.params[0] = p0;
            }
            if let Ok(p1_val) = p12.parse::<u32>() {
                e.params[1] = p1_val;
            }
        });
    } else if category.contains("0x0806") || category.contains("2054") {
        update_record!(0x0806, SpellLineEntry, |e: &mut SpellLineEntry| {
            if !p1.is_empty() {
                e.icon_name = p1.to_string();
            }
            if let Ok(lvl) = p3.parse::<u8>() {
                e.max_level = lvl;
            }
            if let Ok(desc) = p6.parse::<u16>() {
                e.description_id = desc;
            }
        });
    } else if category.contains("0x07F4") || category.contains("2036") {
        update_record!(
            0x07F4,
            TechTreeUpgradeEntry,
            |e: &mut TechTreeUpgradeEntry| {
                if !p1.is_empty() {
                    e.icon_name = p1.to_string();
                }
                if let Ok(bld) = p2.parse::<u16>() {
                    e.building_id = bld;
                }
                if let Ok(t) = p3.parse::<u32>() {
                    e.research_time_ms = t;
                }
                if let Ok(w) = p4.parse::<u16>() {
                    e.costs[0] = w;
                }
                if let Ok(s) = p5.parse::<u16>() {
                    e.costs[1] = s;
                }
                if let Ok(i) = p6.parse::<u16>() {
                    e.costs[2] = i;
                }
                if let Ok(l) = p7.parse::<u16>() {
                    e.costs[3] = l;
                }
                if let Ok(a) = p8.parse::<u16>() {
                    e.costs[4] = a;
                }
                if let Ok(m) = p9.parse::<u16>() {
                    e.costs[5] = m;
                }
                if let Ok(f) = p10.parse::<u16>() {
                    e.costs[6] = f;
                }
                if let Ok(name) = p11.parse::<u16>() {
                    e.name_id = name;
                }
                if let Ok(desc) = p12.parse::<u16>() {
                    e.description_id = desc;
                }
            }
        );
    } else if category.contains("0x07DF") || category.contains("2015") {
        update_record!(0x07DF, WeaponStatsEntry, |e: &mut WeaponStatsEntry| {
            if let Ok(min) = p1.parse::<u16>() {
                e.min_damage = min;
            }
            if let Ok(max) = p2.parse::<u16>() {
                e.max_damage = max;
            }
            if let Ok(spd) = p3.parse::<u16>() {
                e.speed = spd;
            }
            if let Ok(min_r) = p5.parse::<u16>() {
                e.min_range = min_r;
            }
            if let Ok(max_r) = p6.parse::<u16>() {
                e.max_range = max_r;
            }
        });
    } else if category.contains("0x07D5") || category.contains("2005") {
        update_record!(0x07D5, UnitStatsEntry, |e: &mut UnitStatsEntry| {
            if let Ok(lvl) = p1.parse::<u16>() {
                e.unit_level = lvl;
            }
            if let Ok(race) = p2.parse::<u8>() {
                e.unit_race = race;
            }
            if let Ok(str_val) = p3.parse::<u16>() {
                e.strength = str_val;
            }
            if let Ok(sta_val) = p4.parse::<u16>() {
                e.stamina = sta_val;
            }
            if let Ok(agi_val) = p5.parse::<u16>() {
                e.agility = agi_val;
            }
            if let Ok(dex_val) = p6.parse::<u16>() {
                e.dexterity = dex_val;
            }
            if let Ok(int_val) = p7.parse::<u16>() {
                e.intelligence = int_val;
            }
            if let Ok(wis_val) = p8.parse::<u16>() {
                e.wisdom = wis_val;
            }
            if let Ok(cha_val) = p9.parse::<u16>() {
                e.charisma = cha_val;
            }
        });
    } else if category.contains("0x07DC") || category.contains("2012") {
        update_record!(0x07DC, Gfx2dItemEntry, |e: &mut Gfx2dItemEntry| {
            if !p1.is_empty() {
                e.mesh_name = p1.to_string();
            }
        });
    } else if category.contains("0x07E8") || category.contains("2024") {
        update_record!(0x07E8, UnitMasterEntry, |e: &mut UnitMasterEntry| {
            if !p1.is_empty() {
                e.internal_name = p1.to_string();
            }
            if let Ok(xp) = p2.parse::<u32>() {
                e.xp_gain = xp;
            }
            if let Ok(c) = p4.parse::<u32>() {
                e.copper = c;
            }
        });
    } else if category.contains("0x07F8") || category.contains("2040") {
        update_record!(0x07F8, UnitLootTableEntry, |e: &mut UnitLootTableEntry| {
            if let Ok(i1) = p2.parse::<u16>() {
                e.item1 = i1;
            }
            if let Ok(c1) = p3.trim_end_matches('%').parse::<u8>() {
                e.chance1 = c1;
            }
            if let Ok(i2) = p4.parse::<u16>() {
                e.item2 = i2;
            }
            if let Ok(c2) = p5.trim_end_matches('%').parse::<u8>() {
                e.chance2 = c2;
            }
            if let Ok(i3) = p6.parse::<u16>() {
                e.item3 = i3;
            }
        });
    } else if category.contains("0x0811") || category.contains("2065") {
        update_record!(
            0x0811,
            ObjectLootTableEntry,
            |e: &mut ObjectLootTableEntry| {
                if let Ok(i1) = p2.parse::<u16>() {
                    e.item1 = i1;
                }
                if let Ok(c1) = p3.trim_end_matches('%').parse::<u8>() {
                    e.chance1 = c1;
                }
                if let Ok(i2) = p4.parse::<u16>() {
                    e.item2 = i2;
                }
                if let Ok(c2) = p5.trim_end_matches('%').parse::<u8>() {
                    e.chance2 = c2;
                }
                if let Ok(i3) = p6.parse::<u16>() {
                    e.item3 = i3;
                }
            }
        );
    } else if category.contains("0x0802") || category.contains("2050") {
        update_record!(0x0802, ObjectMasterEntry, |e: &mut ObjectMasterEntry| {
            if !p1.is_empty() {
                e.category_name = p1.to_string();
            }
            if let Ok(res) = p2.parse::<u16>() {
                e.resource_amount = res;
            }
            if let Ok(w) = p3.parse::<u16>() {
                e.width = w;
            }
            if let Ok(h) = p4.parse::<u16>() {
                e.height = h;
            }
        });
    } else if category.contains("0x07D3") || category.contains("2003") {
        update_record!(0x07D3, ItemMasterEntry, |e: &mut ItemMasterEntry| {
            if let Ok(b) = p1.parse::<u32>() {
                e.buy_value = b;
            }
            if let Ok(s) = p2.parse::<u32>() {
                e.sell_value = s;
            }
        });
    } else if category.contains("0x07ED") || category.contains("2029") {
        update_record!(
            0x07ED,
            BuildingMasterEntry,
            |e: &mut BuildingMasterEntry| {
                if let Ok(hp) = p1.parse::<u16>() {
                    e.health = hp;
                }
                if let Ok(s) = p2.parse::<u8>() {
                    e.slots = s;
                }
                if let Ok(w) = p3.parse::<u16>() {
                    e.worker_cycle_time = w;
                }
            }
        );
    } else if category.contains("0x07E9") || category.contains("2025") {
        update_record!(0x07E9, UnitEquipmentEntry, |e: &mut UnitEquipmentEntry| {
            if let Ok(item) = p1.parse::<u16>() {
                e.item_id = item;
            }
        });
    } else if category.contains("0x07FA") || category.contains("2042") {
        update_record!(
            0x07FA,
            MerchantInventoryEntry,
            |e: &mut MerchantInventoryEntry| {
                if let Ok(stk) = p1.parse::<u16>() {
                    e.stock = stk;
                }
            }
        );
    } else if category.contains("0x0800") || category.contains("2048") {
        update_record!(
            0x0800,
            ComplexPropertyEntry,
            |e: &mut ComplexPropertyEntry| {
                if let Ok(xp) = p1.parse::<u32>() {
                    e.experience_required = xp;
                }
                if let Ok(hp) = p2.parse::<u16>() {
                    e.health_factor = hp;
                }
                if let Ok(mp) = p3.parse::<u16>() {
                    e.mana_factor = mp;
                }
            }
        );
    } else if category.contains("0x080D") || category.contains("2061") {
        update_record!(0x080D, QuestEntry, |e: &mut QuestEntry| {
            if let Ok(p) = p1.parse::<u32>() {
                e.parent_quest_id = p;
            }
            if let Ok(m) = p2.parse::<u8>() {
                e.is_main_quest = m;
            }
        });
    } else if category.contains("0x080F") || category.contains("2063") {
        update_record!(0x080F, WeaponTypeEntry, |e: &mut WeaponTypeEntry| {
            if let Ok(sh) = p1.parse::<u8>() {
                e.sharpness = sh;
            }
        });
    } else if category.contains("0x0810") || category.contains("2064") {
        update_record!(
            0x0810,
            WeaponMaterialEntry,
            |e: &mut WeaponMaterialEntry| {
                if let Ok(nid) = p1.parse::<u16>() {
                    e.name_id = nid;
                }
            }
        );
    } else if category.contains("0x0818") || category.contains("2072") {
        update_record!(0x0818, ItemSetEntry, |e: &mut ItemSetEntry| {
            if let Ok(desc) = p1.parse::<u16>() {
                e.description_id = desc;
            }
        });
    } else if category.contains("0x07F0") || category.contains("2032") {
        update_record!(
            0x07F0,
            TerrainCultivationEntry,
            |e: &mut TerrainCultivationEntry| {
                if let Ok(b) = p1.parse::<u8>() {
                    e.block_value = b;
                }
            }
        );
    } else if category.contains("0x0805") || category.contains("2053") {
        update_record!(0x0805, PortalEntry, |e: &mut PortalEntry| {
            if let Ok(m) = p1.parse::<u32>() {
                e.map_id = m;
            }
            if let Ok(x) = p2.parse::<u16>() {
                e.pos_x = x;
            }
            if let Ok(y) = p3.parse::<u16>() {
                e.pos_y = y;
            }
        });
    } else if category.contains("0x080A") || category.contains("2058") {
        update_record!(0x080A, DescriptionEntry, |e: &mut DescriptionEntry| {
            if let Ok(t) = p1.parse::<u16>() {
                e.text_id = t;
            }
        });
    }

    Ok(())
}
