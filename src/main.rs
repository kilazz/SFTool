slint::include_modules!();

mod cff;
mod lua;
mod pak;

use slint::{ModelRc, SharedString, StandardListViewItem, VecModel};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

#[derive(Clone)]
pub struct UiLogger {
    sender: mpsc::Sender<String>,
}

impl UiLogger {
    pub fn log(&self, msg: &str) {
        let _ = self.sender.send(format!("{}\n", msg));
    }
}

fn make_cli_logger() -> (UiLogger, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<String>();
    let handle = thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            print!("{}", msg);
        }
    });
    (UiLogger { sender: tx }, handle)
}

fn print_help() {
    println!(
        "\
SFTool v2.1 - SpellForce Modding, Localization & Scripting Suite (CLI Mode)
Usage: SFTool <command> [arguments...]

PAK & VFS Commands:
  unpack_pak <pak_file> <out_dir>
      Extract all files from a SpellForce 1 or SpellForce 2 PAK archive.

  pack_pak <src_dir> <out_pak> [fmt: sf1|sf2] [comp: 0-9]
      Pack directory into PAK (SF1 uses verified in-engine VFS ordering).

  batch_unpack_pak <root_folder>
      Recursively find and extract all .pak archives in root_folder.

  batch_pack_pak <root_folder> [fmt: sf1|sf2] [comp: 0-9]
      Batch pack all '*_extracted' directories back into .pak files.

CFF Database Commands:
  unpack_cff <input_cff> <out_dir>
      Unpack CFF container into binary chunks and export texts to JSON.

  pack_cff <in_dir> <out_cff> [compression_level: 0-9, default: 6]
      Import texts from JSON into chunks and compile into a CFF container.

  create_diff <base_cff_dir> <mod_cff_dir> <out_patch.json>
      Generate a non-destructive mod diff patch between base and modified databases.

  apply_diff <target_cff_dir> <patch.json>
      Apply and merge a mod diff patch into a target working database directory.

Lua 4.0 Scripting Commands:
  decompile_lua <src_dir> <out_dir> [luadec_exe] [--no-resume]
      High-performance parallel Lua 4.0 bytecode decompiler.

  check_lua <scripts_dir> [luac_exe]
      Parallel syntax validation using 'luac4 -p'.

  format_lua <scripts_dir> [--spaces <n>]
      Beautify and indent Lua 4.0 scripts to match original Phenomic source.

General:
  help, --help, -h
      Show this help message.

Note: If no arguments are provided, SFTool launches in GUI mode."
    );
}

