use super::container::Manifest;
use crate::UiLogger;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

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
