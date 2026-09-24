// src/cff/tracer.rs

use super::container::Manifest;
use super::references::EntityReference;
use super::sf1::get_sf1_chunk_info;
use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Cursor;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TracePoint {
    pub category_id: u32,
    pub entity_id: u32,
    pub record_index: usize,
}

pub struct TracerEngine {
    pub history_stack: Vec<TracePoint>,
    pub forward_stack: Vec<TracePoint>,
    pub table_groups: BTreeMap<u32, Vec<u32>>,
    pub redirects: BTreeMap<u32, u32>,
    pub foreign_keys: BTreeMap<u32, Vec<(u32, &'static str, usize, usize)>>,
}

impl Default for TracerEngine {
    fn default() -> Self {
        let mut table_groups = BTreeMap::new();
        let groups: [&[u32]; 6] = [
            &[2005, 2006, 2067],                               // Unit Stats group
            &[2003, 2004, 2013, 2015, 2017, 2014, 2012, 2018], // Items group
            &[2024, 2025, 2026, 2028, 2040, 2001],             // Units group
            &[2029, 2030, 2031],                               // Buildings group
            &[2041, 2042, 2047],                               // Merchants group
            &[2050, 2057, 2065],                               // Objects & Chests group
        ];
        for grp in groups {
            for &cat in grp {
                table_groups.insert(cat, grp.to_vec());
            }
        }

        let mut redirects = BTreeMap::new();
        for &c in &[2006, 2067] {
            redirects.insert(c, 2005);
        }
        for &c in &[2004, 2013, 2015, 2017, 2014, 2012, 2018] {
            redirects.insert(c, 2003);
        }
        for &c in &[2025, 2026, 2028, 2040, 2001] {
            redirects.insert(c, 2024);
        }
        for &c in &[2030, 2031] {
            redirects.insert(c, 2029);
        }
        for &c in &[2042, 2047] {
            redirects.insert(c, 2041);
        }
        for &c in &[2057, 2065] {
            redirects.insert(c, 2050);
        }

        let mut foreign_keys: BTreeMap<u32, Vec<(u32, &'static str, usize, usize)>> =
            BTreeMap::new();

        // 2002 -> Combat Spells
        foreign_keys.insert(
            2002,
            vec![
                (2067, "SpellID", 3, 2),
                (2014, "EffectID", 3, 2),
                (2018, "EffectID", 2, 2),
                (2026, "SpellID", 3, 2),
            ],
        );

        // 2054 -> SpellLines
        foreign_keys.insert(2054, vec![(2002, "SpellLineID", 2, 2)]);

        // 2005 -> UnitStatsID
        foreign_keys.insert(
            2005,
            vec![(2003, "UnitStatsID", 6, 2), (2024, "StatsID", 4, 2)],
        );

        // 2003 -> ItemID
        foreign_keys.insert(
            2003,
            vec![
                (2013, "InstalledScrollItemID", 2, 2),
                (2025, "EquipmentItemID", 3, 2),
                (2040, "LootItemID1", 3, 2),
                (2040, "LootItemID2", 6, 2),
                (2040, "LootItemID3", 9, 2),
                (2042, "MerchantStockItemID", 2, 2),
                (2065, "ChestLootItemID1", 3, 2),
                (2065, "ChestLootItemID2", 6, 2),
                (2065, "ChestLootItemID3", 9, 2),
            ],
        );

        // 2016 -> Localized TextID
        foreign_keys.insert(
            2016,
            vec![
                (2054, "TextID", 2, 2),
                (2003, "NameID", 4, 2),
                (2022, "TextID", 8, 2),
                (2024, "NameID", 2, 2),
                (2029, "NameID", 8, 2),
                (2039, "TextID", 2, 2),
                (2044, "TextID", 1, 2),
                (2050, "NameID", 2, 2),
                (2051, "TextID", 4, 2),
                (2052, "NameID", 69, 2),
                (2053, "NameID", 9, 2),
                (2058, "TextID", 2, 2),
                (2059, "TextID", 2, 2),
                (2059, "ExtTextID", 4, 2),
                (2061, "NameID", 5, 2),
                (2061, "DescriptionID", 7, 2),
                (2063, "NameID", 2, 2),
                (2064, "NameID", 2, 2),
                (2036, "ButtonNameID", 4, 2),
                (2072, "DescriptionID", 1, 2),
            ],
        );

        // 2022 -> Races
        foreign_keys.insert(2022, vec![(2005, "UnitRace", 4, 1)]);

        // 2023 -> Faction/Clan
        foreign_keys.insert(2023, vec![(2022, "FactionID", 9, 2)]);

        // 2024 -> UnitID
        foreign_keys.insert(
            2024,
            vec![
                (2003, "ArmyUnitID", 8, 2),
                (2041, "MerchantUnitID", 2, 2),
                (2025, "UnitID", 0, 2),
                (2026, "UnitID", 0, 2),
                (2028, "ArmyUnitID", 0, 2),
                (2040, "UnitID", 0, 2),
                (2001, "ArmyUnitID", 0, 2),
            ],
        );

        // 2029 -> BuildingID
        foreign_keys.insert(
            2029,
            vec![
                (2003, "BuildingID", 10, 2),
                (2001, "BuildingID", 3, 2),
                (2029, "BuildingReqID", 14, 2),
                (2036, "BuildingID", 2, 2),
                (2030, "BuildingID", 0, 2),
                (2031, "BuildingID", 0, 2),
            ],
        );

        // 2044 -> Resources
        foreign_keys.insert(
            2044,
            vec![(2028, "ResourceType", 2, 1), (2031, "ResourceID", 2, 1)],
        );

        // 2052 -> MapsCatalog
        foreign_keys.insert(2052, vec![(2053, "MapID", 2, 4)]);

        // 2058 -> Descriptions
        foreign_keys.insert(
            2058,
            vec![
                (2054, "DescriptionID", 16, 2),
                (2036, "ButtonDescriptionID", 6, 2),
            ],
        );

        // 2061 -> Quests
        foreign_keys.insert(2061, vec![(2061, "ParentQuestID", 2, 2)]);

        // 2063 -> WeaponTypes
        foreign_keys.insert(2063, vec![(2015, "WeaponType", 12, 2)]);

        // 2064 -> WeaponMaterials
        foreign_keys.insert(2064, vec![(2015, "WeaponMaterial", 14, 2)]);

        // 2072 -> ItemSets
        foreign_keys.insert(2072, vec![(2003, "ItemSetID", 21, 1)]);

        Self {
            history_stack: Vec::new(),
            forward_stack: Vec::new(),
            table_groups,
            redirects,
            foreign_keys,
        }
    }
}

impl TracerEngine {
    pub fn push_step(&mut self, point: TracePoint) {
        if self.history_stack.last() != Some(&point) {
            self.history_stack.push(point);
            self.forward_stack.clear();
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.history_stack.len() > 1
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    pub fn go_back(&mut self) -> Option<TracePoint> {
        if self.history_stack.len() > 1 {
            let current = self.history_stack.pop().unwrap();
            self.forward_stack.push(current);
            self.history_stack.last().copied()
        } else {
            None
        }
    }

    pub fn go_forward(&mut self) -> Option<TracePoint> {
        if let Some(next) = self.forward_stack.pop() {
            self.history_stack.push(next);
            Some(next)
        } else {
            None
        }
    }

    pub fn current(&self) -> Option<TracePoint> {
        self.history_stack.last().copied()
    }

    pub fn get_table_group(&self, category_id: u32) -> Option<&[u32]> {
        self.table_groups.get(&category_id).map(|v| v.as_slice())
    }

    pub fn get_redirect(&self, category_id: u32) -> u32 {
        self.redirects
            .get(&category_id)
            .copied()
            .unwrap_or(category_id)
    }

    pub fn resolve_entity_index(cff_dir: &Path, category_id: u32, target_id: u32) -> Option<usize> {
        let manifest_path = cff_dir.join("manifest.json");
        let manifest_str = fs::read_to_string(manifest_path).ok()?;
        let manifest: Manifest = serde_json::from_str(&manifest_str).ok()?;

        let chunk = manifest.chunks.iter().find(|c| c.id == category_id)?;
        let chunk_bytes = fs::read(cff_dir.join(&chunk.file)).ok()?;
        let stride = get_sf1_chunk_info(category_id)?.stride;
        if stride == 0 || chunk_bytes.len() % stride != 0 {
            return None;
        }

        let count = chunk_bytes.len() / stride;
        for i in 0..count {
            let offset = i * stride;
            let id = if category_id == 2051 || category_id == 2052 {
                Cursor::new(&chunk_bytes[offset..offset + 4])
                    .read_u32::<LittleEndian>()
                    .unwrap_or(0)
            } else if category_id == 2022 || category_id == 2048 || category_id == 2072 {
                chunk_bytes[offset] as u32
            } else {
                Cursor::new(&chunk_bytes[offset..offset + 2])
                    .read_u16::<LittleEndian>()
                    .unwrap_or(0) as u32
            };

            if id == target_id {
                return Some(i);
            }
        }
        None
    }

    pub fn find_references(
        &self,
        cff_dir: &Path,
        category_id: u32,
        target_id: u32,
    ) -> Vec<EntityReference> {
        let mut results = BTreeSet::new();
        let manifest_path = cff_dir.join("manifest.json");
        let Ok(manifest_str) = fs::read_to_string(manifest_path) else {
            return Vec::new();
        };
        let Ok(manifest) = serde_json::from_str::<Manifest>(&manifest_str) else {
            return Vec::new();
        };

        let effective_cat = self.get_redirect(category_id);
        if let Some(links) = self.foreign_keys.get(&effective_cat) {
            for &(source_cat, field_name, offset, size) in links {
                if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == source_cat)
                    && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
                {
                    let stride = get_sf1_chunk_info(source_cat)
                        .map(|i| i.stride)
                        .unwrap_or(0);
                    if stride > 0 && bytes.len() % stride == 0 {
                        let count = bytes.len() / stride;
                        for i in 0..count {
                            let base = i * stride;
                            if base + offset + size <= bytes.len() {
                                let mut cur =
                                    Cursor::new(&bytes[base + offset..base + offset + size]);
                                let val = if size == 1 {
                                    bytes[base + offset] as u32
                                } else if size == 2 {
                                    cur.read_u16::<LittleEndian>().unwrap_or(0) as u32
                                } else if size == 4 {
                                    cur.read_u32::<LittleEndian>().unwrap_or(0)
                                } else {
                                    bytes[base + offset] as u32
                                };
                                if val == target_id {
                                    results.insert(EntityReference {
                                        category_id: source_cat,
                                        category_name: chunk
                                            .name
                                            .clone()
                                            .unwrap_or_else(|| format!("0x{:04X}", source_cat)),
                                        record_index: i,
                                        target_id,
                                        field_name: field_name.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        results.into_iter().collect()
    }
}
