use crate::UiLogger;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use encoding_rs::{WINDOWS_1251, WINDOWS_1252};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::Path;

// -----------------------------------------------------------------------------
// KNOWN SF1 CHUNK DEFINITIONS & VALIDATION
// -----------------------------------------------------------------------------

pub struct Sf1ChunkInfo {
    pub name: &'static str,
    pub stride: usize,
    pub default_c_type: i16,
}

pub fn get_sf1_chunk_info(id: u32) -> Option<Sf1ChunkInfo> {
    match id {
        0x07DC => Some(Sf1ChunkInfo {
            name: "2dGfxItems",
            stride: 69,
            default_c_type: 1,
        }),
        0x07E1 => Some(Sf1ChunkInfo {
            name: "SpellMultiMap",
            stride: 6,
            default_c_type: 1,
        }),
        0x07E2 => Some(Sf1ChunkInfo {
            name: "SpellsBiMap",
            stride: 4,
            default_c_type: 1,
        }),
        0x07F7 => Some(Sf1ChunkInfo {
            name: "TextDialogueMap",
            stride: 4,
            default_c_type: 1,
        }),
        0x07FC => Some(Sf1ChunkInfo {
            name: "TypeCategoryMap",
            stride: 3,
            default_c_type: 1,
        }),
        0x07FF => Some(Sf1ChunkInfo {
            name: "EntityLinkMap",
            stride: 5,
            default_c_type: 1,
        }),
        0x0800 => Some(Sf1ChunkInfo {
            name: "ComplexProperties",
            stride: 15,
            default_c_type: 3,
        }), // Mandatory c_type == 3!
        0x0801 => Some(Sf1ChunkInfo {
            name: "WordValuesArray",
            stride: 2,
            default_c_type: 1,
        }),
        0x080A => Some(Sf1ChunkInfo {
            name: "LocalizedStringIds",
            stride: 4,
            default_c_type: 1,
        }),
        0x080B => Some(Sf1ChunkInfo {
            name: "AudioSpeechParams",
            stride: 6,
            default_c_type: 1,
        }),
        0x080E => Some(Sf1ChunkInfo {
            name: "CompoundKeyTable",
            stride: 9,
            default_c_type: 1,
        }),
        0x0818 => Some(Sf1ChunkInfo {
            name: "SystemLookupMap",
            stride: 4,
            default_c_type: 1,
        }),
        _ => None,
    }
}

pub fn get_sf2_chunk_name(id: u32) -> Option<&'static str> {
    match id {
        0x234E => Some("Abilities"),
        _ => None,
    }
}

// -----------------------------------------------------------------------------
// MANIFEST STRUCTURES
// -----------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct ChunkManifest {
    file: String,
    id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flag1: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flag2: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    occurrence: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    comp_flag: Option<i16>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    c_type: Option<i16>,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    chunks: Vec<ChunkManifest>,
}

#[derive(Serialize, Deserialize)]
struct ChunkMeta {
    format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_strings: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_bytes: Option<usize>,
}

#[derive(PartialEq, Debug)]
enum ChunkFormat {
    Binary,
    Fixed566,
    StringTable,
    DeveloperTable,
    TableBased(usize, usize),
}

struct TableBasedEntry {
    id: u32,
    extra_bytes: Vec<u8>,
    strings: BTreeMap<usize, String>,
}

// -----------------------------------------------------------------------------
// TEXT ENCODING HELPERS
// -----------------------------------------------------------------------------

pub fn decode_by_lang(bytes: &[u8], lang_id: u16) -> String {
    if let Ok(utf8_str) = std::str::from_utf8(bytes) {
        return utf8_str.to_string();
    }
    match lang_id {
        5 => {
            let (cow, _, _) = WINDOWS_1251.decode(bytes);
            cow.into_owned()
        }
        0..=4 => {
            let (cow, _, _) = WINDOWS_1252.decode(bytes);
            cow.into_owned()
        }
        _ => decode_windows(bytes),
    }
}

pub fn encode_by_lang(text: &str, lang_id: u16) -> Vec<u8> {
    let has_cyrillic = text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
    if has_cyrillic || lang_id == 5 {
        let (cow, _, _) = WINDOWS_1251.encode(text);
        cow.into_owned()
    } else {
        let (cow, _, had_errors) = WINDOWS_1252.encode(text);
        if had_errors {
            let (cow_cyrillic, _, _) = WINDOWS_1251.encode(text);
            cow_cyrillic.into_owned()
        } else {
            cow.into_owned()
        }
    }
}

pub fn decode_windows(bytes: &[u8]) -> String {
    if let Ok(utf8_str) = std::str::from_utf8(bytes) {
        return utf8_str.to_string();
    }

    let cyrillic_hits = bytes.iter().filter(|&&b| b >= 0xC0).count();
    if cyrillic_hits >= 2 || (!bytes.is_empty() && bytes.len() <= 3 && cyrillic_hits >= 1) {
        let (cow, _, had_errors) = WINDOWS_1251.decode(bytes);
        if !had_errors {
            return cow.into_owned();
        }
    }

    let (cow, _, _) = WINDOWS_1252.decode(bytes);
    cow.into_owned()
}

pub fn encode_windows(text: &str) -> Vec<u8> {
    let has_cyrillic = text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
    if has_cyrillic {
        let (cow, _, _) = WINDOWS_1251.encode(text);
        cow.into_owned()
    } else {
        let (cow, _, had_errors) = WINDOWS_1252.encode(text);
        if had_errors {
            let (cow_cyrillic, _, _) = WINDOWS_1251.encode(text);
            cow_cyrillic.into_owned()
        } else {
            cow.into_owned()
        }
    }
}

// -----------------------------------------------------------------------------
// FORMAT DETECTION ENGINE
// -----------------------------------------------------------------------------

