// src/logger.rs
use std::sync::mpsc;
use std::thread;

#[derive(Clone)]
pub struct UiLogger {
    sender: mpsc::Sender<String>,
}

impl UiLogger {
    pub fn new(sender: mpsc::Sender<String>) -> Self {
        Self { sender }
    }

    pub fn log(&self, msg: &str) {
        let _ = self.sender.send(format!("{}\n", msg));
    }
}

/// Создает логгер для консольного режима с синхронным выводом в stdout
pub fn make_cli_logger() -> (UiLogger, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<String>();
    let handle = thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            print!("{}", msg);
        }
    });
    (UiLogger::new(tx), handle)
}
