// src/cff/dump.rs

use super::container::Manifest;
use super::sf1::get_sf1_chunk_info;
use super::sf1_schema::*;
use crate::UiLogger;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;
use std::fs::{self, File};
use std::io::{self, Cursor};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct ChunkCoverageInfo {
    pub chunk_file: String,
    pub category_id: u32,
    pub category_name: String,
    pub total_bytes: usize,
    pub stride: usize,
    pub record_count: usize,
    pub unmapped_bytes: usize,
    pub coverage_percent: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FullAuditReport {
    pub total_chunks: usize,
    pub perfect_chunks: usize,
    pub total_db_bytes: usize,
    pub mapped_db_bytes: usize,
    pub unmapped_db_bytes: usize,
    pub overall_coverage_percent: f64,
    pub details: Vec<ChunkCoverageInfo>,
}

pub fn audit_coverage(cff_dir: &Path, logger: &UiLogger) -> io::Result<FullAuditReport> {
    logger.log(&format!(
        "[*] Starting Full Byte Coverage Audit in: {:?}",
        cff_dir
    ));

    let manifest_path = cff_dir.join("manifest.json");
    let manifest_str = fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&manifest_str)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let mut details = Vec::new();
    let mut total_bytes = 0usize;
    let mut mapped_bytes = 0usize;
    let mut unmapped_bytes = 0usize;
    let mut perfect_chunks = 0usize;

    for chunk in &manifest.chunks {
        let chunk_path = cff_dir.join(&chunk.file);
        if !chunk_path.exists() {
            continue;
        }

        let data = fs::read(&chunk_path)?;
        let cat_id = chunk.id;
        let c_len = data.len();
        total_bytes += c_len;

        let info_opt = get_sf1_chunk_info(cat_id);
        let cat_name = info_opt
            .map(|i| i.name)
            .unwrap_or(chunk.name.as_deref().unwrap_or("Unknown"));

        let (stride, rec_count, unmapped) = if cat_id == 2016 {
            // LocalizedStrings
            (566, c_len / 566, c_len % 566)
        } else if let Some(info) = info_opt {
            if info.stride > 0 {
                (info.stride, c_len / info.stride, c_len % info.stride)
            } else {
                // Dynamic collision chunk (0x07EE or 0x0809)
                let mut cur = Cursor::new(&data);
                let mut p_count = 0;
                while (cur.position() as usize) + 5 <= c_len {
                    cur.set_position(cur.position() + 4);
                    let v_count = cur.read_u8().unwrap_or(0) as u64;
                    let poly_bytes = v_count * 4;
                    if cur.position() + poly_bytes > c_len as u64 {
                        break;
                    }
                    cur.set_position(cur.position() + poly_bytes);
                    p_count += 1;
                }
                let consumed = cur.position() as usize;
                (0, p_count, c_len.saturating_sub(consumed))
            }
        } else {
            (1, c_len, 0)
        };

        let chunk_mapped = c_len.saturating_sub(unmapped);
        mapped_bytes += chunk_mapped;
        unmapped_bytes += unmapped;

        let cov_pct = if c_len == 0 {
            100.0
        } else {
            (chunk_mapped as f64 / c_len as f64) * 100.0
        };

        if unmapped == 0 {
            perfect_chunks += 1;
            logger.log(&format!(
                "  [+] 0x{:04X} {:<24} {:>5} records ({:>2} B/rec) -> {:>7} B [100.0% Mapped]",
                cat_id, cat_name, rec_count, stride, c_len
            ));
        } else {
            logger.log(&format!(
                "  [!] 0x{:04X} {:<24} {:>5} records ({:>2} B/rec) -> {:>7} B [{:.1}% Mapped, {} B unmapped!]",
                cat_id, cat_name, rec_count, stride, c_len, cov_pct, unmapped
            ));
        }

        details.push(ChunkCoverageInfo {
            chunk_file: chunk.file.clone(),
            category_id: cat_id,
            category_name: cat_name.to_string(),
            total_bytes: c_len,
            stride,
            record_count: rec_count,
            unmapped_bytes: unmapped,
            coverage_percent: cov_pct,
        });
    }

    let overall_pct = if total_bytes == 0 {
        100.0
    } else {
        (mapped_bytes as f64 / total_bytes as f64) * 100.0
    };

    logger.log("===============================================================================");
    logger.log(&format!(
        "[+] AUDIT COMPLETE: {}/{} Chunks fully mapped (100%). Total Bytes: {} (Unmapped: {}).",
        perfect_chunks,
        details.len(),
        total_bytes,
        unmapped_bytes
    ));
    logger.log(&format!(
        "[+] Overall GameData Coverage: {:.2}%",
        overall_pct
    ));
    logger.log("===============================================================================");

    Ok(FullAuditReport {
        total_chunks: details.len(),
        perfect_chunks,
        total_db_bytes: total_bytes,
        mapped_db_bytes: mapped_bytes,
        unmapped_db_bytes: unmapped_bytes,
        overall_coverage_percent: overall_pct,
        details,
    })
}

