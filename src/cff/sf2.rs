// src/cff/sf2.rs

use super::container::Manifest;
use super::editor::EditorItem;
use super::text::{decode_windows, encode_windows};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs::{self, File};
use std::io::{Cursor, Write};
use std::path::Path;

#[allow(dead_code)]
pub struct Sf2ChunkInfo {
    pub name: &'static str,
    pub default_c_type: i16,
    pub element_type: u16, // 1 = raw/strings, 2 = utf16, 4 = uint32/float
}

pub fn get_sf2_chunk_info(id: u32) -> Option<Sf2ChunkInfo> {
    match id {
        0x2329 => Some(Sf2ChunkInfo {
            name: "UnitAttributes",
            default_c_type: 4,
            element_type: 4,
        }),
        0x232C => Some(Sf2ChunkInfo {
            name: "CategoryTypes",
            default_c_type: 1,
            element_type: 1,
        }),
        0x232F => Some(Sf2ChunkInfo {
            name: "LocalizedNames",
            default_c_type: 2,
            element_type: 2,
        }),
        0x2330 => Some(Sf2ChunkInfo {
            name: "ItemProperties",
            default_c_type: 4,
            element_type: 4,
        }),
        0x2335 => Some(Sf2ChunkInfo {
            name: "VisualMeshes",
            default_c_type: 1,
            element_type: 1,
        }),
        0x2341 => Some(Sf2ChunkInfo {
            name: "StringLinks",
            default_c_type: 1,
            element_type: 1,
        }),
        0x2345 => Some(Sf2ChunkInfo {
            name: "SpellParameters",
            default_c_type: 1,
            element_type: 1,
        }),
        0x2349 => Some(Sf2ChunkInfo {
            name: "MultiplierTriplets",
            default_c_type: 1,
            element_type: 1,
        }),
        0x234E => Some(Sf2ChunkInfo {
            name: "Abilities",
            default_c_type: 1,
            element_type: 1,
        }),
        _ => None,
    }
}

fn set_sf2_labels(slice: &[&str]) -> [String; 12] {
    let mut labels: [String; 12] = Default::default();
    for (i, &s) in slice.iter().take(12).enumerate() {
        labels[i] = s.to_string();
    }
    labels
}

fn extract_clean_asset_path(bytes: &[u8]) -> String {
    let mut best_candidate = String::new();
    let mut current = Vec::new();

    for &b in bytes {
        if (32..=126).contains(&b) && b != b';' && b != b',' && b != b'"' {
            current.push(b);
        } else {
            if current.len() >= 4 {
                let s = String::from_utf8_lossy(&current).trim().to_string();
                if s.contains('/')
                    || s.starts_with("ui_")
                    || s.starts_with("it_")
                    || s.starts_with("figure_")
                {
                    return s;
                }
                if best_candidate.is_empty() && s.len() > 3 {
                    best_candidate = s;
                }
            }
            current.clear();
        }
    }

    if current.len() >= 4 {
        let s = String::from_utf8_lossy(&current).trim().to_string();
        if s.contains('/')
            || s.starts_with("ui_")
            || s.starts_with("it_")
            || s.starts_with("figure_")
        {
            return s;
        }
        if best_candidate.is_empty() {
            best_candidate = s;
        }
    }

    best_candidate
}

fn sanitize_display_text(text: &str, max_len: usize) -> String {
    let mut clean = String::with_capacity(text.len());
    for c in text.chars() {
        if c == '\n' || c == '\r' || c == '\t' {
            clean.push(' ');
        } else if !c.is_control() {
            clean.push(c);
        }
    }
    let trimmed = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.chars().count() > max_len {
        let truncated: String = trimmed.chars().take(max_len).collect();
        format!("{}...", truncated)
    } else {
        trimmed
    }
}

