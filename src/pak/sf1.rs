use crate::UiLogger;
use crate::cff::{decode_windows, encode_windows};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;
use walkdir::WalkDir;

const fn generate_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

const CRC32_TABLE: [u32; 256] = generate_crc32_table();

pub fn calculate_sf1_crc(data: &[u8], prev_crc: u32) -> u32 {
    let mut crc = prev_crc;
    for &b in data {
        crc = (crc >> 8) ^ CRC32_TABLE[((crc ^ (b as u32)) & 0xFF) as usize];
    }
    crc
}

pub fn calc_sf1_path_hash(path: &str) -> u16 {
    let mut h: u32 = 0;
    for &b in path.as_bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u32);
    }
    (h & 0xFFFF) as u16
}

const PHENOMIC_HEADER_TEMPLATE: [u8; 44] = [
    0x00, 0x00, 0x00, 0x00, 0xb0, 0xff, 0x12, 0x00, 0x08, 0x6f, 0x40, 0x00, 0x38, 0xc1, 0x40, 0x00,
    0xff, 0xff, 0xff, 0xff, 0x40, 0x28, 0x32, 0x00, 0x52, 0x48, 0x40, 0x00, 0x1f, 0x00, 0x00, 0x00,
    0xda, 0x31, 0x40, 0x00, 0x1f, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff,
];

pub fn read_reversed_string_from_bytes(data: &[u8], offset: usize) -> String {
    let mut pos = offset;
    let mut chars = Vec::new();
    while pos < data.len() && data[pos] != 0 {
        chars.push(data[pos]);
        pos += 1;
    }
    chars.reverse();
    decode_windows(&chars)
}

struct SF1Entry {
    rel_path: String,
    full_path: std::path::PathBuf,
    filename: String,
    dirname: String,
    h_hi: u8,
    h_lo: u8,
    comp_str: String,
    name_off: u32,
    dir_off: u32,
}

pub fn unpack_sf1(f: &mut File, out_dir: &Path, logger: &UiLogger) -> io::Result<()> {
    f.seek(SeekFrom::Start(84))?;
    let data_start = f.read_u32::<LittleEndian>()?;

    f.seek(SeekFrom::Start(0))?;
    let mut meta_bytes = vec![0u8; data_start as usize];
    f.read_exact(&mut meta_bytes)?;

    fs::create_dir_all(out_dir)?;

    let num_files = Cursor::new(&meta_bytes[76..80]).read_u32::<LittleEndian>()?;
    let name_list_start = 92 + (num_files as usize) * 16;

    for i in 0..num_files as usize {
        let offset_meta = 92 + i * 16;
        let mut cur = Cursor::new(&meta_bytes[offset_meta..offset_meta + 16]);
        let size = cur.read_u32::<LittleEndian>()?;
        let offset = cur.read_u32::<LittleEndian>()?;
        let name_off = cur.read_u32::<LittleEndian>()? & 0x00FFFFFF;
        let dir_off = cur.read_u32::<LittleEndian>()? & 0x00FFFFFF;

        let file_name =
            read_reversed_string_from_bytes(&meta_bytes, name_list_start + name_off as usize + 2);
        let dir_name = if dir_off != 0x00FFFFFF && dir_off != 0 {
            read_reversed_string_from_bytes(&meta_bytes, name_list_start + dir_off as usize)
        } else {
            String::new()
        };

        let full_path = if dir_name.is_empty() {
            file_name
        } else {
            format!("{}\\{}", dir_name, file_name)
        };
        let target = out_dir.join(full_path.replace('\\', "/"));

        if i % 100 == 0 || i == num_files as usize - 1 {
            logger.log(&format!(
                "Extracting SF1 ({}/{}): {}",
                i + 1,
                num_files,
                full_path
            ));
        }

        if let Some(p) = target.parent() {
            fs::create_dir_all(p)?;
        }

        f.seek(SeekFrom::Start((data_start + offset) as u64))?;
        let mut target_file = File::create(target)?;
        let mut chunk = std::io::Read::by_ref(f).take(size as u64);
        io::copy(&mut chunk, &mut target_file)?;
    }
    Ok(())
}

