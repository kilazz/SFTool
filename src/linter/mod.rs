// src/linter/mod.rs

use crate::UiLogger;
use crate::cff::validation::validate_cff_integrity;
use crate::tools::find_tool;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub struct ModLintReport {
    pub lua_scripts_checked: usize,
    pub lua_syntax_errors: usize,
    pub broken_cff_links: usize,
    pub missing_textures: Vec<String>,
}

pub fn verify_mod(
    mod_dir: &Path,
    asset_source: Option<&Path>,
    logger: &UiLogger,
) -> std::io::Result<ModLintReport> {
    logger.log("===============================================================================");
    logger.log(&format!(
        "[*] Starting Pre-Release Mod Audit for: {:?}",
        mod_dir
    ));
    logger.log("===============================================================================");

    // 1. Validate Lua scripts
    let luac4 = find_tool("luac4.exe");
    let mut scripts_checked = 0;
    let mut syntax_errors = 0;

    for entry in WalkDir::new(mod_dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("lua") {
            scripts_checked += 1;
            let (ok, msg) = crate::lua::sf1::check_file_syntax(p, &luac4);
            if !ok {
                syntax_errors += 1;
                logger.log(&format!(
                    "[!] Lua Syntax Error in {:?}: {}",
                    p.file_name().unwrap_or_default(),
                    msg
                ));
            }
        }
    }
    logger.log(&format!(
        "[+] Lua Syntax Check: {} scripts checked, {} errors.",
        scripts_checked, syntax_errors
    ));

    // 2. Validate CFF integrity if manifest.json exists
    let mut broken_cff_links = 0;
    if mod_dir.join("manifest.json").exists()
        && let Ok(report) = validate_cff_integrity(mod_dir, logger)
    {
        broken_cff_links = report.broken_references.len();
    }

    // 3. Scan and verify texture references
    let mut missing_textures = Vec::new();
    let mut checked_textures = BTreeSet::new();

    let check_asset = |asset_name: &str| -> bool {
        if asset_name.is_empty() || asset_name == "<undefined>" {
            return true;
        }
        let clean = asset_name
            .trim()
            .trim_end_matches(".msh")
            .trim_end_matches(".msb");
        if let Some(src) = asset_source {
            crate::cff::find_and_load_texture(mod_dir, src, clean).is_some()
        } else {
            crate::cff::find_and_load_texture(mod_dir, mod_dir, clean).is_some()
        }
    };

    // Check textures referenced in tables_json if present
    let tables_dir = mod_dir.join("tables_json");
    if tables_dir.exists() {
        if let Ok(content) = fs::read_to_string(tables_dir.join("spell_lines.json"))
            && let Ok(lines) =
                serde_json::from_str::<Vec<crate::cff::sf1_schema::SpellLineEntry>>(&content)
        {
            for l in lines {
                if checked_textures.insert(l.icon_name.clone()) && !check_asset(&l.icon_name) {
                    missing_textures.push(format!("Spell Line #{} -> {}", l.line_id, l.icon_name));
                }
            }
        }
        if let Ok(content) = fs::read_to_string(tables_dir.join("tech_tree_upgrades.json"))
            && let Ok(upgrades) =
                serde_json::from_str::<Vec<crate::cff::sf1_schema::TechTreeUpgradeEntry>>(&content)
        {
            for u in upgrades {
                if checked_textures.insert(u.icon_name.clone()) && !check_asset(&u.icon_name) {
                    missing_textures
                        .push(format!("Tech Upgrade #{} -> {}", u.upgrade_id, u.icon_name));
                }
            }
        }
    }

    for m in &missing_textures {
        logger.log(&format!("  [!] Missing Texture Asset: {}", m));
    }

    logger.log("===============================================================================");
    if syntax_errors == 0 && broken_cff_links == 0 && missing_textures.is_empty() {
        logger.log("[+] LINT STATUS: PASSED! Mod is stable, clean, and ready for release.");
    } else {
        logger.log(&format!(
            "[!] LINT STATUS: FAILED! Found {} syntax error(s), {} broken DB link(s), {} missing asset(s).",
            syntax_errors, broken_cff_links, missing_textures.len()
        ));
    }
    logger.log("===============================================================================");

    Ok(ModLintReport {
        lua_scripts_checked: scripts_checked,
        lua_syntax_errors: syntax_errors,
        broken_cff_links,
        missing_textures,
    })
}
