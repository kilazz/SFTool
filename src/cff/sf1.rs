// src/cff/sf1.rs

use super::container::Manifest;
use super::editor::EditorItem;
use super::formulas::{calculate_cascade_loot_chances, calculate_total_xp, calculate_weapon_dps};
use super::text::{decode_windows, encode_windows};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::{self, File};
use std::io::{Cursor, Write};
use std::path::Path;

// -----------------------------------------------------------------------------
// CHUNK DESCRIPTOR REGISTRY (ALL 49 SPELLFORCE 1 CATEGORIES)
// -----------------------------------------------------------------------------

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
        description: "Building requirements to train units (Cat 2001)",
    },
    Sf1ChunkInfo {
        id: 0x07D2,
        name: "SpellsMaster",
        stride: 72,
        default_c_type: 1,
        description: "Combat spells parameters, costs, and ranges (Cat 2002)",
    },
    Sf1ChunkInfo {
        id: 0x07D3,
        name: "ItemsMaster",
        stride: 22,
        default_c_type: 1,
        description: "Master item catalog, types, gold prices, and links (Cat 2003)",
    },
    Sf1ChunkInfo {
        id: 0x07D4,
        name: "ItemStatsModifiers",
        stride: 36,
        default_c_type: 1,
        description: "Equipment attribute and resistance bonuses (Cat 2004)",
    },
    Sf1ChunkInfo {
        id: 0x07D5,
        name: "UnitStats",
        stride: 46,
        default_c_type: 1,
        description: "Creature stats, speeds, and resistances (Cat 2005)",
    },
    Sf1ChunkInfo {
        id: 0x07D6,
        name: "UnitSkills",
        stride: 5,
        default_c_type: 1,
        description: "Skills and magic levels assigned to units (Cat 2006)",
    },
    Sf1ChunkInfo {
        id: 0x07DC,
        name: "2dGfxItems",
        stride: 69,
        default_c_type: 1,
        description: "2D Inventory icons and 3D mesh bindings (Cat 2012)",
    },
    Sf1ChunkInfo {
        id: 0x07DD,
        name: "SpellScrolls",
        stride: 4,
        default_c_type: 1,
        description: "Maps inventory scroll item to spell (Cat 2013)",
    },
    Sf1ChunkInfo {
        id: 0x07DE,
        name: "ItemEffects",
        stride: 5,
        default_c_type: 1,
        description: "Spell effects bound to weapons and armor (Cat 2014)",
    },
    Sf1ChunkInfo {
        id: 0x07DF,
        name: "WeaponStats",
        stride: 16,
        default_c_type: 1,
        description: "Weapon damages, speeds, and materials (Cat 2015)",
    },
    Sf1ChunkInfo {
        id: 0x07E0,
        name: "LocalizedStrings",
        stride: 566,
        default_c_type: 1,
        description: "Fixed 566-byte localized string tables (Cat 2016)",
    },
    Sf1ChunkInfo {
        id: 0x07E1,
        name: "ItemSkillReqs",
        stride: 5,
        default_c_type: 1,
        description: "Skill requirements to equip items (Cat 2017)",
    },
    Sf1ChunkInfo {
        id: 0x07E2,
        name: "SpellsBiMap",
        stride: 4,
        default_c_type: 1,
        description: "Bidirectional linkage between spell and scroll (Cat 2018)",
    },
    Sf1ChunkInfo {
        id: 0x07E6,
        name: "Races",
        stride: 27,
        default_c_type: 1,
        description: "Race visual ranges, morale, and AI behaviors (Cat 2022)",
    },
    Sf1ChunkInfo {
        id: 0x07E7,
        name: "DiplomacyMatrix",
        stride: 3,
        default_c_type: 1,
        description: "Faction relations: Neutral, Friendly, Hostile (Cat 2023)",
    },
    Sf1ChunkInfo {
        id: 0x07E8,
        name: "UnitsMaster",
        stride: 23,
        default_c_type: 1,
        description: "Unit database, loot copper, and XP falloff scaling (Cat 2024)",
    },
    Sf1ChunkInfo {
        id: 0x07E9,
        name: "UnitEquipment",
        stride: 5,
        default_c_type: 1,
        description: "Equipped inventory slots on units (Cat 2025)",
    },
    Sf1ChunkInfo {
        id: 0x07EA,
        name: "UnitSpellbook",
        stride: 5,
        default_c_type: 1,
        description: "Known castable spells per unit (Cat 2026)",
    },
    Sf1ChunkInfo {
        id: 0x07EC,
        name: "UnitResources",
        stride: 4,
        default_c_type: 1,
        description: "Resource costs to produce units (Cat 2028)",
    },
    Sf1ChunkInfo {
        id: 0x07ED,
        name: "BuildingsMaster",
        stride: 24,
        default_c_type: 1,
        description: "Building health, workers, and tech requirements (Cat 2029)",
    },
    Sf1ChunkInfo {
        id: 0x07EE,
        name: "BuildingCollision",
        stride: 8,
        default_c_type: 1,
        description: "2D collision polygon coordinates for buildings (Cat 2030)",
    },
    Sf1ChunkInfo {
        id: 0x07EF,
        name: "BuildingResources",
        stride: 5,
        default_c_type: 1,
        description: "Resource costs to construct buildings (Cat 2031)",
    },
    Sf1ChunkInfo {
        id: 0x07F0,
        name: "TerrainCultivation",
        stride: 4,
        default_c_type: 1,
        description: "Terrain farming flags: grain, mushroom, trees (Cat 2032)",
    },
    Sf1ChunkInfo {
        id: 0x07F4,
        name: "TechTreeUpgrades",
        stride: 34,
        default_c_type: 1,
        description: "Building research upgrades, costs, and times (Cat 2036)",
    },
    Sf1ChunkInfo {
        id: 0x07F7,
        name: "SkillsMaster",
        stride: 4,
        default_c_type: 1,
        description: "Skill classification and tree taxonomy (Cat 2039)",
    },
    Sf1ChunkInfo {
        id: 0x07F8,
        name: "UnitLootTables",
        stride: 12,
        default_c_type: 1,
        description: "Drop tables and chances for monsters (Cat 2040)",
    },
    Sf1ChunkInfo {
        id: 0x07F9,
        name: "MerchantsMaster",
        stride: 4,
        default_c_type: 1,
        description: "Merchants catalog and associated units (Cat 2041)",
    },
    Sf1ChunkInfo {
        id: 0x07FA,
        name: "MerchantInventory",
        stride: 6,
        default_c_type: 1,
        description: "Item inventory quantities sold by merchants (Cat 2042)",
    },
    Sf1ChunkInfo {
        id: 0x07FC,
        name: "Resources",
        stride: 3,
        default_c_type: 1,
        description: "Resource types and text identifiers (Cat 2044)",
    },
    Sf1ChunkInfo {
        id: 0x07FF,
        name: "MerchantPriceMultipliers",
        stride: 5,
        default_c_type: 1,
        description: "Price multipliers per item type (Cat 2047)",
    },
    Sf1ChunkInfo {
        id: 0x0800,
        name: "ComplexProperties",
        stride: 15,
        default_c_type: 3,
        description: "Level progression and health/damage scaling (Cat 2048)",
    },
    Sf1ChunkInfo {
        id: 0x0801,
        name: "HeadMeshes",
        stride: 2,
        default_c_type: 1,
        description: "Character head visual IDs (Cat 2049)",
    },
    Sf1ChunkInfo {
        id: 0x0802,
        name: "ObjectsMaster",
        stride: 22,
        default_c_type: 1,
        description: "Interactive map props, chests, and trees (Cat 2050)",
    },
    Sf1ChunkInfo {
        id: 0x0803,
        name: "NpcMaster",
        stride: 6,
        default_c_type: 1,
        description: "NPC name and speech links (Cat 2051)",
    },
    Sf1ChunkInfo {
        id: 0x0804,
        name: "MapsCatalog",
        stride: 71,
        default_c_type: 1,
        description: "Campaign and multiplayer map catalog (Cat 2052)",
    },
    Sf1ChunkInfo {
        id: 0x0805,
        name: "Portals",
        stride: 11,
        default_c_type: 1,
        description: "World map portal connections and locations (Cat 2053)",
    },
    Sf1ChunkInfo {
        id: 0x0806,
        name: "SpellLines",
        stride: 18,
        default_c_type: 1,
        description: "Magic schools lines and UI icons (Cat 2054)",
    },
    Sf1ChunkInfo {
        id: 0x0807,
        name: "UnknownTriple",
        stride: 3,
        default_c_type: 1,
        description: "Unknown 3-byte parameters (Cat 2055)",
    },
    Sf1ChunkInfo {
        id: 0x0808,
        name: "UnknownHex6",
        stride: 6,
        default_c_type: 1,
        description: "Unknown 6-byte parameters (Cat 2056)",
    },
    Sf1ChunkInfo {
        id: 0x0809,
        name: "ObjectCollision",
        stride: 8,
        default_c_type: 1,
        description: "2D collision polygon coordinates for objects (Cat 2057)",
    },
    Sf1ChunkInfo {
        id: 0x080A,
        name: "Descriptions",
        stride: 4,
        default_c_type: 1,
        description: "Tooltip description string identifiers (Cat 2058)",
    },
    Sf1ChunkInfo {
        id: 0x080B,
        name: "ExtendedDescriptions",
        stride: 6,
        default_c_type: 1,
        description: "Extended lore description identifiers (Cat 2059)",
    },
    Sf1ChunkInfo {
        id: 0x080D,
        name: "Quests",
        stride: 13,
        default_c_type: 1,
        description: "Quests tree, parents, and journal tags (Cat 2061)",
    },
    Sf1ChunkInfo {
        id: 0x080E,
        name: "SkillAttributeReqs",
        stride: 9,
        default_c_type: 1,
        description: "Attributes required per skill level (Cat 2062)",
    },
    Sf1ChunkInfo {
        id: 0x080F,
        name: "WeaponTypes",
        stride: 5,
        default_c_type: 1,
        description: "Weapon types and sharpness ratings (Cat 2063)",
    },
    Sf1ChunkInfo {
        id: 0x0810,
        name: "WeaponMaterials",
        stride: 4,
        default_c_type: 1,
        description: "Weapon crafting materials (Cat 2064)",
    },
    Sf1ChunkInfo {
        id: 0x0811,
        name: "ObjectLootTables",
        stride: 12,
        default_c_type: 1,
        description: "Chest and grave loot drop tables (Cat 2065)",
    },
    Sf1ChunkInfo {
        id: 0x0813,
        name: "UnitSpells",
        stride: 5,
        default_c_type: 1,
        description: "Combat spell slots assigned to units (Cat 2067)",
    },
    Sf1ChunkInfo {
        id: 0x0818,
        name: "ItemSets",
        stride: 4,
        default_c_type: 1,
        description: "Armor and weapon equipment set bonuses (Cat 2072)",
    },
];