fn detect_format(data: &[u8]) -> ChunkFormat {
    if data.len() < 8 {
        return ChunkFormat::Binary;
    }

    if data.len() >= 566 && data.len().is_multiple_of(566) {
        let mut is_f566 = true;
        for i in 0..std::cmp::min(5, data.len() / 566) {
            if data[i * 566 + 565] != 0 {
                is_f566 = false;
                break;
            }
        }
        if is_f566 {
            return ChunkFormat::Fixed566;
        }
    }

    let mut cursor = Cursor::new(data);
    let count = cursor.read_u32::<LittleEndian>().unwrap_or(0);
    if count == 0 || count > 200_000 {
        return ChunkFormat::Binary;
    }

    // Detect Developer Table (Format C)
    let mut is_c = true;
    let mut offset = 4;
    for _ in 0..count {
        if offset + 6 > data.len() || data[offset] != 0x02 {
            is_c = false;
            break;
        }
        offset += 6;

        if offset + 4 > data.len() {
            is_c = false;
            break;
        }
        let mut len_cursor = Cursor::new(&data[offset..offset + 4]);
        let name_len = len_cursor.read_u32::<LittleEndian>().unwrap_or(0xFFFFFF) as usize;
        if name_len > 100_000 || offset + 4 + name_len > data.len() {
            is_c = false;
            break;
        }
        offset += 4 + name_len;

        if offset + 4 > data.len() {
            is_c = false;
            break;
        }
        let mut key_cursor = Cursor::new(&data[offset..offset + 4]);
        let key_len = key_cursor.read_u32::<LittleEndian>().unwrap_or(0xFFFFFF) as usize;
        if key_len > 1000 || offset + 4 + key_len > data.len() {
            is_c = false;
            break;
        }
        offset += 4 + key_len;
    }
    if is_c && offset == data.len() {
        return ChunkFormat::DeveloperTable;
    }

    // Detect String Table (Format A)
    let mut is_a = true;
    offset = 4;
    for _ in 0..count {
        if offset + 5 > data.len() || data[offset] != 0x01 {
            is_a = false;
            break;
        }
        offset += 1;

        let mut key_cursor = Cursor::new(&data[offset..offset + 4]);
        let key_len = key_cursor.read_u32::<LittleEndian>().unwrap_or(0xFFFFFF) as usize;
        if key_len > 1000 || offset + 4 + key_len > data.len() {
            is_a = false;
            break;
        }
        offset += 4 + key_len;

        if offset + 4 > data.len() {
            is_a = false;
            break;
        }
        let mut text_cursor = Cursor::new(&data[offset..offset + 4]);
        let text_len = text_cursor.read_u32::<LittleEndian>().unwrap_or(0xFFFFFF) as usize;
        if text_len > 100_000 || offset + 4 + (text_len * 2) > data.len() {
            is_a = false;
            break;
        }
        offset += 4 + (text_len * 2);
    }
    if is_a && offset == data.len() {
        return ChunkFormat::StringTable;
    }

    // Detect Table Based (Format B)
    for e in 0..=32 {
        for n in 1..=10 {
            let mut is_b = true;
            offset = 4;
            for _ in 0..count {
                if offset + 4 + e > data.len() {
                    is_b = false;
                    break;
                }
                offset += 4 + e;
                for _ in 0..n {
                    if offset + 4 > data.len() {
                        is_b = false;
                        break;
                    }
                    let mut str_cursor = Cursor::new(&data[offset..offset + 4]);
                    let str_len =
                        str_cursor.read_u32::<LittleEndian>().unwrap_or(0xFFFFFF) as usize;
                    if str_len > 100_000 || offset + 4 + (str_len * 2) > data.len() {
                        is_b = false;
                        break;
                    }
                    offset += 4 + (str_len * 2);
                }
            }
            if is_b && offset == data.len() {
                return ChunkFormat::TableBased(n, e);
            }
        }
    }

    ChunkFormat::Binary
}

// -----------------------------------------------------------------------------
// TEXT EXPORT / IMPORT ENGINE
// -----------------------------------------------------------------------------

fn export_text(data: &[u8], json_path: &Path, format: ChunkFormat) -> io::Result<()> {
    let mut texts: BTreeMap<String, String> = BTreeMap::new();
    let meta_path = json_path.with_extension("meta.json");

    let chunk_meta = match format {
        ChunkFormat::Fixed566 => {
            let mut offset = 0;
            while offset + 566 <= data.len() {
                let block = &data[offset..offset + 566];
                let str_id = Cursor::new(&block[0..4]).read_u32::<LittleEndian>()?;
                let lang_id = ((str_id >> 16) & 0xFF) as u16;
                let mut text_bytes = &block[54..566];
                if let Some(null_idx) = text_bytes.iter().position(|&b| b == 0) {
                    text_bytes = &text_bytes[..null_idx];
                }
                texts.insert(
                    format!("f566_{:08}_{}", offset, str_id),
                    decode_by_lang(text_bytes, lang_id),
                );
                offset += 566;
            }
            ChunkMeta {
                format: "fixed_566".to_string(),
                num_strings: None,
                extra_bytes: None,
            }
        }
        ChunkFormat::StringTable => {
            let mut cursor = Cursor::new(data);
            let count = cursor.read_u32::<LittleEndian>()?;
            let mut offset = 4;

            for _ in 0..count {
                offset += 1;
                let mut c = Cursor::new(&data[offset..offset + 4]);
                let key_len = c.read_u32::<LittleEndian>()? as usize;
                offset += 4;
                let key = String::from_utf8_lossy(&data[offset..offset + key_len]).into_owned();
                offset += key_len;

                let mut c = Cursor::new(&data[offset..offset + 4]);
                let text_len = c.read_u32::<LittleEndian>()? as usize;
                offset += 4;
                let u16_slice: Vec<u16> = data[offset..offset + text_len * 2]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&ch| u16::from_le_bytes(ch))
                    .collect();
                let text = String::from_utf16_lossy(&u16_slice);
                offset += text_len * 2;
                texts.insert(key, text);
            }
            ChunkMeta {
                format: "string_table".to_string(),
                num_strings: None,
                extra_bytes: None,
            }
        }
        ChunkFormat::DeveloperTable => {
            let mut cursor = Cursor::new(data);
            let count = cursor.read_u32::<LittleEndian>()?;
            let mut offset = 4;

            for i in 0..count {
                offset += 1;
                let mut c = Cursor::new(&data[offset..offset + 4]);
                let id_val = c.read_u32::<LittleEndian>()?;
                offset += 4;

                let flag = data[offset];
                offset += 1;

                let mut c = Cursor::new(&data[offset..offset + 4]);
                let name_len = c.read_u32::<LittleEndian>()? as usize;
                offset += 4;
                let name = decode_windows(&data[offset..offset + name_len]);
                offset += name_len;

                let mut c = Cursor::new(&data[offset..offset + 4]);
                let key_len = c.read_u32::<LittleEndian>()? as usize;
                offset += 4;
                let key_str = decode_windows(&data[offset..offset + key_len]);
                offset += key_len;

                texts.insert(format!("{:05}_{}_{}_{}", i, id_val, flag, key_str), name);
            }
            ChunkMeta {
                format: "developer_table".to_string(),
                num_strings: None,
                extra_bytes: None,
            }
        }
        ChunkFormat::TableBased(num_strings, extra_bytes) => {
            let mut cursor = Cursor::new(data);
            let count = cursor.read_u32::<LittleEndian>()?;
            let mut offset = 4;

            for i in 0..count {
                let mut c = Cursor::new(&data[offset..offset + 4]);
                let id_val = c.read_u32::<LittleEndian>()?;
                offset += 4;

                let extra_slice = &data[offset..offset + extra_bytes];
                let extra_hex: String = extra_slice.iter().map(|b| format!("{:02x}", b)).collect();
                offset += extra_bytes;

                for s in 0..num_strings {
                    let mut c = Cursor::new(&data[offset..offset + 4]);
                    let str_len = c.read_u32::<LittleEndian>()? as usize;
                    offset += 4;

                    let u16_slice: Vec<u16> = data[offset..offset + str_len * 2]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|&ch| u16::from_le_bytes(ch))
                        .collect();
                    let text = String::from_utf16_lossy(&u16_slice);
                    offset += str_len * 2;

                    texts.insert(format!("{:05}_{}_{}_str{}", i, id_val, extra_hex, s), text);
                }
            }
            ChunkMeta {
                format: "table_based".to_string(),
                num_strings: Some(num_strings),
                extra_bytes: Some(extra_bytes),
            }
        }
        ChunkFormat::Binary => return Ok(()),
    };

    if !texts.is_empty() {
        let f = File::create(json_path)?;
        serde_json::to_writer_pretty(f, &texts)?;

        let mf = File::create(meta_path)?;
        serde_json::to_writer_pretty(mf, &chunk_meta)?;
    }

    Ok(())
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .filter_map(|i| {
            if i + 2 <= hex.len() {
                u8::from_str_radix(&hex[i..i + 2], 16).ok()
            } else {
                None
            }
        })
        .collect()
}

