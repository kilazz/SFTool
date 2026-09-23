use crate::UiLogger;
use rayon::prelude::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Parallel syntax diagnostics for SF2 scripts using 'luac5.1 -p'
pub fn batch_check_syntax_sf2(
    scripts_dir: &Path,
    luac5_exe: &Path,
    logger: &UiLogger,
) -> io::Result<(usize, usize)> {
    if !luac5_exe.exists() {
        logger.log(&format!(
            "[!] Lua 5.1 compiler not found at: {:?}",
            luac5_exe
        ));
        return Ok((0, 0));
    }

    logger.log(&format!(
        "[*] Starting SF2 Lua 5.1 syntax diagnostics: {:?}",
        scripts_dir
    ));

    let files: Vec<PathBuf> = WalkDir::new(scripts_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_file() && e.path().extension().and_then(|s| s.to_str()) == Some("lua")
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let total = files.len();
    if total == 0 {
        logger.log("[!] No Lua files found to check.");
        return Ok((0, 0));
    }

    let passed = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    files.par_iter().for_each(|f| {
        let (ok, msg) = super::sf1::check_file_syntax(f, luac5_exe);
        if ok {
            passed.fetch_add(1, Ordering::Relaxed);
        } else {
            failed.fetch_add(1, Ordering::Relaxed);
            let rel_name = f.strip_prefix(scripts_dir).unwrap_or(f);
            logger.log(&format!(
                "[!] Syntax Error in {}:\n    {}",
                rel_name.display(),
                msg
            ));
        }
    });

    let ok_cnt = passed.load(Ordering::Relaxed);
    let fail_cnt = failed.load(Ordering::Relaxed);

    logger.log(&format!(
        "[+] SF2 Diagnostics finished: Checked {}, Passed: {}, Errors: {}",
        total, ok_cnt, fail_cnt
    ));
    Ok((ok_cnt, fail_cnt))
}

/// StyLua batch formatter chunked by 50 files to avoid Windows 32k CLI overflow
pub fn batch_format_stylua(
    scripts_dir: &Path,
    stylua_exe: &Path,
    use_tabs: bool,
    logger: &UiLogger,
) -> io::Result<()> {
    if !stylua_exe.exists() {
        logger.log(&format!(
            "[!] StyLua executable not found at: {:?}",
            stylua_exe
        ));
        return Ok(());
    }

    logger.log(&format!(
        "[*] Scanning files for StyLua (Lua 5.1 mode): {:?}",
        scripts_dir
    ));

    let files: Vec<PathBuf> = WalkDir::new(scripts_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_file() && e.path().extension().and_then(|s| s.to_str()) == Some("lua")
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let total_files = files.len();
    if total_files == 0 {
        logger.log("[!] No Lua files found to format.");
        return Ok(());
    }

    const CHUNK_SIZE: usize = 50;
    let total_chunks = total_files.div_ceil(CHUNK_SIZE);
    let indent_type = if use_tabs { "Tabs" } else { "Spaces" };

    logger.log(&format!(
        "[*] Formatting {} files in {} batches using StyLua...",
        total_files, total_chunks
    ));

    let mut failed_chunks = 0;

    for (chunk_idx, chunk) in files.chunks(CHUNK_SIZE).enumerate() {
        let mut cmd = Command::new(stylua_exe);
        cmd.arg("--syntax")
            .arg("lua51")
            .arg("--indent-type")
            .arg(indent_type)
            .arg("--no-editorconfig");

        for file_path in chunk {
            cmd.arg(file_path);
        }

        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        match cmd.output() {
            Ok(output) => {
                if !output.status.success() {
                    failed_chunks += 1;
                    let err = String::from_utf8_lossy(&output.stderr);
                    logger.log(&format!(
                        "[!] Batch {}/{} error: {}",
                        chunk_idx + 1,
                        total_chunks,
                        err.trim()
                    ));
                }
            }
            Err(e) => {
                failed_chunks += 1;
                logger.log(&format!("[!] Failed to invoke StyLua: {}", e));
                break;
            }
        }
    }

    if failed_chunks == 0 {
        logger.log(&format!(
            "[+] StyLua successfully formatted all {} files!",
            total_files
        ));
    } else {
        logger.log(&format!(
            "[!] Formatting completed with errors in {} batches.",
            failed_chunks
        ));
    }

    Ok(())
}

pub fn create_map_scaffolding(
    target_dir: &Path,
    project_name: &str,
    map_name: &str,
) -> io::Result<PathBuf> {
    let proj_root = target_dir.join(project_name);
    let map_root = proj_root.join(map_name);
    let dialog_dir = map_root.join("dialog");
    let script_dir = map_root.join("script");

    fs::create_dir_all(&dialog_dir)?;
    fs::create_dir_all(&script_dir)?;

    fs::write(
        proj_root.join("place_the_map_file_here.txt"),
        "Place your compiled .map file here.\n",
    )?;
    fs::write(dialog_dir.join("cutscenetext.lua"), "return {\n\n}\n")?;
    fs::write(dialog_dir.join("outcrytext.lua"), "return {\n\n}\n")?;

    let starter_script = r#"State
{
	StateName = "INIT",

	OnOneTimeEvent
	{
		Conditions = 
		{
			-- Insert initialization conditions
		},
		Actions = 
		{
			-- Insert initialization actions
		},
		GotoState = "MAIN",
	};
};

for i = 1, 3 do
	local sPlayerName = "pl_Human" .. i

	-- ********************************************************************
	-- Setup Event for Player
	-- ********************************************************************
	OnOneTimeEvent
	{
		Conditions = 
		{
		},
		Actions = 
		{
		},
	}
end

State
{
	StateName = "MAIN",

	OnOneTimeEvent
	{
		Conditions =
		{
		},
		Actions =
		{
		},
	},
};
"#;

    let script_file = script_dir.join(format!("_{}.lua", map_name));
    fs::write(&script_file, starter_script)?;

    Ok(map_root)
}