pub fn dump_all_json(cff_dir: &Path, out_dir: &Path, logger: &UiLogger) -> io::Result<usize> {
    fs::create_dir_all(out_dir)?;

    let manifest_path = cff_dir.join("manifest.json");
    let manifest_str = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&manifest_str)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let mut exported = 0usize;

    macro_rules! dump_table {
        ($chunk_id:expr, $entry_type:ident, $filename:expr) => {
            if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == $chunk_id) {
                let p = cff_dir.join(&chunk.file);
                if let Ok(bytes) = fs::read(&p) {
                    let mut records = Vec::new();
                    for chunk_slice in bytes.as_chunks::<{ $entry_type::STRIDE }>().0 {
                        if let Ok(item) = $entry_type::decode(chunk_slice) {
                            records.push(item);
                        }
                    }
                    let out_path = out_dir.join($filename);
                    if let Ok(f) = File::create(&out_path) {
                        if serde_json::to_writer_pretty(f, &records).is_ok() {
                            exported += 1;
                        }
                    }
                }
            }
        };
    }

    dump_table!(0x07D2, SpellEntry, "spells_master.json");
    dump_table!(0x0806, SpellLineEntry, "spell_lines.json");
    dump_table!(0x07F4, TechTreeUpgradeEntry, "tech_tree_upgrades.json");
    dump_table!(0x07D3, ItemMasterEntry, "items_master.json");
    dump_table!(0x07D4, ItemStatsModifierEntry, "item_stats_modifiers.json");
    dump_table!(0x07D5, UnitStatsEntry, "unit_stats.json");
    dump_table!(0x07DC, Gfx2dItemEntry, "gfx_2d_items.json");
    dump_table!(0x07DF, WeaponStatsEntry, "weapon_stats.json");
    dump_table!(0x07E2, SpellsBiMapEntry, "spells_bimap.json");
    dump_table!(0x07E6, RaceEntry, "races.json");
    dump_table!(0x07E8, UnitMasterEntry, "units_master.json");
    dump_table!(0x07ED, BuildingMasterEntry, "buildings_master.json");
    dump_table!(0x07F8, UnitLootTableEntry, "unit_loot_tables.json");
    dump_table!(0x0811, ObjectLootTableEntry, "object_loot_tables.json");
    dump_table!(0x0800, ComplexPropertyEntry, "level_progression.json");
    dump_table!(0x0802, ObjectMasterEntry, "objects_master.json");
    dump_table!(0x07E9, UnitEquipmentEntry, "unit_equipment.json");
    dump_table!(0x07FA, MerchantInventoryEntry, "merchant_inventory.json");
    dump_table!(0x080D, QuestEntry, "quests.json");
    dump_table!(0x080F, WeaponTypeEntry, "weapon_types.json");
    dump_table!(0x0810, WeaponMaterialEntry, "weapon_materials.json");
    dump_table!(0x0818, ItemSetEntry, "item_sets.json");
    dump_table!(0x07F0, TerrainCultivationEntry, "terrain_cultivation.json");
    dump_table!(0x0805, PortalEntry, "portals.json");
    dump_table!(0x080A, DescriptionEntry, "descriptions.json");

    macro_rules! dump_collision {
        ($chunk_id:expr, $filename:expr) => {
            if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == $chunk_id) {
                let p = cff_dir.join(&chunk.file);
                if let Ok(bytes) = fs::read(&p) {
                    let mut polygons = Vec::new();
                    let mut cur = Cursor::new(&bytes);
                    while (cur.position() as usize) + 5 <= bytes.len() {
                        let entity_id = cur.read_u16::<LittleEndian>().unwrap_or(0);
                        let polygon_index = cur.read_u8().unwrap_or(0);
                        let flag = cur.read_u8().unwrap_or(0);
                        let v_count = cur.read_u8().unwrap_or(0);
                        let mut vertices = Vec::new();
                        for _ in 0..v_count {
                            if (cur.position() as usize) + 4 > bytes.len() {
                                break;
                            }
                            let x = cur.read_i16::<LittleEndian>().unwrap_or(0);
                            let y = cur.read_i16::<LittleEndian>().unwrap_or(0);
                            vertices.push((x, y));
                        }
                        polygons.push(CollisionPolygonEntry {
                            entity_id,
                            polygon_index,
                            flag,
                            vertices,
                        });
                    }
                    let out_path = out_dir.join($filename);
                    if let Ok(f) = File::create(&out_path) {
                        if serde_json::to_writer_pretty(f, &polygons).is_ok() {
                            exported += 1;
                        }
                    }
                }
            }
        };
    }

    dump_collision!(0x07EE, "building_collision.json");
    dump_collision!(0x0809, "object_collision.json");

    logger.log(&format!(
        "[+] Auto-exported {} structured tables to {:?}",
        exported, out_dir
    ));
    Ok(exported)
}

