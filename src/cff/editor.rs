// src/cff/editor.rs

use super::container::Manifest;
use super::localization::get_language_tag_map;
use super::sf1::{load_sf1_items, save_sf1_item};
use super::sf2::{load_sf2_items, save_sf2_item};
use super::text::{decode_windows, encode_by_lang};
use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct EditorItem {
    pub id_str: String,
    pub val1: String,
    pub val2: String,
    pub p1: String,
    pub p2: String,
    pub p3: String,
    pub p4: String,
    pub p5: String,
    pub p6: String,
    pub p7: String,
    pub p8: String,
    pub p9: String,
    pub p10: String,
    pub p11: String,
    pub p12: String,
    pub labels: [String; 12],
    pub display: String,
}

pub struct SpellVisualDetails {
    pub spell_name: String,
    pub spell_mesh: String,
    pub scroll_name: String,
    pub scroll_mesh: String,
}

pub fn get_available_categories(cff_dir: &Path) -> Vec<String> {
    let manifest_path = cff_dir.join("manifest.json");
    if let Ok(m_str) = fs::read_to_string(manifest_path)
        && let Ok(manifest) = serde_json::from_str::<Manifest>(&m_str)
        && manifest.format == "sf2"
    {
        return vec![
            "Abilities (0x234E)".to_string(),
            "Visual Meshes (0x2335)".to_string(),
            "Item Properties (0x2330)".to_string(),
            "Localized Strings".to_string(),
        ];
    }
    vec![
        "2D Gfx Items (0x07DC)".to_string(),
        "Spell Lines (0x0806)".to_string(),
        "Spells Master (0x07D2)".to_string(),
        "Tech Tree Upgrades (0x07F4)".to_string(),
        "Items Master (0x07D3)".to_string(),
        "Item Modifiers (0x07D4)".to_string(),
        "Unit Stats (0x07D5)".to_string(),
        "Weapon Stats (0x07DF)".to_string(),
        "Spells Mapping (0x07E2)".to_string(),
        "Races (0x07E6)".to_string(),
        "Units Master (0x07E8)".to_string(),
        "Buildings Master (0x07ED)".to_string(),
        "Building Collision (0x07EE)".to_string(),
        "Unit Loot Tables (0x07F8)".to_string(),
        "Chest Loot Tables (0x0811)".to_string(),
        "Level Progression (0x0800)".to_string(),
        "Objects Master (0x0802)".to_string(),
        "Object Collision (0x0809)".to_string(),
        "Unit Equipment (0x07E9)".to_string(),
        "Merchant Inventory (0x07FA)".to_string(),
        "Quests (0x080D)".to_string(),
        "Weapon Types (0x080F)".to_string(),
        "Weapon Materials (0x0810)".to_string(),
        "Item Sets (0x0818)".to_string(),
        "Terrain Cultivation (0x07F0)".to_string(),
        "Portals (0x0805)".to_string(),
        "Descriptions (0x080A)".to_string(),
        "Localized Strings".to_string(),
    ]
}

fn get_pak_order_key(path: &Path) -> (u32, String) {
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let num_str: String = stem.chars().filter(|c| c.is_ascii_digit()).collect();
    let num = num_str.parse::<u32>().unwrap_or(0);
    (num, stem)
}

fn collect_sorted_paks(dir: &Path) -> Vec<PathBuf> {
    let mut paks = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("pak") {
                paks.push(p);
            }
        }
    }
    paks.sort_by_key(|a| get_pak_order_key(a));
    paks
}

