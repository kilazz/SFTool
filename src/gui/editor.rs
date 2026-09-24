// src/gui/editor.rs

use crate::AppWindow;
use crate::cff;
use crate::logger::UiLogger;
use slint::{ComponentHandle, Image, ModelRc, SharedString, StandardListViewItem, VecModel};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

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

struct PreviewTask {
    generation: u64,
    dir: PathBuf,
    asset_src: PathBuf,
    cat: String,
    id: String,
    val1: String,
}

pub fn register_editor_callbacks(ui: &AppWindow, logger: UiLogger) {
    let undo_stack = Arc::new(Mutex::new(Vec::<HistoryItem>::new()));
    let redo_stack = Arc::new(Mutex::new(Vec::<HistoryItem>::new()));
    let items_cache = Arc::new(Mutex::new(Vec::<cff::EditorItem>::new()));

    // Dedicated debounced worker channel for texture and asset previews
    let (preview_tx, preview_rx) = mpsc::channel::<PreviewTask>();
    let preview_generation = Arc::new(AtomicU64::new(0));

    let preview_gen_worker = preview_generation.clone();
    let ui_w_worker = ui.as_weak();

    // Single long-lived worker thread to process texture decoding
    thread::spawn(move || {
        while let Ok(mut task) = preview_rx.recv() {
            // Coalesce rapid requests: if more tasks are queued, skip directly to the latest
            while let Ok(newer) = preview_rx.try_recv() {
                task = newer;
            }

            // Discard task if already superseded by a newer selection
            if task.generation != preview_gen_worker.load(Ordering::SeqCst) {
                continue;
            }

            if task.cat.contains("0x07DC")
                || task.cat.contains("0x2335")
                || task.cat.contains("0x234E")
            {
                let res = cff::find_and_load_texture(&task.dir, &task.asset_src, &task.val1);
                if task.generation == preview_gen_worker.load(Ordering::SeqCst) {
                    let ui_w = ui_w_worker.clone();
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
                        if let Some((rgba, fname)) = res {
                            ui.set_preview_icon1(crate::dds::rgba_to_slint(rgba));
                            ui.set_asset_status_text(format!("Loaded: {}", fname).into());
                        } else {
                            ui.set_preview_icon1(Image::default());
                            ui.set_asset_status_text("Texture not found.".into());
                        }
                    });
                }
            } else if task.cat.contains("0x07E2") {
                let spell_id = task.id.parse::<u16>().unwrap_or(0);
                let scroll_id = task.val1.parse::<u16>().unwrap_or(0);
                let details = cff::resolve_spell_cross_reference(&task.dir, spell_id, scroll_id);

                let spell_rgba =
                    cff::find_and_load_texture(&task.dir, &task.asset_src, &details.spell_mesh)
                        .map(|(rgba, _)| rgba);
                let scroll_rgba =
                    cff::find_and_load_texture(&task.dir, &task.asset_src, &details.scroll_mesh)
                        .map(|(rgba, _)| rgba);

                if task.generation == preview_gen_worker.load(Ordering::SeqCst) {
                    let ui_w = ui_w_worker.clone();
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
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
            } else if task.generation == preview_gen_worker.load(Ordering::SeqCst) {
                let ui_w = ui_w_worker.clone();
                let _ = ui_w.upgrade_in_event_loop(move |ui| {
                    ui.set_preview_icon1(Image::default());
                    ui.set_preview_icon2(Image::default());
                    ui.set_asset_status_text("No visual asset associated.".into());
                });
            }
        }
    });

    // 1. Load data
    let ui_weak = ui.as_weak();
    let cache_load = items_cache.clone();
    ui.on_load_editor_data(move |cff_dir, cat, flt, lang| {
        let dir = PathBuf::from(cff_dir.as_str());
        let c = cat.to_string();
        let f = flt.to_string();
        let l = lang.to_string();
        let ui_w = ui_weak.clone();
        let cache = cache_load.clone();

        thread::spawn(move || {
            let categories = cff::get_available_categories(&dir);
            let items = cff::load_editor_items(&dir, &c, &f, &l);
            let languages = cff::get_available_languages_display(&dir);
            *cache.lock().unwrap() = items.clone();

            let count = items.len();
            let list_items: Vec<_> = items
                .into_iter()
                .map(|it| StandardListViewItem::from(SharedString::from(it.display)))
                .collect();
            let cat_items: Vec<_> = categories.into_iter().map(SharedString::from).collect();
            let lang_items: Vec<_> = languages.into_iter().map(SharedString::from).collect();

            let _ = ui_w.upgrade_in_event_loop(move |ui| {
                ui.set_available_categories(ModelRc::from(Rc::new(VecModel::from(cat_items))));
                ui.set_available_languages(ModelRc::from(Rc::new(VecModel::from(lang_items))));
                ui.set_editor_entries(ModelRc::from(Rc::new(VecModel::from(list_items))));
                ui.set_status_msg(format!("Loaded {} records into editor.", count).into());
            });
        });
    });

    // 2. Select entry (Debounced via generation counter and worker channel)
    let ui_weak_sel = ui.as_weak();
    let cache_sel = items_cache.clone();
    let p_gen_sel = preview_generation.clone();
    let p_tx_sel = preview_tx.clone();

    ui.on_select_editor_entry(move |idx| {
        let cache = cache_sel.lock().unwrap();
        if let Some(item) = cache.get(idx as usize) {
            // Guard against selecting the pagination/informational footer banner
            if item.id_str.is_empty() {
                return;
            }

            let id = item.id_str.clone();
            let val1 = item.val1.clone();
            let val2 = item.val2.clone();
            drop(cache);

            let task_gen = p_gen_sel.fetch_add(1, Ordering::SeqCst) + 1;
            let tx = p_tx_sel.clone();

            let _ = ui_weak_sel.upgrade_in_event_loop(move |ui| {
                ui.set_editor_field_id(id.clone().into());
                ui.set_editor_field_val1(val1.clone().into());
                ui.set_editor_field_val2(val2.into());

                let cat = ui.get_editor_active_category().to_string();
                let dir = PathBuf::from(ui.get_editor_cff_dir().as_str());
                let asset_src = PathBuf::from(ui.get_editor_asset_source().as_str());

                let _ = tx.send(PreviewTask {
                    generation: task_gen,
                    dir,
                    asset_src,
                    cat,
                    id,
                    val1,
                });
            });
        }
    });

    // 3. Save entry
    let u_save = undo_stack.clone();
    let r_save = redo_stack.clone();
    let cache_save = items_cache;
    let log_save = logger.clone();
    let ui_w_save = ui.as_weak();
    ui.on_save_editor_entry(move |cff_dir, cat, idx, id, v1, v2| {
        let dir = PathBuf::from(cff_dir.as_str());
        let c = cat.to_string();
        let log = log_save.clone();
        let ui_w = ui_w_save.clone();
        let (old_v1, old_v2) = cache_save
            .lock()
            .unwrap()
            .get(idx as usize)
            .map(|it| (it.val1.clone(), it.val2.clone()))
            .unwrap_or_default();

        let u_stack = u_save.clone();
        let r_stack = r_save.clone();
        let v1_s = v1.to_string();
        let v2_s = v2.to_string();
        let id_s = id.to_string();

        thread::spawn(move || {
            if let Err(e) = cff::save_editor_item(&dir, &c, idx as usize, &id_s, &v1_s, &v2_s) {
                log.log(&format!("[!] Editor Save Error: {}", e));
            } else {
                log.log(&format!("[+] Saved entry: {}", id_s));
                if old_v1 != v1_s || old_v2 != v2_s {
                    u_stack.lock().unwrap().push(HistoryItem {
                        cff_dir: dir,
                        category: c,
                        idx: idx as usize,
                        id_str: id_s,
                        val1_before: old_v1,
                        val2_before: old_v2,
                        val1_after: v1_s,
                        val2_after: v2_s,
                    });
                    r_stack.lock().unwrap().clear();
                }

                let can_u = !u_stack.lock().unwrap().is_empty();
                let can_r = !r_stack.lock().unwrap().is_empty();
                let _ = ui_w.upgrade_in_event_loop(move |ui| {
                    ui.set_can_undo(can_u);
                    ui.set_can_redo(can_r);
                    ui.set_status_msg("Saved changes to chunk.".into());
                });
            }
        });
    });

    // 4. Undo
    let u_act = undo_stack.clone();
    let r_act = redo_stack.clone();
    let ui_w_undo = ui.as_weak();
    let log_undo = logger.clone();
    ui.on_editor_undo(move || {
        if let Some(item) = u_act.lock().unwrap().pop() {
            let u_s = u_act.clone();
            let r_s = r_act.clone();
            let log = log_undo.clone();
            let ui_w = ui_w_undo.clone();
            thread::spawn(move || {
                if let Ok(()) = cff::save_editor_item(
                    &item.cff_dir,
                    &item.category,
                    item.idx,
                    &item.id_str,
                    &item.val1_before,
                    &item.val2_before,
                ) {
                    log.log(&format!("[*] Undo: {}", item.id_str));
                    r_s.lock().unwrap().push(item.clone());
                    let can_u = !u_s.lock().unwrap().is_empty();
                    let can_r = !r_s.lock().unwrap().is_empty();
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
                        ui.set_can_undo(can_u);
                        ui.set_can_redo(can_r);
                        ui.set_editor_field_val1(item.val1_before.into());
                        ui.set_editor_field_val2(item.val2_before.into());
                    });
                }
            });
        }
    });

    // 5. Redo
    let u_redo = undo_stack;
    let r_redo = redo_stack;
    let ui_w_redo = ui.as_weak();
    let log_redo = logger.clone();
    ui.on_editor_redo(move || {
        if let Some(item) = r_redo.lock().unwrap().pop() {
            let u_s = u_redo.clone();
            let r_s = r_redo.clone();
            let log = log_redo.clone();
            let ui_w = ui_w_redo.clone();
            thread::spawn(move || {
                if let Ok(()) = cff::save_editor_item(
                    &item.cff_dir,
                    &item.category,
                    item.idx,
                    &item.id_str,
                    &item.val1_after,
                    &item.val2_after,
                ) {
                    log.log(&format!("[*] Redo: {}", item.id_str));
                    u_s.lock().unwrap().push(item.clone());
                    let can_u = !u_s.lock().unwrap().is_empty();
                    let can_r = !r_s.lock().unwrap().is_empty();
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
                        ui.set_can_undo(can_u);
                        ui.set_can_redo(can_r);
                        ui.set_editor_field_val1(item.val1_after.into());
                        ui.set_editor_field_val2(item.val2_after.into());
                    });
                }
            });
        }
    });

    // 6. Record Operations: Add, Duplicate, Delete
    let log_add = logger.clone();
    let ui_w_add = ui.as_weak();
    ui.on_add_editor_entry(move |cff_dir, cat| {
        let dir = PathBuf::from(cff_dir.as_str());
        let c = cat.to_string();
        let log = log_add.clone();
        let ui_w = ui_w_add.clone();
        thread::spawn(move || match cff::add_editor_item(&dir, &c) {
            Ok(new_id) => {
                log.log(&format!("[+] Created new entry: {}", new_id));
                let _ = ui_w.upgrade_in_event_loop(move |ui| {
                    ui.set_status_msg("New record created.".into());
                });
            }
            Err(e) => log.log(&format!("[!] Error creating record: {}", e)),
        });
    });

    let log_dup = logger.clone();
    let ui_w_dup = ui.as_weak();
    ui.on_duplicate_editor_entry(move |cff_dir, cat, idx, id| {
        let dir = PathBuf::from(cff_dir.as_str());
        let c = cat.to_string();
        let id_s = id.to_string();
        let log = log_dup.clone();
        let ui_w = ui_w_dup.clone();
        thread::spawn(
            move || match cff::duplicate_editor_item(&dir, &c, idx as usize, &id_s) {
                Ok(new_id) => {
                    log.log(&format!("[+] Duplicated entry: {} -> {}", id_s, new_id));
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
                        ui.set_status_msg("Record duplicated.".into());
                    });
                }
                Err(e) => log.log(&format!("[!] Error duplicating record: {}", e)),
            },
        );
    });

    let log_del = logger.clone();
    let ui_w_del = ui.as_weak();
    ui.on_delete_editor_entry(move |cff_dir, cat, idx, id| {
        let dir = PathBuf::from(cff_dir.as_str());
        let c = cat.to_string();
        let id_s = id.to_string();
        let log = log_del.clone();
        let ui_w = ui_w_del.clone();
        thread::spawn(
            move || match cff::delete_editor_item(&dir, &c, idx as usize, &id_s) {
                Ok(()) => {
                    log.log(&format!("[+] Deleted entry: {}", id_s));
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
                        ui.set_status_msg("Record deleted.".into());
                    });
                }
                Err(e) => log.log(&format!("[!] Error deleting record: {}", e)),
            },
        );
    });

    // 7. Multi-Language Slot Tools
    let log_exp = logger.clone();
    ui.on_export_single_language(move |cff_dir, lang, out_json| {
        let dir = PathBuf::from(cff_dir.as_str());
        let out_p = PathBuf::from(out_json.as_str());
        let l_str = lang.to_string();
        let log = log_exp.clone();
        thread::spawn(move || {
            let _ = cff::export_single_language(&dir, &l_str, &out_p, &log);
        });
    });

    let log_imp = logger.clone();
    ui.on_import_language_to_slot(move |cff_dir, lang, in_json| {
        let dir = PathBuf::from(cff_dir.as_str());
        let in_p = PathBuf::from(in_json.as_str());
        let l_str = lang.to_string();
        let log = log_imp.clone();
        thread::spawn(move || {
            let _ = cff::import_language_to_slot(&dir, &l_str, &in_p, &log);
        });
    });

    // 8. Language Slot Clone Wizard
    let log_wizard = logger;
    let ui_w_wizard = ui.as_weak();
    ui.on_execute_clone_language(move |cff_dir, src, dst, tag, out_json| {
        let dir = PathBuf::from(cff_dir.as_str());
        let out_p = PathBuf::from(out_json.as_str());
        let t_str = tag.to_string();
        let log = log_wizard.clone();
        let ui_w = ui_w_wizard.clone();
        thread::spawn(move || {
            if let Ok(count) =
                cff::clone_and_export_language(&dir, src as u8, dst as u8, &t_str, &out_p, &log)
            {
                log.log(&format!(
                    "[+] Cloned {} phrases to Slot {} [{}]",
                    count, dst, t_str
                ));
                let _ = ui_w.upgrade_in_event_loop(move |ui| {
                    ui.set_status_msg("Language slot cloned successfully.".into());
                });
            }
        });
    });
}
