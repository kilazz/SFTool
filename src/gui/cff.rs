// src/gui/cff.rs
use crate::AppWindow;
use crate::cff;
use crate::logger::UiLogger;
use slint::ComponentHandle;
use std::path::PathBuf;
use std::thread;

pub fn register_cff_callbacks(ui: &AppWindow, logger: UiLogger) {
    let log_unpack = logger.clone();
    let ui_w_unpack = ui.as_weak();
    ui.on_unpack_cff(move |input, out| {
        let log = log_unpack.clone();
        let in_p = PathBuf::from(input.as_str());
        let out_p = PathBuf::from(out.as_str());
        let ui_w = ui_w_unpack.clone();
        thread::spawn(move || {
            log.log(&format!("[*] Processing CFF unpack: {:?}", in_p));
            if let Err(e) = cff::unpack_all(&in_p, &out_p, &log) {
                log.log(&format!("[!] CFF Error: {}", e));
            } else {
                log.log("[+] CFF Unpack cycle finished.");
                let out_str = out_p.to_string_lossy().into_owned();
                let _ = ui_w.upgrade_in_event_loop(move |ui| {
                    ui.invoke_load_archive_tree(out_str.into());
                });
            }
        });
    });

    let log_pack = logger.clone();
    ui.on_pack_cff(move |input, out, comp| {
        let log = log_pack.clone();
        let in_p = PathBuf::from(input.as_str());
        let out_p = PathBuf::from(out.as_str());
        thread::spawn(move || {
            log.log(&format!("[*] Packing CFF from: {:?}", in_p));
            if let Err(e) = cff::pack_all(&in_p, &out_p, comp as u32, &log) {
                log.log(&format!("[!] CFF Pack Error: {}", e));
            } else {
                log.log("[+] CFF successfully packed.");
            }
        });
    });

    let log_diff = logger.clone();
    ui.on_create_cff_diff(move |b, m, p| {
        let log = log_diff.clone();
        let bp = PathBuf::from(b.as_str());
        let mp = PathBuf::from(m.as_str());
        let pp = PathBuf::from(p.as_str());
        thread::spawn(move || {
            let _ = cff::create_diff(&bp, &mp, &pp, &log);
        });
    });

    let log_patch = logger;
    ui.on_apply_cff_patch(move |t, p| {
        let log = log_patch.clone();
        let tp = PathBuf::from(t.as_str());
        let pp = PathBuf::from(p.as_str());
        thread::spawn(move || {
            let _ = cff::apply_patch(&tp, &pp, &log);
        });
    });
}
