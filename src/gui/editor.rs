// src/gui/editor.rs

use crate::AppWindow;
use crate::cff;
use crate::logger::UiLogger;
use slint::{ComponentHandle, Image, Model, ModelRc, SharedString, StandardListViewItem, VecModel};
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
    record_index: usize, // Absolute zero-based index in the raw binary chunk (.dat)
    saved_fields: Vec<String>,
}

struct PreviewTask {
    generation: u64,
    dir: PathBuf,
    asset_src: PathBuf,
    cat: String,
    id: String,
    val1: String,
    val2: String,
}

pub fn register_editor_callbacks(ui: &AppWindow, logger: UiLogger) {
    let undo_stack = Arc::new(Mutex::new(Vec::<HistoryItem>::new()));
    let redo_stack = Arc::new(Mutex::new(Vec::<HistoryItem>::new()));
    let displayed_items = Arc::new(Mutex::new(Vec::<cff::EditorItem>::new()));

    let (preview_tx, preview_rx) = mpsc::channel::<PreviewTask>();
    let preview_generation = Arc::new(AtomicU64::new(0));

    let preview_gen_worker = preview_generation.clone();
    let ui_w_worker = ui.as_weak();

    // Background thread for texture previews & 2D collision vector rendering
    thread::spawn(move || {
        while let Ok(mut task) = preview_rx.recv() {
            while let Ok(newer) = preview_rx.try_recv() {
                task = newer;
            }

            if task.generation != preview_gen_worker.load(Ordering::SeqCst) {
                continue;
            }

            // 1. Render 2D Vector Collision Footprints
            if task.cat.contains("0x07EE") || task.cat.contains("0x0809") {
                let coords: Vec<(i16, i16)> = task
                    .val2
                    .split(|c: char| c == '|' || c.is_whitespace())
                    .filter_map(|s| {
                        let trimmed = s.trim().trim_matches('(').trim_matches(')');
                        let parts: Vec<&str> = trimmed.split(',').collect();
                        if parts.len() == 2 {
                            let x = parts[0].trim().parse::<i16>().ok()?;
                            let y = parts[1].trim().parse::<i16>().ok()?;
                            Some((x, y))
                        } else {
                            None
                        }
                    })
                    .collect();

                let img = crate::cff::collision_render::render_collision_polygon(&coords, 180, 180);
                if task.generation == preview_gen_worker.load(Ordering::SeqCst) {
                    let ui_w = ui_w_worker.clone();
                    let num_v = coords.len();
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
                        ui.set_preview_icon1(crate::dds::rgba_to_slint(img));
                        ui.set_asset_status_text(
                            format!("Collision Polygon: {} vertices", num_v).into(),
                        );
                    });
                }
            }
            // 2. Decode In-Memory Texture Assets
            else if task.cat.contains("0x07DC")
                || task.cat.contains("0x0806")
                || task.cat.contains("0x07F4")
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
                            ui.set_asset_status_text("Texture asset not found.".into());
                        }
                    });
                }
            }
            // 3. Spells BiMap Cross-Reference (Dual Cards)
            else if task.cat.contains("0x07E2") {
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

    // 1. Load initial editor data with page 0 (Page Size = 250)
    let ui_weak = ui.as_weak();
    let displayed_load = displayed_items.clone();
    let log_load = logger.clone();

    ui.on_load_editor_data(move |cff_dir, cat, flt, lang| {
        let dir = PathBuf::from(cff_dir.as_str());
        let c = cat.to_string();
        let f = flt.to_string();
        let l = lang.to_string();
        let ui_w = ui_weak.clone();
        let displayed = displayed_load.clone();
        let log = log_load.clone();

        thread::spawn(move || {
            let page_size = 250;
            let paged = cff::load_editor_items_paged(&dir, &c, &f, &l, 0, page_size);

            *displayed.lock().unwrap() = paged.items.clone();

            let categories = cff::get_available_categories(&dir);
            let languages = cff::get_available_languages_display(&dir);

            // Automatically construct and populate the Quest Graph in Column 3 when Quests is selected
            if c.contains("0x080D") {
                let ui_w_g = ui_w.clone();
                let dir_g = dir.clone();
                let log_g = log.clone();
                thread::spawn(move || {
                    if let Ok((nodes, edges)) =
                        crate::cff::quests::build_quest_graph_layout(&dir_g, &log_g)
                    {
                        let slint_nodes: Vec<crate::GraphNodeData> = nodes
                            .into_iter()
                            .map(|n| crate::GraphNodeData {
                                id: n.id,
                                title: n.title.into(),
                                subtitle: n.subtitle.into(),
                                is_main: n.is_main,
                                x: n.x,
                                y: n.y,
                                width: n.width,
                                height: n.height,
                                record_index: n.record_index,
                            })
                            .collect();

                        let slint_edges: Vec<crate::GraphEdgeData> = edges
                            .into_iter()
                            .map(|e| crate::GraphEdgeData {
                                from_x: e.from_x,
                                from_y: e.from_y,
                                to_x: e.to_x,
                                to_y: e.to_y,
                            })
                            .collect();

                        let _ = ui_w_g.upgrade_in_event_loop(move |ui| {
                            ui.set_graph_nodes(ModelRc::from(Rc::new(VecModel::from(slint_nodes))));
                            ui.set_graph_edges(ModelRc::from(Rc::new(VecModel::from(slint_edges))));
                        });
                    }
                });
            }

            let list_items: Vec<_> = paged
                .items
                .into_iter()
                .map(|it| StandardListViewItem::from(SharedString::from(it.display)))
                .collect();
            let cat_items: Vec<_> = categories.into_iter().map(SharedString::from).collect();
            let lang_items: Vec<_> = languages.into_iter().map(SharedString::from).collect();

            let total_m = paged.total_matches;
            let cur_p = paged.current_page as i32 + 1;
            let tot_p = paged.total_pages as i32;

            let _ = ui_w.upgrade_in_event_loop(move |ui| {
                ui.set_available_categories(ModelRc::from(Rc::new(VecModel::from(cat_items))));
                ui.set_available_languages(ModelRc::from(Rc::new(VecModel::from(lang_items))));
                ui.set_editor_entries(ModelRc::from(Rc::new(VecModel::from(list_items))));
                ui.set_current_page(cur_p);
                ui.set_total_pages(tot_p);
                ui.set_page_info(
                    format!("Page {} of {} ({} records)", cur_p, tot_p, total_m).into(),
                );
                ui.set_status_msg(format!("Loaded {} matching records.", total_m).into());
            });
        });
    });

    // 2. Pagination Page Switch Callback
    let ui_weak_page = ui.as_weak();
    let displayed_page = displayed_items.clone();
    ui.on_change_page(move |cff_dir, cat, flt, lang, new_page| {
        let dir = PathBuf::from(cff_dir.as_str());
        let c = cat.to_string();
        let f = flt.to_string();
        let l = lang.to_string();
        let target_page = (new_page as usize).saturating_sub(1);
        let ui_w = ui_weak_page.clone();
        let displayed = displayed_page.clone();

        thread::spawn(move || {
            let paged = cff::load_editor_items_paged(&dir, &c, &f, &l, target_page, 250);
            *displayed.lock().unwrap() = paged.items.clone();

            let list_items: Vec<_> = paged
                .items
                .into_iter()
                .map(|it| StandardListViewItem::from(SharedString::from(it.display)))
                .collect();

            let cur_p = paged.current_page as i32 + 1;
            let tot_p = paged.total_pages as i32;
            let total_m = paged.total_matches;

            let _ = ui_w.upgrade_in_event_loop(move |ui| {
                ui.set_editor_entries(ModelRc::from(Rc::new(VecModel::from(list_items))));
                ui.set_current_page(cur_p);
                ui.set_total_pages(tot_p);
                ui.set_page_info(
                    format!("Page {} of {} ({} records)", cur_p, tot_p, total_m).into(),
                );
            });
        });
    });

    // 3. Select entry and populate inspector
    let ui_weak_sel = ui.as_weak();
    let displayed_sel = displayed_items.clone();
    let p_gen_sel = preview_generation.clone();
    let p_tx_sel = preview_tx.clone();

    ui.on_select_editor_entry(move |idx| {
        if idx < 0 {
            return;
        }

        let item_opt = {
            let cache = displayed_sel.lock().unwrap();
            cache.get(idx as usize).cloned().or_else(|| {
                cache
                    .iter()
                    .find(|it| it.record_index == idx as usize)
                    .cloned()
            })
        };

        if let Some(item) = item_opt {
            if item.id_str.is_empty() {
                return;
            }

            let id = item.id_str.clone();
            let val1 = item.val1.clone();
            let val2 = item.val2.clone();
            let p1 = item.p1.clone();
            let p2 = item.p2.clone();
            let p3 = item.p3.clone();
            let p4 = item.p4.clone();
            let p5 = item.p5.clone();
            let p6 = item.p6.clone();
            let p7 = item.p7.clone();
            let p8 = item.p8.clone();
            let p9 = item.p9.clone();
            let p10 = item.p10.clone();
            let p11 = item.p11.clone();
            let p12 = item.p12.clone();
            let l = item.labels.clone();

            let task_gen = p_gen_sel.fetch_add(1, Ordering::SeqCst) + 1;
            let tx = p_tx_sel.clone();

            let _ = ui_weak_sel.upgrade_in_event_loop(move |ui| {
                let node_id = id.parse::<i32>().unwrap_or(-1);
                ui.set_selected_node_id(node_id);

                ui.set_editor_field_id(id.clone().into());
                ui.set_editor_field_val1(val1.clone().into());
                ui.set_editor_field_val2(val2.clone().into());

                ui.set_editor_field_p1(p1.into());
                ui.set_editor_field_p2(p2.into());
                ui.set_editor_field_p3(p3.into());
                ui.set_editor_field_p4(p4.into());
                ui.set_editor_field_p5(p5.into());
                ui.set_editor_field_p6(p6.into());
                ui.set_editor_field_p7(p7.into());
                ui.set_editor_field_p8(p8.into());
                ui.set_editor_field_p9(p9.into());
                ui.set_editor_field_p10(p10.into());
                ui.set_editor_field_p11(p11.into());
                ui.set_editor_field_p12(p12.into());

                ui.set_editor_label_p1(l[0].clone().into());
                ui.set_editor_label_p2(l[1].clone().into());
                ui.set_editor_label_p3(l[2].clone().into());
                ui.set_editor_label_p4(l[3].clone().into());
                ui.set_editor_label_p5(l[4].clone().into());
                ui.set_editor_label_p6(l[5].clone().into());
                ui.set_editor_label_p7(l[6].clone().into());
                ui.set_editor_label_p8(l[7].clone().into());
                ui.set_editor_label_p9(l[8].clone().into());
                ui.set_editor_label_p10(l[9].clone().into());
                ui.set_editor_label_p11(l[10].clone().into());
                ui.set_editor_label_p12(l[11].clone().into());

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
                    val2,
                });
            });
        }
    });

    // 4. Safe entry save using verified absolute record_index
    let u_save = undo_stack.clone();
    let r_save = redo_stack.clone();
    let log_save = logger.clone();
    let ui_w_save = ui.as_weak();
    let displayed_save = displayed_items.clone();

    ui.on_save_editor_entry(move |cff_dir, cat, idx, fields_model| {
        let dir = PathBuf::from(cff_dir.as_str());
        let c = cat.to_string();
        let log = log_save.clone();
        let ui_w = ui_w_save.clone();

        let real_record_index = {
            let lock = displayed_save.lock().unwrap();
            lock.get(idx as usize)
                .map(|item| item.record_index)
                .unwrap_or(idx as usize)
        };

        let fields: Vec<String> = fields_model.iter().map(|s| s.to_string()).collect();
        let u_stack = u_save.clone();
        let r_stack = r_save.clone();

        thread::spawn(move || {
            if let Err(e) = cff::save_editor_item(&dir, &c, real_record_index, &fields) {
                log.log(&format!("[!] Editor Save Error: {}", e));
            } else {
                log.log(&format!(
                    "[+] Successfully saved record (Chunk Index #{}) [ID: {:?}]",
                    real_record_index,
                    fields.first()
                ));
                u_stack.lock().unwrap().push(HistoryItem {
                    cff_dir: dir,
                    category: c,
                    record_index: real_record_index,
                    saved_fields: fields,
                });
                r_stack.lock().unwrap().clear();

                let can_u = !u_stack.lock().unwrap().is_empty();
                let can_r = !r_stack.lock().unwrap().is_empty();
                let _ = ui_w.upgrade_in_event_loop(move |ui| {
                    ui.set_can_undo(can_u);
                    ui.set_can_redo(can_r);
                    ui.set_status_msg("Saved record changes to chunk.".into());
                });
            }
        });
    });

    // 5. Safe Undo using absolute record_index
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
                    item.record_index,
                    &item.saved_fields,
                ) {
                    log.log(&format!(
                        "[*] Reverted changes for Record #{} (Undo).",
                        item.record_index
                    ));
                    r_s.lock().unwrap().push(item.clone());
                    let can_u = !u_s.lock().unwrap().is_empty();
                    let can_r = !r_s.lock().unwrap().is_empty();
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
                        ui.set_can_undo(can_u);
                        ui.set_can_redo(can_r);
                    });
                }
            });
        }
    });

    // 6. Safe Redo using absolute record_index
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
                    item.record_index,
                    &item.saved_fields,
                ) {
                    log.log(&format!(
                        "[*] Reapplied changes for Record #{} (Redo).",
                        item.record_index
                    ));
                    u_s.lock().unwrap().push(item.clone());
                    let can_u = !u_s.lock().unwrap().is_empty();
                    let can_r = !r_s.lock().unwrap().is_empty();
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
                        ui.set_can_undo(can_u);
                        ui.set_can_redo(can_r);
                    });
                }
            });
        }
    });

    // 7. Record Operations: Add, Duplicate, Delete
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
    let displayed_dup = displayed_items.clone();
    ui.on_duplicate_editor_entry(move |cff_dir, cat, idx, id| {
        let dir = PathBuf::from(cff_dir.as_str());
        let c = cat.to_string();
        let id_s = id.to_string();
        let log = log_dup.clone();
        let ui_w = ui_w_dup.clone();

        let real_record_index = {
            let lock = displayed_dup.lock().unwrap();
            lock.get(idx as usize)
                .map(|item| item.record_index)
                .unwrap_or(idx as usize)
        };

        thread::spawn(move || {
            match cff::duplicate_editor_item(&dir, &c, real_record_index, &id_s) {
                Ok(new_id) => {
                    log.log(&format!("[+] Duplicated entry: {} -> {}", id_s, new_id));
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
                        ui.set_status_msg("Record duplicated.".into());
                    });
                }
                Err(e) => log.log(&format!("[!] Error duplicating record: {}", e)),
            }
        });
    });

    let log_del = logger.clone();
    let ui_w_del = ui.as_weak();
    let displayed_del = displayed_items.clone();
    ui.on_delete_editor_entry(move |cff_dir, cat, idx, id| {
        let dir = PathBuf::from(cff_dir.as_str());
        let c = cat.to_string();
        let id_s = id.to_string();
        let log = log_del.clone();
        let ui_w = ui_w_del.clone();

        let real_record_index = {
            let lock = displayed_del.lock().unwrap();
            lock.get(idx as usize)
                .map(|item| item.record_index)
                .unwrap_or(idx as usize)
        };

        thread::spawn(
            move || match cff::delete_editor_item(&dir, &c, real_record_index, &id_s) {
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

    // 8. Cycle-safe Quest Node Graph Loader
    let ui_w_graph = ui.as_weak();
    let log_graph = logger.clone();
    ui.on_load_quest_graph(move |cff_dir| {
        let dir = PathBuf::from(cff_dir.as_str());
        let ui_w = ui_w_graph.clone();
        let log = log_graph.clone();

        thread::spawn(move || {
            if let Ok((nodes, edges)) = crate::cff::quests::build_quest_graph_layout(&dir, &log) {
                let slint_nodes: Vec<crate::GraphNodeData> = nodes
                    .into_iter()
                    .map(|n| crate::GraphNodeData {
                        id: n.id,
                        title: n.title.into(),
                        subtitle: n.subtitle.into(),
                        is_main: n.is_main,
                        x: n.x,
                        y: n.y,
                        width: n.width,
                        height: n.height,
                        record_index: n.record_index,
                    })
                    .collect();

                let slint_edges: Vec<crate::GraphEdgeData> = edges
                    .into_iter()
                    .map(|e| crate::GraphEdgeData {
                        from_x: e.from_x,
                        from_y: e.from_y,
                        to_x: e.to_x,
                        to_y: e.to_y,
                    })
                    .collect();

                let count = slint_nodes.len();
                let _ = ui_w.upgrade_in_event_loop(move |ui| {
                    ui.set_graph_nodes(ModelRc::from(Rc::new(VecModel::from(slint_nodes))));
                    ui.set_graph_edges(ModelRc::from(Rc::new(VecModel::from(slint_edges))));
                    ui.set_status_msg(
                        format!("Generated Cycle-Safe Quest Graph ({} nodes).", count).into(),
                    );
                });
            }
        });
    });

    // 9. Multi-Language Single Language Export
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

    // 10. Multi-Language Single Language Import
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

    // 11. One-Click Language Slot Clone Wizard
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