fn import_text(json_path: &Path, chunk_path: &Path) -> io::Result<()> {
    let json_data = fs::read_to_string(json_path)?;
    let mut texts: BTreeMap<String, String> = serde_json::from_str(&json_data)?;
    if texts.is_empty() {
        return Ok(());
    }

    let mut is_table_based = false;
    let mut is_developer_table = false;
    let mut is_fixed_566 = false;
    let mut num_strings = 0;

    let meta_path = json_path.with_extension("meta.json");
    if meta_path.exists()
        && let Ok(meta_data) = fs::read_to_string(&meta_path)
        && let Ok(meta) = serde_json::from_str::<ChunkMeta>(&meta_data)
    {
        match meta.format.as_str() {
            "fixed_566" => is_fixed_566 = true,
            "developer_table" => is_developer_table = true,
            "table_based" => {
                is_table_based = true;
                num_strings = meta.num_strings.unwrap_or(0);
            }
            _ => {}
        }
    } else if let Some(fmt_tag) = texts.remove("_format") {
        if fmt_tag == "fixed_566" {
            is_fixed_566 = true;
        } else if fmt_tag == "developer_table" {
            is_developer_table = true;
        } else if fmt_tag.starts_with("table_based_") {
            is_table_based = true;
            let parts: Vec<&str> = fmt_tag.split('_').collect();
            if let Some(n_str) = parts.get(2) {
                num_strings = n_str.parse::<usize>().unwrap_or(0);
            }
        }
    } else if let Some(first_key) = texts.keys().next() {
        if first_key.starts_with("f566_") {
            is_fixed_566 = true;
        } else {
            let parts: Vec<&str> = first_key.splitn(4, '_').collect();
            if parts.len() == 4
                && parts[0].parse::<u32>().is_ok()
                && parts[1].parse::<u32>().is_ok()
            {
                if parts[3].starts_with("str") {
                    is_table_based = true;
                } else {
                    is_developer_table = true;
                }
            }
        }
    }

    if is_table_based && num_strings == 0 {
        let mut max_idx = 0;
        for key in texts.keys() {
            let k_parts: Vec<&str> = key.splitn(4, '_').collect();
            if k_parts.len() == 4
                && k_parts[3].starts_with("str")
                && let Ok(str_idx) = k_parts[3][3..].parse::<usize>()
            {
                max_idx = max_idx.max(str_idx + 1);
            }
        }
        num_strings = max_idx;
    }

    if chunk_path.exists() {
        let mut bak_path = chunk_path.to_path_buf();
        bak_path.set_extension("dat.bak");
        if !bak_path.exists() {
            let _ = fs::copy(chunk_path, bak_path);
        }
    }

    if is_fixed_566 {
        let mut orig_data = fs::read(chunk_path)?;
        for (key, val) in &texts {
            if !key.starts_with("f566_") {
                continue;
            }
            let parts: Vec<&str> = key.split('_').collect();
            let offset = parts
                .get(1)
                .and_then(|s| s.parse::<usize>().ok())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Invalid offset in key: {}", key),
                    )
                })?;

            let str_id = parts
                .get(2)
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            let lang_id = ((str_id >> 16) & 0xFF) as u16;

            let mut text_bytes = encode_by_lang(val, lang_id);
            if text_bytes.len() > 511 {
                text_bytes.truncate(511);
            }
            let mut padded = vec![0u8; 512];
            padded[..text_bytes.len()].copy_from_slice(&text_bytes);

            if offset + 566 <= orig_data.len() {
                orig_data[offset + 54..offset + 566].copy_from_slice(&padded);
            }
        }
        File::create(chunk_path)?.write_all(&orig_data)?;
        return Ok(());
    }

    let mut out = File::create(chunk_path)?;

    if is_developer_table {
        let mut entries: BTreeMap<u32, (u32, u8, String, String)> = BTreeMap::new();
        for (key, val) in &texts {
            let parts: Vec<&str> = key.splitn(4, '_').collect();
            let idx = parts
                .first()
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Missing or invalid index field in key: {}", key),
                    )
                })?;

            let id_val = parts
                .get(1)
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Missing or invalid ID field in key: {}", key),
                    )
                })?;

            let flag = parts
                .get(2)
                .and_then(|s| s.parse::<u8>().ok())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Missing or invalid flag field in key: {}", key),
                    )
                })?;

            let dev_key = parts.get(3).unwrap_or(&"").to_string();
            entries.insert(idx, (id_val, flag, val.clone(), dev_key));
        }

        out.write_u32::<LittleEndian>(entries.len() as u32)?;
        for (_, (id_val, flag, name, dev_key)) in entries {
            out.write_u8(0x02)?;
            out.write_u32::<LittleEndian>(id_val)?;
            out.write_u8(flag)?;

            let name_enc = encode_windows(&name);
            out.write_u32::<LittleEndian>(name_enc.len() as u32)?;
            out.write_all(&name_enc)?;

            let key_enc = encode_windows(&dev_key);
            out.write_u32::<LittleEndian>(key_enc.len() as u32)?;
            out.write_all(&key_enc)?;
        }
    } else if is_table_based {
        let mut entries: BTreeMap<u32, TableBasedEntry> = BTreeMap::new();
        for (key, val) in &texts {
            let parts: Vec<&str> = key.splitn(4, '_').collect();
            let idx = parts
                .first()
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Missing or invalid index field in key: {}", key),
                    )
                })?;

            let id_val = parts
                .get(1)
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Missing or invalid ID field in key: {}", key),
                    )
                })?;

            let extra_bytes = parts
                .get(2)
                .map(|&hex_str| hex_to_bytes(hex_str))
                .unwrap_or_default();

            let str_idx_str = parts.get(3).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Missing string index target in key: {}", key),
                )
            })?;

            if str_idx_str.len() < 4 || !str_idx_str.starts_with("str") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Malformed string marker in key: {}", key),
                ));
            }

            let str_idx = str_idx_str[3..].parse::<usize>().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to parse index in table string '{}': {}", key, e),
                )
            })?;

            let entry = entries.entry(idx).or_insert_with(|| TableBasedEntry {
                id: id_val,
                extra_bytes,
                strings: BTreeMap::new(),
            });
            entry.strings.insert(str_idx, val.clone());
        }

        out.write_u32::<LittleEndian>(entries.len() as u32)?;
        for (_, entry) in entries {
            out.write_u32::<LittleEndian>(entry.id)?;
            out.write_all(&entry.extra_bytes)?;
            for s in 0..num_strings {
                let empty = String::new();
                let text_val = entry.strings.get(&s).unwrap_or(&empty);
                let utf16: Vec<u16> = text_val.encode_utf16().collect();

                out.write_u32::<LittleEndian>(utf16.len() as u32)?;
                for &u in &utf16 {
                    out.write_u16::<LittleEndian>(u)?;
                }
            }
        }
    } else {
        out.write_u32::<LittleEndian>(texts.len() as u32)?;
        for (key, val) in texts {
            out.write_u8(0x01)?;
            let key_bytes = key.as_bytes();
            out.write_u32::<LittleEndian>(key_bytes.len() as u32)?;
            out.write_all(key_bytes)?;

            let utf16: Vec<u16> = val.encode_utf16().collect();
            out.write_u32::<LittleEndian>(utf16.len() as u32)?;
            for u in utf16 {
                out.write_u16::<LittleEndian>(u)?;
            }
        }
    }

    Ok(())
}

