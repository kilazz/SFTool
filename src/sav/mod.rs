// src/sav/mod.rs

pub mod avatar;

use crate::UiLogger;
use byteorder::{LittleEndian, ReadBytesExt};
use flate2::read::ZlibDecoder;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::Path;

const MAGIC_SF1_CFF: [u8; 4] = [0x02, 0xc5, 0x72, 0xdd];
const MAGIC_SF1_PLAT: [u8; 4] = [0x12, 0xdd, 0x72, 0xdd];

#[derive(Debug)]
pub struct SavChunkHeader {
    pub chunk_id: u16,
    pub occurrence: i16,
    pub is_packed: i16,
    pub packed_size: i32,
    pub data_type: i16,
    pub unpacked_size: i32,
}

pub struct SavInspectReport {
    pub total_chunks: usize,
    pub nested_containers_found: usize,
}

pub fn inspect_and_unpack_sav(
    sav_path: &Path,
    out_dir: &Path,
    logger: &UiLogger,
) -> io::Result<SavInspectReport> {
    logger.log(&format!(
        "[*] Opening SpellForce SaveGame file: {:?}",
        sav_path
    ));
    fs::create_dir_all(out_dir)?;

    let mut data = Vec::new();
    File::open(sav_path)?.read_to_end(&mut data)?;

    if data.len() < 20 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Save file is too small to contain a valid SFChunk header!",
        ));
    }

    let mut cursor = Cursor::new(&data);
    cursor.set_position(20); // Skip 20-byte container header

    let mut total_chunks = 0;
    let mut nested_containers_found = 0;

    while (cursor.position() as usize) + 12 <= data.len() {
        let chunk_id = cursor.read_u16::<LittleEndian>()?;
        let occurrence = cursor.read_i16::<LittleEndian>()?;
        let is_packed = cursor.read_i16::<LittleEndian>()?;
        let packed_size = cursor.read_i32::<LittleEndian>()?;
        let data_type = cursor.read_i16::<LittleEndian>()?;

        if packed_size <= 0 {
            break;
        }

        let (unpacked_bytes, _unpacked_size) = if is_packed == 0 {
            let mut buf = vec![0u8; packed_size as usize];
            if cursor.position() as usize + (packed_size as usize) > data.len() {
                break;
            }
            cursor.read_exact(&mut buf)?;
            let len = buf.len() as i32;
            (buf, len)
        } else {
            let uncomp_sz = cursor.read_i32::<LittleEndian>()?;
            let mut comp = vec![0u8; packed_size as usize];
            if cursor.position() as usize + (packed_size as usize) > data.len() {
                break;
            }
            cursor.read_exact(&mut comp)?;
            let mut uncomp = Vec::new();
            let mut decoder = ZlibDecoder::new(comp.as_slice());
            if decoder.read_to_end(&mut uncomp).is_err() {
                uncomp = comp;
            }
            (uncomp, uncomp_sz)
        };

        let chunk_filename = format!(
            "chunk_{:04}_id0x{:04X}_type{}_occ{}.dat",
            total_chunks, chunk_id, data_type, occurrence
        );
        let target_path = out_dir.join(&chunk_filename);
        File::create(&target_path)?.write_all(&unpacked_bytes)?;

        // Detect embedded CFF containers inside the unpacked chunk payload
        if let Some(nested_offset) = find_nested_container_offset(&unpacked_bytes) {
            nested_containers_found += 1;
            let sub_dir = out_dir.join(format!("nested_chunk_{:04}", total_chunks));
            fs::create_dir_all(&sub_dir)?;
            logger.log(&format!(
                "[+] Found embedded CFF container in Chunk #{} (ID 0x{:04X}) at byte offset {}!",
                total_chunks, chunk_id, nested_offset
            ));
            File::create(sub_dir.join("embedded_payload.cff"))?
                .write_all(&unpacked_bytes[nested_offset..])?;
        }

        total_chunks += 1;
    }

    logger.log(&format!(
        "[+] Successfully unpacked {} save chunks! Found {} nested state containers.",
        total_chunks, nested_containers_found
    ));

    Ok(SavInspectReport {
        total_chunks,
        nested_containers_found,
    })
}

fn find_nested_container_offset(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 24 {
        return None;
    }
    for i in 0..bytes.len().saturating_sub(4) {
        if bytes[i..i + 4] == MAGIC_SF1_CFF || bytes[i..i + 4] == MAGIC_SF1_PLAT {
            return Some(i);
        }
    }
    None
}
