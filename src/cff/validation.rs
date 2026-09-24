// src/cff/validation.rs

use super::container::Manifest;
use super::sf1::get_sf1_chunk_info;
use super::tracer::TracerEngine;
use crate::UiLogger;
use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Cursor};
use std::path::Path;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BrokenReference {
    pub source_category: u32,
    pub source_chunk: String,
    pub record_index: usize,
    pub field_name: String,
    pub target_category: u32,
    pub missing_id: u32,
}

#[allow(dead_code)]
pub struct DatabaseValidationReport {
    pub total_tables_checked: usize,
    pub total_foreign_keys_checked: usize,
    pub broken_references: Vec<BrokenReference>,
}

pub fn validate_cff_integrity(
    cff_dir: &Path,
    logger: &UiLogger,
) -> io::Result<DatabaseValidationReport> {
    logger.log(&format!(
        "[*] Initiating CFF Relational Integrity Audit in: {:?}",
        cff_dir
    ));

    let manifest_path = cff_dir.join("manifest.json");
    let manifest_str = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&manifest_str)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let tracer = TracerEngine::default();
    let mut broken_references = Vec::new();
    let mut total_fk_checked = 0;
    let mut checked_tables = BTreeSet::new();

    // 1. Cache all existing primary IDs per category
    let mut existing_ids: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();

    for chunk in &manifest.chunks {
        let chunk_path = cff_dir.join(&chunk.file);
        if !chunk_path.exists() {
            continue;
        }

        if let Ok(bytes) = fs::read(&chunk_path) {
            let cat_id = chunk.id;
            checked_tables.insert(cat_id);

            if let Some(info) = get_sf1_chunk_info(cat_id) {
                if info.stride > 0 && bytes.len() % info.stride == 0 {
                    let count = bytes.len() / info.stride;
                    let ids = existing_ids.entry(cat_id).or_default();
                    for i in 0..count {
                        let offset = i * info.stride;
                        let id = if cat_id == 2051 || cat_id == 2052 {
                            Cursor::new(&bytes[offset..offset + 4])
                                .read_u32::<LittleEndian>()
                                .unwrap_or(0)
                        } else if cat_id == 2022 || cat_id == 2048 || cat_id == 2072 {
                            bytes[offset] as u32
                        } else {
                            Cursor::new(&bytes[offset..offset + 2])
                                .read_u16::<LittleEndian>()
                                .unwrap_or(0) as u32
                        };
                        ids.insert(id);
                    }
                }
            } else if cat_id == 0x07E0 {
                // Fixed 566-byte localized string tables
                if bytes.len() >= 566 && bytes.len() % 566 == 0 {
                    let count = bytes.len() / 566;
                    let ids = existing_ids.entry(2016).or_default();
                    for i in 0..count {
                        let off = i * 566;
                        let str_id = Cursor::new(&bytes[off..off + 4])
                            .read_u32::<LittleEndian>()
                            .unwrap_or(0);
                        ids.insert(str_id & 0xFFFF);
                    }
                }
            }
        }
    }

    // 2. Cross-verify every configured foreign key link
    for (target_cat, links) in &tracer.foreign_keys {
        let valid_targets = match existing_ids.get(target_cat) {
            Some(set) => set,
            None => continue,
        };

        for &(source_cat, field_name, offset, size) in links {
            if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == source_cat) {
                let chunk_path = cff_dir.join(&chunk.file);
                if let Ok(bytes) = fs::read(&chunk_path) {
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
                                let referenced_id = if size == 1 {
                                    bytes[base + offset] as u32
                                } else if size == 2 {
                                    cur.read_u16::<LittleEndian>().unwrap_or(0) as u32
                                } else {
                                    cur.read_u32::<LittleEndian>().unwrap_or(0)
                                };

                                total_fk_checked += 1;

                                // Value 0 usually designates null/unset link
                                if referenced_id != 0 && !valid_targets.contains(&referenced_id) {
                                    broken_references.push(BrokenReference {
                                        source_category: source_cat,
                                        source_chunk: chunk.file.clone(),
                                        record_index: i,
                                        field_name: field_name.to_string(),
                                        target_category: *target_cat,
                                        missing_id: referenced_id,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    logger.log(&format!(
        "[+] Audit Complete: {} tables checked, {} FK instances verified. Broken references found: {}",
        checked_tables.len(),
        total_fk_checked,
        broken_references.len()
    ));

    for br in &broken_references {
        logger.log(&format!(
            "  [!] Missing Link: {} [Cat 0x{:04X}] (Rec #{}) [{}] -> Target Cat 0x{:04X} missing ID {}",
            br.source_chunk,
            br.source_category,
            br.record_index,
            br.field_name,
            br.target_category,
            br.missing_id
        ));
    }

    Ok(DatabaseValidationReport {
        total_tables_checked: checked_tables.len(),
        total_foreign_keys_checked: total_fk_checked,
        broken_references,
    })
}
