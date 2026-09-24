// src/lua/mod.rs

pub mod sf1;
pub mod sf2;
pub mod sf2_api;
pub mod sql;

use std::fs::File;
use std::io::Read;
use std::path::Path;

pub use sf2_api::LuaApiItem;
#[allow(unused_imports)]
pub use sql::*;

/// Inspects the header of a file to detect whether it is compiled Lua bytecode
/// and identifies the engine version based on byte offset 4.
pub fn detect_lua_version(file_path: &Path) -> Option<&'static str> {
    if let Ok(mut f) = File::open(file_path) {
        let mut header = [0u8; 5];
        if f.read_exact(&mut header).is_ok() && header.starts_with(b"\x1bLua") {
            return match header[4] {
                0x40 => Some("SpellForce 1 (Lua 4.0)"),
                0x50 => Some("SpellForce 2 (Lua 5.0)"),
                0x51 => Some("SpellForce 2 (Lua 5.1 / Addon)"),
                _ => Some("Unknown Lua Bytecode"),
            };
        }
    }
    None
}

pub fn is_compiled_lua(file_path: &Path) -> bool {
    detect_lua_version(file_path).is_some()
}
