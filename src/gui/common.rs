// src/gui/common.rs
use crate::AppWindow;
use slint::SharedString;

pub fn register_dialog_callbacks(ui: &AppWindow) {
    ui.on_browse_file(|ext| {
        let ext_str = ext.as_str();
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Target File", &[ext_str])
            .pick_file()
        {
            SharedString::from(path.to_string_lossy().into_owned())
        } else {
            SharedString::new()
        }
    });

    ui.on_browse_folder(|| {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            SharedString::from(path.to_string_lossy().into_owned())
        } else {
            SharedString::new()
        }
    });

    ui.on_save_file(|ext| {
        let ext_str = ext.as_str();
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Target File", &[ext_str])
            .save_file()
        {
            SharedString::from(path.to_string_lossy().into_owned())
        } else {
            SharedString::new()
        }
    });
}
