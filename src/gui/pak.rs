// src/gui/pak.rs
use crate::AppWindow;
use crate::logger::UiLogger;
use crate::pak;
use byteorder::{LittleEndian, ReadBytesExt};
use slint::{ComponentHandle, ModelRc, SharedString, StandardListViewItem, VecModel};
use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

/// Inspects .cff files directly to list internal chunk pseudo-paths in the VFS tree
fn list_cff_chunk_paths(cff_path: &Path) -> std::io::Result<Vec<String>> {
    let mut data = Vec::new();
    let mut f = File::open(cff_path)?;
    f.read_to_end(&mut data)?;

    if data.len() < 20 {
        return Ok(Vec::new());
    }

    let is_sf1 = &data[0..4] == b"\x02\xc5r\xdd" || {
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
        h2 == 2 && h3 == 2 && h4 == 1 && h5 == 0
    };

    let mut cursor = Cursor::new(&data);
    cursor.set_position(20);
    let mut files = Vec::new();
    let mut chunk_idx = 0;

    while (cursor.position() as usize) < data.len() {
        if is_sf1 {
            if cursor.position() as usize + 12 > data.len() {
                break;
            }
            let id = cursor.read_u16::<LittleEndian>()? as u32;
            let _occ = cursor.read_i16::<LittleEndian>()?;
            let comp_flag = cursor.read_i16::<LittleEndian>()?;
            let comp_size = cursor.read_i32::<LittleEndian>()?;
            let _c_type = cursor.read_i16::<LittleEndian>()?;

            let req_bytes = if comp_flag == 0 {
                comp_size as usize
            } else {
                4 + (comp_size as usize)
            };
            if comp_size <= 0 || cursor.position() as usize + req_bytes > data.len() {
                break;
            }
            cursor.seek(SeekFrom::Current(req_bytes as i64))?;

            let name = crate::cff::get_sf1_chunk_info(id)
                .map(|i| i.name)
                .unwrap_or("Unknown");
            files.push(format!(
                "chunks/chunk_{:03}_0x{:04X}_{}.dat",
                chunk_idx, id, name
            ));
        } else {
            if cursor.position() as usize + 16 > data.len() {
                break;
            }
            let id = cursor.read_u32::<LittleEndian>()?;
            let _f1 = cursor.read_u16::<LittleEndian>()?;
            let comp_size = cursor.read_u32::<LittleEndian>()? as usize;
            let _f2 = cursor.read_u16::<LittleEndian>()?;
            let _us = cursor.read_u32::<LittleEndian>()?;

            if cursor.position() as usize + comp_size > data.len() {
                break;
            }
            cursor.seek(SeekFrom::Current(comp_size as i64))?;

            let name = crate::cff::container::get_sf2_chunk_name(id).unwrap_or("Unknown");
            files.push(format!(
                "chunks/chunk_{:03}_0x{:04X}_{}.dat",
                chunk_idx, id, name
            ));
        }
        chunk_idx += 1;
    }

    Ok(files)
}

pub fn register_pak_callbacks(ui: &AppWindow, logger: UiLogger) {
    let tree_items_state = Arc::new(Mutex::new(Vec::<pak::TreeItem>::new()));

    // VFS Navigation click
    let tree_items_click = tree_items_state.clone();
    let ui_weak_click = ui.as_weak();
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
                ui.set_archive_files(ModelRc::from(Rc::new(VecModel::from(list_items))));
            });
        }
    });

    // Загрузка VFS дерева файлов при выборе PAK, CFF или рабочей папки
    let tree_items_load = tree_items_state;
    let ui_weak_load = ui.as_weak();
    let log_tree = logger.clone();
    ui.on_load_archive_tree(move |path_str| {
        let p = PathBuf::from(path_str.as_str());
        let tree_store = tree_items_load.clone();
        let ui_w = ui_weak_load.clone();
        let log = log_tree.clone();

        thread::spawn(move || {
            let file_paths = if p.is_file() {
                let ext = p
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "cff" {
                    match list_cff_chunk_paths(&p) {
                        Ok(files) => files,
                        Err(e) => {
                            log.log(&format!("[!] Error scanning CFF container: {}", e));
                            return;
                        }
                    }
                } else {
                    match pak::list_pak_files(&p) {
                        Ok(files) => files,
                        Err(e) => {
                            log.log(&format!("[!] Error reading PAK file table: {}", e));
                            return;
                        }
                    }
                }
            } else if p.is_dir() {
                match pak::list_directory_files(&p) {
                    Ok(files) => files,
                    Err(e) => {
                        log.log(&format!("[!] Error scanning directory: {}", e));
                        return;
                    }
                }
            } else {
                return;
            };

            log.log(&format!(
                "[+] Loaded {} entries into VFS Tree from {:?}",
                file_paths.len(),
                p.file_name().unwrap_or_default()
            ));

            let items = pak::generate_tree_items(&file_paths);
            let visible_nodes = pak::get_visible_tree_nodes(&items);
            *tree_store.lock().unwrap() = items;

            let list_items: Vec<_> = visible_nodes
                .into_iter()
                .map(|t| StandardListViewItem::from(SharedString::from(t)))
                .collect();

            let count_str = format!("VFS Tree: {} items loaded.", file_paths.len());
            let _ = ui_w.upgrade_in_event_loop(move |ui| {
                ui.set_archive_files(ModelRc::from(Rc::new(VecModel::from(list_items))));
                ui.set_status_msg(count_str.into());
            });
        });
    });

    // Unpack PAK
    let log_unpack = logger.clone();
    ui.on_unpack_pak(move |input, out| {
        let log = log_unpack.clone();
        let in_p = PathBuf::from(input.as_str());
        let out_p = PathBuf::from(out.as_str());
        thread::spawn(move || {
            log.log(&format!("[*] Unpacking PAK archive: {:?}", in_p));
            if let Err(e) = pak::unpack_pak(&in_p, &out_p, &log) {
                log.log(&format!("[!] Error unpacking PAK: {}", e));
            } else {
                log.log("[+] PAK unpack finished successfully.");
            }
        });
    });

    // Pack PAK
    let log_pack = logger.clone();
    ui.on_pack_pak(move |src, out, fmt, algo, comp| {
        let log = log_pack.clone();
        let src_p = PathBuf::from(src.as_str());
        let out_p = PathBuf::from(out.as_str());
        let fmt_s = fmt.to_string();
        let algo_s = algo.to_string();
        thread::spawn(move || {
            log.log(&format!("[*] Packing directory into PAK: {:?}", src_p));
            if let Err(e) = pak::pack_pak(&src_p, &out_p, &fmt_s, &algo_s, comp as u32, &log) {
                log.log(&format!("[!] Error packing PAK: {}", e));
            } else {
                log.log("[+] PAK pack finished successfully.");
            }
        });
    });

    // Batch Unpack
    let log_b_unpack = logger.clone();
    ui.on_batch_unpack_pak(move |root| {
        let log = log_b_unpack.clone();
        let p = PathBuf::from(root.as_str());
        thread::spawn(move || {
            let _ = pak::batch_unpack_paks(&p, &log);
        });
    });

    // Batch Pack
    let log_b_pack = logger;
    ui.on_batch_pack_pak(move |root, fmt, algo, comp| {
        let log = log_b_pack.clone();
        let p = PathBuf::from(root.as_str());
        let fmt_s = fmt.to_string();
        let algo_s = algo.to_string();
        thread::spawn(move || {
            let _ = pak::batch_pack_folders(&p, &fmt_s, &algo_s, comp as u32, &log);
        });
    });
}
