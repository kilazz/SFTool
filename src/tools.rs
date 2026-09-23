use std::path::{Path, PathBuf};

/// Searches for an executable in local development paths, relative to the app binary, or PATH.
pub fn find_tool(tool_name: &str) -> PathBuf {
    // Direct path check in ./bin/ (development mode)
    let local_bin = Path::new("bin").join(tool_name);
    if local_bin.is_file() {
        return local_bin;
    }

    // Direct path check in root directory
    let local_root = PathBuf::from(tool_name);
    if local_root.is_file() {
        return local_root;
    }

    // Check relative to current executable (release/installed mode)
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        let exe_bin = exe_dir.join("bin").join(tool_name);
        if exe_bin.is_file() {
            return exe_bin;
        }
        let exe_adjacent = exe_dir.join(tool_name);
        if exe_adjacent.is_file() {
            return exe_adjacent;
        }
    }

    // Fallback to system PATH
    PathBuf::from(tool_name)
}

/// Discovers default binaries for SF1 and SF2 tools located in bin/
pub fn get_default_toolpaths() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    (
        find_tool("luadec_32_deb.exe"),
        find_tool("luac4.exe"),
        find_tool("luac5.1.exe"), // Verified exact filename from your bin/ listing
        find_tool("stylua.exe"),
    )
}
