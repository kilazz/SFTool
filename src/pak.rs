use crate::UiLogger;
use crate::cff::{decode_windows, encode_windows};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;
use walkdir::WalkDir;

// -----------------------------------------------------------------------------
// CRC32 & SF1 PATH HASH ENGINE
// -----------------------------------------------------------------------------

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

/// In-engine 16-bit K&R path hash from SpellForce 1 executable (FUN_004a4180).
/// Formula: h = h * 31 + char
pub fn calc_sf1_path_hash(path: &str) -> u16 {
    let mut h: u32 = 0;
    for &b in path.as_bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u32);
    }
    (h & 0xFFFF) as u16
}

// 44-byte Phenomic Engine header template block (offsets 28..72)
const PHENOMIC_HEADER_TEMPLATE: [u8; 44] = [
    0x00, 0x00, 0x00, 0x00, 0xb0, 0xff, 0x12, 0x00, 0x08, 0x6f, 0x40, 0x00, 0x38, 0xc1, 0x40, 0x00,
    0xff, 0xff, 0xff, 0xff, 0x40, 0x28, 0x32, 0x00, 0x52, 0x48, 0x40, 0x00, 0x1f, 0x00, 0x00, 0x00,
    0xda, 0x31, 0x40, 0x00, 0x1f, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff,
];

// -----------------------------------------------------------------------------
// DYNAMIC TREEVIEW ENGINE
// -----------------------------------------------------------------------------

#[derive(Clone)]
pub struct TreeItem {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub indent: usize,
    pub expanded: bool,
}

pub fn generate_tree_items(file_paths: &[String]) -> Vec<TreeItem> {
    let mut dirs_set = HashSet::new();

    for path in file_paths {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current = String::new();
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(part);
            dirs_set.insert(current.clone());
        }
    }

    let mut all_paths = HashSet::new();
    for dir in &dirs_set {
        all_paths.insert((dir.clone(), true));
    }
    for file in file_paths {
        all_paths.insert((file.clone(), false));
    }

    let mut sorted_paths: Vec<(String, bool)> = all_paths.into_iter().collect();
    sorted_paths.sort_by(|a, b| a.0.cmp(&b.0));

    let mut items = Vec::new();
    for (path, is_dir) in sorted_paths {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let name = parts.last().cloned().unwrap_or_default().to_string();
        let indent = parts.len().saturating_sub(1);

        items.push(TreeItem {
            path,
            name,
            is_dir,
            indent,
            expanded: true,
        });
    }
    items
}

fn is_visible(item: &TreeItem, items: &[TreeItem]) -> bool {
    let parts: Vec<&str> = item.path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 1 {
        return true;
    }

    let mut current = String::new();
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(part);

        let parent_collapsed = items
            .iter()
            .any(|it| it.is_dir && it.path == current && !it.expanded);
        if parent_collapsed {
            return false;
        }
    }
    true
}

pub fn get_visible_tree_nodes(items: &[TreeItem]) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        if is_visible(item, items) {
            let prefix = "  ".repeat(item.indent);
            let state_icon = if item.is_dir {
                if item.expanded {
                    "▼ 📁 "
                } else {
                    "▶ 📁 "
                }
            } else {
                "  📄 "
            };
            out.push(format!("{}{}{}", prefix, state_icon, item.name));
        }
    }
    out
}

pub fn toggle_tree_node(items: &mut [TreeItem], visible_index: usize) -> bool {
    let mut visible_indices = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        if is_visible(item, items) {
            visible_indices.push(idx);
        }
    }

    if let Some(&full_idx) = visible_indices
        .get(visible_index)
        .filter(|&&idx| items[idx].is_dir)
    {
        items[full_idx].expanded = !items[full_idx].expanded;
        true
    } else {
        false
    }
}

pub fn list_directory_files(dir_path: &Path) -> io::Result<Vec<String>> {
    let mut file_list = Vec::new();
    for entry in WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            let rel = entry.path().strip_prefix(dir_path).unwrap_or(entry.path());
            file_list.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(file_list)
}

fn read_reversed_string_from_bytes(data: &[u8], offset: usize) -> String {
    let mut pos = offset;
    let mut chars = Vec::new();
    while pos < data.len() && data[pos] != 0 {
        chars.push(data[pos]);
        pos += 1;
    }
    chars.reverse();
    decode_windows(&chars)
}