pub fn find_and_load_texture(
    cff_dir: &Path,
    asset_source: &Path,
    mesh_name: &str,
) -> Option<(image::RgbaImage, String)> {
    let clean_name = mesh_name
        .trim()
        .trim_end_matches(".msh")
        .trim_end_matches(".msb");

    if clean_name.is_empty() {
        return None;
    }

    let extensions = ["dds", "tga", "png"];

    if asset_source.is_file() {
        if let Some((bytes, fname)) =
            crate::pak::read_file_from_pak(asset_source, clean_name, &extensions)
        {
            return decode_raw_texture_bytes(&bytes, &fname);
        }
    } else if asset_source.is_dir() {
        if let Some(res) = check_loose_texture_dirs(asset_source, clean_name, &extensions) {
            return Some(res);
        }
        let sorted_paks = collect_sorted_paks(asset_source);
        for pak_path in sorted_paks.iter().rev() {
            if let Some((bytes, fname)) =
                crate::pak::read_file_from_pak(pak_path, clean_name, &extensions)
            {
                return decode_raw_texture_bytes(&bytes, &fname);
            }
        }
    }

    let mut search_dirs = Vec::new();
    search_dirs.push(cff_dir.to_path_buf());
    if let Some(p) = cff_dir.parent() {
        search_dirs.push(p.to_path_buf());
        if let Some(gp) = p.parent() {
            search_dirs.push(gp.to_path_buf());
        }
    }

    for dir in &search_dirs {
        if let Some(res) = check_loose_texture_dirs(dir, clean_name, &extensions) {
            return Some(res);
        }
        let sorted_paks = collect_sorted_paks(dir);
        for pak_path in sorted_paks.iter().rev() {
            if let Some((bytes, fname)) =
                crate::pak::read_file_from_pak(pak_path, clean_name, &extensions)
            {
                return decode_raw_texture_bytes(&bytes, &fname);
            }
        }
    }

    None
}

fn check_loose_texture_dirs(
    base_dir: &Path,
    clean_name: &str,
    extensions: &[&str],
) -> Option<(image::RgbaImage, String)> {
    let check_dirs = vec![
        base_dir.to_path_buf(),
        base_dir.join("textures"),
        base_dir.join("textures").join("gui"),
        base_dir.join("textures").join("ui"),
        base_dir.join("ui"),
        base_dir.join("gui"),
    ];

    for d in check_dirs {
        if !d.is_dir() {
            continue;
        }
        for ext in extensions {
            let f = d.join(format!("{}.{}", clean_name, ext));
            if f.is_file()
                && let Ok(bytes) = fs::read(&f)
            {
                let fname = f.file_name().unwrap().to_string_lossy().to_string();
                return decode_raw_texture_bytes(&bytes, &fname);
            }
        }
    }
    None
}

fn decode_raw_texture_bytes(bytes: &[u8], filename: &str) -> Option<(image::RgbaImage, String)> {
    if filename.to_lowercase().ends_with(".dds") {
        if let Ok(rgba) = crate::dds::decode_dds_to_rgba(bytes, Some(128)) {
            return Some((rgba, filename.to_string()));
        }
    } else if let Ok(dyn_img) = image::load_from_memory(bytes) {
        return Some((dyn_img.into_rgba8(), filename.to_string()));
    }
    None
}

pub fn resolve_spell_cross_reference(
    cff_dir: &Path,
    spell_id: u16,
    scroll_id: u16,
) -> SpellVisualDetails {
    let mut spell_mesh = String::new();
    let mut scroll_mesh = String::new();

    let manifest_path = cff_dir.join("manifest.json");
    if let Ok(m_str) = fs::read_to_string(manifest_path)
        && let Ok(manifest) = serde_json::from_str::<Manifest>(&m_str)
        && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DC)
    {
        let chunk_path = cff_dir.join(&chunk.file);
        if let Ok(bytes) = fs::read(chunk_path) {
            let count = bytes.len() / 69;
            for i in 0..count {
                let offset = i * 69;
                let id = Cursor::new(&bytes[offset..offset + 2])
                    .read_u16::<LittleEndian>()
                    .unwrap_or(0);
                let flag = bytes[offset + 2];
                let mesh_bytes = &bytes[offset + 3..offset + 67];
                let end = mesh_bytes
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(mesh_bytes.len());
                let m_name = decode_windows(&mesh_bytes[..end]);

                if id == spell_id && (flag == 2 || spell_mesh.is_empty()) {
                    spell_mesh = m_name.clone();
                }
                if id == scroll_id && (flag == 1 || scroll_mesh.is_empty()) {
                    scroll_mesh = m_name;
                }
            }
        }
    }

    if scroll_mesh.is_empty() {
        scroll_mesh = "ui_item_spellscroll".to_string();
    }

    SpellVisualDetails {
        spell_name: format!("Spell #{}", spell_id),
        spell_mesh,
        scroll_name: format!("Scroll #{}", scroll_id),
        scroll_mesh,
    }
}