pub fn get_sf1_chunk_info(id: u32) -> Option<Sf1ChunkInfo> {
    SF1_CATEGORY_TABLE
        .iter()
        .find(|info| info.id == id)
        .copied()
}

pub fn describe_category(id: u32) -> Option<&'static str> {
    get_sf1_chunk_info(id).map(|info| info.description)
}

// -----------------------------------------------------------------------------
// HELPER STRING PARSERS
// -----------------------------------------------------------------------------

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
        None
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
        None
    }
}

// -----------------------------------------------------------------------------
// EDITOR DATA LOADERS
// -----------------------------------------------------------------------------

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

    // 1. 2D Gfx Items (0x07DC / Cat 2012)
    if (category.contains("0x07DC") || category.contains("2012"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DC)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        let num_records = bytes.len() / 69;
        for i in 0..num_records {
            let offset = i * 69;
            let item_id = Cursor::new(&bytes[offset..offset + 2])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let flag = bytes[offset + 2];
            let mesh_bytes = &bytes[offset + 3..offset + 67];
            let end = mesh_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(mesh_bytes.len());
            let mesh_name = decode_windows(&mesh_bytes[..end]);
            let extra = Cursor::new(&bytes[offset + 67..offset + 69])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);

            let display = format!("ID: {:<5} [Flag: {}] | Mesh: {}", item_id, flag, mesh_name);
            if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                items.push(EditorItem {
                    id_str: item_id.to_string(),
                    val1: mesh_name,
                    val2: format!("Flag: {}, Extra: {}", flag, extra),
                    display,
                });
            }
        }
    }
    // 2. Spells BiMap (0x07E2 / Cat 2018)
    else if (category.contains("0x07E2") || category.contains("2018"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E2)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        let num_records = bytes.len() / 4;
        for i in 0..num_records {
            let offset = i * 4;
            let spell_id = Cursor::new(&bytes[offset..offset + 2])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let related_id = Cursor::new(&bytes[offset + 2..offset + 4])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);

            let display = format!(
                "Spell ID: {:<5} -> Scroll Item ID: {}",
                spell_id, related_id
            );
            if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                items.push(EditorItem {
                    id_str: spell_id.to_string(),
                    val1: related_id.to_string(),
                    val2: "BiMap Entry".into(),
                    display,
                });
            }
        }
    }
    // 3. Weapon Stats (0x07DF / Cat 2015)
    else if (category.contains("0x07DF") || category.contains("2015"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DF)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        let num_records = bytes.len() / 16;
        for i in 0..num_records {
            let offset = i * 16;
            let item_id = Cursor::new(&bytes[offset..offset + 2])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let min_dmg = Cursor::new(&bytes[offset + 2..offset + 4])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let max_dmg = Cursor::new(&bytes[offset + 4..offset + 6])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let min_rng = Cursor::new(&bytes[offset + 6..offset + 8])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let max_rng = Cursor::new(&bytes[offset + 8..offset + 10])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let speed = Cursor::new(&bytes[offset + 10..offset + 12])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let wpn_type = Cursor::new(&bytes[offset + 12..offset + 14])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let material = Cursor::new(&bytes[offset + 14..offset + 16])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);

            let dps = calculate_weapon_dps(min_dmg, max_dmg, speed);
            let display = format!(
                "Weapon #{:<5} | Dmg: {}-{} [DPS: {:.1}] | Spd: {}% | Type: {}",
                item_id, min_dmg, max_dmg, dps, speed, wpn_type
            );
            if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                items.push(EditorItem {
                    id_str: item_id.to_string(),
                    val1: format!("{}-{}", min_dmg, max_dmg),
                    val2: format!(
                        "Speed: {} | Range: {}-{} | Type: {} | Material: {} | DPS: {:.2}",
                        speed, min_rng, max_rng, wpn_type, material, dps
                    ),
                    display,
                });
            }
        }
    }
    // 4. Units Master (0x07E8 / Cat 2024)
    else if (category.contains("0x07E8") || category.contains("2024"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E8)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        let num_records = bytes.len() / 23;
        for i in 0..num_records {
            let offset = i * 23;
            let unit_id = Cursor::new(&bytes[offset..offset + 2])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let name_id = Cursor::new(&bytes[offset + 2..offset + 4])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let stats_id = Cursor::new(&bytes[offset + 4..offset + 6])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let xp_gain = Cursor::new(&bytes[offset + 6..offset + 10])
                .read_u32::<LittleEndian>()
                .unwrap_or(0);
            let xp_falloff = Cursor::new(&bytes[offset + 10..offset + 12])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let copper = Cursor::new(&bytes[offset + 12..offset + 16])
                .read_u32::<LittleEndian>()
                .unwrap_or(0);
            let armor = Cursor::new(&bytes[offset + 20..offset + 22])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);

            let max_farm_xp = calculate_total_xp(xp_gain, xp_falloff, 500);
            let display = format!(
                "Unit #{:<5} [Stats: {}] | Armor: {} | Base XP: {} | Loot: {}c",
                unit_id, stats_id, armor, xp_gain, copper
            );
            if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                items.push(EditorItem {
                    id_str: unit_id.to_string(),
                    val1: xp_gain.to_string(),
                    val2: format!(
                        "Armor: {} | Falloff: {} | Copper: {} | NameID: {} | Max XP: {}",
                        armor, xp_falloff, copper, name_id, max_farm_xp
                    ),
                    display,
                });
            }
        }
    }
    // 5. Unit Loot Tables (0x07F8 / Cat 2040)
    else if (category.contains("0x07F8") || category.contains("2040"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07F8)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
    {
        let num_records = bytes.len() / 12;
        for i in 0..num_records {
            let offset = i * 12;
            let unit_id = Cursor::new(&bytes[offset..offset + 2])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let slot = bytes[offset + 2];
            let item1 = Cursor::new(&bytes[offset + 3..offset + 5])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let ch1 = bytes[offset + 5];
            let item2 = Cursor::new(&bytes[offset + 6..offset + 8])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);
            let ch2 = bytes[offset + 8];
            let item3 = Cursor::new(&bytes[offset + 9..offset + 11])
                .read_u16::<LittleEndian>()
                .unwrap_or(0);

            let (eff1, eff2, eff3) = calculate_cascade_loot_chances(ch1, ch2);
            let display = format!(
                "Loot Unit #{:<5} [Slot {}] | Items: {}, {}, {}",
                unit_id, slot, item1, item2, item3
            );
            if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                items.push(EditorItem {
                    id_str: format!("{}:{}", unit_id, slot),
                    val1: format!("{}, {}, {}", item1, item2, item3),
                    val2: format!(
                        "Chances: {}%, {}% | Cascading: [{:.1}%, {:.1}%, {:.1}%]",
                        ch1, ch2, eff1, eff2, eff3
                    ),
                    display,
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
    _id_str: &str,
    val1: &str,
    val2: &str,
) -> std::io::Result<()> {
    let manifest_path = cff_dir.join("manifest.json");
    let m_str = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&m_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    // 1. 2D Gfx Items (0x07DC)
    if (category.contains("0x07DC") || category.contains("2012"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DC)
    {
        let chunk_path = cff_dir.join(&chunk.file);
        let mut bytes = fs::read(&chunk_path)?;
        let offset = index * 69;
        if offset + 69 <= bytes.len() {
            let enc_mesh = encode_windows(val1);
            let mut padded_mesh = vec![0u8; 64];
            let len = enc_mesh.len().min(63);
            padded_mesh[..len].copy_from_slice(&enc_mesh[..len]);
            bytes[offset + 3..offset + 67].copy_from_slice(&padded_mesh);
            File::create(chunk_path)?.write_all(&bytes)?;
        }
    }
    // 2. Spells BiMap (0x07E2)
    else if (category.contains("0x07E2") || category.contains("2018"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E2)
    {
        let chunk_path = cff_dir.join(&chunk.file);
        let mut bytes = fs::read(&chunk_path)?;
        let offset = index * 4;
        if offset + 4 <= bytes.len()
            && let Ok(new_rel) = val1.parse::<u16>()
        {
            let mut cur = Cursor::new(&mut bytes[offset + 2..offset + 4]);
            cur.write_u16::<LittleEndian>(new_rel)?;
            File::create(chunk_path)?.write_all(&bytes)?;
        }
    }
    // 3. Weapon Stats (0x07DF / Cat 2015)
    else if (category.contains("0x07DF") || category.contains("2015"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DF)
    {
        let chunk_path = cff_dir.join(&chunk.file);
        let mut bytes = fs::read(&chunk_path)?;
        let offset = index * 16;
        if offset + 16 <= bytes.len() {
            // Parse damage from val1: e.g. "10-25" or "15"
            let (min_dmg, max_dmg) = if val1.contains('-') {
                let parts: Vec<&str> = val1.split('-').collect();
                let min = parts
                    .first()
                    .and_then(|s| s.trim().parse::<u16>().ok())
                    .unwrap_or(1);
                let max = parts
                    .get(1)
                    .and_then(|s| s.trim().parse::<u16>().ok())
                    .unwrap_or(min);
                (min, max)
            } else {
                let d = val1.trim().parse::<u16>().unwrap_or(1);
                (d, d)
            };

            let mut cur = Cursor::new(&mut bytes[offset + 2..offset + 6]);
            cur.write_u16::<LittleEndian>(min_dmg)?;
            cur.write_u16::<LittleEndian>(max_dmg)?;

            if let Some(spd) = parse_numeric_key_u16(val2, "Speed") {
                let mut cur_spd = Cursor::new(&mut bytes[offset + 10..offset + 12]);
                cur_spd.write_u16::<LittleEndian>(spd)?;
            }

            File::create(chunk_path)?.write_all(&bytes)?;
        }
    }
    // 4. Units Master (0x07E8 / Cat 2024)
    else if (category.contains("0x07E8") || category.contains("2024"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E8)
    {
        let chunk_path = cff_dir.join(&chunk.file);
        let mut bytes = fs::read(&chunk_path)?;
        let offset = index * 23;
        if offset + 23 <= bytes.len() {
            if let Ok(new_xp) = val1.trim().parse::<u32>() {
                let mut cur_xp = Cursor::new(&mut bytes[offset + 6..offset + 10]);
                cur_xp.write_u32::<LittleEndian>(new_xp)?;
            }

            if let Some(armor) = parse_numeric_key_u16(val2, "Armor") {
                let mut cur_arm = Cursor::new(&mut bytes[offset + 20..offset + 22]);
                cur_arm.write_u16::<LittleEndian>(armor)?;
            }

            if let Some(falloff) = parse_numeric_key_u16(val2, "Falloff") {
                let mut cur_fo = Cursor::new(&mut bytes[offset + 10..offset + 12]);
                cur_fo.write_u16::<LittleEndian>(falloff)?;
            }

            if let Some(copper) = parse_numeric_key_u32(val2, "Copper") {
                let mut cur_cp = Cursor::new(&mut bytes[offset + 12..offset + 16]);
                cur_cp.write_u32::<LittleEndian>(copper)?;
            }

            File::create(chunk_path)?.write_all(&bytes)?;
        }
    }
    // 5. Unit Loot Tables (0x07F8 / Cat 2040)
    else if (category.contains("0x07F8") || category.contains("2040"))
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07F8)
    {
        let chunk_path = cff_dir.join(&chunk.file);
        let mut bytes = fs::read(&chunk_path)?;
        let offset = index * 12;
        if offset + 12 <= bytes.len() {
            let items: Vec<u16> = val1
                .split(',')
                .filter_map(|s| s.trim().parse::<u16>().ok())
                .collect();

            if let Some(&i1) = items.first() {
                let mut cur = Cursor::new(&mut bytes[offset + 3..offset + 5]);
                cur.write_u16::<LittleEndian>(i1)?;
            }
            if let Some(&i2) = items.get(1) {
                let mut cur = Cursor::new(&mut bytes[offset + 6..offset + 8]);
                cur.write_u16::<LittleEndian>(i2)?;
            }
            if let Some(&i3) = items.get(2) {
                let mut cur = Cursor::new(&mut bytes[offset + 9..offset + 11]);
                cur.write_u16::<LittleEndian>(i3)?;
            }

            if let Some(ch1) = parse_numeric_key_u16(val2, "Chances") {
                bytes[offset + 5] = (ch1.min(100)) as u8;
            }
            if let Some(pos) = val2.find(',') {
                let second_half = &val2[pos + 1..];
                let ch2_str: String = second_half
                    .chars()
                    .skip_while(|c| c.is_whitespace())
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(ch2) = ch2_str.parse::<u8>() {
                    bytes[offset + 8] = ch2.min(100);
                }
            }

            File::create(chunk_path)?.write_all(&bytes)?;
        }
    }

    Ok(())
}
