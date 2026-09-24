// src/dds/repair.rs

use crate::UiLogger;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::fs;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use walkdir::WalkDir;

const DDS_MAGIC: u32 = 0x20534444; // b"DDS "
const DDSD_MIPMAPCOUNT: u32 = 0x00020000;
const DDSCAPS_MIPMAP: u32 = 0x00400000;
const DDSCAPS_COMPLEX: u32 = 0x00000008;

pub struct DdsRepairReport {
    pub total_scanned: usize,
    pub broken_found: usize,
    pub fixed_count: usize,
}

#[allow(dead_code)]
pub fn calculate_expected_mipmaps(mut width: u32, mut height: u32) -> u32 {
    let mut mips = 1;
    while width > 1 || height > 1 {
        width = (width / 2).max(1);
        height = (height / 2).max(1);
        mips += 1;
    }
    mips
}

pub fn repair_single_dds(file_path: &Path) -> std::io::Result<bool> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(file_path)?;
    let mut header = [0u8; 128];
    file.read_exact(&mut header)?;

    let mut cur = Cursor::new(&header);
    let magic = cur.read_u32::<LittleEndian>()?;
    if magic != DDS_MAGIC {
        return Ok(false);
    }

    let _dw_size = cur.read_u32::<LittleEndian>()?;
    let dw_flags = cur.read_u32::<LittleEndian>()?;
    let height = cur.read_u32::<LittleEndian>()?;
    let width = cur.read_u32::<LittleEndian>()?;

    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return Ok(false);
    }

    cur.set_position(28);
    let current_mips = cur.read_u32::<LittleEndian>()?;

    cur.set_position(76);
    let pf_size = cur.read_u32::<LittleEndian>()?;
    if pf_size != 32 {
        return Ok(false);
    }
    let pf_flags = cur.read_u32::<LittleEndian>()?;
    let pf_fourcc = cur.read_u32::<LittleEndian>()?;
    let pf_rgb_bitcount = cur.read_u32::<LittleEndian>()?;

    let file_len = file.metadata()?.len() as usize;
    let header_size = if pf_fourcc == u32::from_le_bytes(*b"DX10") {
        148
    } else {
        128
    };

    if file_len < header_size {
        return Ok(false);
    }

    let is_compressed = (pf_flags & 0x00000004) != 0;
    let block_bytes = if is_compressed {
        match &pf_fourcc.to_le_bytes() {
            b"DXT1" | b"ATI1" | b"BC4U" | b"BC4S" => 8,
            b"DXT2" | b"DXT3" | b"DXT4" | b"DXT5" | b"ATI2" | b"BC5U" | b"BC5S" => 16,
            _ => 16,
        }
    } else {
        0
    };
    let bpp = if !is_compressed {
        if pf_rgb_bitcount > 0 {
            pf_rgb_bitcount as usize
        } else {
            32
        }
    } else {
        0
    };

    // Calculate how many complete mipmaps are actually present in the file payload
    let mut available_payload = file_len - header_size;
    let mut actual_mips = 0u32;
    let mut w = width;
    let mut h = height;

    while available_payload > 0 {
        let level_size = if is_compressed {
            let bw = w.div_ceil(4);
            let bh = h.div_ceil(4);
            (bw as usize) * (bh as usize) * block_bytes
        } else {
            ((w as usize) * bpp).div_ceil(8) * (h as usize)
        };

        if available_payload >= level_size && level_size > 0 {
            available_payload -= level_size;
            actual_mips += 1;
            if w == 1 && h == 1 {
                break;
            }
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        } else {
            break;
        }
    }

    // Corrupted payload that cannot even hold a single full image surface
    if actual_mips == 0 {
        return Ok(false);
    }

    let mut modified = false;

    // 1. Repair dwMipMapCount if there is a mismatch or an invalid 0 count
    if current_mips != actual_mips {
        file.seek(SeekFrom::Start(28))?;
        file.write_u32::<LittleEndian>(actual_mips)?;
        modified = true;
    }

    // 2. Synchronize the DDSD_MIPMAPCOUNT flag in dwFlags
    let should_have_mip_flag = actual_mips > 1;
    let has_mip_flag = (dw_flags & DDSD_MIPMAPCOUNT) != 0;

    if should_have_mip_flag != has_mip_flag {
        let new_flags = if should_have_mip_flag {
            dw_flags | DDSD_MIPMAPCOUNT
        } else {
            dw_flags & !DDSD_MIPMAPCOUNT
        };
        file.seek(SeekFrom::Start(8))?;
        file.write_u32::<LittleEndian>(new_flags)?;
        modified = true;
    }

    // 3. Synchronize DDSCAPS_MIPMAP and DDSCAPS_COMPLEX in dwCaps1 (offset 108)
    file.seek(SeekFrom::Start(108))?;
    let mut caps_buf = [0u8; 4];
    file.read_exact(&mut caps_buf)?;
    let dw_caps1 = u32::from_le_bytes(caps_buf);

    let has_caps_mip = (dw_caps1 & DDSCAPS_MIPMAP) != 0;
    if should_have_mip_flag != has_caps_mip {
        let new_caps = if should_have_mip_flag {
            dw_caps1 | DDSCAPS_MIPMAP | DDSCAPS_COMPLEX
        } else {
            dw_caps1 & !DDSCAPS_MIPMAP
        };
        file.seek(SeekFrom::Start(108))?;
        file.write_u32::<LittleEndian>(new_caps)?;
        modified = true;
    }

    Ok(modified)
}

pub fn batch_repair_dds(dir_path: &Path, logger: &UiLogger) -> std::io::Result<DdsRepairReport> {
    logger.log(&format!(
        "[*] Starting DDS Texture Repair scan in: {:?}",
        dir_path
    ));

    let mut report = DdsRepairReport {
        total_scanned: 0,
        broken_found: 0,
        fixed_count: 0,
    };

    for entry in WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file()
            && p.extension()
                .and_then(|s| s.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("dds"))
                .unwrap_or(false)
        {
            report.total_scanned += 1;
            match repair_single_dds(p) {
                Ok(true) => {
                    report.broken_found += 1;
                    report.fixed_count += 1;
                    logger.log(&format!(
                        "[+] Repaired DDS header: {:?}",
                        p.file_name().unwrap_or_default()
                    ));
                }
                Ok(false) => {}
                Err(e) => {
                    logger.log(&format!(
                        "[!] Error reading {:?}: {}",
                        p.file_name().unwrap_or_default(),
                        e
                    ));
                }
            }
        }
    }

    logger.log(&format!(
        "[+] Scan completed! Scanned: {}, Corrupt headers fixed: {}",
        report.total_scanned, report.fixed_count
    ));
    Ok(report)
}
