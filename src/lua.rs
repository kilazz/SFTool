use crate::UiLogger;
use rayon::prelude::*;
use regex::Regex;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn is_compiled_lua(file_path: &Path) -> bool {
    if let Ok(mut f) = File::open(file_path) {
        let mut header = [0u8; 4];
        if f.read_exact(&mut header).is_ok() {
            return header.starts_with(b"\x1bLua");
        }
    }
    false
}

fn extract_error_summary(stdout_text: &str) -> String {
    let lines: Vec<&str> = stdout_text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.is_empty() {
        return "No output recorded.".into();
    }

    let mut summary = Vec::new();
    for line in &lines {
        let lower = line.to_lowercase();
        if line.contains("Parser error") || line.contains("Exit Code:") || lower.contains("error") {
            summary.push(*line);
        }
    }

    summary.push("--- Last lines ---");
    let tail_start = lines.len().saturating_sub(5);
    for line in &lines[tail_start..] {
        summary.push(*line);
    }
    summary.join("\n")
}

pub fn decompile_single_file(
    src_file: &Path,
    src_dir: &Path,
    dst_dir: &Path,
    luadec_exe: &Path,
    resume: bool,
) -> (PathBuf, String, String) {
    let rel_path = src_file
        .strip_prefix(src_dir)
        .unwrap_or(src_file)
        .to_path_buf();
    let dst_file = dst_dir.join(&rel_path);

    if let Some(parent) = dst_file.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if resume && dst_file.exists() && fs::metadata(&dst_file).map(|m| m.len()).unwrap_or(0) > 0 {
        return (rel_path, "SKIPPED".into(), String::new());
    }

    if !is_compiled_lua(src_file) {
        return match fs::copy(src_file, &dst_file) {
            Ok(_) => (rel_path, "COPIED_TEXT".into(), String::new()),
            Err(e) => (rel_path, "EXCEPTION".into(), format!("Copy Error: {}", e)),
        };
    }

    let mut cmd = Command::new(luadec_exe);
    cmd.arg(src_file).arg(&dst_file);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            if output.status.success() && dst_file.exists() {
                (rel_path, "OK".into(), String::new())
            } else {
                let _ = fs::remove_file(&dst_file);
                let stdout_str = String::from_utf8_lossy(&output.stdout);
                let stderr_str = String::from_utf8_lossy(&output.stderr);
                let summary = extract_error_summary(&stdout_str);
                let err_msg = format!(
                    "[Exit: {:?}] {}\n{}\nSTDERR: {}",
                    output.status.code(),
                    rel_path.display(),
                    summary,
                    stderr_str
                );
                (
                    rel_path,
                    format!("FAILED_{:?}", output.status.code()),
                    err_msg,
                )
            }
        }
        Err(e) => {
            let _ = fs::remove_file(&dst_file);
            (
                rel_path,
                "EXCEPTION".into(),
                format!("Execution Error: {}", e),
            )
        }
    }
}

pub fn batch_decompile(
    src_dir: &Path,
    dst_dir: &Path,
    luadec_exe: &Path,
    resume: bool,
    logger: &UiLogger,
) -> io::Result<()> {
    logger.log(&format!(
        "[*] Initializing Lua 4.0 batch decompile: {:?} -> {:?}",
        src_dir, dst_dir
    ));

    let files: Vec<PathBuf> = WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();

    let total = files.len();
    if total == 0 {
        logger.log("[!] No files found to decompile.");
        return Ok(());
    }

    let ok_count = AtomicUsize::new(0);
    let skipped_count = AtomicUsize::new(0);
    let copied_count = AtomicUsize::new(0);
    let fail_count = AtomicUsize::new(0);

    files.par_iter().for_each(|src_file| {
        let (_rel, status, err_msg) =
            decompile_single_file(src_file, src_dir, dst_dir, luadec_exe, resume);

        if status == "OK" {
            ok_count.fetch_add(1, Ordering::Relaxed);
        } else if status == "SKIPPED" {
            skipped_count.fetch_add(1, Ordering::Relaxed);
        } else if status == "COPIED_TEXT" {
            copied_count.fetch_add(1, Ordering::Relaxed);
        } else {
            fail_count.fetch_add(1, Ordering::Relaxed);
            if !err_msg.is_empty() {
                logger.log(&err_msg);
            }
        }
    });

    logger.log(&format!(
        "[+] Decompilation Complete: Total: {}, OK: {}, Plain Text: {}, Skipped: {}, Failed: {}",
        total,
        ok_count.load(Ordering::Relaxed),
        copied_count.load(Ordering::Relaxed),
        skipped_count.load(Ordering::Relaxed),
        fail_count.load(Ordering::Relaxed)
    ));
    Ok(())
}

// -----------------------------------------------------------------------------
// SYNTAX CHECKER (luac4 -p)
// -----------------------------------------------------------------------------

