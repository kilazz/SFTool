// src/cff/localization.rs

use super::container::Manifest;
use super::editor::load_editor_items;
use super::text::{decode_by_lang, encode_by_lang};
use crate::UiLogger;
use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Cursor};
use std::path::Path;

/// Canonical default languages supported natively by the vanilla SpellForce.exe engine (0..=4).
pub fn get_language_tag_map(cff_dir: &Path) -> BTreeMap<u8, String> {
    let mut map = BTreeMap::new();
    map.insert(0, "DE".to_string());
    map.insert(1, "EN".to_string());
    map.insert(2, "FR".to_string());
    map.insert(3, "ES".to_string());
    map.insert(4, "IT".to_string());

    // Custom modding slots (e.g. 5=RU, 6=PL) are loaded dynamically from project metadata
    let meta_path = cff_dir.join(".lang_tags.json");
    if meta_path.exists()
        && let Ok(content) = fs::read_to_string(&meta_path)
        && let Ok(saved_tags) = serde_json::from_str::<BTreeMap<String, String>>(&content)
    {
        for (slot_str, tag) in saved_tags {
            if let Ok(slot) = slot_str.parse::<u8>() {
                map.insert(slot, tag);
            }
        }
    }
    map
}

pub fn save_language_tag(cff_dir: &Path, slot: u8, tag: &str) -> io::Result<()> {
    let mut tags = get_language_tag_map(cff_dir);
    tags.insert(slot, tag.to_uppercase());

    let mut str_map = BTreeMap::new();
    for (s, t) in tags {
        str_map.insert(s.to_string(), t);
    }

    let meta_path = cff_dir.join(".lang_tags.json");
    let json_bytes = serde_json::to_vec_pretty(&str_map)?;
    crate::tools::atomic_write(&meta_path, &json_bytes)?;
    Ok(())
}

pub fn get_available_languages_display(cff_dir: &Path) -> Vec<String> {
    let mut list = vec!["All Languages".to_string()];
    let tags = get_language_tag_map(cff_dir);

    let manifest_path = cff_dir.join("manifest.json");
    let mut detected_slots = BTreeSet::new();

    // Dynamically scan GameData.cff string chunks to discover what slots are actually present
    if let Ok(m_str) = fs::read_to_string(manifest_path)
        && let Ok(manifest) = serde_json::from_str::<Manifest>(&m_str)
    {
        for chunk in &manifest.chunks {
            let chunk_path = cff_dir.join(&chunk.file);
            if let Ok(data) = fs::read(&chunk_path)
                && data.len() >= 566
                && data.len().is_multiple_of(566)
            {
                let num_blocks = data.len() / 566;
                for i in 0..num_blocks {
                    let offset = i * 566;
                    let str_id = Cursor::new(&data[offset..offset + 4])
                        .read_u32::<LittleEndian>()
                        .unwrap_or(0);
                    let lang_id = ((str_id >> 16) & 0xFF) as u8;
                    detected_slots.insert(lang_id);
                }
            }
        }
    }

    // Fallback to the 5 official vanilla slots if container is empty or unindexed
    if detected_slots.is_empty() {
        for i in 0..=4 {
            detected_slots.insert(i);
        }
    }

    for slot in detected_slots {
        let tag = tags
            .get(&slot)
            .cloned()
            .unwrap_or_else(|| format!("L{}", slot));
        let lang_name = match slot {
            0 => "German",
            1 => "English",
            2 => "French",
            3 => "Spanish",
            4 => "Italian",
            _ => "Custom / Mod",
        };
        list.push(format!("Slot {}: {} [{}]", slot, lang_name, tag));
    }

    list
}

pub fn parse_slot_number(lang_str: &str) -> u8 {
    if lang_str.contains("Slot 0") {
        0
    } else if lang_str.contains("Slot 1") {
        1
    } else if lang_str.contains("Slot 2") {
        2
    } else if lang_str.contains("Slot 3") {
        3
    } else if lang_str.contains("Slot 4") {
        4
    } else if let Some(part) = lang_str.strip_prefix("Slot ") {
        part.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u8>()
            .unwrap_or(1)
    } else {
        1
    }
}