// -----------------------------------------------------------------------------
// MAIN UNPACK / PACK LOGIC
// -----------------------------------------------------------------------------

pub fn unpack_all(input: &Path, out_dir: &Path, logger: &UiLogger) -> io::Result<()> {
    fs::create_dir_all(out_dir)?;
    let json_dir = out_dir.join("texts_json");
    fs::create_dir_all(&json_dir)?;

    let mut data = Vec::new();
    File::open(input)?.read_to_end(&mut data)?;

    if data.len() < 20 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid CFF Header: file is too small!",
        ));
    }

    let mut fmt_type = "sf2".to_string();
    let sig = &data[0..4];
    if sig == b"\x02\xc5r\xdd" {
        fmt_type = "sf1".to_string();
        logger.log("[*] Detected SpellForce 1 CFF container.");
    } else if sig == b"\x12\xdd\x72\xdd" {
        let h2 = Cursor::new(&data[4..8])
            .read_u32::<LittleEndian>()
            .unwrap_or(0);
        let h3 = Cursor::new(&data[8..12])
            .read_u32::<LittleEndian>()
            .unwrap_or(0);
        let h4 = Cursor::new(&data[12..16])
            .read_u32::<LittleEndian>()
            .unwrap_or(0);
        let h5 = Cursor::new(&data[16..20])
            .read_u32::<LittleEndian>()
            .unwrap_or(0);
        if h2 == 2 && h3 == 2 && h4 == 1 && h5 == 0 {
            fmt_type = "sf1".to_string();
            logger.log("[*] Detected SpellForce 1 CFF (Platinum Edition).");
        } else if data.len() > 36 {
            let cs_2 = Cursor::new(&data[26..28])
                .read_u16::<LittleEndian>()
                .unwrap_or(1);
            let us_2 = Cursor::new(&data[30..34])
                .read_u32::<LittleEndian>()
                .unwrap_or(0);
            if cs_2 == 0 && (us_2 as usize) > data.len() {
                fmt_type = "sf1".to_string();
                logger.log("[*] Detected SpellForce 1 CFF (Heuristics).");
            } else {
                logger.log("[*] Detected SpellForce 2 CFF container.");
            }
        } else {
            logger.log("[*] Detected SpellForce 2 CFF container.");
        }
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid CFF signature!",
        ));
    }

    File::create(out_dir.join("header.bin"))?.write_all(&data[0..20])?;

    let mut manifest = Manifest {
        format: fmt_type.clone(),
        chunks: Vec::new(),
    };
    let mut cursor = Cursor::new(&data);
    cursor.set_position(20);

    let mut chunk_idx = 0;
    let mut text_extracted = 0;

    while (cursor.position() as usize) < data.len() {
        if chunk_idx > 0 && chunk_idx % 500 == 0 {
            logger.log(&format!("Unpacked {} chunks...", chunk_idx));
        }

        let (chunk_manifest, uncomp_data) = if fmt_type == "sf1" {
            if cursor.position() as usize + 12 > data.len() {
                break;
            }
            let id = cursor.read_u16::<LittleEndian>()? as u32;
            let occurrence = cursor.read_i16::<LittleEndian>()?;
            let comp_flag = cursor.read_i16::<LittleEndian>()?;
            let comp_size = cursor.read_i32::<LittleEndian>()?;
            let c_type = cursor.read_i16::<LittleEndian>()?;

            if comp_size <= 0 || cursor.position() as usize + (comp_size as usize) > data.len() {
                logger.log(&format!(
                    "[!] Warning: Corrupted SF1 chunk {} with invalid size {}",
                    chunk_idx, comp_size
                ));
                break;
            }

            let uncomp_bytes = if comp_flag == 0 {
                let mut buf = vec![0u8; comp_size as usize];
                cursor.read_exact(&mut buf)?;
                buf
            } else {
                if cursor.position() as usize + 4 > data.len() {
                    break;
                }
                let _uncomp_size = cursor.read_i32::<LittleEndian>()?;
                let mut comp_data = vec![0u8; comp_size as usize];
                cursor.read_exact(&mut comp_data)?;

                let mut uncomp_data = Vec::new();

                let mut decoder = ZlibDecoder::new(comp_data.as_slice());
                if decoder.read_to_end(&mut uncomp_data).is_err() {
                    uncomp_data = comp_data;
                }
                uncomp_data
            };

            let name = get_sf1_chunk_info(id).map(|info| info.name.to_string());

            (
                ChunkManifest {
                    file: format!("chunk_{}.dat", chunk_idx),
                    id,
                    name,
                    flag1: None,
                    flag2: None,
                    occurrence: Some(occurrence),
                    comp_flag: Some(comp_flag),
                    c_type: Some(c_type),
                },
                uncomp_bytes,
            )
        } else {
            if cursor.position() as usize + 16 > data.len() {
                break;
            }
            let id = cursor.read_u32::<LittleEndian>()?;
            let flag1 = cursor.read_u16::<LittleEndian>()?;
            let comp_size = cursor.read_u32::<LittleEndian>()? as usize;
            let flag2 = cursor.read_u16::<LittleEndian>()?;
            let _uncomp_size = cursor.read_u32::<LittleEndian>()?;

            if cursor.position() as usize + comp_size > data.len() {
                logger.log(&format!(
                    "[!] Warning: Corrupted SF2 chunk {} with overflow size {}",
                    chunk_idx, comp_size
                ));
                break;
            }

            let mut comp_data = vec![0u8; comp_size];
            cursor.read_exact(&mut comp_data)?;

            let mut uncomp_data = Vec::new();
            let mut decoder = ZlibDecoder::new(comp_data.as_slice());
            if decoder.read_to_end(&mut uncomp_data).is_err() {
                uncomp_data = comp_data;
            }

            let name = get_sf2_chunk_name(id).map(|s| s.to_string());

            (
                ChunkManifest {
                    file: format!("chunk_{}.dat", chunk_idx),
                    id,
                    name,
                    flag1: Some(flag1),
                    flag2: Some(flag2),
                    occurrence: None,
                    comp_flag: None,
                    c_type: None,
                },
                uncomp_data,
            )
        };

        let chunk_name = chunk_manifest.file.clone();
        let chunk_path = out_dir.join(&chunk_name);
        File::create(&chunk_path)?.write_all(&uncomp_data)?;
        manifest.chunks.push(chunk_manifest);

        let fmt = detect_format(&uncomp_data);
        if fmt != ChunkFormat::Binary {
            let json_path = json_dir.join(format!("chunk_{}_strings.json", chunk_idx));
            if export_text(&uncomp_data, &json_path, fmt).is_ok() {
                text_extracted += 1;
            }
        }
        chunk_idx += 1;
    }

    let manifest_file = File::create(out_dir.join("manifest.json"))?;
    serde_json::to_writer_pretty(manifest_file, &manifest)?;

    logger.log(&format!("Unpacked {} chunks successfully.", chunk_idx));
    logger.log(&format!(
        "Exported {} text datasets to JSON.",
        text_extracted
    ));

    Ok(())
}

