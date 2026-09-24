// src/gui/lua.rs
use crate::AppWindow;
use crate::logger::UiLogger;
use crate::lua;
use crate::tools;
use slint::{ComponentHandle, Model, ModelRc, SharedString, StandardListViewItem, VecModel};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

enum ActiveSqlDatabase {
    Items(BTreeMap<u32, lua::sql::SqlItemEntry>),
    Buildings(BTreeMap<u32, lua::sql::SqlBuildingEntry>),
    Objects(BTreeMap<u32, lua::sql::SqlObjectEntry>),
    Heads(BTreeMap<u32, lua::sql::SqlHeadEntry>),
    None,
}

#[derive(Clone, Debug)]
enum SqlSnapshot {
    Item(u32, lua::sql::SqlItemEntry),
    Building(u32, lua::sql::SqlBuildingEntry),
    Object(u32, lua::sql::SqlObjectEntry),
    Head(u32, lua::sql::SqlHeadEntry),
}

#[derive(Clone, Debug)]
struct SqlHistoryAction {
    idx: usize,
    before: SqlSnapshot,
    after: SqlSnapshot,
}

pub fn register_lua_callbacks(ui: &AppWindow, logger: UiLogger) {
    // -------------------------------------------------------------------------
    // 1. SPELLFORCE 1 LUA 4.0 TOOLS
    // -------------------------------------------------------------------------
    let log_dec = logger.clone();
    ui.on_decompile_lua(move |src, dst, luadec, resume| {
        let log = log_dec.clone();
        let s = PathBuf::from(src.as_str());
        let d = PathBuf::from(dst.as_str());
        let exe = PathBuf::from(luadec.as_str());
        thread::spawn(move || {
            let _ = lua::sf1::batch_decompile(&s, &d, &exe, resume, &log);
        });
    });

    let log_chk = logger.clone();
    ui.on_check_lua_syntax(move |src, luac| {
        let log = log_chk.clone();
        let s = PathBuf::from(src.as_str());
        let exe = PathBuf::from(luac.as_str());
        thread::spawn(move || {
            let _ = lua::sf1::batch_check_syntax(&s, &exe, &log);
        });
    });

    let log_fmt = logger.clone();
    ui.on_format_lua_scripts(move |src, use_tabs| {
        let log = log_fmt.clone();
        let s = PathBuf::from(src.as_str());
        let indent = if use_tabs { "\t" } else { "  " };
        thread::spawn(move || {
            let _ = lua::sf1::batch_format(&s, indent, &log);
        });
    });

    // -------------------------------------------------------------------------
    // 2. SPELLFORCE 1 VISUAL ASSET BINDINGS & UNDO/REDO
    // -------------------------------------------------------------------------
    let sql_store = Arc::new(Mutex::new(ActiveSqlDatabase::None));
    let sql_undo_stack = Arc::new(Mutex::new(Vec::<SqlHistoryAction>::new()));
    let sql_redo_stack = Arc::new(Mutex::new(Vec::<SqlHistoryAction>::new()));

    let sql_store_load = sql_store.clone();
    let sql_undo_load = sql_undo_stack.clone();
    let sql_redo_load = sql_redo_stack.clone();
    let ui_w_sql_load = ui.as_weak();
    let log_sql_load = logger.clone();

    ui.on_load_sql_file(move |file_path, file_type| {
        let p = PathBuf::from(file_path.as_str());
        let ftype = file_type.to_string();
        let store = sql_store_load.clone();
        let u_stack = sql_undo_load.clone();
        let r_stack = sql_redo_load.clone();
        let log = log_sql_load.clone();
        let ui_w = ui_w_sql_load.clone();

        thread::spawn(move || {
            log.log(&format!(
                "[*] Loading SQL visual bindings: {:?} ({})",
                p, ftype
            ));

            u_stack.lock().unwrap().clear();
            r_stack.lock().unwrap().clear();

            match ftype.as_str() {
                "sql_item.lua" => match lua::sql::load_sql_items(&p) {
                    Ok(data) => {
                        let count = data.len();
                        let items: Vec<StandardListViewItem> = data
                            .iter()
                            .map(|(id, it)| {
                                StandardListViewItem::from(SharedString::from(format!(
                                    "Item #{:<5} | M: {} | F: {}",
                                    id, it.mesh_male_cold, it.mesh_female_cold
                                )))
                            })
                            .collect();
                        *store.lock().unwrap() = ActiveSqlDatabase::Items(data);
                        log.log(&format!(
                            "[+] Successfully loaded {} visual items from {:?}",
                            count,
                            p.file_name().unwrap_or_default()
                        ));
                        let _ = ui_w.upgrade_in_event_loop(move |ui| {
                            ui.set_sql_entries(ModelRc::from(Rc::new(VecModel::from(items))));
                            ui.set_sql_selected_idx(-1);
                            ui.set_can_sql_undo(false);
                            ui.set_can_sql_redo(false);
                            ui.set_sql_label_val1("Male Cold Mesh:".into());
                            ui.set_sql_label_val2("Female Cold Mesh:".into());
                            ui.set_sql_label_val3("AnimSet:".into());
                            ui.set_sql_label_val4("Selection Size:".into());
                            ui.set_status_msg("Loaded sql_item.lua successfully.".into());
                        });
                    }
                    Err(e) => log.log(&format!("[!] Error parsing sql_item.lua: {}", e)),
                },
                "sql_building.lua" => match lua::sql::load_sql_buildings(&p) {
                    Ok(data) => {
                        let count = data.len();
                        let items: Vec<StandardListViewItem> = data
                            .iter()
                            .map(|(id, b)| {
                                StandardListViewItem::from(SharedString::from(format!(
                                    "Building #{:<5} | Meshes: {}",
                                    id,
                                    b.meshes.join(", ")
                                )))
                            })
                            .collect();
                        *store.lock().unwrap() = ActiveSqlDatabase::Buildings(data);
                        log.log(&format!(
                            "[+] Successfully loaded {} visual buildings from {:?}",
                            count,
                            p.file_name().unwrap_or_default()
                        ));
                        let _ = ui_w.upgrade_in_event_loop(move |ui| {
                            ui.set_sql_entries(ModelRc::from(Rc::new(VecModel::from(items))));
                            ui.set_sql_selected_idx(-1);
                            ui.set_can_sql_undo(false);
                            ui.set_can_sql_redo(false);
                            ui.set_sql_label_val1("Meshes (comma separated):".into());
                            ui.set_sql_label_val2("Selection Scaling:".into());
                            ui.set_sql_label_val3("Unused:".into());
                            ui.set_sql_label_val4("Unused:".into());
                            ui.set_status_msg("Loaded sql_building.lua successfully.".into());
                        });
                    }
                    Err(e) => log.log(&format!("[!] Error parsing sql_building.lua: {}", e)),
                },
                "sql_object.lua" => match lua::sql::load_sql_objects(&p) {
                    Ok(data) => {
                        let count = data.len();
                        let items: Vec<StandardListViewItem> = data
                            .iter()
                            .map(|(id, o)| {
                                StandardListViewItem::from(SharedString::from(format!(
                                    "Object #{:<5} | Name: {} | Mesh: {}",
                                    id,
                                    o.name,
                                    o.meshes.first().cloned().unwrap_or_default()
                                )))
                            })
                            .collect();
                        *store.lock().unwrap() = ActiveSqlDatabase::Objects(data);
                        log.log(&format!(
                            "[+] Successfully loaded {} visual objects from {:?}",
                            count,
                            p.file_name().unwrap_or_default()
                        ));
                        let _ = ui_w.upgrade_in_event_loop(move |ui| {
                            ui.set_sql_entries(ModelRc::from(Rc::new(VecModel::from(items))));
                            ui.set_sql_selected_idx(-1);
                            ui.set_can_sql_undo(false);
                            ui.set_can_sql_redo(false);
                            ui.set_sql_label_val1("Object Name:".into());
                            ui.set_sql_label_val2("Meshes (comma separated):".into());
                            ui.set_sql_label_val3("Scale (Percentage):".into());
                            ui.set_sql_label_val4("Shadow, Billboard (true/false):".into());
                            ui.set_status_msg("Loaded sql_object.lua successfully.".into());
                        });
                    }
                    Err(e) => log.log(&format!("[!] Error parsing sql_object.lua: {}", e)),
                },
                "sql_head.lua" => match lua::sql::load_sql_heads(&p) {
                    Ok(data) => {
                        let count = data.len();
                        let items: Vec<StandardListViewItem> = data
                            .iter()
                            .map(|(id, h)| {
                                StandardListViewItem::from(SharedString::from(format!(
                                    "Head #{:<5} | Male: {} | Female: {}",
                                    id, h.mesh_male, h.mesh_female
                                )))
                            })
                            .collect();
                        *store.lock().unwrap() = ActiveSqlDatabase::Heads(data);
                        log.log(&format!(
                            "[+] Successfully loaded {} visual heads from {:?}",
                            count,
                            p.file_name().unwrap_or_default()
                        ));
                        let _ = ui_w.upgrade_in_event_loop(move |ui| {
                            ui.set_sql_entries(ModelRc::from(Rc::new(VecModel::from(items))));
                            ui.set_sql_selected_idx(-1);
                            ui.set_can_sql_undo(false);
                            ui.set_can_sql_redo(false);
                            ui.set_sql_label_val1("Male Head Mesh:".into());
                            ui.set_sql_label_val2("Female Head Mesh:".into());
                            ui.set_sql_label_val3("Unused:".into());
                            ui.set_sql_label_val4("Unused:".into());
                            ui.set_status_msg("Loaded sql_head.lua successfully.".into());
                        });
                    }
                    Err(e) => log.log(&format!("[!] Error parsing sql_head.lua: {}", e)),
                },
                _ => log.log(&format!("[!] Unknown SQL file type: {}", ftype)),
            }
        });
    });

    let sql_store_sel = sql_store.clone();
    let ui_w_sql_sel = ui.as_weak();
    ui.on_select_sql_entry(move |idx| {
        if idx < 0 {
            return;
        }
        let store = sql_store_sel.lock().unwrap();
        let ui_w = ui_w_sql_sel.clone();

        match &*store {
            ActiveSqlDatabase::Items(map) => {
                if let Some((&id, it)) = map.iter().nth(idx as usize) {
                    let v1 = it.mesh_male_cold.clone();
                    let v2 = it.mesh_female_cold.clone();
                    let v3 = it.anim_set.clone();
                    let v4 = it.selection_size.to_string();
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
                        ui.set_sql_field_id(id.to_string().into());
                        ui.set_sql_field_val1(v1.into());
                        ui.set_sql_field_val2(v2.into());
                        ui.set_sql_field_val3(v3.into());
                        ui.set_sql_field_val4(v4.into());
                    });
                }
            }
            ActiveSqlDatabase::Buildings(map) => {
                if let Some((&id, b)) = map.iter().nth(idx as usize) {
                    let v1 = b.meshes.join(", ");
                    let v2 = b.selection_scaling.to_string();
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
                        ui.set_sql_field_id(id.to_string().into());
                        ui.set_sql_field_val1(v1.into());
                        ui.set_sql_field_val2(v2.into());
                        ui.set_sql_field_val3("".into());
                        ui.set_sql_field_val4("".into());
                    });
                }
            }
            ActiveSqlDatabase::Objects(map) => {
                if let Some((&id, o)) = map.iter().nth(idx as usize) {
                    let v1 = o.name.clone();
                    let v2 = o.meshes.join(", ");
                    let v3 = o.scale.to_string();
                    let v4 = format!("{}, {}", o.shadow, o.billboarded);
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
                        ui.set_sql_field_id(id.to_string().into());
                        ui.set_sql_field_val1(v1.into());
                        ui.set_sql_field_val2(v2.into());
                        ui.set_sql_field_val3(v3.into());
                        ui.set_sql_field_val4(v4.into());
                    });
                }
            }
            ActiveSqlDatabase::Heads(map) => {
                if let Some((&id, h)) = map.iter().nth(idx as usize) {
                    let v1 = h.mesh_male.clone();
                    let v2 = h.mesh_female.clone();
                    let _ = ui_w.upgrade_in_event_loop(move |ui| {
                        ui.set_sql_field_id(id.to_string().into());
                        ui.set_sql_field_val1(v1.into());
                        ui.set_sql_field_val2(v2.into());
                        ui.set_sql_field_val3("".into());
                        ui.set_sql_field_val4("".into());
                    });
                }
            }
            ActiveSqlDatabase::None => {}
        }
    });

    let sql_store_save = sql_store.clone();
    let sql_u_save = sql_undo_stack.clone();
    let sql_r_save = sql_redo_stack.clone();
    let ui_w_sql_save = ui.as_weak();
    let log_sql_save = logger.clone();

    ui.on_save_sql_entry(move |idx, val1, val2, val3, val4| {
        if idx < 0 {
            return;
        }
        let mut store = sql_store_save.lock().unwrap();
        let log = log_sql_save.clone();
        let ui_w = ui_w_sql_save.clone();
        let u_stack = sql_u_save.clone();
        let r_stack = sql_r_save.clone();

        let mut action = None;
        let mut updated_display = String::new();

        match &mut *store {
            ActiveSqlDatabase::Items(map) => {
                if let Some((&id, it)) = map.iter_mut().nth(idx as usize) {
                    let before = SqlSnapshot::Item(id, it.clone());
                    it.mesh_male_cold = val1.to_string();
                    it.mesh_female_cold = val2.to_string();
                    it.anim_set = val3.to_string();
                    if let Ok(sz) = val4.parse::<f64>() {
                        it.selection_size = sz;
                    }
                    let after = SqlSnapshot::Item(id, it.clone());
                    action = Some(SqlHistoryAction {
                        idx: idx as usize,
                        before,
                        after,
                    });
                    updated_display = format!(
                        "Item #{:<5} | M: {} | F: {}",
                        id, it.mesh_male_cold, it.mesh_female_cold
                    );
                    log.log(&format!("[+] Updated in-memory record for Item #{}", id));
                }
            }
            ActiveSqlDatabase::Buildings(map) => {
                if let Some((&id, b)) = map.iter_mut().nth(idx as usize) {
                    let before = SqlSnapshot::Building(id, b.clone());
                    b.meshes = val1
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if let Ok(sc) = val2.parse::<f64>() {
                        b.selection_scaling = sc;
                    }
                    let after = SqlSnapshot::Building(id, b.clone());
                    action = Some(SqlHistoryAction {
                        idx: idx as usize,
                        before,
                        after,
                    });
                    updated_display =
                        format!("Building #{:<5} | Meshes: {}", id, b.meshes.join(", "));
                    log.log(&format!(
                        "[+] Updated in-memory record for Building #{}",
                        id
                    ));
                }
            }
            ActiveSqlDatabase::Objects(map) => {
                if let Some((&id, o)) = map.iter_mut().nth(idx as usize) {
                    let before = SqlSnapshot::Object(id, o.clone());
                    o.name = val1.to_string();
                    o.meshes = val2
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if let Ok(sc) = val3.parse::<f64>() {
                        o.scale = sc;
                    }
                    let after = SqlSnapshot::Object(id, o.clone());
                    action = Some(SqlHistoryAction {
                        idx: idx as usize,
                        before,
                        after,
                    });
                    updated_display = format!(
                        "Object #{:<5} | Name: {} | Mesh: {}",
                        id,
                        o.name,
                        o.meshes.first().cloned().unwrap_or_default()
                    );
                    log.log(&format!("[+] Updated in-memory record for Object #{}", id));
                }
            }
            ActiveSqlDatabase::Heads(map) => {
                if let Some((&id, h)) = map.iter_mut().nth(idx as usize) {
                    let before = SqlSnapshot::Head(id, h.clone());
                    h.mesh_male = val1.to_string();
                    h.mesh_female = val2.to_string();
                    let after = SqlSnapshot::Head(id, h.clone());
                    action = Some(SqlHistoryAction {
                        idx: idx as usize,
                        before,
                        after,
                    });
                    updated_display = format!(
                        "Head #{:<5} | Male: {} | Female: {}",
                        id, h.mesh_male, h.mesh_female
                    );
                    log.log(&format!("[+] Updated in-memory record for Head #{}", id));
                }
            }
            ActiveSqlDatabase::None => {}
        }

        if let Some(act) = action {
            u_stack.lock().unwrap().push(act);
            r_stack.lock().unwrap().clear();

            let can_u = !u_stack.lock().unwrap().is_empty();
            let can_r = !r_stack.lock().unwrap().is_empty();
            let _ = ui_w.upgrade_in_event_loop(move |ui| {
                ui.set_can_sql_undo(can_u);
                ui.set_can_sql_redo(can_r);
                if !updated_display.is_empty() {
                    let mut entries: Vec<StandardListViewItem> =
                        ui.get_sql_entries().iter().collect();
                    if (idx as usize) < entries.len() {
                        entries[idx as usize] =
                            StandardListViewItem::from(SharedString::from(updated_display));
                        ui.set_sql_entries(ModelRc::from(Rc::new(VecModel::from(entries))));
                    }
                }
                ui.set_status_msg("Visual SQL record updated in-memory.".into());
            });
        }
    });

    // SQL Undo Callback
    let sql_store_undo = sql_store.clone();
    let sql_u_undo = sql_undo_stack.clone();
    let sql_r_undo = sql_redo_stack.clone();
    let ui_w_undo = ui.as_weak();
    let log_undo = logger.clone();

    ui.on_sql_undo(move || {
        let mut u_stack = sql_u_undo.lock().unwrap();
        if let Some(act) = u_stack.pop() {
            let mut store = sql_store_undo.lock().unwrap();
            let mut r_stack = sql_r_undo.lock().unwrap();
            let log = log_undo.clone();
            let ui_w = ui_w_undo.clone();

            match &act.before {
                SqlSnapshot::Item(id, item) => {
                    if let ActiveSqlDatabase::Items(map) = &mut *store {
                        map.insert(*id, item.clone());
                    }
                }
                SqlSnapshot::Building(id, building) => {
                    if let ActiveSqlDatabase::Buildings(map) = &mut *store {
                        map.insert(*id, building.clone());
                    }
                }
                SqlSnapshot::Object(id, object) => {
                    if let ActiveSqlDatabase::Objects(map) = &mut *store {
                        map.insert(*id, object.clone());
                    }
                }
                SqlSnapshot::Head(id, head) => {
                    if let ActiveSqlDatabase::Heads(map) = &mut *store {
                        map.insert(*id, head.clone());
                    }
                }
            }

            r_stack.push(act.clone());
            let can_u = !u_stack.is_empty();
            let can_r = !r_stack.is_empty();

            log.log("[*] Reverted visual SQL changes (Undo).");
            let _ = ui_w.upgrade_in_event_loop(move |ui| {
                ui.set_can_sql_undo(can_u);
                ui.set_can_sql_redo(can_r);
                if ui.get_sql_selected_idx() == act.idx as i32 {
                    ui.invoke_select_sql_entry(act.idx as i32);
                }
                ui.set_status_msg("Reverted visual SQL record (Undo).".into());
            });
        }
    });

    // SQL Redo Callback
    let sql_store_redo = sql_store.clone();
    let sql_u_redo = sql_undo_stack.clone();
    let sql_r_redo = sql_redo_stack.clone();
    let ui_w_redo = ui.as_weak();
    let log_redo = logger.clone();

    ui.on_sql_redo(move || {
        let mut r_stack = sql_r_redo.lock().unwrap();
        if let Some(act) = r_stack.pop() {
            let mut store = sql_store_redo.lock().unwrap();
            let mut u_stack = sql_u_redo.lock().unwrap();
            let log = log_redo.clone();
            let ui_w = ui_w_redo.clone();

            match &act.after {
                SqlSnapshot::Item(id, item) => {
                    if let ActiveSqlDatabase::Items(map) = &mut *store {
                        map.insert(*id, item.clone());
                    }
                }
                SqlSnapshot::Building(id, building) => {
                    if let ActiveSqlDatabase::Buildings(map) = &mut *store {
                        map.insert(*id, building.clone());
                    }
                }
                SqlSnapshot::Object(id, object) => {
                    if let ActiveSqlDatabase::Objects(map) = &mut *store {
                        map.insert(*id, object.clone());
                    }
                }
                SqlSnapshot::Head(id, head) => {
                    if let ActiveSqlDatabase::Heads(map) = &mut *store {
                        map.insert(*id, head.clone());
                    }
                }
            }

            u_stack.push(act.clone());
            let can_u = !u_stack.is_empty();
            let can_r = !r_stack.is_empty();

            log.log("[*] Reapplied visual SQL changes (Redo).");
            let _ = ui_w.upgrade_in_event_loop(move |ui| {
                ui.set_can_sql_undo(can_u);
                ui.set_can_sql_redo(can_r);
                if ui.get_sql_selected_idx() == act.idx as i32 {
                    ui.invoke_select_sql_entry(act.idx as i32);
                }
                ui.set_status_msg("Reapplied visual SQL record (Redo).".into());
            });
        }
    });

    let sql_store_write = sql_store;
    let log_sql_write = logger.clone();
    ui.on_write_sql_file(move |file_path, _file_type| {
        let p = PathBuf::from(file_path.as_str());
        let store = sql_store_write.clone();
        let log = log_sql_write.clone();

        thread::spawn(move || {
            let lock = store.lock().unwrap();
            match &*lock {
                ActiveSqlDatabase::Items(map) => {
                    if let Err(e) = lua::sql::save_sql_items(&p, map) {
                        log.log(&format!("[!] Failed to save sql_item.lua: {}", e));
                    } else {
                        log.log(&format!("[+] Saved {} visual items to {:?}", map.len(), p));
                    }
                }
                ActiveSqlDatabase::Buildings(map) => {
                    if let Err(e) = lua::sql::save_sql_buildings(&p, map) {
                        log.log(&format!("[!] Failed to save sql_building.lua: {}", e));
                    } else {
                        log.log(&format!(
                            "[+] Saved {} visual buildings to {:?}",
                            map.len(),
                            p
                        ));
                    }
                }
                ActiveSqlDatabase::Objects(map) => {
                    if let Err(e) = lua::sql::save_sql_objects(&p, map) {
                        log.log(&format!("[!] Failed to save sql_object.lua: {}", e));
                    } else {
                        log.log(&format!(
                            "[+] Saved {} visual objects to {:?}",
                            map.len(),
                            p
                        ));
                    }
                }
                ActiveSqlDatabase::Heads(map) => {
                    if let Err(e) = lua::sql::save_sql_heads(&p, map) {
                        log.log(&format!("[!] Failed to save sql_head.lua: {}", e));
                    } else {
                        log.log(&format!("[+] Saved {} visual heads to {:?}", map.len(), p));
                    }
                }
                ActiveSqlDatabase::None => {
                    log.log("[!] No SQL database loaded in memory to write.");
                }
            }
        });
    });

    // -------------------------------------------------------------------------
    // 3. SPELLFORCE 2 LUA 5.1 & API TOOLS
    // -------------------------------------------------------------------------
    let log_stylua = logger.clone();
    ui.on_format_lua_stylua(move |src, exe, use_tabs| {
        let log = log_stylua.clone();
        let s = PathBuf::from(src.as_str());
        let p = tools::find_tool(exe.as_str());
        thread::spawn(move || {
            let _ = lua::sf2::batch_format_stylua(&s, &p, use_tabs, &log);
        });
    });

    let log_sf2_chk = logger.clone();
    ui.on_check_lua_syntax_sf2(move |src, luac| {
        let log = log_sf2_chk.clone();
        let s = PathBuf::from(src.as_str());
        let exe = tools::find_tool(luac.as_str());
        thread::spawn(move || {
            let _ = lua::sf2::batch_check_syntax_sf2(&s, &exe, &log);
        });
    });

    let api_matches = Arc::new(Mutex::new(Vec::<&'static lua::LuaApiItem>::new()));
    let ui_w_search = ui.as_weak();
    let matches_search = api_matches.clone();
    ui.on_search_api_query(move |q, c| {
        let filtered = lua::sf2_api::search_api(q.as_str(), c.as_str());
        *matches_search.lock().unwrap() = filtered.clone();
        let list_items: Vec<_> = filtered
            .iter()
            .map(|item| StandardListViewItem::from(SharedString::from(item.name)))
            .collect();
        let _ = ui_w_search.upgrade_in_event_loop(move |ui| {
            ui.set_api_list(ModelRc::from(Rc::new(VecModel::from(list_items))));
        });
    });

    let ui_w_sel = ui.as_weak();
    let matches_sel = api_matches;
    ui.on_select_api_entry(move |idx| {
        let lock = matches_sel.lock().unwrap();
        if let Some(item) = lock.get(idx as usize) {
            let name = item.name.to_string();
            let snippet = item.snippet.to_string();
            let desc = item.description.to_string();
            let _ = ui_w_sel.upgrade_in_event_loop(move |ui| {
                ui.set_selected_fn_name(name.into());
                ui.set_selected_fn_snippet(snippet.into());
                ui.set_selected_fn_desc(desc.into());
            });
        }
    });

    let log_copy = logger.clone();
    ui.on_copy_snippet_to_clipboard(move |text| {
        let log = log_copy.clone();
        let t = text.to_string();
        thread::spawn(move || {
            if let Ok(mut cb) = arboard::Clipboard::new()
                && cb.set_text(t).is_ok()
            {
                log.log("[+] Snippet copied to clipboard!");
            }
        });
    });

    let log_scaffold = logger.clone();
    ui.on_create_map_project(move |dest, proj, map| {
        let log = log_scaffold.clone();
        let d = PathBuf::from(dest.as_str());
        let p = proj.to_string();
        let m = map.to_string();
        thread::spawn(move || match lua::sf2::create_map_scaffolding(&d, &p, &m) {
            Ok(path) => log.log(&format!("[+] Created map project scaffold: {:?}", path)),
            Err(e) => log.log(&format!("[!] Error creating scaffold: {}", e)),
        });
    });

    let log_emmy = logger;
    ui.on_export_emmylua_defs(move |out_path| {
        let log = log_emmy.clone();
        let p = PathBuf::from(out_path.as_str());
        thread::spawn(move || match lua::sf2_api::export_emmylua_definitions(&p) {
            Ok(count) => log.log(&format!(
                "[+] Exported {} EmmyLua definitions to {:?}",
                count, p
            )),
            Err(e) => log.log(&format!("[!] Error exporting EmmyLua definitions: {}", e)),
        });
    });
}
