// src/main.rs

slint::include_modules!();

mod cff;
mod cli;
mod dds;
mod gui;
mod logger;
mod lua;
mod pak;
pub mod sav;
pub mod terrain;
mod tools;

pub use logger::UiLogger;

fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<String> = std::env::args().collect();

    // Run CLI mode if command-line arguments are provided
    if args.len() > 1 {
        if let Err(e) = cli::handle_cli(&args) {
            eprintln!("[!] CLI Execution Error: {}", e);
            std::process::exit(1);
        }
        return Ok(());
    }

    // Otherwise launch the native Slint GUI
    gui::run_gui()
}