fn handle_cli(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let cmd = args[1].to_lowercase();
    match cmd.as_str() {
        "--help" | "-h" | "help" => print_help(),
        "unpack_cff" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool unpack_cff <input_cff> <out_dir>");
                return Ok(());
            }
            let in_path = PathBuf::from(&args[2]);
            let out_dir = PathBuf::from(&args[3]);
            let (logger, handle) = make_cli_logger();
            cff::unpack_all(&in_path, &out_dir, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "pack_cff" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool pack_cff <in_dir> <out_cff> [compression_level]");
                return Ok(());
            }
            let in_dir = PathBuf::from(&args[2]);
            let out_file = PathBuf::from(&args[3]);
            let comp = args.get(4).and_then(|s| s.parse::<u32>().ok()).unwrap_or(6);
            let (logger, handle) = make_cli_logger();
            cff::pack_all(&in_dir, &out_file, comp, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "create_diff" => {
            if args.len() < 5 {
                eprintln!(
                    "Usage: SFTool create_diff <base_cff_dir> <mod_cff_dir> <out_patch.json>"
                );
                return Ok(());
            }
            let (logger, handle) = make_cli_logger();
            cff::create_diff(
                Path::new(&args[2]),
                Path::new(&args[3]),
                Path::new(&args[4]),
                &logger,
            )?;
            drop(logger);
            let _ = handle.join();
        }
        "apply_diff" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool apply_diff <target_cff_dir> <patch.json>");
                return Ok(());
            }
            let (logger, handle) = make_cli_logger();
            cff::apply_patch(Path::new(&args[2]), Path::new(&args[3]), &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "unpack_pak" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool unpack_pak <pak_file> <out_dir>");
                return Ok(());
            }
            let pak_path = PathBuf::from(&args[2]);
            let out_dir = PathBuf::from(&args[3]);
            let (logger, handle) = make_cli_logger();
            pak::unpack_pak(&pak_path, &out_dir, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "pack_pak" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool pack_pak <src_dir> <out_pak> [fmt: sf1/sf2] [comp]");
                return Ok(());
            }
            let src_dir = PathBuf::from(&args[2]);
            let out_file = PathBuf::from(&args[3]);
            let fmt = args.get(4).map(|s| s.as_str()).unwrap_or("sf1");
            let comp = args.get(5).and_then(|s| s.parse::<u32>().ok()).unwrap_or(6);
            let (logger, handle) = make_cli_logger();
            pak::pack_pak(&src_dir, &out_file, fmt, comp, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "batch_unpack_pak" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool batch_unpack_pak <root_folder>");
                return Ok(());
            }
            let root = PathBuf::from(&args[2]);
            let (logger, handle) = make_cli_logger();
            pak::batch_unpack_paks(&root, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "batch_pack_pak" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool batch_pack_pak <root_folder> [fmt: sf1/sf2] [comp]");
                return Ok(());
            }
            let root = PathBuf::from(&args[2]);
            let fmt = args.get(3).map(|s| s.as_str()).unwrap_or("sf1");
            let comp = args.get(4).and_then(|s| s.parse::<u32>().ok()).unwrap_or(6);
            let (logger, handle) = make_cli_logger();
            pak::batch_pack_folders(&root, fmt, comp, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "decompile_lua" => {
            if args.len() < 4 {
                eprintln!(
                    "Usage: SFTool decompile_lua <src_dir> <out_dir> [luadec_exe] [--no-resume]"
                );
                return Ok(());
            }
            let src = PathBuf::from(&args[2]);
            let out = PathBuf::from(&args[3]);
            let luadec = args
                .get(4)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("luadec_32_deb.exe"));
            let resume = !args.iter().any(|a| a == "--no-resume");
            let (logger, handle) = make_cli_logger();
            lua::batch_decompile(&src, &out, &luadec, resume, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "check_lua" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool check_lua <scripts_dir> [luac_exe]");
                return Ok(());
            }
            let src = PathBuf::from(&args[2]);
            let luac = args
                .get(3)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("luac4.exe"));
            let (logger, handle) = make_cli_logger();
            lua::batch_check_syntax(&src, &luac, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "format_lua" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool format_lua <scripts_dir> [--spaces <n>]");
                return Ok(());
            }
            let src = PathBuf::from(&args[2]);
            let unit = if let Some(idx) = args.iter().position(|a| a == "--spaces") {
                let n = args
                    .get(idx + 1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(2);
                " ".repeat(n)
            } else {
                "\t".to_string()
            };
            let (logger, handle) = make_cli_logger();
            lua::batch_format(&src, &unit, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        unknown => {
            eprintln!("[!] Unknown CLI command: '{}'", unknown);
            print_help();
        }
    }
    Ok(())
}

fn run_gui() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let ui_handle = ui.as_weak();

    ui.set_log_text("System Ready.\n".into());
    ui.set_status_msg("Ready.".into());

    let (log_tx, log_rx) = mpsc::channel::<String>();
    let logger_base = UiLogger { sender: log_tx };

    let ui_weak_log = ui_handle.clone();
    thread::spawn(move || {
        let mut logs = VecDeque::with_capacity(300);
        while let Ok(msg) = log_rx.recv() {
            logs.push_back(msg);

            while let Ok(m) = log_rx.try_recv() {
                logs.push_back(m);
            }

            while logs.len() > 250 {
                logs.pop_front();
            }

            let combined = logs.iter().cloned().collect::<String>();
            let _ = ui_weak_log.upgrade_in_event_loop(move |ui| {
                ui.set_log_text(combined.into());
            });

            thread::sleep(std::time::Duration::from_millis(60));
        }
    });

    let tree_items_state = Arc::new(Mutex::new(Vec::<pak::TreeItem>::new()));

    // ---------------- FILE DIALOG CALLBACKS ----------------
    let ui_weak_browse = ui_handle.clone();
    let tree_items_browse = tree_items_state.clone();
    ui.on_browse_file(move |ext| {
        let ext_str = ext.as_str();
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Archive/Database/Executable", &[ext_str])
            .pick_file()
        {
            let path_str = path.to_string_lossy().into_owned();
            let ui_weak = ui_weak_browse.clone();
            let tree_state = tree_items_browse.clone();
            let ext_clone = ext_str.to_string();

            thread::spawn(move || {
                if ext_clone == "pak"
                    && let Ok(file_paths) = pak::list_pak_files(&path)
                {
                    let items = pak::generate_tree_items(&file_paths);
                    let tree_strings = pak::get_visible_tree_nodes(&items);

                    *tree_state.lock().unwrap() = items;

                    let list_items: Vec<_> = tree_strings
                        .into_iter()
                        .map(|s| StandardListViewItem::from(SharedString::from(s)))
                        .collect();

                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        let slint_model = ModelRc::from(Rc::new(VecModel::from(list_items)));
                        ui.set_archive_files(slint_model);
                    });
                }
            });
            SharedString::from(path_str)
        } else {
            SharedString::new()
        }
    });

    let ui_weak_folder = ui_handle.clone();
    let tree_items_folder = tree_items_state.clone();
    ui.on_browse_folder(move || {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            let path_str = path.to_string_lossy().into_owned();
            let ui_weak = ui_weak_folder.clone();
            let tree_state = tree_items_folder.clone();

            thread::spawn(move || {
                if let Ok(file_paths) = pak::list_directory_files(&path) {
                    let items = pak::generate_tree_items(&file_paths);
                    let tree_strings = pak::get_visible_tree_nodes(&items);

                    *tree_state.lock().unwrap() = items;

                    let list_items: Vec<_> = tree_strings
                        .into_iter()
                        .map(|t| StandardListViewItem::from(SharedString::from(t)))
                        .collect();

                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        let slint_model = ModelRc::from(Rc::new(VecModel::from(list_items)));
                        ui.set_archive_files(slint_model);
                    });
                }
            });
            SharedString::from(path_str)
        } else {
            SharedString::new()
        }
    });

    ui.on_save_file(|ext| {
        let ext_str = ext.as_str();
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Archive/Database", &[ext_str])
            .save_file()
        {
            SharedString::from(path.to_string_lossy().into_owned())
        } else {
            SharedString::new()
        }
    });

    // ---------------- ARCHIVE TREE INTERACTION ----------------
    let tree_items_click = tree_items_state.clone();
    let ui_weak_click = ui_handle.clone();
    ui.on_archive_item_clicked(move |visible_index| {
        if visible_index < 0 {
            return;
        }
        let mut items = tree_items_click.lock().unwrap();
        if pak::toggle_tree_node(&mut items, visible_index as usize) {
            let visible_nodes = pak::get_visible_tree_nodes(&items);
            let list_items: Vec<_> = visible_nodes
                .into_iter()
                .map(|t| StandardListViewItem::from(SharedString::from(t)))
                .collect();
            let _ = ui_weak_click.upgrade_in_event_loop(move |ui| {
                let slint_model = ModelRc::from(Rc::new(VecModel::from(list_items)));
                ui.set_archive_files(slint_model);
            });
        }
    });

    // ---------------- PAK OPERATIONS ----------------
    let logger_pak_unpack = logger_base.clone();
    ui.on_unpack_pak(move |input, out| {
        let logger = logger_pak_unpack.clone();
        let in_path = PathBuf::from(input.as_str());
        let out_path = PathBuf::from(out.as_str());

        thread::spawn(move || {
            logger.log(&format!("[*] Unpacking PAK archive: {:?}", in_path));
            if let Err(e) = pak::unpack_pak(&in_path, &out_path, &logger) {
                logger.log(&format!("[!] Error unpacking PAK: {}", e));
            } else {
                logger.log("[+] PAK Unpack cycle completed successfully.");
            }
        });
    });

    let logger_pak_pack = logger_base.clone();
    ui.on_pack_pak(move |src, out, fmt, comp| {
        let logger = logger_pak_pack.clone();
        let src_path = PathBuf::from(src.as_str());
        let out_path = PathBuf::from(out.as_str());
        let fmt_str = fmt.to_string();

        thread::spawn(move || {
            logger.log(&format!("[*] Packing directory into PAK: {:?}", src_path));
            if let Err(e) = pak::pack_pak(&src_path, &out_path, &fmt_str, comp as u32, &logger) {
                logger.log(&format!("[!] Error packing PAK: {}", e));
            } else {
                logger.log("[+] PAK Pack cycle completed successfully.");
            }
        });
    });

    let logger_batch_unpack = logger_base.clone();
    ui.on_batch_unpack_pak(move |root_folder| {
        let logger = logger_batch_unpack.clone();
        let root_path = PathBuf::from(root_folder.as_str());

        thread::spawn(move || {
            if let Err(e) = pak::batch_unpack_paks(&root_path, &logger) {
                logger.log(&format!("[!] Batch Unpack Error: {}", e));
            }
        });
    });

    let logger_batch_pack = logger_base.clone();
    ui.on_batch_pack_pak(move |root_folder, fmt, comp| {
        let logger = logger_batch_pack.clone();
        let root_path = PathBuf::from(root_folder.as_str());
        let fmt_str = fmt.to_string();

        thread::spawn(move || {
            if let Err(e) = pak::batch_pack_folders(&root_path, &fmt_str, comp as u32, &logger) {
                logger.log(&format!("[!] Batch Pack Error: {}", e));
            }
        });
    });

    // ---------------- CFF OPERATIONS & DIFF ENGINE ----------------
    let logger_cff_unpack = logger_base.clone();
    ui.on_unpack_cff(move |input, out| {
        let logger = logger_cff_unpack.clone();
        let in_path = PathBuf::from(input.as_str());
        let out_path = PathBuf::from(out.as_str());

        thread::spawn(move || {
            logger.log(&format!(
                "[*] Processing CFF full unpack & JSON export: {:?}",
                in_path
            ));
            if let Err(e) = cff::unpack_all(&in_path, &out_path, &logger) {
                logger.log(&format!("[!] Error processing CFF: {}", e));
            } else {
                logger.log("[+] CFF Unpack cycle completed successfully.");
            }
        });
    });

    let logger_cff_pack = logger_base.clone();
    ui.on_pack_cff(move |input, out, comp| {
        let logger = logger_cff_pack.clone();
        let in_path = PathBuf::from(input.as_str());
        let out_path = PathBuf::from(out.as_str());

        thread::spawn(move || {
            logger.log(&format!(
                "[*] Compiling JSON texts and packing CFF from: {:?}",
                in_path
            ));
            if let Err(e) = cff::pack_all(&in_path, &out_path, comp as u32, &logger) {
                logger.log(&format!("[!] Error packing CFF: {}", e));
            } else {
                logger.log("[+] CFF Pack cycle completed successfully.");
            }
        });
    });

    let logger_cff_diff = logger_base.clone();
    ui.on_create_cff_diff(move |base_dir, mod_dir, out_patch| {
        let logger = logger_cff_diff.clone();
        let b = PathBuf::from(base_dir.as_str());
        let m = PathBuf::from(mod_dir.as_str());
        let p = PathBuf::from(out_patch.as_str());
        thread::spawn(move || {
            if let Err(e) = cff::create_diff(&b, &m, &p, &logger) {
                logger.log(&format!("[!] Diff Generation Error: {}", e));
            }
        });
    });

    let logger_cff_apply = logger_base.clone();
    ui.on_apply_cff_patch(move |target_dir, patch_file| {
        let logger = logger_cff_apply.clone();
        let t = PathBuf::from(target_dir.as_str());
        let p = PathBuf::from(patch_file.as_str());
        thread::spawn(move || {
            if let Err(e) = cff::apply_patch(&t, &p, &logger) {
                logger.log(&format!("[!] Patch Apply Error: {}", e));
            }
        });
    });

    // ---------------- BALANCE EDITOR CALLBACKS ----------------
    let editor_items_cache = Arc::new(Mutex::new(Vec::<cff::EditorItem>::new()));
    let ui_weak_ed = ui_handle.clone();
    let cache_load = editor_items_cache.clone();

    ui.on_load_editor_data(move |cff_dir, category| {
        let dir = PathBuf::from(cff_dir.as_str());
        let cat = category.to_string();
        let ui_weak = ui_weak_ed.clone();
        let cache = cache_load.clone();

        thread::spawn(move || {
            let filter = ui_weak
                .upgrade()
                .map(|ui| ui.get_editor_filter().to_string())
                .unwrap_or_default();
            let items = cff::load_editor_items(&dir, &cat, &filter);
            *cache.lock().unwrap() = items.clone();

            let list_items: Vec<_> = items
                .into_iter()
                .map(|it| StandardListViewItem::from(SharedString::from(it.display)))
                .collect();

            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                let slint_model = ModelRc::from(Rc::new(VecModel::from(list_items)));
                ui.set_editor_entries(slint_model);
                ui.set_status_msg("Loaded balance records.".into());
            });
        });
    });

    let ui_weak_sel = ui_handle.clone();
    let cache_sel = editor_items_cache.clone();
    ui.on_select_editor_entry(move |idx| {
        let cache = cache_sel.lock().unwrap();
        if let Some(item) = cache.get(idx as usize) {
            let id = item.id_str.clone();
            let val1 = item.val1.clone();
            let val2 = item.val2.clone();
            let _ = ui_weak_sel.upgrade_in_event_loop(move |ui| {
                ui.set_editor_field_id(id.into());
                ui.set_editor_field_val1(val1.into());
                ui.set_editor_field_val2(val2.into());
            });
        }
    });

    let logger_ed_save = logger_base.clone();
    let ui_weak_save = ui_handle.clone();
    ui.on_save_editor_entry(move |cff_dir, category, idx, id_str, val1, val2| {
        let dir = PathBuf::from(cff_dir.as_str());
        let cat = category.to_string();
        let logger = logger_ed_save.clone();
        let ui_weak = ui_weak_save.clone();

        thread::spawn(move || {
            if let Err(e) = cff::save_editor_item(
                &dir,
                &cat,
                idx as usize,
                id_str.as_str(),
                val1.as_str(),
                val2.as_str(),
            ) {
                logger.log(&format!("[!] Editor Save Error: {}", e));
            } else {
                logger.log(&format!(
                    "[+] Successfully updated entry: {}",
                    id_str.as_str()
                ));
                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    ui.set_status_msg("Changes saved to disk.".into());
                });
            }
        });
    });

    let ui_weak_add = ui_handle.clone();
    let cache_add = editor_items_cache.clone();
    let logger_add = logger_base.clone();

    ui.on_add_editor_entry(move |cff_dir, category| {
        let dir = PathBuf::from(cff_dir.as_str());
        let cat = category.to_string();
        let ui_weak = ui_weak_add.clone();
        let cache = cache_add.clone();
        let logger = logger_add.clone();

        thread::spawn(move || match cff::add_editor_item(&dir, &cat) {
            Ok(new_id) => {
                logger.log(&format!("[+] Created new record: {}", new_id));
                let filter = ui_weak
                    .upgrade()
                    .map(|ui| ui.get_editor_filter().to_string())
                    .unwrap_or_default();
                let items = cff::load_editor_items(&dir, &cat, &filter);
                let new_idx = items
                    .iter()
                    .position(|it| it.id_str == new_id)
                    .unwrap_or(items.len().saturating_sub(1));
                *cache.lock().unwrap() = items.clone();

                let list_items: Vec<_> = items
                    .into_iter()
                    .map(|it| StandardListViewItem::from(SharedString::from(it.display)))
                    .collect();

                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let slint_model = ModelRc::from(Rc::new(VecModel::from(list_items)));
                    ui.set_editor_entries(slint_model);
                    ui.set_editor_selected_idx(new_idx as i32);
                    ui.invoke_select_editor_entry(new_idx as i32);
                    ui.set_status_msg("New entry created.".into());
                });
            }
            Err(e) => {
                logger.log(&format!("[!] Error adding record: {}", e));
            }
        });
    });

    let ui_weak_dup = ui_handle.clone();
    let cache_dup = editor_items_cache.clone();
    let logger_dup = logger_base.clone();

    ui.on_duplicate_editor_entry(move |cff_dir, category, idx, id_str| {
        let dir = PathBuf::from(cff_dir.as_str());
        let cat = category.to_string();
        let ui_weak = ui_weak_dup.clone();
        let cache = cache_dup.clone();
        let logger = logger_dup.clone();
        let id_s = id_str.to_string();

        thread::spawn(
            move || match cff::duplicate_editor_item(&dir, &cat, idx as usize, &id_s) {
                Ok(new_id) => {
                    logger.log(&format!("[+] Successfully duplicated record: {}", new_id));
                    let filter = ui_weak
                        .upgrade()
                        .map(|ui| ui.get_editor_filter().to_string())
                        .unwrap_or_default();
                    let items = cff::load_editor_items(&dir, &cat, &filter);
                    let new_idx = items
                        .iter()
                        .position(|it| it.id_str == new_id)
                        .unwrap_or(items.len().saturating_sub(1));
                    *cache.lock().unwrap() = items.clone();

                    let list_items: Vec<_> = items
                        .into_iter()
                        .map(|it| StandardListViewItem::from(SharedString::from(it.display)))
                        .collect();

                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        let slint_model = ModelRc::from(Rc::new(VecModel::from(list_items)));
                        ui.set_editor_entries(slint_model);
                        ui.set_editor_selected_idx(new_idx as i32);
                        ui.invoke_select_editor_entry(new_idx as i32);
                        ui.set_status_msg("Record duplicated.".into());
                    });
                }
                Err(e) => {
                    logger.log(&format!("[!] Error duplicating record: {}", e));
                }
            },
        );
    });

    // ---------------- LUA SCRIPTING CALLBACKS ----------------
    let logger_lua_dec = logger_base.clone();
    ui.on_decompile_lua(move |src, dst, luadec, resume| {
        let logger = logger_lua_dec.clone();
        let src_path = PathBuf::from(src.as_str());
        let dst_path = PathBuf::from(dst.as_str());
        let luadec_path = PathBuf::from(luadec.as_str());

        thread::spawn(move || {
            if let Err(e) =
                lua::batch_decompile(&src_path, &dst_path, &luadec_path, resume, &logger)
            {
                logger.log(&format!("[!] Decompile Error: {}", e));
            }
        });
    });

    let logger_lua_chk = logger_base.clone();
    ui.on_check_lua_syntax(move |src, luac| {
        let logger = logger_lua_chk.clone();
        let src_path = PathBuf::from(src.as_str());
        let luac_path = PathBuf::from(luac.as_str());

        thread::spawn(move || {
            if let Err(e) = lua::batch_check_syntax(&src_path, &luac_path, &logger) {
                logger.log(&format!("[!] Syntax Check Error: {}", e));
            }
        });
    });

    let logger_lua_fmt = logger_base.clone();
    ui.on_format_lua_scripts(move |src, use_tabs| {
        let logger = logger_lua_fmt.clone();
        let src_path = PathBuf::from(src.as_str());
        let indent = if use_tabs { "\t" } else { "  " };

        thread::spawn(move || {
            if let Err(e) = lua::batch_format(&src_path, indent, &logger) {
                logger.log(&format!("[!] Format Error: {}", e));
            }
        });
    });

    ui.run()
}

fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        if let Err(e) = handle_cli(&args) {
            eprintln!("[!] CLI Execution Error: {}", e);
            std::process::exit(1);
        }
        return Ok(());
    }

    run_gui()
}