pub fn load_editor_items(
    cff_dir: &Path,
    category: &str,
    filter: &str,
    lang_filter: &str,
) -> Vec<EditorItem> {
    if category.contains("0x2335") || category.contains("0x234E") || category.contains("0x2330") {
        return load_sf2_items(cff_dir, category, filter);
    }
    if category != "Localized Strings" {
        return load_sf1_items(cff_dir, category, filter);
    }

    const MAX_UNFILTERED_ITEMS: usize = 1000;
    let mut items = Vec::new();
    let filter_lower = filter.to_lowercase();
    let tags = get_language_tag_map(cff_dir);
    let json_dir = cff_dir.join("texts_json");
    let mut total_matches = 0;

    if let Ok(entries) = fs::read_dir(json_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json")
                && !path.to_string_lossy().ends_with(".meta.json")
                && let Ok(content) = fs::read_to_string(&path)
                && let Ok(map) = serde_json::from_str::<BTreeMap<String, String>>(&content)
            {
                let fname = path.file_stem().unwrap().to_string_lossy().to_string();
                for (k, v) in map {
                    let (l_id, b_id, camp_id, lang_tag) = if k.starts_with("f566_") {
                        let parts: Vec<&str> = k.split('_').collect();
                        let str_id = parts
                            .get(2)
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(0);
                        let lang = ((str_id >> 16) & 0xFF) as u8;
                        let camp = (str_id >> 24) as u8;
                        let base = (str_id & 0xFFFF) as u16;
                        let tag = tags
                            .get(&lang)
                            .cloned()
                            .unwrap_or_else(|| format!("L{}", lang));
                        (lang, base, camp, tag)
                    } else {
                        (1, 0, 0, "TXT".to_string())
                    };

                    if lang_filter != "All Languages"
                        && !lang_filter.contains(&format!("Slot {}", l_id))
                    {
                        continue;
                    }

                    let camp_str = match camp_id {
                        1 => " [BoW]",
                        2 => " [SotP]",
                        _ => "",
                    };

                    let display = format!("[{}]{} #{:<5} | {}", lang_tag, camp_str, b_id, v);
                    if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                        total_matches += 1;
                        if items.len() < MAX_UNFILTERED_ITEMS {
                            items.push(EditorItem {
                                id_str: format!("{}:{}", fname, k),
                                val1: v.clone(),
                                val2: format!(
                                    "Slot: {} ({}){} | Base ID: {}",
                                    l_id, lang_tag, camp_str, b_id
                                ),
                                display,
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }
    }

    if total_matches > MAX_UNFILTERED_ITEMS {
        items.push(EditorItem {
            val2: format!(
                "Displaying first {} of {} total entries.",
                MAX_UNFILTERED_ITEMS, total_matches
            ),
            display: format!(
                "--- [Showing first {} of {} strings. Refine search filter to narrow results] ---",
                MAX_UNFILTERED_ITEMS, total_matches
            ),
            ..Default::default()
        });
    }

    items
}

pub fn save_editor_item(
    cff_dir: &Path,
    category: &str,
    index: usize,
    fields: &[String],
) -> io::Result<()> {
    if category.contains("0x2335") || category.contains("0x234E") || category.contains("0x2330") {
        let val1 = fields.get(1).map(|s| s.as_str()).unwrap_or("");
        return save_sf2_item(cff_dir, category, index, val1);
    }

    if category != "Localized Strings" {
        return save_sf1_item(cff_dir, category, index, fields);
    }

    let id_str = fields.first().map(|s| s.as_str()).unwrap_or("");
    let val1 = fields.get(13).map(|s| s.as_str()).unwrap_or("");

    let parts: Vec<&str> = id_str.splitn(2, ':').collect();
    if parts.len() == 2 {
        let fname = format!("{}.json", parts[0]);
        let key = parts[1];
        let json_path = cff_dir.join("texts_json").join(&fname);

        if json_path.exists() {
            let mut map: BTreeMap<String, String> =
                serde_json::from_str(&fs::read_to_string(&json_path)?)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
            map.insert(key.to_string(), val1.to_string());
            let encoded = serde_json::to_vec_pretty(&map)?;
            crate::tools::atomic_write(&json_path, &encoded)?;
        }

        if key.starts_with("f566_") {
            let k_parts: Vec<&str> = key.split('_').collect();
            if let Some(offset) = k_parts.get(1).and_then(|s| s.parse::<usize>().ok()) {
                let str_id = k_parts
                    .get(2)
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                let lang_id = ((str_id >> 16) & 0xFF) as u16;

                let chunk_dat_name = parts[0].replace("_strings", ".dat");
                let chunk_dat_path = cff_dir.join(&chunk_dat_name);
                if chunk_dat_path.exists() {
                    let mut b = fs::read(&chunk_dat_path)?;
                    if offset + 566 <= b.len() {
                        let mut text_bytes = encode_by_lang(val1, lang_id);
                        if text_bytes.len() > 511 {
                            text_bytes.truncate(511);
                        }
                        let mut padded = vec![0u8; 512];
                        padded[..text_bytes.len()].copy_from_slice(&text_bytes);
                        b[offset + 54..offset + 566].copy_from_slice(&padded);
                        crate::tools::atomic_write(&chunk_dat_path, &b)?;
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn add_editor_item(cff_dir: &Path, category: &str) -> io::Result<String> {
    let manifest_path = cff_dir.join("manifest.json");
    let m_str = fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&m_str)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    if category.contains("0x07DC") {
        if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DC) {
            let chunk_path = cff_dir.join(&chunk.file);
            let mut bytes = if chunk_path.exists() {
                fs::read(&chunk_path)?
            } else {
                Vec::new()
            };

            let mut max_id = 0u16;
            let num_records = bytes.len() / 69;
            for i in 0..num_records {
                let id = Cursor::new(&bytes[i * 69..i * 69 + 2])
                    .read_u16::<LittleEndian>()
                    .unwrap_or(0);
                max_id = max_id.max(id);
            }
            let new_id = max_id.saturating_add(1);

            let mut record = vec![0u8; 69];
            record[0..2].copy_from_slice(&new_id.to_le_bytes());
            record[2] = 1;
            let default_mesh = b"ui_item_new_asset";
            let len = default_mesh.len().min(63);
            record[3..3 + len].copy_from_slice(&default_mesh[..len]);

            bytes.extend_from_slice(&record);
            crate::tools::atomic_write(&chunk_path, &bytes)?;
            return Ok(new_id.to_string());
        }
    } else if category.contains("0x07E2") {
        if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E2) {
            let chunk_path = cff_dir.join(&chunk.file);
            let mut bytes = if chunk_path.exists() {
                fs::read(&chunk_path)?
            } else {
                Vec::new()
            };

            let mut max_id = 0u16;
            let num_records = bytes.len() / 4;
            for i in 0..num_records {
                let id = Cursor::new(&bytes[i * 4..i * 4 + 2])
                    .read_u16::<LittleEndian>()
                    .unwrap_or(0);
                max_id = max_id.max(id);
            }
            let new_id = max_id.saturating_add(1);

            let mut record = vec![0u8; 4];
            record[0..2].copy_from_slice(&new_id.to_le_bytes());

            bytes.extend_from_slice(&record);
            crate::tools::atomic_write(&chunk_path, &bytes)?;
            return Ok(new_id.to_string());
        }
    } else {
        let mut target_f566_chunk = None;
        for chunk in &manifest.chunks {
            let p = cff_dir.join(&chunk.file);
            if let Ok(meta) = fs::metadata(&p)
                && meta.len() >= 566
                && meta.len() % 566 == 0
            {
                target_f566_chunk = Some(chunk.file.clone());
                break;
            }
        }

        if let Some(chunk_file) = target_f566_chunk {
            let chunk_path = cff_dir.join(&chunk_file);
            let mut bytes = fs::read(&chunk_path)?;

            let mut max_base_id = 0u16;
            let count = bytes.len() / 566;
            for i in 0..count {
                let off = i * 566;
                let sid = Cursor::new(&bytes[off..off + 4])
                    .read_u32::<LittleEndian>()
                    .unwrap_or(0);
                max_base_id = max_base_id.max((sid & 0xFFFF) as u16);
            }

            let new_base_id = max_base_id.saturating_add(1);
            let default_lang: u8 = 1;
            let default_camp: u8 = 0;
            let new_str_id = ((default_camp as u32) << 24)
                | ((default_lang as u32) << 16)
                | (new_base_id as u32);
            let new_offset = bytes.len();

            let mut record = vec![0u8; 566];
            record[0..4].copy_from_slice(&new_str_id.to_le_bytes());
            let default_text = "New Localized String";
            let text_bytes = encode_by_lang(default_text, default_lang as u16);
            let len = text_bytes.len().min(511);
            record[54..54 + len].copy_from_slice(&text_bytes[..len]);

            bytes.extend_from_slice(&record);
            crate::tools::atomic_write(&chunk_path, &bytes)?;

            let stem = chunk_file.trim_end_matches(".dat");
            let json_dir = cff_dir.join("texts_json");
            fs::create_dir_all(&json_dir)?;
            let json_path = json_dir.join(format!("{}_strings.json", stem));

            let mut map: BTreeMap<String, String> = if json_path.exists() {
                serde_json::from_str(&fs::read_to_string(&json_path)?).unwrap_or_default()
            } else {
                BTreeMap::new()
            };

            let new_key = format!("f566_{:08}_{}", new_offset, new_str_id);
            map.insert(new_key.clone(), default_text.to_string());
            let encoded = serde_json::to_vec_pretty(&map)?;
            crate::tools::atomic_write(&json_path, &encoded)?;

            return Ok(format!("{}_strings:{}", stem, new_key));
        }
    }
    Err(io::Error::other("Failed to create new record"))
}

pub fn duplicate_editor_item(
    cff_dir: &Path,
    category: &str,
    index: usize,
    id_str: &str,
) -> io::Result<String> {
    let manifest_path = cff_dir.join("manifest.json");
    let m_str = fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&m_str)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    if category.contains("0x07DC") {
        if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DC) {
            let chunk_path = cff_dir.join(&chunk.file);
            let mut bytes = fs::read(&chunk_path)?;
            let offset = index * 69;
            if offset + 69 <= bytes.len() {
                let mut max_id = 0u16;
                let num_records = bytes.len() / 69;
                for i in 0..num_records {
                    let id = Cursor::new(&bytes[i * 69..i * 69 + 2])
                        .read_u16::<LittleEndian>()
                        .unwrap_or(0);
                    max_id = max_id.max(id);
                }
                let new_id = max_id.saturating_add(1);

                let mut cloned = bytes[offset..offset + 69].to_vec();
                cloned[0..2].copy_from_slice(&new_id.to_le_bytes());

                bytes.extend_from_slice(&cloned);
                crate::tools::atomic_write(&chunk_path, &bytes)?;
                return Ok(new_id.to_string());
            }
        }
    } else if category.contains("0x07E2") {
        if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E2) {
            let chunk_path = cff_dir.join(&chunk.file);
            let mut bytes = fs::read(&chunk_path)?;
            let offset = index * 4;
            if offset + 4 <= bytes.len() {
                let mut max_id = 0u16;
                let num_records = bytes.len() / 4;
                for i in 0..num_records {
                    let id = Cursor::new(&bytes[i * 4..i * 4 + 2])
                        .read_u16::<LittleEndian>()
                        .unwrap_or(0);
                    max_id = max_id.max(id);
                }
                let new_id = max_id.saturating_add(1);

                let mut cloned = bytes[offset..offset + 4].to_vec();
                cloned[0..2].copy_from_slice(&new_id.to_le_bytes());

                bytes.extend_from_slice(&cloned);
                crate::tools::atomic_write(&chunk_path, &bytes)?;
                return Ok(new_id.to_string());
            }
        }
    } else {
        let parts: Vec<&str> = id_str.splitn(2, ':').collect();
        if parts.len() == 2 {
            let fname = format!("{}.json", parts[0]);
            let key = parts[1];
            let json_path = cff_dir.join("texts_json").join(&fname);

            if key.starts_with("f566_") {
                let k_parts: Vec<&str> = key.split('_').collect();
                if let Some(src_offset) = k_parts.get(1).and_then(|s| s.parse::<usize>().ok()) {
                    let chunk_dat_name = parts[0].replace("_strings", ".dat");
                    let chunk_dat_path = cff_dir.join(&chunk_dat_name);
                    if chunk_dat_path.exists() {
                        let mut b = fs::read(&chunk_dat_path)?;
                        if src_offset + 566 <= b.len() {
                            let mut max_base_id = 0u16;
                            let count = b.len() / 566;
                            for i in 0..count {
                                let off = i * 566;
                                let sid = Cursor::new(&b[off..off + 4])
                                    .read_u32::<LittleEndian>()
                                    .unwrap_or(0);
                                max_base_id = max_base_id.max((sid & 0xFFFF) as u16);
                            }
                            let new_base_id = max_base_id.saturating_add(1);

                            let src_block = &b[src_offset..src_offset + 566];
                            let src_str_id = Cursor::new(&src_block[0..4])
                                .read_u32::<LittleEndian>()
                                .unwrap_or(0);
                            let camp_id = src_str_id >> 24;
                            let lang_id = (src_str_id >> 16) & 0xFF;
                            let new_str_id =
                                (camp_id << 24) | (lang_id << 16) | (new_base_id as u32);

                            let mut cloned_block = src_block.to_vec();
                            cloned_block[0..4].copy_from_slice(&new_str_id.to_le_bytes());
                            let new_offset = b.len();

                            b.extend_from_slice(&cloned_block);
                            crate::tools::atomic_write(&chunk_dat_path, &b)?;

                            let mut map: BTreeMap<String, String> = if json_path.exists() {
                                serde_json::from_str(&fs::read_to_string(&json_path)?)
                                    .unwrap_or_default()
                            } else {
                                BTreeMap::new()
                            };

                            let original_val = map.get(key).cloned().unwrap_or_default();
                            let new_key = format!("f566_{:08}_{}", new_offset, new_str_id);
                            map.insert(new_key.clone(), original_val);
                            let encoded = serde_json::to_vec_pretty(&map)?;
                            crate::tools::atomic_write(&json_path, &encoded)?;

                            return Ok(format!("{}:{}", parts[0], new_key));
                        }
                    }
                }
            }
        }
    }
    Err(io::Error::other("Failed to duplicate record"))
}

pub fn delete_editor_item(
    cff_dir: &Path,
    category: &str,
    index: usize,
    id_str: &str,
) -> io::Result<()> {
    let manifest_path = cff_dir.join("manifest.json");
    let m_str = fs::read_to_string(&manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&m_str)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    if category.contains("0x07DC") {
        if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DC) {
            let chunk_path = cff_dir.join(&chunk.file);
            let mut bytes = fs::read(&chunk_path)?;
            let offset = index * 69;
            if offset + 69 <= bytes.len() {
                bytes.drain(offset..offset + 69);
                crate::tools::atomic_write(&chunk_path, &bytes)?;
                return Ok(());
            }
        }
    } else if category.contains("0x07E2") {
        if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E2) {
            let chunk_path = cff_dir.join(&chunk.file);
            let mut bytes = fs::read(&chunk_path)?;
            let offset = index * 4;
            if offset + 4 <= bytes.len() {
                bytes.drain(offset..offset + 4);
                crate::tools::atomic_write(&chunk_path, &bytes)?;
                return Ok(());
            }
        }
    } else {
        let parts: Vec<&str> = id_str.splitn(2, ':').collect();
        if parts.len() == 2 {
            let fname = format!("{}.json", parts[0]);
            let key = parts[1];
            let json_path = cff_dir.join("texts_json").join(&fname);

            if key.starts_with("f566_") {
                let k_parts: Vec<&str> = key.split('_').collect();
                if let Some(target_offset) = k_parts.get(1).and_then(|s| s.parse::<usize>().ok()) {
                    let chunk_dat_name = parts[0].replace("_strings", ".dat");
                    let chunk_dat_path = cff_dir.join(&chunk_dat_name);
                    if chunk_dat_path.exists() {
                        let mut b = fs::read(&chunk_dat_path)?;
                        if target_offset + 566 <= b.len() {
                            b.drain(target_offset..target_offset + 566);
                            crate::tools::atomic_write(&chunk_dat_path, &b)?;
                        }
                    }

                    if json_path.exists() {
                        let map: BTreeMap<String, String> = serde_json::from_str(
                            &fs::read_to_string(&json_path)?,
                        )
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
                        let mut updated_map = BTreeMap::new();
                        for (k, v) in map {
                            if k == key {
                                continue;
                            }
                            if k.starts_with("f566_") {
                                let kp: Vec<&str> = k.split('_').collect();
                                if let (Some(off), Some(sid)) =
                                    (kp.get(1).and_then(|s| s.parse::<usize>().ok()), kp.get(2))
                                    && off > target_offset
                                {
                                    let new_k = format!("f566_{:08}_{}", off - 566, sid);
                                    updated_map.insert(new_k, v);
                                    continue;
                                }
                            }
                            updated_map.insert(k, v);
                        }
                        let encoded = serde_json::to_vec_pretty(&updated_map)?;
                        crate::tools::atomic_write(&json_path, &encoded)?;
                    }
                    return Ok(());
                }
            }
        }
    }
    Err(io::Error::other("Failed to delete record"))
}
