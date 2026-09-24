use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Searches for an executable in local development paths, relative to the app binary, or PATH.
pub fn find_tool(tool_name: &str) -> PathBuf {
    let local_bin = Path::new("bin").join(tool_name);
    if local_bin.is_file() {
        return local_bin;
    }

    let local_root = PathBuf::from(tool_name);
    if local_root.is_file() {
        return local_root;
    }

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

    PathBuf::from(tool_name)
}

pub fn get_default_toolpaths() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    (
        find_tool("luadec_32_deb.exe"),
        find_tool("luac4.exe"),
        find_tool("luac5.1.exe"),
        find_tool("stylua.exe"),
    )
}

/// Атомарная запись данных в файл через временный буфер
pub fn atomic_write(path: &Path, data: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let temp_name = format!(
        ".tmp_{}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
    );
    let temp_path = parent.join(temp_name);

    {
        let mut file = File::create(&temp_path)?;
        file.write_all(data)?;
        file.sync_all()?;
    }

    #[cfg(windows)]
    {
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }

    if let Err(e) = fs::rename(&temp_path, path) {
        let _ = fs::copy(&temp_path, path);
        let _ = fs::remove_file(&temp_path);
        return Err(e);
    }

    Ok(())
}
