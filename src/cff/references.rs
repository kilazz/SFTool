// src/cff/references.rs

use super::container::Manifest;
use super::sf1::get_sf1_chunk_info;
use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Cursor;
use std::path::Path;

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct EntityReference {
    pub category_id: u32,
    pub category_name: String,
    pub record_index: usize,
    pub target_id: u32,
    pub field_name: String,
}

pub struct RelationalGraph {
    redirects: BTreeMap<u32, u32>,
    field_links: BTreeMap<u32, Vec<(u32, &'static str, usize, usize)>>,
}

impl Default for RelationalGraph {
    fn default() -> Self {
        let mut redirects = BTreeMap::new();
        // Item stats redirect to 2003 / 0x07D3
        for cat in [2004, 2012, 2013, 2014, 2015, 2017, 2018] {
            redirects.insert(cat, 2003);
        }
        // Unit sub-tables redirect to 2024 / 0x07E8
        for cat in [2025, 2026, 2028, 2040, 2001] {
            redirects.insert(cat, 2024);
        }
        // Building sub-tables redirect to 2029 / 0x07ED
        for cat in [2030, 2031] {
            redirects.insert(cat, 2029);
        }
        // Stats sub-tables redirect to 2005 / 0x07D5
        for cat in [2006, 2067] {
            redirects.insert(cat, 2005);
        }

        let mut field_links: BTreeMap<u32, Vec<(u32, &'static str, usize, usize)>> =
            BTreeMap::new();

        // 2016 (TextID)
        field_links.insert(
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
                (2061, "NameID", 5, 2),
                (2061, "DescriptionID", 7, 2),
                (2063, "NameID", 2, 2),
                (2064, "NameID", 2, 2),
                (2036, "ButtonNameID", 4, 2),
                (2072, "DescriptionID", 1, 2),
            ],
        );

        // 2005 (UnitStatsID)
        field_links.insert(
            2005,
            vec![(2003, "UnitStatsID", 6, 2), (2024, "StatsID", 4, 2)],
        );

        // 2003 (ItemID)
        field_links.insert(
            2003,
            vec![
                (2013, "InstalledScrollItemID", 2, 2),
                (2025, "EquipmentItemID", 3, 2),
                (2040, "LootItemID1", 3, 2),
                (2040, "LootItemID2", 6, 2),
                (2040, "LootItemID3", 9, 2),
                (2042, "MerchantStockItemID", 2, 2),
                (2065, "ChestLootItemID1", 3, 2),
            ],
        );

        // 2024 (UnitID)
        field_links.insert(
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

        // 2029 (BuildingID)
        field_links.insert(
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

        Self {
            redirects,
            field_links,
        }
    }
}

pub fn find_all_references(
    cff_dir: &Path,
    category_id: u32,
    target_id: u32,
) -> Vec<EntityReference> {
    let graph = RelationalGraph::default();
    let mut results = BTreeSet::new();

    let manifest_path = cff_dir.join("manifest.json");
    let Ok(m_str) = fs::read_to_string(manifest_path) else {
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<Manifest>(&m_str) else {
        return Vec::new();
    };

    let effective_cat = graph
        .redirects
        .get(&category_id)
        .copied()
        .unwrap_or(category_id);

    if let Some(links) = graph.field_links.get(&effective_cat) {
        for &(source_cat, field_name, offset, size) in links {
            if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == source_cat) {
                let chunk_path = cff_dir.join(&chunk.file);
                if let Ok(bytes) = fs::read(chunk_path) {
                    let stride = get_sf1_chunk_info(source_cat)
                        .map(|i| i.stride)
                        .unwrap_or(0);
                    if stride > 0 && bytes.len() % stride == 0 {
                        let num_records = bytes.len() / stride;
                        for rec_idx in 0..num_records {
                            let base = rec_idx * stride;
                            if base + offset + size <= bytes.len() {
                                let mut cur =
                                    Cursor::new(&bytes[base + offset..base + offset + size]);
                                let val = if size == 2 {
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
                                        record_index: rec_idx,
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
    }

    results.into_iter().collect()
}