pub fn check_file_syntax(file_path: &Path, luac_exe: &Path) -> (bool, String) {
    let mut cmd = Command::new(luac_exe);
    cmd.arg("-p").arg(file_path);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            let msg = if output.status.success() {
                String::new()
            } else {
                let err = String::from_utf8_lossy(&output.stderr);
                if err.is_empty() {
                    String::from_utf8_lossy(&output.stdout).to_string()
                } else {
                    err.to_string()
                }
            };
            (output.status.success(), msg.trim().to_string())
        }
        Err(e) => (false, format!("Execution failure: {}", e)),
    }
}

pub fn batch_check_syntax(
    scripts_dir: &Path,
    luac_exe: &Path,
    logger: &UiLogger,
) -> io::Result<()> {
    logger.log(&format!(
        "[*] Checking Lua 4.0 syntax in: {:?}",
        scripts_dir
    ));

    let files: Vec<PathBuf> = WalkDir::new(scripts_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_file() && e.path().extension().and_then(|s| s.to_str()) == Some("lua")
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let total = files.len();
    let passed = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    files.par_iter().for_each(|f| {
        let (ok, msg) = check_file_syntax(f, luac_exe);
        if ok {
            passed.fetch_add(1, Ordering::Relaxed);
        } else {
            failed.fetch_add(1, Ordering::Relaxed);
            logger.log(&format!(
                "[!] Syntax Error in {:?}:\n    {}",
                f.file_name().unwrap_or_default(),
                msg
            ));
        }
    });

    logger.log(&format!(
        "[+] Syntax check finished! Checked: {}, Passed: {}, Errors: {}",
        total,
        passed.load(Ordering::Relaxed),
        failed.load(Ordering::Relaxed)
    ));
    Ok(())
}

// -----------------------------------------------------------------------------
// LUA 4.0 FORMATTER & PRETTY-PRINTER
// -----------------------------------------------------------------------------

pub fn format_lua_source(source_text: &str, indent_unit: &str) -> String {
    // 1. Normalize syntactic sugar: Name = function(...) -> function Name(...)
    let re_sugar =
        Regex::new(r"(?m)^([ \t]*)([a-zA-Z_][a-zA-Z0-9_.:]*)\s*=\s*function\s*\(").unwrap();
    let normalized = re_sugar.replace_all(source_text, "${1}function ${2}(");

    // 2. Collapse empty tables: {\s*\n\s*} -> {}
    let re_tables = Regex::new(r"\{\s*\n\s*\}").unwrap();
    let collapsed = re_tables.replace_all(&normalized, "{}");

    // Tokenizer Regex matching original SpellForce scripts
    let token_regex = Regex::new(
        r#"(?x)
        (?P<COMMENT_MULTI>--\[\[[\s\S]*?\]\])
        |(?P<COMMENT_SINGLE>--[^\n]*)
        |(?P<STRING_MULTI>\[\[[\s\S]*?\]\])
        |(?P<STRING_DOUBLE>"(?:\\.|[^"\\])*")
        |(?P<STRING_SINGLE>'(?:\\.|[^'\\])*')
        |(?P<NEWLINE>\n)
        |(?P<WHITESPACE>[^\S\n]+)
        |(?P<KEYWORD>\b(and|or|not|function|then|do|repeat|else|elseif|end|until|return|local|for|while|if|in|break)\b)
        |(?P<VARARG>\.\.\.)
        |(?P<OP_MULTI>==|~=|<=|>=|\.\.)
        |(?P<IDENT>[a-zA-Z_][a-zA-Z0-9_]*)
        |(?P<NUMBER>\b\d+(\.\d+)?([eE][+-]?\d+)?\b|\.\d+([eE][+-]?\d+)?\b)
        |(?P<SYMBOL>[{}()\[\]=+\-*/^,;:.%<>])
        |(?P<MISC>.)"#,
    )
    .unwrap();

    let mut lines: Vec<Vec<(String, String)>> = vec![vec![]];

    for cap in token_regex.captures_iter(&collapsed) {
        let (kind, val) = if cap.name("NEWLINE").is_some() {
            ("NEWLINE", "\n")
        } else if let Some(m) = cap.name("COMMENT_MULTI") {
            ("COMMENT_MULTI", m.as_str())
        } else if let Some(m) = cap.name("COMMENT_SINGLE") {
            ("COMMENT_SINGLE", m.as_str())
        } else if let Some(m) = cap.name("STRING_MULTI") {
            ("STRING_MULTI", m.as_str())
        } else if let Some(m) = cap.name("STRING_DOUBLE") {
            ("STRING_DOUBLE", m.as_str())
        } else if let Some(m) = cap.name("STRING_SINGLE") {
            ("STRING_SINGLE", m.as_str())
        } else if let Some(m) = cap.name("WHITESPACE") {
            ("WHITESPACE", m.as_str())
        } else if let Some(m) = cap.name("KEYWORD") {
            ("KEYWORD", m.as_str())
        } else if let Some(m) = cap.name("IDENT") {
            ("IDENT", m.as_str())
        } else if let Some(m) = cap.name("NUMBER") {
            ("NUMBER", m.as_str())
        } else if let Some(m) = cap.name("SYMBOL") {
            ("SYMBOL", m.as_str())
        } else {
            ("MISC", cap.get(0).map(|m| m.as_str()).unwrap_or(""))
        };

        if kind == "NEWLINE" {
            lines.push(vec![]);
        } else if kind != "WHITESPACE" {
            lines
                .last_mut()
                .unwrap()
                .push((kind.to_string(), val.to_string()));
        }
    }

    let mut formatted_lines = Vec::new();
    let mut current_indent = 0usize;

    for token_list in lines {
        if token_list.is_empty() {
            if formatted_lines
                .last()
                .map(|s: &String| !s.is_empty())
                .unwrap_or(false)
            {
                formatted_lines.push(String::new());
            }
            continue;
        }

        let code_tokens: Vec<&(String, String)> = token_list
            .iter()
            .filter(|(k, _)| !k.starts_with("COMMENT") && !k.starts_with("STRING"))
            .collect();

        let mut open_count = 0usize;
        let mut close_count = 0usize;
        let mut starts_with_unindent = false;
        let first_val = code_tokens.first().map(|(_, v)| v.as_str()).unwrap_or("");

        if matches!(first_val, "end" | "until" | "else" | "elseif" | "}") {
            starts_with_unindent = true;
        }

        for (_, val) in &code_tokens {
            match val.as_str() {
                "then" | "do" | "repeat" | "{" | "function" => open_count += 1,
                "end" | "until" | "}" => close_count += 1,
                _ => {}
            }
        }

        if first_val == "elseif" && open_count > 0 {
            open_count -= 1;
        }

        let effective_indent = if starts_with_unindent {
            current_indent.saturating_sub(1)
        } else {
            current_indent
        };

        let line_str = render_line_tokens(&token_list);
        formatted_lines.push(format!(
            "{}{}",
            indent_unit.repeat(effective_indent),
            line_str
        ));

        current_indent = (current_indent + open_count).saturating_sub(close_count);
    }

    while formatted_lines
        .last()
        .map(|s| s.is_empty())
        .unwrap_or(false)
    {
        formatted_lines.pop();
    }
    formatted_lines.join("\n") + "\n"
}

