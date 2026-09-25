// src/pak/mod.rs

pub mod addon;
pub mod sf1;
pub mod sf2;
pub mod vfs_tree;

pub use vfs_tree::*;

use crate::UiLogger;
use byteorder::{LittleEndian, ReadBytesExt};
use flate2::read::ZlibDecoder;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use walkdir::WalkDir;

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
                return sf1::unpack_sf1(&mut f, out_dir, logger);
            }
        }
    }

    f.seek(SeekFrom::Start(0))?;
    let mut magic = [0u8; 3];
    f.read_exact(&mut magic)?;
    let version = f.read_u8()?;

    if &magic == b"PAK" && version == 1 {
        logger.log("Detected SpellForce 2 Archive Format.");
        return sf2::unpack_sf2(&mut f, out_dir, logger);
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Unknown or invalid PAK format!",
    ))
}

pub fn pack_pak(
    src_dir: &Path,
    out_file: &Path,
    fmt: &str,
    comp_level: u32,
    logger: &UiLogger,
) -> io::Result<()> {
    if fmt.contains('1') {
        sf1::pack_sf1(src_dir, out_file, logger)
    } else {
        sf2::pack_sf2(src_dir, out_file, comp_level, logger)
    }
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

                let name_list_start = f.stream_position()?;
                let meta_len = (data_start as u64).saturating_sub(name_list_start);
                let mut meta_data = vec![0u8; meta_len as usize];
                f.read_exact(&mut meta_data)?;

                for (name_off, dir_off) in name_offs {
                    let file_name =
                        sf1::read_reversed_string_from_bytes(&meta_data, name_off as usize + 2);
                    let dir_name = if dir_off != 0x00FFFFFF && dir_off != 0 {
                        sf1::read_reversed_string_from_bytes(&meta_data, dir_off as usize)
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
            file_list.push(crate::cff::decode_windows(&name_bytes).replace('\\', "/"));
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

pub fn read_file_from_pak(
    pak_path: &Path,
    target_stem: &str,
    extensions: &[&str],
) -> Option<(Vec<u8>, String)> {
    let mut f = File::open(pak_path).ok()?;
    let total_len = f.seek(SeekFrom::End(0)).ok()?;
    if total_len < 28 {
        return None;
    }

    // 1. Try SpellForce 1 (MASSIVE PAKFILE V 4.0)
    f.seek(SeekFrom::Start(0)).ok()?;
    if f.read_u32::<LittleEndian>().ok()? == 4 {
        let mut magic = [0u8; 24];
        f.read_exact(&mut magic).ok()?;
        if magic.starts_with(b"MASSIVE PAKFILE") {
            f.seek(SeekFrom::Start(76)).ok()?;
            let num_files = f.read_u32::<LittleEndian>().ok()?;
            let _root_idx = f.read_u32::<LittleEndian>().ok()?;
            let data_start = f.read_u32::<LittleEndian>().ok()?;

            f.seek(SeekFrom::Start(92)).ok()?;
            let mut entries = Vec::with_capacity(num_files as usize);
            for _ in 0..num_files {
                let size = f.read_u32::<LittleEndian>().ok()?;
                let offset = f.read_u32::<LittleEndian>().ok()?;
                let name_off = f.read_u32::<LittleEndian>().ok()? & 0x00FFFFFF;
                let dir_off = f.read_u32::<LittleEndian>().ok()? & 0x00FFFFFF;
                entries.push((size, offset, name_off, dir_off));
            }

            let name_list_start = f.stream_position().ok()?;
            let meta_len = (data_start as u64).saturating_sub(name_list_start);
            let mut meta_data = vec![0u8; meta_len as usize];
            f.read_exact(&mut meta_data).ok()?;

            let stem_lower = target_stem.to_lowercase();

            for (size, offset, name_off, _dir_off) in entries {
                let file_name =
                    sf1::read_reversed_string_from_bytes(&meta_data, name_off as usize + 2);
                let fn_lower = file_name.to_lowercase();

                for ext in extensions {
                    let expected = format!("{}.{}", stem_lower, ext);
                    if fn_lower == expected
                        || fn_lower.ends_with(&format!("/{}", expected))
                        || fn_lower.ends_with(&format!("\\{}", expected))
                    {
                        f.seek(SeekFrom::Start((data_start + offset) as u64)).ok()?;
                        let mut buf = vec![0u8; size as usize];
                        f.read_exact(&mut buf).ok()?;
                        return Some((buf, file_name));
                    }
                }
            }
            return None;
        }
    }

    // 2. Try SpellForce 2 (PAK\x01)
    f.seek(SeekFrom::Start(0)).ok()?;
    let mut magic = [0u8; 3];
    f.read_exact(&mut magic).ok()?;
    let version = f.read_u8().ok()?;

    if &magic == b"PAK" && version == 1 {
        let dir_offset = f.read_u32::<LittleEndian>().ok()?;
        let _uncomp_size = f.read_u32::<LittleEndian>().ok()?;
        let comp_size = f.read_u32::<LittleEndian>().ok()?;

        f.seek(SeekFrom::Start(dir_offset as u64)).ok()?;
        let mut comp_data = vec![0u8; comp_size as usize];
        f.read_exact(&mut comp_data).ok()?;

        let mut uncomp_data = Vec::new();
        let mut decoder = ZlibDecoder::new(comp_data.as_slice());
        decoder.read_to_end(&mut uncomp_data).ok()?;

        let mut cursor = io::Cursor::new(&uncomp_data);
        let file_count = cursor.read_i32::<LittleEndian>().ok()?;
        let stem_lower = target_stem.to_lowercase();

        for _ in 0..file_count {
            let name_len = cursor.read_i32::<LittleEndian>().ok()?;
            let mut name_bytes = vec![0u8; name_len as usize];
            cursor.read_exact(&mut name_bytes).ok()?;
            let f_offset = cursor.read_u32::<LittleEndian>().ok()?;
            let next_offset = cursor.read_u32::<LittleEndian>().ok()?;
            let size = next_offset.saturating_sub(f_offset);

            let filename = crate::cff::decode_windows(&name_bytes).replace('\\', "/");
            let fn_lower = filename.to_lowercase();

            for ext in extensions {
                let expected = format!("{}.{}", stem_lower, ext);
                if fn_lower == expected || fn_lower.ends_with(&format!("/{}", expected)) {
                    f.seek(SeekFrom::Start(f_offset as u64)).ok()?;
                    let mut buf = vec![0u8; size as usize];
                    f.read_exact(&mut buf).ok()?;
                    return Some((buf, filename));
                }
            }
        }
    }

    None
}