pub fn export_single_language(
    cff_dir: &Path,
    lang_filter: &str,
    out_json: &Path,
    logger: &UiLogger,
) -> io::Result<usize> {
    let slot = parse_slot_number(lang_filter);
    logger.log(&format!(
        "[*] Exporting clean language JSON for Slot {}...",
        slot
    ));

    let items = load_editor_items(cff_dir, "Localized Strings", "", lang_filter);
    let mut clean_map: BTreeMap<String, String> = BTreeMap::new();

    for it in &items {
        if it.id_str.is_empty() {
            continue; // Skip UI pagination banner
        }
        if let Some(pos) = it.val2.find("Base ID: ") {
            let id_part = &it.val2[pos + 9..];
            clean_map.insert(id_part.to_string(), it.val1.clone());
        } else {
            clean_map.insert(it.id_str.clone(), it.val1.clone());
        }
    }

    let count = clean_map.len();
    let json_bytes = serde_json::to_vec_pretty(&clean_map)?;
    crate::tools::atomic_write(out_json, &json_bytes)?;

    logger.log(&format!(
        "[+] Successfully exported {} clean phrases to {:?}",
        count, out_json
    ));
    Ok(count)
}

pub fn import_language_to_slot(
    cff_dir: &Path,
    lang_filter: &str,
    in_json: &Path,
    logger: &UiLogger,
) -> io::Result<usize> {
    let slot = parse_slot_number(lang_filter);
    replace_slot_from_json(cff_dir, slot as u16, in_json, logger)
}

pub fn clone_and_export_language(
    cff_dir: &Path,
    src_slot: u8,
    dst_slot: u8,
    custom_tag: &str,
    export_json_path: &Path,
    logger: &UiLogger,
) -> io::Result<usize> {
    let tag = custom_tag.trim().to_uppercase();
    logger.log(&format!(
        "[*] Starting One-Click Language Clone: Slot {} -> Slot {} [{}]",
        src_slot, dst_slot, tag
    ));

    save_language_tag(cff_dir, dst_slot, &tag)?;
    let cloned_count = clone_language_slot(cff_dir, src_slot as u16, dst_slot as u16, logger)?;

    let items = load_editor_items(
        cff_dir,
        "Localized Strings",
        "",
        &format!("Slot {}", dst_slot),
    );
    let mut clean_map: BTreeMap<String, String> = BTreeMap::new();

    for it in &items {
        if it.id_str.is_empty() {
            continue;
        }
        if let Some(pos) = it.val2.find("Base ID: ") {
            let id_part = &it.val2[pos + 9..];
            clean_map.insert(id_part.to_string(), it.val1.clone());
        } else {
            clean_map.insert(it.id_str.clone(), it.val1.clone());
        }
    }

    let json_bytes = serde_json::to_vec_pretty(&clean_map)?;
    crate::tools::atomic_write(export_json_path, &json_bytes)?;

    logger.log(&format!(
        "[+] Successfully cloned {} phrases! Clean JSON saved to: {:?}",
        cloned_count, export_json_path
    ));
    Ok(cloned_count)
}

pub fn clone_language_slot(
    cff_dir: &Path,
    src_slot: u16,
    dst_slot: u16,
    logger: &UiLogger,
) -> io::Result<usize> {
    logger.log(&format!(
        "[*] Cloning Language Slot {} -> Slot {}...",
        src_slot, dst_slot
    ));

    let manifest_path = cff_dir.join("manifest.json");
    let m_str = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&m_str)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let mut cloned_count = 0;
    let json_dir = cff_dir.join("texts_json");
    let _ = fs::create_dir_all(&json_dir);

    for chunk in &manifest.chunks {
        let chunk_path = cff_dir.join(&chunk.file);
        if !chunk_path.exists() {
            continue;
        }
        let mut data = fs::read(&chunk_path)?;
        if data.len() < 566 || !data.len().is_multiple_of(566) {
            continue;
        }

        let num_blocks = data.len() / 566;
        let mut new_blocks = Vec::new();
        let mut cloned_entries = Vec::new();

        for i in 0..num_blocks {
            let offset = i * 566;
            let block = &data[offset..offset + 566];
            let str_id = Cursor::new(&block[0..4]).read_u32::<LittleEndian>()?;
            let lang_id = ((str_id >> 16) & 0xFF) as u16;
            let camp_id = str_id >> 24;
            let base_id = (str_id & 0xFFFF) as u16;

            if lang_id == src_slot {
                let mut new_block = block.to_vec();
                let new_str_id = (camp_id << 24) | ((dst_slot as u32) << 16) | (base_id as u32);
                new_block[0..4].copy_from_slice(&new_str_id.to_le_bytes());

                let new_offset = data.len() + new_blocks.len();
                let mut text_bytes = &block[54..566];
                if let Some(null_idx) = text_bytes.iter().position(|&b| b == 0) {
                    text_bytes = &text_bytes[..null_idx];
                }
                let text = decode_by_lang(text_bytes, src_slot);

                new_blocks.extend_from_slice(&new_block);
                cloned_entries.push((new_offset, new_str_id, text));
                cloned_count += 1;
            }
        }

        if !new_blocks.is_empty() {
            data.extend_from_slice(&new_blocks);
            crate::tools::atomic_write(&chunk_path, &data)?;

            logger.log(&format!(
                "[+] Appended {} blocks to {}",
                new_blocks.len() / 566,
                chunk.file
            ));

            let stem = chunk.file.trim_end_matches(".dat");
            let json_path = json_dir.join(format!("{}_strings.json", stem));
            let mut map: BTreeMap<String, String> = if json_path.exists() {
                serde_json::from_str(&fs::read_to_string(&json_path)?).unwrap_or_default()
            } else {
                BTreeMap::new()
            };

            for (new_offset, new_str_id, text) in cloned_entries {
                let new_key = format!("f566_{:08}_{}", new_offset, new_str_id);
                map.insert(new_key, text);
            }
            let encoded = serde_json::to_vec_pretty(&map)?;
            crate::tools::atomic_write(&json_path, &encoded)?;
        }
    }

    Ok(cloned_count)
}