fn render_line_tokens(tokens: &[(String, String)]) -> String {
    if tokens.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut prev_kind = "";
    let mut prev_val = "";

    for (kind, val) in tokens {
        let is_spaced_op = matches!(
            val.as_str(),
            "=" | "==" | "~=" | "<=" | ">=" | "+" | "-" | "*" | "/" | "^" | ".."
        );
        let prev_is_spaced_op = matches!(
            prev_val,
            "=" | "==" | "~=" | "<=" | ">=" | "+" | "-" | "*" | "/" | "^" | ".."
        );
        let is_word = matches!(kind.as_str(), "IDENT" | "KEYWORD" | "NUMBER" | "VARARG");
        let prev_is_word = matches!(prev_kind, "IDENT" | "KEYWORD" | "NUMBER" | "VARARG");
        let is_string = kind.starts_with("STRING");
        let prev_is_string = prev_kind.starts_with("STRING");

        if matches!(val.as_str(), "," | ";" | ")" | "]" | "}")
            || matches!(prev_val, "(" | "[" | "{")
        {
            // No spacing needed
        } else if is_spaced_op
            || prev_is_spaced_op
            || matches!(prev_val, "," | ";")
            || (prev_is_word && is_word)
            || (matches!(prev_val, ")" | "]" | "}") && matches!(kind.as_str(), "KEYWORD" | "IDENT"))
            || (prev_is_string && matches!(kind.as_str(), "KEYWORD" | "IDENT"))
            || (prev_is_word && is_string)
            || prev_kind == "COMMENT_SINGLE"
        {
            out.push(' ');
        }

        out.push_str(val);
        prev_kind = kind;
        prev_val = val;
    }

    out.trim().to_string()
}

pub fn batch_format(dir_path: &Path, indent_unit: &str, logger: &UiLogger) -> io::Result<()> {
    logger.log(&format!("[*] Formatting Lua files in: {:?}", dir_path));

    let files: Vec<PathBuf> = WalkDir::new(dir_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_file() && e.path().extension().and_then(|s| s.to_str()) == Some("lua")
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let count = AtomicUsize::new(0);

    files.par_iter().for_each(|f| {
        if let Ok(content) = fs::read_to_string(f) {
            let formatted = format_lua_source(&content, indent_unit);
            if formatted != content && fs::write(f, formatted).is_ok() {
                count.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    logger.log(&format!(
        "[+] Formatted {} files.",
        count.load(Ordering::Relaxed)
    ));
    Ok(())
}