pub fn list_pak_files(pak_path: &Path) -> io::Result<Vec<String>> {
    let mut f = File::open(pak_path)?;
    let total_len = f.seek(SeekFrom::End(0))?;
    let mut file_list = Vec::new();

    if total_len >= 28 {
        f.seek(SeekFrom::Start(0))?;
        if f.read_u32::<LittleEndian>()? == 4 {
            let mut magic = [0u8; 24];
            f.read_exact(&mut magic)?;
            if magic.starts_with(b"MASSIVE PAKFILE") {
                f.seek(SeekFrom::Start(76))?;
                let num_files = f.read_u32::<LittleEndian>()?;
                let _root_idx = f.read_u32::<LittleEndian>()?;
                let data_start = f.read_u32::<LittleEndian>()?;

                f.seek(SeekFrom::Start(92))?;
                let mut name_offs = Vec::with_capacity(num_files as usize);
                for _ in 0..num_files {
                    let _size = f.read_u32::<LittleEndian>()?;
                    let _offset = f.read_u32::<LittleEndian>()?;
                    let name_off = f.read_u32::<LittleEndian>()? & 0x00FFFFFF;
                    let dir_off = f.read_u32::<LittleEndian>()? & 0x00FFFFFF;
                    name_offs.push((name_off, dir_off));
                }

                // Read only metadata block up to data_start
                let name_list_start = f.stream_position()?;
                let meta_len = (data_start as u64).saturating_sub(name_list_start);
                let mut meta_data = vec![0u8; meta_len as usize];
                f.read_exact(&mut meta_data)?;

                for (name_off, dir_off) in name_offs {
                    let file_name =
                        read_reversed_string_from_bytes(&meta_data, name_off as usize + 2);
                    let dir_name = if dir_off != 0x00FFFFFF && dir_off != 0 {
                        read_reversed_string_from_bytes(&meta_data, dir_off as usize)
                    } else {
                        String::new()
                    };

                    let full_path = if dir_name.is_empty() {
                        file_name
                    } else {
                        format!("{}\\{}", dir_name, file_name)
                    };
                    file_list.push(full_path.replace('\\', "/"));
                }
                return Ok(file_list);
            }
        }
    }

    // Fallback to SpellForce 2 PAK parsing
    f.seek(SeekFrom::Start(0))?;
    let mut magic = [0u8; 3];
    f.read_exact(&mut magic)?;
    let version = f.read_u8()?;

    if &magic == b"PAK" && version == 1 {
        let dir_offset = f.read_u32::<LittleEndian>()?;
        let _uncomp_size = f.read_u32::<LittleEndian>()?;
        let comp_size = f.read_u32::<LittleEndian>()?;

        f.seek(SeekFrom::Start(dir_offset as u64))?;
        let mut comp_data = vec![0u8; comp_size as usize];
        f.read_exact(&mut comp_data)?;

        let mut uncomp_data = Vec::new();
        let mut decoder = ZlibDecoder::new(comp_data.as_slice());
        decoder.read_to_end(&mut uncomp_data)?;

        let mut cursor = io::Cursor::new(&uncomp_data);
        let file_count = cursor.read_i32::<LittleEndian>()?;

        for _ in 0..file_count {
            let name_len = cursor.read_i32::<LittleEndian>()?;
            let mut name_bytes = vec![0u8; name_len as usize];
            cursor.read_exact(&mut name_bytes)?;
            file_list.push(decode_windows(&name_bytes).replace('\\', "/"));
            cursor.read_u32::<LittleEndian>()?;
            cursor.read_u32::<LittleEndian>()?;
        }
        return Ok(file_list);
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Unsupported or corrupt PAK archive format.",
    ))
}

// -----------------------------------------------------------------------------
// UNPACK ENGINE
// -----------------------------------------------------------------------------

pub fn unpack_pak(pak_path: &Path, out_dir: &Path, logger: &UiLogger) -> io::Result<()> {
    let mut f = File::open(pak_path)?;
    let total_len = f.seek(SeekFrom::End(0))?;

    if total_len >= 28 {
        f.seek(SeekFrom::Start(0))?;
        if f.read_u32::<LittleEndian>()? == 4 {
            let mut magic = [0u8; 24];
            f.read_exact(&mut magic)?;
            if magic.starts_with(b"MASSIVE PAKFILE") {
                logger.log("Detected SpellForce 1 Archive Format.");
                return unpack_sf1(&mut f, out_dir, logger);
            }
        }
    }

    f.seek(SeekFrom::Start(0))?;
    let mut magic = [0u8; 3];
    f.read_exact(&mut magic)?;
    let version = f.read_u8()?;

    if &magic == b"PAK" && version == 1 {
        logger.log("Detected SpellForce 2 Archive Format.");
        return unpack_sf2(&mut f, out_dir, logger);
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Unknown or invalid PAK format!",
    ))
}