pub fn pack_sf1(src_dir: &Path, out_file: &Path, logger: &UiLogger) -> io::Result<()> {
    logger.log("[*] Compiling SF1 archive with in-engine VFS ordering...");

    let mut all_items = Vec::new();
    for entry in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            let name = entry.file_name().to_string_lossy();
            if !name.starts_with('.') {
                let rel = entry
                    .path()
                    .strip_prefix(src_dir)
                    .unwrap_or(entry.path())
                    .to_string_lossy()
                    .replace('/', "\\")
                    .to_lowercase();
                all_items.push((rel, entry.path().to_path_buf()));
            }
        }
    }

    let num_files = all_items.len();
    if num_files == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No files found in source directory to pack!",
        ));
    }

    let mut file_entries: Vec<SF1Entry> = all_items
        .into_iter()
        .map(|(rel_path, full_path)| {
            let h = calc_sf1_path_hash(&rel_path);
            let p = Path::new(&rel_path);
            let filename = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let dirname = p
                .parent()
                .unwrap_or(Path::new(""))
                .to_string_lossy()
                .to_string();

            let rev_fname: String = filename.chars().rev().collect();
            let rev_dname: String = dirname.chars().rev().collect();
            let comp_str = if dirname.is_empty() {
                rev_fname
            } else {
                format!("{}\\{}", rev_fname, rev_dname)
            };

            SF1Entry {
                rel_path,
                full_path,
                filename,
                dirname,
                h_hi: ((h >> 8) & 0xFF) as u8,
                h_lo: (h & 0xFF) as u8,
                comp_str,
                name_off: 0,
                dir_off: 0,
            }
        })
        .collect();

    file_entries.sort_by(|a, b| {
        a.h_hi
            .cmp(&b.h_hi)
            .then_with(|| a.h_lo.cmp(&b.h_lo))
            .then_with(|| a.comp_str.cmp(&b.comp_str))
    });

    let mut root_idx = (num_files.saturating_sub(1) / 2) as u32;
    for (idx, entry) in file_entries.iter().enumerate() {
        if entry.rel_path == "mesh\\meshes.txt" {
            root_idx = idx as u32;
            break;
        }
    }

    let mut string_table = Vec::new();
    let mut dir_offsets: HashMap<String, u32> = HashMap::new();

    for entry in &mut file_entries {
        entry.name_off = string_table.len() as u32;
        string_table.push(entry.h_hi);
        string_table.push(entry.h_lo);

        let mut rev_name_bytes = encode_windows(&entry.filename);
        rev_name_bytes.reverse();
        string_table.extend_from_slice(&rev_name_bytes);
        string_table.push(0);

        if !entry.dirname.is_empty() {
            if let Some(&off) = dir_offsets.get(&entry.dirname) {
                entry.dir_off = off;
            } else {
                let off = string_table.len() as u32;
                dir_offsets.insert(entry.dirname.clone(), off);
                let mut rev_dir_bytes = encode_windows(&entry.dirname);
                rev_dir_bytes.reverse();
                string_table.extend_from_slice(&rev_dir_bytes);
                string_table.push(0);
                entry.dir_off = off;
            }
        } else {
            entry.dir_off = 0;
        }
    }

    let pad_str = (4 - (string_table.len() % 4)) % 4;
    if pad_str > 0 {
        string_table.resize(string_table.len() + pad_str, 0);
    }

    let header_size = 92u32;
    let file_table_size = (file_entries.len() * 16) as u32;
    let data_start_offset = header_size + file_table_size + string_table.len() as u32;

    let mut out = File::create(out_file)?;
    out.seek(SeekFrom::Start(data_start_offset as u64))?;

    let mut file_table = Vec::with_capacity(file_entries.len() * 16);
    let mut current_offset = 0u32;

    for (i, entry) in file_entries.iter().enumerate() {
        if i % 100 == 0 || i == num_files - 1 {
            logger.log(&format!(
                "Packing SF1 ({}/{}): {}",
                i + 1,
                num_files,
                entry.rel_path
            ));
        }

        let (file_size, padding) = if entry.full_path.exists() {
            let mut f = File::open(&entry.full_path)?;
            let size = io::copy(&mut f, &mut out)?;
            let pad = (4 - (size % 4)) % 4;
            if pad > 0 {
                out.write_all(&vec![0; pad as usize])?;
            }
            (size as u32, pad as u32)
        } else {
            (0, 0)
        };

        let mut ft_buf = Cursor::new(vec![0u8; 16]);
        ft_buf.write_u32::<LittleEndian>(file_size)?;
        ft_buf.write_u32::<LittleEndian>(current_offset)?;
        ft_buf.write_u32::<LittleEndian>(entry.name_off & 0x00FFFFFF)?;
        ft_buf.write_u32::<LittleEndian>(entry.dir_off & 0x00FFFFFF)?;
        file_table.extend(ft_buf.into_inner());

        current_offset += file_size + padding;
    }

    let mut total_archive_size = data_start_offset + current_offset;
    let padding_total = (4096 - (total_archive_size % 4096)) % 4096;
    if padding_total > 0 {
        out.write_all(&vec![0; padding_total as usize])?;
        total_archive_size += padding_total;
    }

    let mut header = vec![0u8; 92];
    let mut hw = Cursor::new(&mut header);
    hw.write_u32::<LittleEndian>(4)?;
    let mut magic = b"MASSIVE PAKFILE V 4.0\r\n\0".to_vec();
    magic.resize(24, 0);
    hw.write_all(&magic)?;
    hw.write_all(&PHENOMIC_HEADER_TEMPLATE)?;

    hw.seek(SeekFrom::Start(72))?;
    hw.write_u32::<LittleEndian>(0xFFFFFFFF)?;
    hw.write_u32::<LittleEndian>(num_files as u32)?;
    hw.write_u32::<LittleEndian>(root_idx)?;
    hw.write_u32::<LittleEndian>(data_start_offset)?;
    hw.write_u32::<LittleEndian>(total_archive_size)?;

    let seed = calculate_sf1_crc(&header, 0xFFFFFFFF);
    let file_table_crc = calculate_sf1_crc(&file_table, seed);
    let final_crc = calculate_sf1_crc(&string_table, file_table_crc);

    let mut hw = Cursor::new(&mut header[72..76]);
    hw.write_u32::<LittleEndian>(final_crc)?;

    out.seek(SeekFrom::Start(0))?;
    out.write_all(&header)?;
    out.write_all(&file_table)?;
    out.write_all(&string_table)?;

    logger.log(&format!(
        "[+] SF1 Archive packed successfully! File: {:?}, Checksum: 0x{:08X}",
        out_file.file_name().unwrap_or_default(),
        final_crc
    ));
    Ok(())
}
