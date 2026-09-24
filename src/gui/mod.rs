// src/gui/mod.rs
pub mod cff;
pub mod common;
pub mod editor;
pub mod lua;
pub mod pak;

use crate::AppWindow;
use crate::logger::UiLogger;
use crate::tools;
use slint::ComponentHandle;
use std::collections::VecDeque;
use std::sync::mpsc;
use std::thread;

pub fn run_gui() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let ui_handle = ui.as_weak();

    ui.set_log_text("System Ready.\n".into());
    ui.set_status_msg("Ready.".into());

    let (default_luadec, default_luac4, _, _) = tools::get_default_toolpaths();
    ui.set_luadec_path(default_luadec.to_string_lossy().into_owned().into());
    ui.set_luac_path(default_luac4.to_string_lossy().into_owned().into());

    // Logging thread channel
    let (log_tx, log_rx) = mpsc::channel::<String>();
    let logger = UiLogger::new(log_tx);

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

    // Callback registrations
    common::register_dialog_callbacks(&ui);
    pak::register_pak_callbacks(&ui, logger.clone());
    cff::register_cff_callbacks(&ui, logger.clone());
    editor::register_editor_callbacks(&ui, logger.clone());
    lua::register_lua_callbacks(&ui, logger);

    ui.run()
}