pub fn pack_all(
    in_dir: &Path,
    out_file: &Path,
    comp_level: u32,
    logger: &UiLogger,
) -> io::Result<()> {
    let manifest_path = in_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "manifest.json not found in the source directory!",
        ));
    }

    let manifest_data = fs::read_to_string(manifest_path)?;
    let manifest: Manifest = serde_json::from_str(&manifest_data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let fmt_type = manifest.format.clone();
    let json_dir = in_dir.join("texts_json");

    if let Ok(entries) = fs::read_dir(&json_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str())
                && file_name.starts_with("chunk_")
                && file_name.ends_with("_strings.json")
            {
                let parts: Vec<&str> = file_name.split('_').collect();
                if let Some(&chunk_idx) = parts.get(1) {
                    let target_dat = in_dir.join(format!("chunk_{}.dat", chunk_idx));
                    if target_dat.exists()
                        && let Err(e) = import_text(&path, &target_dat)
                    {
                        logger.log(&format!("[!] Error compiling {}: {}", file_name, e));
                    }
                }
            }
        }
    }

    let mut out = File::create(out_file)?;

    let header_path = in_dir.join("header.bin");
    if header_path.exists() {
        let mut header = Vec::new();
        File::open(header_path)?.read_to_end(&mut header)?;
        out.write_all(&header)?;
    } else if fmt_type == "sf1" {
        out.write_all(b"\x02\xc5\x72\xdd")?;
        out.write_all(&[0u8; 16])?;
    } else {
        out.write_all(b"\x12\xdd\x72\xdd\x03\x00\x00\x00")?;
        out.write_all(&[0u8; 12])?;
    }

    for (idx, chunk) in manifest.chunks.into_iter().enumerate() {
        if idx > 0 && idx % 500 == 0 {
            logger.log(&format!("Packed {} chunks...", idx));
        }

        let mut uncomp_data = Vec::new();
        let chunk_path = in_dir.join(&chunk.file);
        if !chunk_path.exists() {
            continue;
        }
        File::open(chunk_path)?.read_to_end(&mut uncomp_data)?;

        if fmt_type == "sf1" {
            if let Some(info) = get_sf1_chunk_info(chunk.id)
                && !uncomp_data.is_empty()
                && uncomp_data.len() % info.stride != 0
            {
                logger.log(&format!(
                    "[!] Warning: Chunk 0x{:04X} ({}) length {} is not divisible by expected stride {}! May crash in engine.",
                    chunk.id, info.name, uncomp_data.len(), info.stride
                ));
            }

            let comp_flag = chunk.comp_flag.unwrap_or(1);
            let occurrence = chunk.occurrence.unwrap_or(0);

            let c_type = chunk.c_type.unwrap_or_else(|| {
                get_sf1_chunk_info(chunk.id)
                    .map(|i| i.default_c_type)
                    .unwrap_or(1)
            });

            if comp_flag == 0 {
                out.write_i16::<LittleEndian>(chunk.id as i16)?;
                out.write_i16::<LittleEndian>(occurrence)?;
                out.write_i16::<LittleEndian>(0)?;
                out.write_i32::<LittleEndian>(uncomp_data.len() as i32)?;
                out.write_i16::<LittleEndian>(c_type)?;
                out.write_all(&uncomp_data)?;
            } else {
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(comp_level));
                encoder.write_all(&uncomp_data)?;
                let comp_data = encoder.finish()?;

                out.write_i16::<LittleEndian>(chunk.id as i16)?;
                out.write_i16::<LittleEndian>(occurrence)?;
                out.write_i16::<LittleEndian>(comp_flag)?;
                out.write_i32::<LittleEndian>(comp_data.len() as i32)?;
                out.write_i16::<LittleEndian>(c_type)?;
                out.write_i32::<LittleEndian>(uncomp_data.len() as i32)?;
                out.write_all(&comp_data)?;
            }
        } else {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(comp_level));
            encoder.write_all(&uncomp_data)?;
            let comp_data = encoder.finish()?;

            out.write_u32::<LittleEndian>(chunk.id)?;
            out.write_u16::<LittleEndian>(chunk.flag1.unwrap_or(0))?;
            out.write_u32::<LittleEndian>(comp_data.len() as u32)?;
            out.write_u16::<LittleEndian>(chunk.flag2.unwrap_or(0))?;
            out.write_u32::<LittleEndian>(uncomp_data.len() as u32)?;
            out.write_all(&comp_data)?;
        }
    }

    logger.log("CFF container successfully packed and finalized!");
    Ok(())
}