pub fn compile_all_json_to_dat(cff_dir: &Path, logger: &UiLogger) -> io::Result<usize> {
    let tables_dir = cff_dir.join("tables_json");
    if !tables_dir.exists() {
        return Ok(0);
    }

    let manifest_path = cff_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Ok(0);
    }

    let manifest_str = fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&manifest_str)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let mut compiled = 0usize;

    macro_rules! compile_table {
        ($chunk_id:expr, $entry_type:ident, $filename:expr) => {
            let json_path = tables_dir.join($filename);
            if json_path.exists()
                && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == $chunk_id)
            {
                if let Ok(content) = fs::read_to_string(&json_path)
                    && let Ok(records) = serde_json::from_str::<Vec<$entry_type>>(&content)
                {
                    let mut buf = Vec::with_capacity(records.len() * $entry_type::STRIDE);
                    let mut rec_buf = [0u8; $entry_type::STRIDE];
                    let mut ok = true;
                    for r in &records {
                        if r.encode(&mut rec_buf).is_ok() {
                            buf.extend_from_slice(&rec_buf);
                        } else {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        let chunk_path = cff_dir.join(&chunk.file);
                        if crate::tools::atomic_write(&chunk_path, &buf).is_ok() {
                            compiled += 1;
                        }
                    }
                }
            }
        };
    }

    compile_table!(0x07D2, SpellEntry, "spells_master.json");
    compile_table!(0x0806, SpellLineEntry, "spell_lines.json");
    compile_table!(0x07F4, TechTreeUpgradeEntry, "tech_tree_upgrades.json");
    compile_table!(0x07D3, ItemMasterEntry, "items_master.json");
    compile_table!(0x07D4, ItemStatsModifierEntry, "item_stats_modifiers.json");
    compile_table!(0x07D5, UnitStatsEntry, "unit_stats.json");
    compile_table!(0x07DC, Gfx2dItemEntry, "gfx_2d_items.json");
    compile_table!(0x07DF, WeaponStatsEntry, "weapon_stats.json");
    compile_table!(0x07E2, SpellsBiMapEntry, "spells_bimap.json");
    compile_table!(0x07E6, RaceEntry, "races.json");
    compile_table!(0x07E8, UnitMasterEntry, "units_master.json");
    compile_table!(0x07ED, BuildingMasterEntry, "buildings_master.json");
    compile_table!(0x07F8, UnitLootTableEntry, "unit_loot_tables.json");
    compile_table!(0x0811, ObjectLootTableEntry, "object_loot_tables.json");
    compile_table!(0x0800, ComplexPropertyEntry, "level_progression.json");
    compile_table!(0x0802, ObjectMasterEntry, "objects_master.json");
    compile_table!(0x07E9, UnitEquipmentEntry, "unit_equipment.json");
    compile_table!(0x07FA, MerchantInventoryEntry, "merchant_inventory.json");
    compile_table!(0x080D, QuestEntry, "quests.json");
    compile_table!(0x080F, WeaponTypeEntry, "weapon_types.json");
    compile_table!(0x0810, WeaponMaterialEntry, "weapon_materials.json");
    compile_table!(0x0818, ItemSetEntry, "item_sets.json");
    compile_table!(0x07F0, TerrainCultivationEntry, "terrain_cultivation.json");
    compile_table!(0x0805, PortalEntry, "portals.json");
    compile_table!(0x080A, DescriptionEntry, "descriptions.json");

    macro_rules! compile_collision {
        ($chunk_id:expr, $filename:expr) => {
            let json_path = tables_dir.join($filename);
            if json_path.exists()
                && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == $chunk_id)
            {
                if let Ok(content) = fs::read_to_string(&json_path)
                    && let Ok(polygons) =
                        serde_json::from_str::<Vec<CollisionPolygonEntry>>(&content)
                {
                    let mut buf = Vec::new();
                    let mut ok = true;
                    for poly in &polygons {
                        if buf.write_u16::<LittleEndian>(poly.entity_id).is_err()
                            || buf.write_u8(poly.polygon_index).is_err()
                            || buf.write_u8(poly.flag).is_err()
                            || buf.write_u8(poly.vertices.len() as u8).is_err()
                        {
                            ok = false;
                            break;
                        }
                        for &(x, y) in &poly.vertices {
                            if buf.write_i16::<LittleEndian>(x).is_err()
                                || buf.write_i16::<LittleEndian>(y).is_err()
                            {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok {
                        let chunk_path = cff_dir.join(&chunk.file);
                        if crate::tools::atomic_write(&chunk_path, &buf).is_ok() {
                            compiled += 1;
                        }
                    }
                }
            }
        };
    }

    compile_collision!(0x07EE, "building_collision.json");
    compile_collision!(0x0809, "object_collision.json");

    if compiled > 0 {
        logger.log(&format!(
            "[+] Recompiled {} structured tables from tables_json into .dat chunks.",
            compiled
        ));
    }
    Ok(compiled)
}