pub fn load_sf2_items(cff_dir: &Path, category: &str, filter: &str) -> Vec<EditorItem> {
    let mut items = Vec::new();
    let filter_lower = filter.to_lowercase();
    let manifest_path = cff_dir.join("manifest.json");

    let Ok(m_str) = fs::read_to_string(manifest_path) else {
        return items;
    };
    let Ok(manifest) = serde_json::from_str::<Manifest>(&m_str) else {
        return items;
    };

    // 1. Visual Meshes (0x2335)
    if category.contains("0x2335")
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x2335)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
        && bytes.len() >= 4
    {
        let count = Cursor::new(&bytes[0..4])
            .read_u32::<LittleEndian>()
            .unwrap_or(0) as usize;
        let mut offset = 4;
        for i in 0..count {
            if offset + 4 > bytes.len() {
                break;
            }
            let id = Cursor::new(&bytes[offset..offset + 4])
                .read_u32::<LittleEndian>()
                .unwrap_or(0);
            offset += 4;

            if offset + 4 > bytes.len() {
                break;
            }
            let str_len = Cursor::new(&bytes[offset..offset + 4])
                .read_u32::<LittleEndian>()
                .unwrap_or(0) as usize;
            offset += 4;

            let raw_slice = if str_len > 0 && offset + str_len <= bytes.len() {
                let s = &bytes[offset..offset + str_len];
                offset += str_len;
                s
            } else {
                &[]
            };

            offset += 16;

            let clean_mesh = extract_clean_asset_path(raw_slice);
            let display_mesh = sanitize_display_text(&clean_mesh, 60);

            let display = format!("ID: {:<5} [SF2 Mesh] | {}", id, display_mesh);
            if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                items.push(EditorItem {
                    record_index: i,
                    id_str: id.to_string(),
                    val1: clean_mesh.clone(),
                    val2: format!("Record #{}", i),
                    p1: clean_mesh,
                    labels: set_sf2_labels(&["Mesh Asset Path:"]),
                    display,
                    ..Default::default()
                });
            }
        }
    }
    // 2. Abilities (0x234E)
    else if category.contains("0x234E")
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x234E)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
        && bytes.len() >= 4
    {
        let count = Cursor::new(&bytes[0..4])
            .read_u32::<LittleEndian>()
            .unwrap_or(0) as usize;
        let mut offset = 4;
        for i in 0..count {
            if offset + 4 > bytes.len() {
                break;
            }
            let ability_id = Cursor::new(&bytes[offset..offset + 4])
                .read_u32::<LittleEndian>()
                .unwrap_or(0);
            offset += 4;

            if offset + 4 > bytes.len() {
                break;
            }
            let str_len = Cursor::new(&bytes[offset..offset + 4])
                .read_u32::<LittleEndian>()
                .unwrap_or(0) as usize;
            offset += 4;

            let raw_slice = if str_len > 0 && offset + str_len <= bytes.len() {
                let s = &bytes[offset..offset + str_len];
                offset += str_len;
                s
            } else {
                &[]
            };

            let clean_icon = extract_clean_asset_path(raw_slice);
            let display_text = sanitize_display_text(&decode_windows(raw_slice), 65);

            let display = format!("Ability #{:<5} | {}", ability_id, display_text);
            if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                let script_desc = sanitize_display_text(&decode_windows(raw_slice), 120);
                items.push(EditorItem {
                    record_index: i,
                    id_str: ability_id.to_string(),
                    val1: clean_icon.clone(),
                    val2: script_desc.clone(),
                    p1: clean_icon,
                    p2: script_desc,
                    labels: set_sf2_labels(&["Ability Icon Asset:", "Description / Script:"]),
                    display,
                    ..Default::default()
                });
            }
        }
    }
    // 3. Item Properties (0x2330)
    else if category.contains("0x2330")
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x2330)
        && let Ok(bytes) = fs::read(cff_dir.join(&chunk.file))
        && bytes.len() >= 4
    {
        let count = Cursor::new(&bytes[0..4])
            .read_u32::<LittleEndian>()
            .unwrap_or(0) as usize;
        if let Some(stride) = bytes.len().saturating_sub(4).checked_div(count)
            && stride > 0
        {
            for i in 0..count {
                let base = 4 + i * stride;
                if base + stride > bytes.len() {
                    break;
                }

                let item_id = Cursor::new(&bytes[base..base + 2])
                    .read_u16::<LittleEndian>()
                    .unwrap_or(i as u16);
                let price = if stride >= 0x114 {
                    Cursor::new(&bytes[base + 0x110..base + 0x114])
                        .read_u32::<LittleEndian>()
                        .unwrap_or(0)
                } else {
                    0
                };
                let req_lvl = if stride >= 0x116 {
                    Cursor::new(&bytes[base + 0x114..base + 0x116])
                        .read_u16::<LittleEndian>()
                        .unwrap_or(0)
                } else {
                    0
                };

                let clean_mesh = if stride >= 0x134 {
                    extract_clean_asset_path(&bytes[base + 0x118..base + 0x134])
                } else {
                    extract_clean_asset_path(&bytes[base..base + stride])
                };

                let extra_info = if stride == 404 {
                    let min_dmg = Cursor::new(&bytes[base + 0x134..base + 0x136])
                        .read_u16::<LittleEndian>()
                        .unwrap_or(0);
                    let max_dmg = Cursor::new(&bytes[base + 0x136..base + 0x138])
                        .read_u16::<LittleEndian>()
                        .unwrap_or(0);
                    let armor = Cursor::new(&bytes[base + 0x144..base + 0x146])
                        .read_u16::<LittleEndian>()
                        .unwrap_or(0);
                    let str_bonus = Cursor::new(&bytes[base + 0x174..base + 0x176])
                        .read_i16::<LittleEndian>()
                        .unwrap_or(0);
                    let agi_bonus = Cursor::new(&bytes[base + 0x176..base + 0x178])
                        .read_i16::<LittleEndian>()
                        .unwrap_or(0);
                    let int_bonus = Cursor::new(&bytes[base + 0x178..base + 0x17A])
                        .read_i16::<LittleEndian>()
                        .unwrap_or(0);
                    format!(
                        "Price: {} Gold | Req Lvl: {} | Dmg: {}-{} | Armor: {} | Str/Agi/Int: {}/{}/{}",
                        price, req_lvl, min_dmg, max_dmg, armor, str_bonus, agi_bonus, int_bonus
                    )
                } else {
                    format!("Price: {} Gold | Req Level: {}", price, req_lvl)
                };

                let display_mesh = sanitize_display_text(&clean_mesh, 40);
                let display = format!(
                    "Item #{:<5} [Lvl: {}] | Price: {}g | {}",
                    item_id, req_lvl, price, display_mesh
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        record_index: i,
                        id_str: item_id.to_string(),
                        val1: clean_mesh.clone(),
                        val2: extra_info.clone(),
                        p1: clean_mesh,
                        p2: price.to_string(),
                        p3: req_lvl.to_string(),
                        p4: extra_info,
                        labels: set_sf2_labels(&[
                            "Visual Mesh / Icon:",
                            "Gold Price:",
                            "Required Level:",
                            "Combat Attributes:",
                        ]),
                        display,
                        ..Default::default()
                    });
                }
            }
        }
    }

    items
}

