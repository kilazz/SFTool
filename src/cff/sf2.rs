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

/// Extracts the clean, primary 3D mesh path from binary buffers containing composite model parts.
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

/// Cleans and sanitizes strings into a single line to prevent Slint ListView row overlapping.
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
                    id_str: id.to_string(),
                    val1: clean_mesh,
                    val2: format!("Record #{}", i),
                    display,
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
        for _ in 0..count {
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
                items.push(EditorItem {
                    id_str: ability_id.to_string(),
                    val1: clean_icon,
                    val2: sanitize_display_text(&decode_windows(raw_slice), 120),
                    display,
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
                let price = if stride > 276 {
                    Cursor::new(&bytes[base + 0x110..base + 0x114])
                        .read_u32::<LittleEndian>()
                        .unwrap_or(0)
                } else {
                    0
                };
                let req_lvl = if stride > 278 {
                    Cursor::new(&bytes[base + 0x114..base + 0x116])
                        .read_u16::<LittleEndian>()
                        .unwrap_or(0)
                } else {
                    0
                };

                let clean_mesh = if stride > 0x134 {
                    extract_clean_asset_path(&bytes[base + 0x118..base + 0x134])
                } else {
                    extract_clean_asset_path(&bytes[base..base + stride])
                };

                let display_mesh = sanitize_display_text(&clean_mesh, 40);
                let display = format!(
                    "Item #{:<5} [Lvl: {}] | Price: {}g | {}",
                    item_id, req_lvl, price, display_mesh
                );
                if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                    items.push(EditorItem {
                        id_str: item_id.to_string(),
                        val1: clean_mesh,
                        val2: format!("Price: {} Gold | Req Level: {}", price, req_lvl),
                        display,
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
    index: usize,
    val1: &str,
) -> std::io::Result<()> {
    let manifest_path = cff_dir.join("manifest.json");
    let m_str = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&m_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    // Handle Item Properties (0x2330)
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
                    let base = 4 + index * stride;

                    if base + 0x134 <= bytes.len() {
                        let enc = encode_windows(val1);
                        let mut padded = vec![0u8; 28];
                        let len = enc.len().min(27);
                        padded[..len].copy_from_slice(&enc[..len]);
                        bytes[base + 0x118..base + 0x134].copy_from_slice(&padded);
                        File::create(chunk_path)?.write_all(&bytes)?;
                    }
                }
            }
        }
        return Ok(());
    }

    // Handle Visual Meshes (0x2335) and Abilities (0x234E)
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

            if i == index {
                let enc = encode_windows(val1);
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