// -----------------------------------------------------------------------------
// CFF MOD CONFLICT RESOLVER: DIFF & PATCH ENGINE
// -----------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct CffModPatch {
    pub patch_name: String,
    pub target_format: String,
    pub text_diffs: BTreeMap<String, BTreeMap<String, String>>,
    pub chunk_diffs: BTreeMap<String, Vec<u8>>,
}

pub fn create_diff(
    base_dir: &Path,
    mod_dir: &Path,
    patch_out: &Path,
    logger: &UiLogger,
) -> io::Result<()> {
    logger.log(&format!(
        "[*] Generating CFF Diff Patch: {:?} vs {:?}",
        base_dir, mod_dir
    ));

    let base_manifest_path = base_dir.join("manifest.json");
    let mod_manifest_path = mod_dir.join("manifest.json");

    if !base_manifest_path.exists() || !mod_manifest_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "manifest.json must exist in both base and mod directories!",
        ));
    }

    let base_m: Manifest = serde_json::from_str(&fs::read_to_string(base_manifest_path)?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let mod_m: Manifest = serde_json::from_str(&fs::read_to_string(mod_manifest_path)?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let mut text_diffs = BTreeMap::new();
    let base_json_dir = base_dir.join("texts_json");
    let mod_json_dir = mod_dir.join("texts_json");

    if mod_json_dir.exists() {
        for entry in fs::read_dir(&mod_json_dir)?.filter_map(|e| e.ok()) {
            let mod_file = entry.path();
            if mod_file.extension().and_then(|s| s.to_str()) == Some("json")
                && !mod_file.to_string_lossy().ends_with(".meta.json")
            {
                let fname = mod_file.file_name().unwrap().to_string_lossy().to_string();
                let base_file = base_json_dir.join(&fname);

                let mod_map: BTreeMap<String, String> =
                    serde_json::from_str(&fs::read_to_string(&mod_file)?).unwrap_or_default();

                let base_map: BTreeMap<String, String> = if base_file.exists() {
                    serde_json::from_str(&fs::read_to_string(&base_file)?).unwrap_or_default()
                } else {
                    BTreeMap::new()
                };

                let mut diff_for_file = BTreeMap::new();
                for (k, v) in mod_map {
                    if base_map.get(&k) != Some(&v) {
                        diff_for_file.insert(k, v);
                    }
                }

                if !diff_for_file.is_empty() {
                    logger.log(&format!(
                        "[+] Found {} changed strings in {}",
                        diff_for_file.len(),
                        fname
                    ));
                    text_diffs.insert(fname, diff_for_file);
                }
            }
        }
    }

    let mut chunk_diffs = BTreeMap::new();
    for chunk in mod_m.chunks {
        let mod_chunk_path = mod_dir.join(&chunk.file);
        let base_chunk_path = base_dir.join(&chunk.file);

        if mod_chunk_path.exists() {
            let mod_bytes = fs::read(&mod_chunk_path)?;
            let is_different = if base_chunk_path.exists() {
                let base_bytes = fs::read(&base_chunk_path)?;
                base_bytes != mod_bytes
            } else {
                true
            };

            if is_different {
                logger.log(&format!("[+] Found modified chunk data: {}", chunk.file));
                chunk_diffs.insert(chunk.file, mod_bytes);
            }
        }
    }

    let patch = CffModPatch {
        patch_name: patch_out
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        target_format: base_m.format,
        text_diffs,
        chunk_diffs,
    };

    let f = File::create(patch_out)?;
    serde_json::to_writer_pretty(f, &patch)?;

    logger.log(&format!(
        "[+] Diff patch successfully generated! Text files changed: {}, Chunks changed: {}",
        patch.text_diffs.len(),
        patch.chunk_diffs.len()
    ));
    Ok(())
}

pub fn apply_patch(target_dir: &Path, patch_path: &Path, logger: &UiLogger) -> io::Result<()> {
    logger.log(&format!(
        "[*] Applying CFF Patch: {:?} -> {:?}",
        patch_path, target_dir
    ));

    let patch_str = fs::read_to_string(patch_path)?;
    let patch: CffModPatch = serde_json::from_str(&patch_str)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let json_dir = target_dir.join("texts_json");
    fs::create_dir_all(&json_dir)?;

    let mut merged_strings = 0;
    for (fname, diffs) in patch.text_diffs {
        let target_file = json_dir.join(&fname);
        let mut target_map: BTreeMap<String, String> = if target_file.exists() {
            serde_json::from_str(&fs::read_to_string(&target_file)?).unwrap_or_default()
        } else {
            BTreeMap::new()
        };

        for (k, v) in diffs {
            target_map.insert(k, v);
            merged_strings += 1;
        }

        let f = File::create(&target_file)?;
        serde_json::to_writer_pretty(f, &target_map)?;
    }

    for (fname, data) in patch.chunk_diffs {
        let chunk_target = target_dir.join(&fname);
        File::create(&chunk_target)?.write_all(&data)?;
    }

    logger.log(&format!(
        "[+] Patch successfully applied! Merged {} localized strings without conflicts.",
        merged_strings
    ));
    Ok(())
}

// -----------------------------------------------------------------------------
// INTERACTIVE ENTITY / BALANCE EDITOR ENGINE
// -----------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct EditorItem {
    pub id_str: String,
    pub val1: String,
    pub val2: String,
    pub display: String,
}