pub fn replace_slot_from_json(
    cff_dir: &Path,
    target_slot: u16,
    json_path: &Path,
    logger: &UiLogger,
) -> io::Result<usize> {
    logger.log(&format!(
        "[*] Overwriting Language Slot {} from {:?}...",
        target_slot, json_path
    ));

    let translation_content = fs::read_to_string(json_path)?;
    let translations: BTreeMap<String, String> = serde_json::from_str(&translation_content)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let manifest_path = cff_dir.join("manifest.json");
    let m_str = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&m_str)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let mut updated_count = 0;

    for chunk in &manifest.chunks {
        let chunk_path = cff_dir.join(&chunk.file);
        if !chunk_path.exists() {
            continue;
        }
        let mut data = fs::read(&chunk_path)?;
        if data.len() < 566 || !data.len().is_multiple_of(566) {
            continue;
        }

        let num_blocks = data.len() / 566;
        for i in 0..num_blocks {
            let offset = i * 566;
            let str_id = Cursor::new(&data[offset..offset + 4]).read_u32::<LittleEndian>()?;
            let lang_id = ((str_id >> 16) & 0xFF) as u16;
            let base_id = (str_id & 0xFFFF) as u16;

            if lang_id == target_slot {
                let lookup_keys = [
                    base_id.to_string(),
                    format!("L{}_{}", lang_id, base_id),
                    format!("f566_{:08}_{}", offset, str_id),
                ];

                for key in &lookup_keys {
                    if let Some(new_text) = translations.get(key) {
                        let mut text_bytes = encode_by_lang(new_text, target_slot);
                        if text_bytes.len() > 511 {
                            text_bytes.truncate(511);
                        }
                        let mut padded = vec![0u8; 512];
                        padded[..text_bytes.len()].copy_from_slice(&text_bytes);
                        data[offset + 54..offset + 566].copy_from_slice(&padded);
                        updated_count += 1;
                        break;
                    }
                }
            }
        }

        crate::tools::atomic_write(&chunk_path, &data)?;
    }

    let json_dir = cff_dir.join("texts_json");
    if json_dir.exists()
        && let Ok(entries) = fs::read_dir(&json_dir)
    {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) == Some("json")
                && !p.to_string_lossy().ends_with(".meta.json")
                && let Ok(content) = fs::read_to_string(&p)
                && let Ok(mut map) = serde_json::from_str::<BTreeMap<String, String>>(&content)
            {
                for (k, v) in map.iter_mut() {
                    if k.starts_with("f566_") {
                        let parts: Vec<&str> = k.split('_').collect();
                        if let Some(str_id) = parts.get(2).and_then(|s| s.parse::<u32>().ok()) {
                            let l_id = ((str_id >> 16) & 0xFF) as u16;
                            let b_id = (str_id & 0xFFFF) as u16;
                            if l_id == target_slot
                                && let Some(new_val) = translations
                                    .get(&b_id.to_string())
                                    .or_else(|| translations.get(&format!("L{}_{}", l_id, b_id)))
                            {
                                *v = new_val.clone();
                            }
                        }
                    }
                }
                let encoded = serde_json::to_vec_pretty(&map)?;
                crate::tools::atomic_write(&p, &encoded)?;
            }
        }
    }

    logger.log(&format!(
        "[+] Successfully replaced {} phrases in Slot {}!",
        updated_count, target_slot
    ));
    Ok(updated_count)
}
