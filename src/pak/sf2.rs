use crate::UiLogger;
use crate::cff::{decode_windows, encode_windows};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use walkdir::WalkDir;

pub fn unpack_sf2(f: &mut File, out_dir: &Path, logger: &UiLogger) -> io::Result<()> {
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

pub fn pack_sf2(
    src_dir: &Path,
    out_file: &Path,
    comp_level: u32,
    logger: &UiLogger,
) -> io::Result<()> {
    logger.log("[*] Compiling SF2 archive with VFS sorting and payload deduplication...");

    // 1. Collect all files and compute their relative paths
    let mut file_entries: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            let rel_path = entry
                .path()
                .strip_prefix(src_dir)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('/', "\\")
                .to_lowercase();
            file_entries.push((rel_path, entry.path().to_path_buf()));
        }
    }

    // 2. OPTIMIZATION: Sort paths alphabetically.
    // This ensures deterministic builds and optimizes the engine's internal binary search.
    file_entries.sort_by(|a, b| a.0.cmp(&b.0));

    let num_files = file_entries.len();
    if num_files == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No files found in source directory to pack!",
        ));
    }

    let mut f = File::create(out_file)?;
    f.write_all(b"PAK\x01")?;
    f.write_all(&[0u8; 12])?;

    let mut entries = Vec::with_capacity(num_files);

    // Deduplication registry: (Size, SHA-256) -> Offset in payload section
    let mut seen_payloads: HashMap<(u32, [u8; 32]), u32> = HashMap::new();
    let mut dedup_count = 0usize;
    let mut saved_bytes = 0u64;

    // 3. Process files and write payloads with deduplication
    for (idx, (rel_path, file_path)) in file_entries.into_iter().enumerate() {
        if idx % 100 == 0 || idx == num_files.saturating_sub(1) {
            logger.log(&format!(
                "Packing SF2 ({}/{}): {}",
                idx + 1,
                num_files,
                rel_path
            ));
        }

        // Read the entire file into memory
        let file_bytes = fs::read(&file_path)?;
        let size = file_bytes.len() as u32;

        // Calculate SHA-256 hash for deduplication
        let mut hasher = Sha256::new();
        hasher.update(&file_bytes);
        let hash: [u8; 32] = hasher.finalize().into();

        // Check if identical payload already exists in the archive
        let offset = if let Some(&existing_offset) = seen_payloads.get(&(size, hash)) {
            // Reuse the existing offset
            dedup_count += 1;
            saved_bytes += size as u64;
            existing_offset
        } else {
            // Write new payload and record its offset
            let cur_offset = f.stream_position()? as u32;
            f.write_all(&file_bytes)?;
            seen_payloads.insert((size, hash), cur_offset);
            cur_offset
        };

        // Store the entry data for the directory table
        entries.push((rel_path, offset, size));
    }

    // 4. Build the directory table
    let dir_offset = f.stream_position()? as u32;
    let mut dir_buf = Vec::new();
    dir_buf.write_i32::<LittleEndian>(entries.len() as i32)?;

    for (name, offset, size) in entries {
        let encoded = encode_windows(&name);
        dir_buf.write_i32::<LittleEndian>(encoded.len() as i32)?;
        dir_buf.write_all(&encoded)?;
        dir_buf.write_u32::<LittleEndian>(offset)?;

        // SF2 calculates size as (next_offset - f_offset).
        // Providing (offset + size) creates the correct boundary regardless of physical layout.
        dir_buf.write_u32::<LittleEndian>(offset + size)?;
    }

    // 5. Compress the directory table and update the header
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

    // Log deduplication statistics
    if dedup_count > 0 {
        logger.log(&format!(
            "[+] SF2 Deduplication saved: {} duplicates merged, {:.2} MB saved in archive payload.",
            dedup_count,
            saved_bytes as f64 / 1_048_576.0
        ));
    }

    logger.log("[+] SF2 Archive packed successfully.");
    Ok(())
}