pub fn load_editor_items(
    cff_dir: &Path,
    category: &str,
    filter: &str,
    lang_filter: &str,
) -> Vec<EditorItem> {
    let mut items = Vec::new();
    let filter_lower = filter.to_lowercase();

    if category.contains("0x07DC") {
        let manifest_path = cff_dir.join("manifest.json");
        if let Ok(m_str) = fs::read_to_string(manifest_path)
            && let Ok(manifest) = serde_json::from_str::<Manifest>(&m_str)
            && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DC)
        {
            let chunk_path = cff_dir.join(&chunk.file);
            if let Ok(bytes) = fs::read(chunk_path) {
                let num_records = bytes.len() / 69;
                for i in 0..num_records {
                    let offset = i * 69;
                    let item_id = Cursor::new(&bytes[offset..offset + 2])
                        .read_u16::<LittleEndian>()
                        .unwrap_or(0);
                    let flag = bytes[offset + 2];
                    let mesh_bytes = &bytes[offset + 3..offset + 67];
                    let mesh_end = mesh_bytes
                        .iter()
                        .position(|&b| b == 0)
                        .unwrap_or(mesh_bytes.len());
                    let mesh_name = decode_windows(&mesh_bytes[..mesh_end]);
                    let extra = Cursor::new(&bytes[offset + 67..offset + 69])
                        .read_u16::<LittleEndian>()
                        .unwrap_or(0);

                    let display =
                        format!("ID: {:<5} [Flag: {}] | Mesh: {}", item_id, flag, mesh_name);
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
        }
    } else if category.contains("0x07E2") {
        let manifest_path = cff_dir.join("manifest.json");
        if let Ok(m_str) = fs::read_to_string(manifest_path)
            && let Ok(manifest) = serde_json::from_str::<Manifest>(&m_str)
            && let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E2)
        {
            let chunk_path = cff_dir.join(&chunk.file);
            if let Ok(bytes) = fs::read(chunk_path) {
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
                        "Spell ID: {:<5} -> Target/Scroll ID: {}",
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
        }
    } else {
        // Localized Strings with Bitmask Language Math & Campaign Detection
        let json_dir = cff_dir.join("texts_json");
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

                            // BITMASK EXTRACTION: Prevents [EXT] fallback bug
                            let lang = ((str_id >> 16) & 0xFF) as u8;
                            let camp = (str_id >> 24) as u8;
                            let base = (str_id & 0xFFFF) as u16;

                            let tag = match lang {
                                0 => "DE",
                                1 => "EN",
                                2 => "FR",
                                3 => "ES",
                                4 => "IT",
                                5 => "RU",
                                _ => "CUSTOM",
                            };
                            (lang, base, camp, tag)
                        } else {
                            (1, 0, 0, "TXT")
                        };

                        let lang_match = match lang_filter {
                            "Slot 0: German (DE)" => l_id == 0,
                            "Slot 1: English (EN)" => l_id == 1,
                            "Slot 2: French (FR)" => l_id == 2,
                            "Slot 3: Spanish (ES)" => l_id == 3,
                            "Slot 4: Italian (IT)" => l_id == 4,
                            "Slot 5: Custom (RU/PL/etc.)" => l_id >= 5,
                            _ => true,
                        };

                        if !lang_match {
                            continue;
                        }

                        let camp_str = match camp_id {
                            1 => " [BoW]",
                            2 => " [SotP]",
                            _ => "",
                        };

                        let display = format!("[{}]{} #{:<5} | {}", lang_tag, camp_str, b_id, v);
                        if filter.is_empty() || display.to_lowercase().contains(&filter_lower) {
                            items.push(EditorItem {
                                id_str: format!("{}:{}", fname, k),
                                val1: v,
                                val2: format!(
                                    "Slot: {} ({}){} | Base ID: {}",
                                    l_id, lang_tag, camp_str, b_id
                                ),
                                display,
                            });
                        }
                    }
                }
            }
        }
    }
    items
}

pub fn save_editor_item(
    cff_dir: &Path,
    category: &str,
    index: usize,
    id_str: &str,
    val1: &str,
    _val2: &str,
) -> io::Result<()> {
    if category.contains("0x07DC") {
        let manifest_path = cff_dir.join("manifest.json");
        let m_str = fs::read_to_string(manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&m_str)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DC) {
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
    } else if category.contains("0x07E2") {
        let manifest_path = cff_dir.join("manifest.json");
        let m_str = fs::read_to_string(manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&m_str)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E2) {
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
    } else {
        let parts: Vec<&str> = id_str.splitn(2, ':').collect();
        if parts.len() == 2 {
            let fname = format!("{}.json", parts[0]);
            let key = parts[1];
            let json_path = cff_dir.join("texts_json").join(fname);
            if json_path.exists() {
                let mut map: BTreeMap<String, String> =
                    serde_json::from_str(&fs::read_to_string(&json_path)?)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
                map.insert(key.to_string(), val1.to_string());
                let f = File::create(json_path)?;
                serde_json::to_writer_pretty(f, &map)?;
            }
        }
    }
    Ok(())
}

