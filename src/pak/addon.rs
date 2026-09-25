// src/pak/addon.rs

use crate::UiLogger;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn build_addon_mod_pak(
    mod_files_dir: &Path,
    game_data_dir: &Path,
    mod_name: &str,
    logger: &UiLogger,
) -> io::Result<PathBuf> {
    logger.log("===============================================================================");
    logger.log(&format!(
        "[*] Building Standalone Addon PAK Layer: [{}]",
        mod_name
    ));
    logger.log("===============================================================================");

    // 1. Discover highest existing pak number (sf0.pak ... sf35.pak)
    let mut max_num = 35u32;
    if let Ok(entries) = fs::read_dir(game_data_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str())
                && stem.starts_with("sf")
            {
                let num_str: String = stem.chars().filter(|c| c.is_ascii_digit()).collect();
                if let Ok(n) = num_str.parse::<u32>() {
                    max_num = max_num.max(n);
                }
            }
        }
    }

    // Target index 99 ensures highest VFS override priority without conflicts
    let target_pak_name = format!(
        "sf99_{}.pak",
        mod_name.trim().replace(' ', "_").to_lowercase()
    );
    let target_path = game_data_dir.join(&target_pak_name);

    logger.log(&format!(
        "[+] Detected highest base game archive: sf{}.pak. Assigning addon name: {}",
        max_num, target_pak_name
    ));

    crate::pak::sf1::pack_sf1(mod_files_dir, &target_path, logger)?;
    logger.log(&format!(
        "[+] Addon mod package successfully deployed to: {:?}",
        target_path
    ));
    Ok(target_path)
}