fn unpack_sf2(f: &mut File, out_dir: &Path, logger: &UiLogger) -> io::Result<()> {
    let dir_offset = f.read_u32::<LittleEndian>()?;
    let _uncomp_size = f.read_u32::<LittleEndian>()?;
    let comp_size = f.read_u32::<LittleEndian>()?;

    f.seek(SeekFrom::Start(dir_offset as u64))?;
    let mut comp_data = vec![0u8; comp_size as usize];
    f.read_exact(&mut comp_data)?;

    let mut uncomp_data = Vec::new();
    let mut decoder = ZlibDecoder::new(comp_data.as_slice());
    decoder.read_to_end(&mut uncomp_data)?;

    let mut cursor = io::Cursor::new(&uncomp_data);
    let file_count = cursor.read_i32::<LittleEndian>()?;

    for i in 0..file_count {
        let name_len = cursor.read_i32::<LittleEndian>()?;
        let mut name_bytes = vec![0u8; name_len as usize];
        cursor.read_exact(&mut name_bytes)?;
        let filename = decode_windows(&name_bytes).replace('\\', "/");

        let f_offset = cursor.read_u32::<LittleEndian>()?;
        let next_offset = cursor.read_u32::<LittleEndian>()?;
        let size = next_offset.saturating_sub(f_offset);

        if i % 100 == 0 || i == file_count - 1 {
            logger.log(&format!(
                "Extracting SF2 ({}/{}): {}",
                i + 1,
                file_count,
                filename
            ));
        }

        let target = out_dir.join(&filename);
        if let Some(p) = target.parent() {
            fs::create_dir_all(p)?;
        }

        f.seek(SeekFrom::Start(f_offset as u64))?;
        let mut target_file = File::create(target)?;
        let mut chunk = std::io::Read::by_ref(f).take(size as u64);
        io::copy(&mut chunk, &mut target_file)?;
    }
    Ok(())
}

fn unpack_sf1(f: &mut File, out_dir: &Path, logger: &UiLogger) -> io::Result<()> {
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

// -----------------------------------------------------------------------------
// PACK ENGINE
// -----------------------------------------------------------------------------

pub fn pack_pak(
    src_dir: &Path,
    out_file: &Path,
    fmt: &str,
    comp_level: u32,
    logger: &UiLogger,
) -> io::Result<()> {
    if fmt.contains('1') {
        pack_sf1(src_dir, out_file, logger)
    } else {
        pack_sf2(src_dir, out_file, comp_level, logger)
    }
}

fn pack_sf2(src_dir: &Path, out_file: &Path, comp_level: u32, logger: &UiLogger) -> io::Result<()> {
    let mut files = Vec::new();
    for entry in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }

    let mut f = File::create(out_file)?;
    f.write_all(b"PAK\x01")?;
    f.write_all(&[0u8; 12])?;

    let mut entries = Vec::new();
    let num_files = files.len();

    for (idx, file_path) in files.iter().enumerate() {
        let rel_path = file_path
            .strip_prefix(src_dir)
            .unwrap_or(file_path)
            .to_string_lossy()
            .replace('/', "\\")
            .to_lowercase();

        if idx % 100 == 0 || idx == num_files.saturating_sub(1) {
            logger.log(&format!(
                "Packing SF2 ({}/{}): {}",
                idx + 1,
                num_files,
                rel_path
            ));
        }

        let offset = f.stream_position()?;
        let mut in_f = File::open(file_path)?;
        io::copy(&mut in_f, &mut f)?;
        let size = f.stream_position()? - offset;
        entries.push((rel_path, offset as u32, size as u32));
    }

    let dir_offset = f.stream_position()? as u32;
    let mut dir_buf = Vec::new();
    dir_buf.write_i32::<LittleEndian>(entries.len() as i32)?;

    for (name, offset, size) in entries {
        let encoded = encode_windows(&name);
        dir_buf.write_i32::<LittleEndian>(encoded.len() as i32)?;
        dir_buf.write_all(&encoded)?;
        dir_buf.write_u32::<LittleEndian>(offset)?;
        dir_buf.write_u32::<LittleEndian>(offset + size)?;
    }

    let uncomp_size = dir_buf.len() as u32;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(comp_level));
    encoder.write_all(&dir_buf)?;
    let comp_data = encoder.finish()?;
    let comp_size = comp_data.len() as u32;

    f.write_all(&comp_data)?;
    f.seek(SeekFrom::Start(4))?;
    f.write_u32::<LittleEndian>(dir_offset)?;
    f.write_u32::<LittleEndian>(uncomp_size)?;
    f.write_u32::<LittleEndian>(comp_size)?;

    logger.log("SF2 Archive packed successfully.");
    Ok(())
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

