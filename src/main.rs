// src/main.rs
slint::include_modules!();

mod cff;
mod cli;
mod dds;
mod gui;
mod logger;
mod lua;
mod pak;
mod tools;

pub use logger::UiLogger;

fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<String> = std::env::args().collect();

    // Если переданы аргументы командной строки — работаем в режиме CLI
    if args.len() > 1 {
        if let Err(e) = cli::handle_cli(&args) {
            eprintln!("[!] CLI Execution Error: {}", e);
            std::process::exit(1);
        }
        return Ok(());
    }

    // Иначе запускаем Slint GUI
    gui::run_gui()
}
