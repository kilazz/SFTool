slint::include_modules!();

mod cff;
mod dds;
mod lua;
mod pak;
mod tools;

use slint::{Image, ModelRc, SharedString, StandardListViewItem, VecModel};
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

  clone_slot <cff_dir> <src_slot: 0-4> <dst_slot: 5>
      Clone an existing language slot into a new slot (e.g. Russian slot 5).

  replace_slot <cff_dir> <target_slot: 0-5> <translation.json>
      Replace all phrases of a language slot with texts from a JSON file.

Lua Scripting Commands:
  decompile_lua <src_dir> <out_dir> [luadec_exe] [--no-resume]
      High-performance parallel Lua 4.0 bytecode decompiler (SF1).

  check_lua <scripts_dir> [luac_exe]
      Parallel syntax validation using 'luac -p' (defaults to luac4.exe).

  check_lua5 <scripts_dir> [luac5_exe]
      Parallel syntax validation using 'luac5.1 -p' (SF2).

  format_lua <scripts_dir> [--spaces <n>]
      Beautify and indent Lua 4.0 scripts to match original Phenomic source.

  format_stylua <scripts_dir> [stylua_exe] [--spaces]
      Batch format SpellForce 2 Lua 5.1 scripts using StyLua.

  create_map <target_dir> <project_name> <map_name>
      Generate standard SpellForce 2 map scaffolding (scripts, dialogs).

  export_emmylua <out_file.lua>
      Export complete SpellForce 2 Lua API definitions for VS Code (EmmyLua).

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
            let (logger, handle) = make_cli_logger();
            cff::unpack_all(Path::new(&args[2]), Path::new(&args[3]), &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "pack_cff" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool pack_cff <in_dir> <out_cff> [compression_level]");
                return Ok(());
            }
            let comp = args.get(4).and_then(|s| s.parse::<u32>().ok()).unwrap_or(6);
            let (logger, handle) = make_cli_logger();
            cff::pack_all(Path::new(&args[2]), Path::new(&args[3]), comp, &logger)?;
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
        "clone_slot" => {
            if args.len() < 5 {
                eprintln!("Usage: SFTool clone_slot <cff_dir> <src_slot> <dst_slot>");
                return Ok(());
            }
            let src = args[3].parse::<u16>().unwrap_or(1);
            let dst = args[4].parse::<u16>().unwrap_or(5);
            let (logger, handle) = make_cli_logger();
            cff::clone_language_slot(Path::new(&args[2]), src, dst, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "replace_slot" => {
            if args.len() < 5 {
                eprintln!("Usage: SFTool replace_slot <cff_dir> <target_slot> <translation.json>");
                return Ok(());
            }
            let slot = args[3].parse::<u16>().unwrap_or(1);
            let (logger, handle) = make_cli_logger();
            cff::replace_slot_from_json(Path::new(&args[2]), slot, Path::new(&args[4]), &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "unpack_pak" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool unpack_pak <pak_file> <out_dir>");
                return Ok(());
            }
            let (logger, handle) = make_cli_logger();
            pak::unpack_pak(Path::new(&args[2]), Path::new(&args[3]), &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "pack_pak" => {
            if args.len() < 4 {
                eprintln!("Usage: SFTool pack_pak <src_dir> <out_pak> [fmt: sf1/sf2] [comp]");
                return Ok(());
            }
            let fmt = args.get(4).map(|s| s.as_str()).unwrap_or("sf1");
            let comp = args.get(5).and_then(|s| s.parse::<u32>().ok()).unwrap_or(6);
            let (logger, handle) = make_cli_logger();
            pak::pack_pak(Path::new(&args[2]), Path::new(&args[3]), fmt, comp, &logger)?;
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
            let luadec = args
                .get(4)
                .map(PathBuf::from)
                .unwrap_or_else(|| tools::find_tool("luadec_32_deb.exe"));
            let resume = !args.iter().any(|a| a == "--no-resume");
            let (logger, handle) = make_cli_logger();
            lua::sf1::batch_decompile(
                Path::new(&args[2]),
                Path::new(&args[3]),
                &luadec,
                resume,
                &logger,
            )?;
            drop(logger);
            let _ = handle.join();
        }
        "check_lua" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool check_lua <scripts_dir> [luac_exe]");
                return Ok(());
            }
            let luac = args
                .get(3)
                .map(PathBuf::from)
                .unwrap_or_else(|| tools::find_tool("luac4.exe"));
            let (logger, handle) = make_cli_logger();
            lua::sf1::batch_check_syntax(Path::new(&args[2]), &luac, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "check_lua5" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool check_lua5 <scripts_dir> [luac5_exe]");
                return Ok(());
            }
            let luac = args
                .get(3)
                .map(PathBuf::from)
                .unwrap_or_else(|| tools::find_tool("luac5.1.exe"));
            let (logger, handle) = make_cli_logger();
            let _ = lua::sf2::batch_check_syntax_sf2(Path::new(&args[2]), &luac, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "format_lua" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool format_lua <scripts_dir> [--spaces <n>]");
                return Ok(());
            }
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
            lua::sf1::batch_format(Path::new(&args[2]), &unit, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "format_stylua" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool format_stylua <scripts_dir> [stylua_exe] [--spaces]");
                return Ok(());
            }
            let stylua = args
                .get(3)
                .map(PathBuf::from)
                .unwrap_or_else(|| tools::find_tool("stylua.exe"));
            let use_tabs = !args.iter().any(|a| a == "--spaces");
            let (logger, handle) = make_cli_logger();
            lua::sf2::batch_format_stylua(Path::new(&args[2]), &stylua, use_tabs, &logger)?;
            drop(logger);
            let _ = handle.join();
        }
        "create_map" => {
            if args.len() < 5 {
                eprintln!("Usage: SFTool create_map <target_dir> <project_name> <map_name>");
                return Ok(());
            }
            let path = lua::sf2::create_map_scaffolding(
                Path::new(&args[2]),
                args[3].as_str(),
                args[4].as_str(),
            )?;
            println!("[+] Map project created at: {:?}", path);
        }
        "export_emmylua" => {
            if args.len() < 3 {
                eprintln!("Usage: SFTool export_emmylua <out_file.lua>");
                return Ok(());
            }
            let count = lua::sf2_api::export_emmylua_definitions(Path::new(&args[2]))?;
            println!(
                "[+] Exported {} EmmyLua definitions to {:?}",
                count, args[2]
            );
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

    let (default_luadec, default_luac4, _, _) = tools::get_default_toolpaths();
    ui.set_luadec_path(default_luadec.to_string_lossy().into_owned().into());
    ui.set_luac_path(default_luac4.to_string_lossy().into_owned().into());

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
            .add_filter("Archive/Database/Script", &[ext_str])
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
            .add_filter("Archive/Database/Script", &[ext_str])
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

    // ---------------- BALANCE EDITOR & UNDO / REDO ----------------
    #[derive(Clone, Debug)]
    struct HistoryItem {
        cff_dir: PathBuf,
        category: String,
        idx: usize,
        id_str: String,
        val1_before: String,
        val2_before: String,
        val1_after: String,
        val2_after: String,
    }

    let undo_stack = Arc::new(Mutex::new(Vec::<HistoryItem>::new()));
    let redo_stack = Arc::new(Mutex::new(Vec::<HistoryItem>::new()));

    let editor_items_cache = Arc::new(Mutex::new(Vec::<cff::EditorItem>::new()));
    let ui_weak_ed = ui_handle.clone();
    let cache_load = editor_items_cache.clone();

    ui.on_load_editor_data(move |cff_dir, category, filter, lang_filter| {
        let dir = PathBuf::from(cff_dir.as_str());
        let cat = category.to_string();
        let flt = filter.to_string();
        let lang = lang_filter.to_string();
        let ui_weak = ui_weak_ed.clone();
        let cache = cache_load.clone();

        thread::spawn(move || {
            let detected_cats = cff::get_available_categories(&dir);
            let items = cff::load_editor_items(&dir, &cat, &flt, &lang);
            let available_langs = cff::get_available_languages_display(&dir);
            *cache.lock().unwrap() = items.clone();

            let list_items: Vec<_> = items
                .into_iter()
                .map(|it| StandardListViewItem::from(SharedString::from(it.display)))
                .collect();

            let lang_items: Vec<_> = available_langs
                .into_iter()
                .map(SharedString::from)
                .collect();

            let cat_items: Vec<_> = detected_cats.into_iter().map(SharedString::from).collect();

            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                ui.set_available_categories(ModelRc::from(Rc::new(VecModel::from(cat_items))));
                ui.set_available_languages(ModelRc::from(Rc::new(VecModel::from(lang_items))));
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
            let ui_weak = ui_weak_sel.clone();

            let _ = ui_weak_sel.upgrade_in_event_loop(move |ui| {
                ui.set_editor_field_id(id.clone().into());
                ui.set_editor_field_val1(val1.clone().into());
                ui.set_editor_field_val2(val2.into());

                let cat = ui.get_editor_active_category();
                let dir = PathBuf::from(ui.get_editor_cff_dir().as_str());
                let asset_src = PathBuf::from(ui.get_editor_asset_source().as_str());

                thread::spawn(move || {
                    if cat.contains("0x07DC") || cat.contains("0x2335") || cat.contains("0x234E") {
                        if let Some((rgba, fname)) =
                            cff::find_and_load_texture(&dir, &asset_src, &val1)
                        {
                            let status = format!("Loaded: {}", fname);
                            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                                ui.set_preview_icon1(crate::dds::rgba_to_slint(rgba));
                                ui.set_asset_status_text(status.into());
                            });
                        } else {
                            let status = format!("Texture asset '{}' not found.", val1);
                            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                                ui.set_preview_icon1(Image::default());
                                ui.set_asset_status_text(status.into());
                            });
                        }
                    } else if cat.contains("0x07E2") {
                        let spell_id = id.parse::<u16>().unwrap_or(0);
                        let scroll_id = val1.parse::<u16>().unwrap_or(0);
                        let details = cff::resolve_spell_cross_reference(&dir, spell_id, scroll_id);

                        let spell_rgba =
                            cff::find_and_load_texture(&dir, &asset_src, &details.spell_mesh)
                                .map(|(rgba, _)| rgba);

                        let scroll_rgba =
                            cff::find_and_load_texture(&dir, &asset_src, &details.scroll_mesh)
                                .map(|(rgba, _)| rgba);

                        let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                            let spell_img = spell_rgba
                                .map(crate::dds::rgba_to_slint)
                                .unwrap_or_default();
                            let scroll_img = scroll_rgba
                                .map(crate::dds::rgba_to_slint)
                                .unwrap_or_default();

                            ui.set_preview_icon1(spell_img);
                            ui.set_preview_icon2(scroll_img);
                            ui.set_spell_name_disp(details.spell_name.into());
                            ui.set_scroll_name_disp(details.scroll_name.into());
                        });
                    }
                });
            });
        }
    });

    let undo_save = undo_stack.clone();
    let redo_save = redo_stack.clone();
    let cache_save = editor_items_cache.clone();
    let logger_ed_save = logger_base.clone();
    let ui_weak_save = ui_handle.clone();

    ui.on_save_editor_entry(move |cff_dir, category, idx, id_str, val1, val2| {
        let dir = PathBuf::from(cff_dir.as_str());
        let cat = category.to_string();
        let logger = logger_ed_save.clone();
        let ui_weak = ui_weak_save.clone();

        let (old_v1, old_v2) = {
            let cache = cache_save.lock().unwrap();
            if let Some(item) = cache.get(idx as usize) {
                (item.val1.clone(), item.val2.clone())
            } else {
                (String::new(), String::new())
            }
        };

        let u_stack = undo_save.clone();
        let r_stack = redo_save.clone();
        let v1_str = val1.to_string();
        let v2_str = val2.to_string();
        let id_s = id_str.to_string();

        thread::spawn(move || {
            if let Err(e) =
                cff::save_editor_item(&dir, &cat, idx as usize, id_s.as_str(), &v1_str, &v2_str)
            {
                logger.log(&format!("[!] Editor Save Error: {}", e));
            } else {
                logger.log(&format!("[+] Successfully updated entry: {}", id_s));

                if old_v1 != v1_str || old_v2 != v2_str {
                    let mut u = u_stack.lock().unwrap();
                    u.push(HistoryItem {
                        cff_dir: dir,
                        category: cat,
                        idx: idx as usize,
                        id_str: id_s,
                        val1_before: old_v1,
                        val2_before: old_v2,
                        val1_after: v1_str,
                        val2_after: v2_str,
                    });
                    r_stack.lock().unwrap().clear();
                }

                let can_u = !u_stack.lock().unwrap().is_empty();
                let can_r = !r_stack.lock().unwrap().is_empty();

                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    ui.set_can_undo(can_u);
                    ui.set_can_redo(can_r);
                    ui.set_status_msg("Changes saved to disk.".into());
                });
            }
        });
    });

    let undo_act = undo_stack.clone();
    let redo_act = redo_stack.clone();
    let ui_weak_undo = ui_handle.clone();
    let logger_undo = logger_base.clone();

    ui.on_editor_undo(move || {
        let action = undo_act.lock().unwrap().pop();
        if let Some(item) = action {
            let u_stack = undo_act.clone();
            let r_stack = redo_act.clone();
            let logger = logger_undo.clone();
            let ui_weak = ui_weak_undo.clone();

            thread::spawn(move || {
                if let Err(e) = cff::save_editor_item(
                    &item.cff_dir,
                    &item.category,
                    item.idx,
                    &item.id_str,
                    &item.val1_before,
                    &item.val2_before,
                ) {
                    logger.log(&format!("[!] Undo Error: {}", e));
                } else {
                    logger.log(&format!("[*] Reverted (Undo): {}", item.id_str));
                    r_stack.lock().unwrap().push(item.clone());

                    let can_u = !u_stack.lock().unwrap().is_empty();
                    let can_r = !r_stack.lock().unwrap().is_empty();

                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        ui.set_can_undo(can_u);
                        ui.set_can_redo(can_r);
                        ui.set_editor_field_val1(item.val1_before.into());
                        ui.set_editor_field_val2(item.val2_before.into());
                        let cff_p = ui.get_editor_cff_dir();
                        let cat_p = ui.get_editor_active_category();
                        let flt_p = ui.get_editor_filter();
                        let lang_p = ui.get_editor_lang_filter();
                        ui.invoke_load_editor_data(cff_p, cat_p, flt_p, lang_p);
                        ui.set_status_msg("Reverted to previous state.".into());
                    });
                }
            });
        }
    });

    let undo_redo = undo_stack.clone();
    let redo_redo = redo_stack.clone();
    let ui_weak_redo = ui_handle.clone();
    let logger_redo = logger_base.clone();

    ui.on_editor_redo(move || {
        let action = redo_redo.lock().unwrap().pop();
        if let Some(item) = action {
            let u_stack = undo_redo.clone();
            let r_stack = redo_redo.clone();
            let logger = logger_redo.clone();
            let ui_weak = ui_weak_redo.clone();

            thread::spawn(move || {
                if let Err(e) = cff::save_editor_item(
                    &item.cff_dir,
                    &item.category,
                    item.idx,
                    &item.id_str,
                    &item.val1_after,
                    &item.val2_after,
                ) {
                    logger.log(&format!("[!] Redo Error: {}", e));
                } else {
                    logger.log(&format!("[*] Reapplied (Redo): {}", item.id_str));
                    u_stack.lock().unwrap().push(item.clone());

                    let can_u = !u_stack.lock().unwrap().is_empty();
                    let can_r = !r_stack.lock().unwrap().is_empty();

                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        ui.set_can_undo(can_u);
                        ui.set_can_redo(can_r);
                        ui.set_editor_field_val1(item.val1_after.into());
                        ui.set_editor_field_val2(item.val2_after.into());
                        let cff_p = ui.get_editor_cff_dir();
                        let cat_p = ui.get_editor_active_category();
                        let flt_p = ui.get_editor_filter();
                        let lang_p = ui.get_editor_lang_filter();
                        ui.invoke_load_editor_data(cff_p, cat_p, flt_p, lang_p);
                        ui.set_status_msg("Reapplied undone change.".into());
                    });
                }
            });
        }
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
                let lang_filter = ui_weak
                    .upgrade()
                    .map(|ui| ui.get_editor_lang_filter().to_string())
                    .unwrap_or_default();
                let items = cff::load_editor_items(&dir, &cat, &filter, &lang_filter);
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
                    let lang_filter = ui_weak
                        .upgrade()
                        .map(|ui| ui.get_editor_lang_filter().to_string())
                        .unwrap_or_default();
                    let items = cff::load_editor_items(&dir, &cat, &filter, &lang_filter);
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

    let ui_weak_del = ui_handle.clone();
    let cache_del = editor_items_cache.clone();
    let logger_del = logger_base.clone();

    ui.on_delete_editor_entry(move |cff_dir, category, idx, id_str| {
        let dir = PathBuf::from(cff_dir.as_str());
        let cat = category.to_string();
        let ui_weak = ui_weak_del.clone();
        let cache = cache_del.clone();
        let logger = logger_del.clone();
        let id_s = id_str.to_string();

        thread::spawn(
            move || match cff::delete_editor_item(&dir, &cat, idx as usize, &id_s) {
                Ok(()) => {
                    logger.log(&format!("[-] Successfully deleted record: {}", id_s));
                    let filter = ui_weak
                        .upgrade()
                        .map(|ui| ui.get_editor_filter().to_string())
                        .unwrap_or_default();
                    let lang_filter = ui_weak
                        .upgrade()
                        .map(|ui| ui.get_editor_lang_filter().to_string())
                        .unwrap_or_default();
                    let items = cff::load_editor_items(&dir, &cat, &filter, &lang_filter);
                    let total = items.len();
                    *cache.lock().unwrap() = items.clone();

                    let list_items: Vec<_> = items
                        .into_iter()
                        .map(|it| StandardListViewItem::from(SharedString::from(it.display)))
                        .collect();

                    let next_idx = if total == 0 {
                        -1
                    } else if idx as usize >= total {
                        (total - 1) as i32
                    } else {
                        idx
                    };

                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        let slint_model = ModelRc::from(Rc::new(VecModel::from(list_items)));
                        ui.set_editor_entries(slint_model);
                        ui.set_editor_selected_idx(next_idx);
                        if next_idx >= 0 {
                            ui.invoke_select_editor_entry(next_idx);
                        } else {
                            ui.set_editor_field_id("".into());
                            ui.set_editor_field_val1("".into());
                            ui.set_editor_field_val2("".into());
                        }
                        ui.set_status_msg("Record deleted.".into());
                    });
                }
                Err(e) => {
                    logger.log(&format!("[!] Error deleting record: {}", e));
                }
            },
        );
    });

    let logger_exp_lang = logger_base.clone();
    let ui_weak_exp = ui_handle.clone();
    ui.on_export_single_language(move |cff_dir, lang, out_f| {
        let dir = PathBuf::from(cff_dir.as_str());
        let l = lang.to_string();
        let out = PathBuf::from(out_f.as_str());
        let logger = logger_exp_lang.clone();
        let ui_weak = ui_weak_exp.clone();

        thread::spawn(move || {
            if let Err(e) = cff::export_single_language(&dir, &l, &out, &logger) {
                logger.log(&format!("[!] Export Error: {}", e));
            } else {
                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    ui.set_status_msg("Exported clean language JSON.".into());
                });
            }
        });
    });

    let logger_imp_lang = logger_base.clone();
    let ui_weak_imp = ui_handle.clone();
    ui.on_import_language_to_slot(move |cff_dir, lang, in_f| {
        let dir = PathBuf::from(cff_dir.as_str());
        let l = lang.to_string();
        let in_file = PathBuf::from(in_f.as_str());
        let logger = logger_imp_lang.clone();
        let ui_weak = ui_weak_imp.clone();

        thread::spawn(move || {
            if let Err(e) = cff::import_language_to_slot(&dir, &l, &in_file, &logger) {
                logger.log(&format!("[!] Import Error: {}", e));
            } else {
                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    let cff_p = ui.get_editor_cff_dir();
                    let cat_p = ui.get_editor_active_category();
                    let flt_p = ui.get_editor_filter();
                    let lang_p = ui.get_editor_lang_filter();
                    ui.invoke_load_editor_data(cff_p, cat_p, flt_p, lang_p);
                    ui.set_status_msg("Imported translations to slot.".into());
                });
            }
        });
    });

    let logger_clone_exec = logger_base.clone();
    let ui_weak_clone_exec = ui_handle.clone();
    ui.on_execute_clone_language(move |cff_dir, src_slot, dst_slot, tag, out_json| {
        let dir = PathBuf::from(cff_dir.as_str());
        let out_p = PathBuf::from(out_json.as_str());
        let t_str = tag.to_string();
        let logger = logger_clone_exec.clone();
        let ui_weak = ui_weak_clone_exec.clone();

        thread::spawn(move || {
            match cff::clone_and_export_language(
                &dir,
                src_slot as u8,
                dst_slot as u8,
                &t_str,
                &out_p,
                &logger,
            ) {
                Ok(count) => {
                    logger.log(&format!(
                        "[+] Done! Cloned {} phrases to Slot {} [{}]",
                        count, dst_slot, t_str
                    ));
                    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                        let cff_p = ui.get_editor_cff_dir();
                        let cat_p = ui.get_editor_active_category();
                        let flt_p = ui.get_editor_filter();
                        let new_filter = format!("Slot {}: Custom [{}]", dst_slot, t_str);
                        ui.set_editor_lang_filter(new_filter.clone().into());
                        ui.invoke_load_editor_data(cff_p, cat_p, flt_p, new_filter.into());
                        ui.set_status_msg("Cloned language and exported JSON.".into());
                    });
                }
                Err(e) => {
                    logger.log(&format!("[!] Clone Wizard Error: {}", e));
                }
            }
        });
    });

    // ---------------- LUA SCRIPTING & TOOLS ----------------
    let logger_lua_dec = logger_base.clone();
    ui.on_decompile_lua(move |src, dst, luadec, resume| {
        let logger = logger_lua_dec.clone();
        let src_path = PathBuf::from(src.as_str());
        let dst_path = PathBuf::from(dst.as_str());
        let luadec_path = PathBuf::from(luadec.as_str());

        thread::spawn(move || {
            if let Err(e) =
                lua::sf1::batch_decompile(&src_path, &dst_path, &luadec_path, resume, &logger)
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
            if let Err(e) = lua::sf1::batch_check_syntax(&src_path, &luac_path, &logger) {
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
            if let Err(e) = lua::sf1::batch_format(&src_path, indent, &logger) {
                logger.log(&format!("[!] Format Error: {}", e));
            }
        });
    });

    let logger_lua_sf2_chk = logger_base.clone();
    ui.on_check_lua_syntax_sf2(move |src, luac| {
        let logger = logger_lua_sf2_chk.clone();
        let src_path = PathBuf::from(src.as_str());
        let luac_path = tools::find_tool(luac.as_str());

        thread::spawn(move || {
            let _ = lua::sf2::batch_check_syntax_sf2(&src_path, &luac_path, &logger);
        });
    });

    let logger_lua_stylua = logger_base.clone();
    ui.on_format_lua_stylua(move |src, stylua, use_tabs| {
        let logger = logger_lua_stylua.clone();
        let src_path = PathBuf::from(src.as_str());
        let stylua_path = tools::find_tool(stylua.as_str());

        thread::spawn(move || {
            if let Err(e) =
                lua::sf2::batch_format_stylua(&src_path, &stylua_path, use_tabs, &logger)
            {
                logger.log(&format!("[!] StyLua Error: {}", e));
            }
        });
    });

    // ---------------- LUA SF2 API REFERENCE & SCAFFOLDING ----------------
    let current_api_matches = Arc::new(Mutex::new(Vec::<&'static lua::LuaApiItem>::new()));

    let update_api_view = {
        let ui_weak = ui_handle.clone();
        let matches_cache = current_api_matches.clone();
        move |query: &str, category: &str| {
            let filtered = lua::sf2_api::search_api(query, category);
            *matches_cache.lock().unwrap() = filtered.clone();

            let list_items: Vec<_> = filtered
                .iter()
                .map(|item| StandardListViewItem::from(SharedString::from(item.name)))
                .collect();

            let first_name = filtered.first().map(|i| i.name).unwrap_or("");
            let first_snippet = filtered.first().map(|i| i.snippet).unwrap_or("");
            let first_desc = filtered.first().map(|i| i.description).unwrap_or("");

            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                ui.set_api_list(ModelRc::from(Rc::new(VecModel::from(list_items))));
                ui.set_selected_fn_name(first_name.into());
                ui.set_selected_fn_snippet(first_snippet.into());
                ui.set_selected_fn_desc(first_desc.into());
            });
        }
    };

    update_api_view("", "All Categories");

    let ui_weak_search = ui_handle.clone();
    let matches_search = current_api_matches.clone();
    ui.on_search_api_query(move |q, c| {
        let filtered = lua::sf2_api::search_api(q.as_str(), c.as_str());
        *matches_search.lock().unwrap() = filtered.clone();

        let list_items: Vec<_> = filtered
            .iter()
            .map(|item| StandardListViewItem::from(SharedString::from(item.name)))
            .collect();

        let _ = ui_weak_search.upgrade_in_event_loop(move |ui| {
            ui.set_api_list(ModelRc::from(Rc::new(VecModel::from(list_items))));
        });
    });

    let ui_weak_sel_api = ui_handle.clone();
    let matches_sel = current_api_matches.clone();
    ui.on_select_api_entry(move |idx| {
        let lock = matches_sel.lock().unwrap();
        if let Some(item) = lock.get(idx as usize) {
            let name = item.name.to_string();
            let snippet = item.snippet.to_string();
            let desc = item.description.to_string();
            let _ = ui_weak_sel_api.upgrade_in_event_loop(move |ui| {
                ui.set_selected_fn_name(name.into());
                ui.set_selected_fn_snippet(snippet.into());
                ui.set_selected_fn_desc(desc.into());
            });
        }
    });

    let logger_copy = logger_base.clone();
    ui.on_copy_snippet_to_clipboard(move |text| {
        let logger = logger_copy.clone();
        let t = text.to_string();
        thread::spawn(move || {
            if let Ok(mut clipboard) = arboard::Clipboard::new()
                && clipboard.set_text(t).is_ok()
            {
                logger.log("[+] Snippet copied to system clipboard!");
            }
        });
    });

    let logger_scaffold = logger_base.clone();
    ui.on_create_map_project(move |dest, proj, map| {
        let logger = logger_scaffold.clone();
        let d = PathBuf::from(dest.as_str());
        let p = proj.to_string();
        let m = map.to_string();
        thread::spawn(move || match lua::sf2::create_map_scaffolding(&d, &p, &m) {
            Ok(path) => {
                logger.log(&format!(
                    "[+] Successfully created map scaffold at: {:?}",
                    path
                ));
            }
            Err(e) => {
                logger.log(&format!("[!] Error creating map scaffold: {}", e));
            }
        });
    });

    let logger_emmy = logger_base.clone();
    ui.on_export_emmylua_defs(move |out_path| {
        let logger = logger_emmy.clone();
        let p = PathBuf::from(out_path.as_str());
        thread::spawn(move || match lua::sf2_api::export_emmylua_definitions(&p) {
            Ok(count) => {
                logger.log(&format!(
                    "[+] Exported {} EmmyLua definitions to {:?}",
                    count, p
                ));
            }
            Err(e) => {
                logger.log(&format!("[!] Error exporting EmmyLua definitions: {}", e));
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
