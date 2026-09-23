use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use encoding_rs::{WINDOWS_1251, WINDOWS_1252};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Cursor, Write};
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub struct ChunkMeta {
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_strings: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_bytes: Option<usize>,
}

#[derive(PartialEq, Debug)]
pub enum ChunkFormat {
    Binary,
    Fixed566,
    StringTable,
    DeveloperTable,
    TableBased(usize, usize),
}

pub struct TableBasedEntry {
    pub id: u32,
    pub extra_bytes: Vec<u8>,
    pub strings: BTreeMap<usize, String>,
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

pub fn hex_to_bytes(hex: &str) -> Vec<u8> {
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

// -----------------------------------------------------------------------------
// CHUNK FORMAT DETECTION
// -----------------------------------------------------------------------------

pub fn detect_format(data: &[u8]) -> ChunkFormat {
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

pub fn export_text(data: &[u8], json_path: &Path, format: ChunkFormat) -> io::Result<()> {
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

pub fn import_text(json_path: &Path, chunk_path: &Path) -> io::Result<()> {
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