/// Compiles a SpellForce 1 .PAK archive using the verified in-engine VFS ordering:
/// 16-bit K&R path hash prefix, verified comparator sorting, directory non-zero offset safety,
/// and 4-byte DWORD alignment to prevent D3DERR_INVALIDCALL.
fn pack_sf1(src_dir: &Path, out_file: &Path, logger: &UiLogger) -> io::Result<()> {
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

    // In-engine binary search sort comparator (100% match with original Phenomic VFS order)
    file_entries.sort_by(|a, b| {
        a.h_hi
            .cmp(&b.h_hi)
            .then_with(|| a.h_lo.cmp(&b.h_lo))
            .then_with(|| a.comp_str.cmp(&b.comp_str))
    });

    // Locate root index (midpoint or mesh\meshes.txt index)
    let mut root_idx = (num_files.saturating_sub(1) / 2) as u32;
    for (idx, entry) in file_entries.iter().enumerate() {
        if entry.rel_path == "mesh\\meshes.txt" {
            root_idx = idx as u32;
            break;
        }
    }

    // Build String Table
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
            entry.dir_off = 0; // 0 indicates root directory in the engine
        }
    }

    // 4-byte DWORD alignment for String Table (prevents D3DERR_INVALIDCALL)
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

    // Build Phenomic 92-byte Header
    let mut header = vec![0u8; 92];
    let mut hw = Cursor::new(&mut header);
    hw.write_u32::<LittleEndian>(4)?;
    let mut magic = b"MASSIVE PAKFILE V 4.0\r\n\0".to_vec();
    magic.resize(24, 0);
    hw.write_all(&magic)?;
    hw.write_all(&PHENOMIC_HEADER_TEMPLATE)?; // 44 bytes at offsets 28..72

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

pub fn batch_unpack_paks(root_dir: &Path, logger: &UiLogger) -> io::Result<()> {
    logger.log(&format!("[*] Initializing batch unpack: {:?}", root_dir));
    let mut found = 0;
    for entry in WalkDir::new(root_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file()
            && entry.path().extension().and_then(|s| s.to_str()) == Some("pak")
        {
            let pak_path = entry.path();
            let parent_dir = pak_path.parent().unwrap_or_else(|| Path::new("."));
            let file_stem = pak_path.file_stem().unwrap_or_default().to_string_lossy();
            let out_dir = parent_dir.join(format!("{}_extracted", file_stem));

            logger.log(&format!(
                "[*] Batch Extracting: {}",
                pak_path.file_name().unwrap_or_default().to_string_lossy()
            ));
            if let Err(e) = unpack_pak(pak_path, &out_dir, logger) {
                logger.log(&format!("[!] Error: {}", e));
            } else {
                found += 1;
            }
        }
    }
    logger.log(&format!(
        "[+] Batch unpack completed. Extracted {} archives.",
        found
    ));
    Ok(())
}

pub fn batch_pack_folders(
    root_dir: &Path,
    fmt: &str,
    comp_level: u32,
    logger: &UiLogger,
) -> io::Result<()> {
    logger.log(&format!("[*] Initializing batch pack: {:?}", root_dir));
    let mut compiled = 0;

    let entries = fs::read_dir(root_dir)?;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            let folder_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if !folder_name.to_lowercase().ends_with("_extracted") {
                continue;
            }

            let base_name = folder_name[..folder_name.len().saturating_sub(10)].to_string();
            let out_pak_path = root_dir.join(format!("{}.pak", base_name));
            logger.log(&format!("[*] Compiling directory {:?}", folder_name));

            if let Err(e) = pack_pak(&path, &out_pak_path, fmt, comp_level, logger) {
                logger.log(&format!("[!] Error: {}", e));
            } else {
                compiled += 1;
            }
        }
    }
    logger.log(&format!(
        "[+] Batch pack completed. Compiled {} directories.",
        compiled
    ));
    Ok(())
}