pub fn add_editor_item(cff_dir: &Path, category: &str) -> io::Result<String> {
    if category.contains("0x07DC") {
        let manifest_path = cff_dir.join("manifest.json");
        let m_str = fs::read_to_string(manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&m_str)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

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
            let default_mesh = encode_windows("ui_item_new_asset");
            let len = default_mesh.len().min(63);
            record[3..3 + len].copy_from_slice(&default_mesh[..len]);

            bytes.extend_from_slice(&record);
            File::create(chunk_path)?.write_all(&bytes)?;
            return Ok(new_id.to_string());
        }
    } else if category.contains("0x07E2") {
        let manifest_path = cff_dir.join("manifest.json");
        let m_str = fs::read_to_string(manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&m_str)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

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
            record[2..4].copy_from_slice(&0u16.to_le_bytes());

            bytes.extend_from_slice(&record);
            File::create(chunk_path)?.write_all(&bytes)?;
            return Ok(new_id.to_string());
        }
    } else {
        let json_dir = cff_dir.join("texts_json");
        if let Ok(entries) = fs::read_dir(json_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json")
                    && !path.to_string_lossy().ends_with(".meta.json")
                {
                    let fname = path.file_stem().unwrap().to_string_lossy().to_string();
                    let mut map: BTreeMap<String, String> =
                        serde_json::from_str(&fs::read_to_string(&path)?).map_err(|e| {
                            io::Error::new(io::ErrorKind::InvalidData, e.to_string())
                        })?;

                    let new_key = format!("custom_str_{}", map.len() + 1);
                    map.insert(new_key.clone(), "New Localized String".into());
                    let f = File::create(&path)?;
                    serde_json::to_writer_pretty(f, &map)?;

                    return Ok(format!("{}:{}", fname, new_key));
                }
            }
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
    if category.contains("0x07DC") {
        let manifest_path = cff_dir.join("manifest.json");
        let m_str = fs::read_to_string(manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&m_str)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

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
                File::create(chunk_path)?.write_all(&bytes)?;
                return Ok(new_id.to_string());
            }
        }
    } else if category.contains("0x07E2") {
        let manifest_path = cff_dir.join("manifest.json");
        let m_str = fs::read_to_string(manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&m_str)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

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
                File::create(chunk_path)?.write_all(&bytes)?;
                return Ok(new_id.to_string());
            }
        }
    } else {
        let parts: Vec<&str> = id_str.splitn(2, ':').collect();
        if parts.len() == 2 {
            let fname = format!("{}.json", parts[0]);
            let key = parts[1];
            let json_path = cff_dir.join("texts_json").join(fname);
            if json_path.exists() {
                let mut map: BTreeMap<String, String> =
                    serde_json::from_str(&fs::read_to_string(&json_path)?)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

                let original_val = map.get(key).cloned().unwrap_or_default();
                let new_key = format!("{}_copy", key);
                map.insert(new_key.clone(), original_val);
                let f = File::create(json_path)?;
                serde_json::to_writer_pretty(f, &map)?;

                return Ok(format!("{}:{}", parts[0], new_key));
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
    if category.contains("0x07DC") {
        let manifest_path = cff_dir.join("manifest.json");
        let m_str = fs::read_to_string(manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&m_str)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07DC) {
            let chunk_path = cff_dir.join(&chunk.file);
            let mut bytes = fs::read(&chunk_path)?;
            let offset = index * 69;
            if offset + 69 <= bytes.len() {
                bytes.drain(offset..offset + 69);
                File::create(chunk_path)?.write_all(&bytes)?;
                return Ok(());
            }
        }
    } else if category.contains("0x07E2") {
        let manifest_path = cff_dir.join("manifest.json");
        let m_str = fs::read_to_string(manifest_path)?;
        let manifest: Manifest = serde_json::from_str(&m_str)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

        if let Some(chunk) = manifest.chunks.iter().find(|c| c.id == 0x07E2) {
            let chunk_path = cff_dir.join(&chunk.file);
            let mut bytes = fs::read(&chunk_path)?;
            let offset = index * 4;
            if offset + 4 <= bytes.len() {
                bytes.drain(offset..offset + 4);
                File::create(chunk_path)?.write_all(&bytes)?;
                return Ok(());
            }
        }
    } else {
        let parts: Vec<&str> = id_str.splitn(2, ':').collect();
        if parts.len() == 2 {
            let fname = format!("{}.json", parts[0]);
            let key = parts[1];
            let json_path = cff_dir.join("texts_json").join(&fname);
            if json_path.exists() {
                let mut map: BTreeMap<String, String> =
                    serde_json::from_str(&fs::read_to_string(&json_path)?)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
                map.remove(key);
                let f = File::create(&json_path)?;
                serde_json::to_writer_pretty(f, &map)?;
            }

            if key.starts_with("f566_") {
                let k_parts: Vec<&str> = key.split('_').collect();
                if let Some(offset) = k_parts.get(1).and_then(|s| s.parse::<usize>().ok()) {
                    let chunk_dat_name = parts[0].replace("_strings", ".dat");
                    let chunk_dat_path = cff_dir.join(&chunk_dat_name);
                    if chunk_dat_path.exists() {
                        let mut b = fs::read(&chunk_dat_path)?;
                        if offset + 566 <= b.len() {
                            b.drain(offset..offset + 566);
                            File::create(&chunk_dat_path)?.write_all(&b)?;
                        }
                    }
                }
            }
            return Ok(());
        }
    }
    Err(io::Error::other("Failed to delete record"))
}

// -----------------------------------------------------------------------------
// UNIFIED MULTI-LANGUAGE CLONING & SLOT IMPORT / EXPORT TOOLS
// -----------------------------------------------------------------------------

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
    } else if lang_str.contains("Slot 5") {
        5
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
        if let Some(pos) = it.val2.find("Base ID: ") {
            let id_part = &it.val2[pos + 9..];
            clean_map.insert(id_part.to_string(), it.val1.clone());
        } else {
            clean_map.insert(it.id_str.clone(), it.val1.clone());
        }
    }

    let count = clean_map.len();
    let f = File::create(out_json)?;
    serde_json::to_writer_pretty(f, &clean_map)?;

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

    // 1. Clone inside binary chunk_*.dat files
    for chunk in &manifest.chunks {
        let chunk_path = cff_dir.join(&chunk.file);
        if !chunk_path.exists() {
            continue;
        }
        let data = fs::read(&chunk_path)?;
        if data.len() < 566 || !data.len().is_multiple_of(566) {
            continue;
        }

        let num_blocks = data.len() / 566;
        let mut new_blocks = Vec::new();

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
                new_blocks.extend_from_slice(&new_block);
                cloned_count += 1;
            }
        }

        if !new_blocks.is_empty() {
            let mut file = fs::OpenOptions::new().append(true).open(&chunk_path)?;
            file.write_all(&new_blocks)?;
            logger.log(&format!(
                "[+] Appended {} blocks to {}",
                new_blocks.len() / 566,
                chunk.file
            ));
        }
    }

    // 2. Synchronize texts_json files so GUI refreshes immediately
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
                let mut to_add = Vec::new();
                for (k, v) in &map {
                    if k.starts_with("f566_") {
                        let parts: Vec<&str> = k.split('_').collect();
                        if let Some(str_id) = parts.get(2).and_then(|s| s.parse::<u32>().ok()) {
                            let l_id = ((str_id >> 16) & 0xFF) as u16;
                            let b_id = (str_id & 0xFFFF) as u16;
                            let camp_id = str_id >> 24;
                            if l_id == src_slot {
                                let new_str_id =
                                    (camp_id << 24) | ((dst_slot as u32) << 16) | (b_id as u32);
                                let new_k = format!("f566_{}_{}", parts[1], new_str_id);
                                to_add.push((new_k, v.clone()));
                            }
                        }
                    }
                }
                for (k, v) in to_add {
                    map.insert(k, v);
                }
                let _ = serde_json::to_writer_pretty(File::create(&p)?, &map);
            }
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

    // 1. Overwrite in binary chunk_*.dat files
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

        File::create(&chunk_path)?.write_all(&data)?;
    }

    // 2. Overwrite in texts_json so GUI immediately refreshes
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
                let _ = serde_json::to_writer_pretty(File::create(&p)?, &map);
            }
        }
    }

    logger.log(&format!(
        "[+] Successfully replaced {} phrases in Slot {}!",
        updated_count, target_slot
    ));
    Ok(updated_count)
}