pub fn save_sf2_item(
    cff_dir: &Path,
    category: &str,
    record_index: usize,
    fields: &[String],
) -> std::io::Result<()> {
    let manifest_path = cff_dir.join("manifest.json");
    let m_str = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&m_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    let p1 = fields.get(1).map(|s| s.as_str()).unwrap_or("");
    let p2 = fields.get(2).map(|s| s.as_str()).unwrap_or("");
    let p3 = fields.get(3).map(|s| s.as_str()).unwrap_or("");

    // Item Properties (0x2330)
    if category.contains("0x2330") {
        if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x2330) {
            let chunk_path = cff_dir.join(&chunk.file);
            let mut bytes = fs::read(&chunk_path)?;
            if bytes.len() >= 4 {
                let count = Cursor::new(&bytes[0..4])
                    .read_u32::<LittleEndian>()
                    .unwrap_or(0) as usize;
                if let Some(stride) = bytes.len().saturating_sub(4).checked_div(count)
                    && stride > 0
                {
                    let base = 4 + record_index * stride;

                    // 1. Mesh name
                    if base + 0x134 <= bytes.len() && !p1.is_empty() {
                        let enc = encode_windows(p1);
                        let mut padded = vec![0u8; 28];
                        let len = enc.len().min(27);
                        padded[..len].copy_from_slice(&enc[..len]);
                        bytes[base + 0x118..base + 0x134].copy_from_slice(&padded);
                    }
                    // 2. Gold Price
                    if base + 0x114 <= bytes.len()
                        && let Ok(price) = p2.parse::<u32>()
                    {
                        bytes[base + 0x110..base + 0x114].copy_from_slice(&price.to_le_bytes());
                    }
                    // 3. Required Level
                    if base + 0x116 <= bytes.len()
                        && let Ok(lvl) = p3.parse::<u16>()
                    {
                        bytes[base + 0x114..base + 0x116].copy_from_slice(&lvl.to_le_bytes());
                    }

                    File::create(chunk_path)?.write_all(&bytes)?;
                }
            }
        }
        return Ok(());
    }

    // Visual Meshes (0x2335) or Abilities (0x234E)
    let target_id = if category.contains("0x2335") {
        0x2335
    } else if category.contains("0x234E") {
        0x234E
    } else {
        return Ok(());
    };

    if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == target_id) {
        let chunk_path = cff_dir.join(&chunk.file);
        let bytes = fs::read(&chunk_path)?;
        if bytes.len() < 4 {
            return Ok(());
        }

        let count = Cursor::new(&bytes[0..4])
            .read_u32::<LittleEndian>()
            .unwrap_or(0) as usize;
        let mut out = Vec::new();
        out.write_u32::<LittleEndian>(count as u32)?;

        let mut offset = 4;
        for i in 0..count {
            if offset + 4 > bytes.len() {
                break;
            }
            let id = Cursor::new(&bytes[offset..offset + 4]).read_u32::<LittleEndian>()?;
            offset += 4;
            out.write_u32::<LittleEndian>(id)?;

            if offset + 4 > bytes.len() {
                break;
            }
            let str_len =
                Cursor::new(&bytes[offset..offset + 4]).read_u32::<LittleEndian>()? as usize;
            offset += 4;

            if i == record_index {
                let enc = encode_windows(p1);
                out.write_u32::<LittleEndian>(enc.len() as u32)?;
                out.write_all(&enc)?;
                offset += str_len;
            } else {
                out.write_u32::<LittleEndian>(str_len as u32)?;
                if str_len > 0 && offset + str_len <= bytes.len() {
                    out.write_all(&bytes[offset..offset + str_len])?;
                    offset += str_len;
                }
            }
        }

        if offset < bytes.len() {
            out.write_all(&bytes[offset..])?;
        }
        File::create(chunk_path)?.write_all(&out)?;
    }

    Ok(())
}
