use super::text::{ChunkFormat, detect_format, export_text, import_text};
use crate::UiLogger;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::Path;

pub use super::sf1::get_sf1_chunk_info;
pub use super::sf2::get_sf2_chunk_info;

pub fn get_sf2_chunk_name(id: u32) -> Option<&'static str> {
    get_sf2_chunk_info(id).map(|info| info.name)
}

#[derive(Serialize, Deserialize)]
pub struct ChunkManifest {
    pub file: String,
    pub id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flag1: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flag2: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurrence: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comp_flag: Option<i16>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub c_type: Option<i16>,
}

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub chunks: Vec<ChunkManifest>,
}

// -----------------------------------------------------------------------------
// UNPACK ALL & PACK ALL
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

            let required_bytes = if comp_flag == 0 {
                comp_size as usize
            } else {
                4 + (comp_size as usize)
            };

            if comp_size <= 0 || cursor.position() as usize + required_bytes > data.len() {
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
