use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use encoding_rs::{GBK, UTF_8};
use sha2::{Digest, Sha256};
use tauri::Emitter;

/// Safe replacement for `eprintln!` that doesn't panic when stderr is unavailable.
/// After sleep/wake cycles or when launched without a terminal, stderr may become
/// unwritable — `safe_eprintln!` panics in that case, but this macro silently ignores the error.
macro_rules! safe_eprintln {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

pub mod debug_log;
mod terminal;

/// Get the local data directory without using the `dirs` crate.
/// The `dirs` crate uses NSSearchPathForDirectoriesInDomains on macOS which can
/// trigger TCC permission prompts (Apple Music, etc.) in some environments.
pub fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join("Library/Application Support");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(local);
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata);
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg);
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".local/share");
        }
    }
    PathBuf::from(".")
}

/// Get the home directory without using the `dirs` crate.
pub fn app_home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            return Some(PathBuf::from(profile));
        }
    }
    std::env::var("HOME").ok().map(PathBuf::from)
}

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

mod fileops;
mod watcher;
mod git;
mod ai;
mod license;
mod python_setup;
mod mcp;
mod bm25;

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB

// --- Snapshot file storage ---

fn snapshots_dir() -> PathBuf {
    let data_dir = app_data_dir();
    let dir = data_dir.join("inkess").join("snapshots");
    fs::create_dir_all(&dir).ok();
    dir
}

fn path_hash(file_path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file_path.as_bytes());
    format!("{:x}", hasher.finalize())[..12].to_string()
}

// --- Path validation ---

pub const BLOCKED_PATHS: &[&str] = &[
    // Unix
    "/.ssh", "/.gnupg", "/.aws", "/.kube",
    "/.docker", "/.config/gcloud", "/.azure",
    "/etc/shadow", "/etc/passwd",
    "/.bash_history", "/.zsh_history", "/.node_repl_history",
    "/.npmrc", "/.pypirc",
    "/.config/gh", "/.config/hub",
    // Windows
    "\\.ssh", "\\.gnupg", "\\.aws", "\\.kube",
    "\\.docker", "\\.config\\gcloud", "\\.azure",
    "\\AppData\\Roaming\\gnupg",
    "\\.bash_history", "\\.npmrc", "\\.pypirc",
    "\\.node_repl_history",
    "\\.config\\gh", "\\.config\\hub",
];

pub fn validate_path(path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    // Try canonicalize; if file doesn't exist yet, canonicalize parent directory
    let canonical = if p.exists() {
        p.canonicalize().map_err(|_| "Cannot access path".to_string())?
    } else {
        let parent = p.parent().ok_or_else(|| "Invalid path".to_string())?;
        let canon_parent = parent.canonicalize()
            .map_err(|_| "Cannot access parent directory".to_string())?;
        canon_parent.join(p.file_name().ok_or_else(|| "Invalid filename".to_string())?)
    };
    let path_str = canonical.to_string_lossy();
    for blocked in BLOCKED_PATHS {
        if path_str.contains(blocked) {
            return Err("Permission denied".to_string());
        }
    }
    Ok(canonical)
}

// --- App settings (persisted to JSON file) ---

fn settings_path() -> PathBuf {
    let data_dir = app_data_dir();
    let dir = data_dir.join("inkess");
    fs::create_dir_all(&dir).ok();
    dir.join("settings.json")
}

#[tauri::command]
fn save_settings(settings: serde_json::Value) -> Result<(), String> {
    let path = settings_path();
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| format!("Failed to save settings: {}", e))
}

#[tauri::command]
fn load_settings() -> serde_json::Value {
    let path = settings_path();
    fs::read_to_string(&path)
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or(serde_json::json!({}))
}

// --- Debug log commands ---

#[tauri::command]
fn get_debug_logs() -> Vec<debug_log::LogEntry> {
    debug_log::LOG_BUFFER.lock().map(|b| b.entries()).unwrap_or_default()
}

#[tauri::command]
fn clear_debug_logs() {
    if let Ok(mut b) = debug_log::LOG_BUFFER.lock() {
        b.clear();
    }
}

// --- Terminal Logs ---

#[derive(serde::Serialize)]
struct TerminalLogEntry {
    filename: String,
    session_id: String,
    started: String,
    provider: String,
    cwd: String,
    size_bytes: u64,
    recovered: bool,
}

fn parse_log_header(content: &str) -> (String, String, String, String, bool) {
    let mut session = String::new();
    let mut started = String::new();
    let mut provider = String::new();
    let mut cwd = String::new();
    let recovered = !content.contains("# closed:");
    for line in content.lines().take(6) {
        if let Some(v) = line.strip_prefix("# session: ") { session = v.to_string(); }
        if let Some(v) = line.strip_prefix("# started: ") { started = v.to_string(); }
        if let Some(v) = line.strip_prefix("# provider: ") { provider = v.to_string(); }
        if let Some(v) = line.strip_prefix("# cwd: ") { cwd = v.to_string(); }
    }
    (session, started, provider, cwd, recovered)
}

#[tauri::command]
fn get_system_env_vars() -> Vec<(String, String)> {
    let mut vars: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| !k.is_empty())
        .collect();
    vars.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    vars
}

#[tauri::command]
fn get_shell_env_vars() -> Vec<(String, String)> {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() { return vec![]; }
    let files = [".bashrc", ".zshrc", ".bash_profile", ".zprofile", ".profile"];
    let mut result: Vec<(String, String)> = vec![];
    let mut seen = std::collections::HashSet::new();
    for f in &files {
        let path = std::path::Path::new(&home).join(f);
        if let Ok(content) = fs::read_to_string(&path) {
            for line in content.lines() {
                let trimmed = line.trim();
                // Match: export KEY=VALUE or export KEY="VALUE"
                if let Some(rest) = trimmed.strip_prefix("export ") {
                    let rest = rest.trim();
                    if let Some(eq_pos) = rest.find('=') {
                        let key = rest[..eq_pos].trim().to_string();
                        let mut val = rest[eq_pos + 1..].trim().to_string();
                        // Strip surrounding quotes
                        if (val.starts_with('"') && val.ends_with('"'))
                            || (val.starts_with('\'') && val.ends_with('\''))
                        {
                            val = val[1..val.len() - 1].to_string();
                        }
                        if !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_') && seen.insert(key.clone()) {
                            result.push((key, val));
                        }
                    }
                }
            }
        }
    }
    result.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    result
}

/// Parse shell RC files for function definitions containing `export` statements.
/// Supports `name() {` and `function name {` syntax.
#[derive(serde::Serialize)]
struct ShellFunction {
    name: String,
    env_vars: Vec<(String, String)>,
}

#[tauri::command]
fn parse_shell_functions() -> Vec<ShellFunction> {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() { return vec![]; }
    let files = [".zshrc", ".bashrc", ".bash_profile", ".zprofile", ".profile"];
    let mut functions: Vec<ShellFunction> = vec![];
    let mut seen_names = std::collections::HashSet::new();

    for f in &files {
        let path = std::path::Path::new(&home).join(f);
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let trimmed = lines[i].trim();
            // Skip comment lines
            if trimmed.starts_with('#') { i += 1; continue; }
            // Match: name() { or name () { or function name { or function name{
            let func_name = if let Some(rest) = trimmed.strip_prefix("function ") {
                let rest = rest.trim();
                let name_end = rest.find(|c: char| c == '{' || c == '(' || (!c.is_alphanumeric() && c != '_' && c != '-')).unwrap_or(rest.len());
                if name_end > 0 { Some(rest[..name_end].trim_end().to_string()) } else { None }
            } else if let Some(paren_pos) = trimmed.find("()") {
                let candidate = trimmed[..paren_pos].trim();
                if !candidate.is_empty() && candidate.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                    Some(candidate.to_string())
                } else { None }
            } else { None };

            if let Some(name) = func_name {
                // Collect exports within braces, tracking quote state to avoid false brace matches
                let mut env_vars: Vec<(String, String)> = vec![];
                let mut depth = 0i32;
                let mut started = false;
                let mut j = i;
                'outer: while j < lines.len() {
                    let mut in_single_quote = false;
                    let mut in_double_quote = false;
                    let mut escape_next = false;
                    for ch in lines[j].chars() {
                        if escape_next { escape_next = false; continue; }
                        if ch == '\\' && in_double_quote { escape_next = true; continue; }
                        if ch == '\'' && !in_double_quote { in_single_quote = !in_single_quote; continue; }
                        if ch == '"' && !in_single_quote { in_double_quote = !in_double_quote; continue; }
                        if ch == '#' && !in_single_quote && !in_double_quote { break; } // rest is comment
                        if !in_single_quote && !in_double_quote {
                            if ch == '{' { depth += 1; started = true; }
                            if ch == '}' { depth -= 1; }
                            if started && depth == 0 { break 'outer; }
                        }
                    }
                    // Parse export lines inside the function
                    if started && depth > 0 {
                        let line_trimmed = lines[j].trim();
                        if let Some(rest) = line_trimmed.strip_prefix("export ") {
                            let rest = rest.trim();
                            if let Some(eq_pos) = rest.find('=') {
                                let key = rest[..eq_pos].trim().to_string();
                                let mut val = rest[eq_pos + 1..].trim().to_string();
                                if (val.starts_with('"') && val.ends_with('"'))
                                    || (val.starts_with('\'') && val.ends_with('\''))
                                {
                                    val = val[1..val.len() - 1].to_string();
                                }
                                if !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                    env_vars.push((key, val));
                                }
                            }
                        }
                    }
                    j += 1;
                }
                if !env_vars.is_empty() && seen_names.insert(name.clone()) {
                    functions.push(ShellFunction { name, env_vars });
                }
                i = j + 1;
            } else {
                i += 1;
            }
        }
    }
    functions
}

#[tauri::command]
fn delete_terminal_log(filename: String) -> Result<(), String> {
    if !filename.ends_with(".log")
        || !filename.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err("Invalid filename".to_string());
    }
    let path = app_data_dir().join("inkess").join("terminal-logs").join(&filename);
    if !path.exists() { return Err("Log not found".to_string()); }
    fs::remove_file(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_terminal_logs() -> Vec<TerminalLogEntry> {
    let dir = app_data_dir().join("inkess").join("terminal-logs");
    if !dir.exists() { return vec![]; }

    // Cleanup: delete logs older than 3 days
    let three_days = std::time::Duration::from_secs(3 * 24 * 3600);
    let now = std::time::SystemTime::now();

    let mut entries: Vec<TerminalLogEntry> = vec![];
    let Ok(rd) = fs::read_dir(&dir) else { return vec![] };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("log") { continue; }
        let meta = match fs::metadata(&path) { Ok(m) => m, Err(_) => continue };
        // Delete old logs
        if let Ok(modified) = meta.modified() {
            if let Ok(age) = now.duration_since(modified) {
                if age > three_days {
                    let _ = fs::remove_file(&path);
                    continue;
                }
            }
        }
        let content = fs::read_to_string(&path).unwrap_or_default();
        let (session, started, provider, cwd, recovered) = parse_log_header(&content);
        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        entries.push(TerminalLogEntry {
            filename, session_id: session, started, provider, cwd,
            size_bytes: meta.len(), recovered,
        });
    }
    entries.sort_by(|a, b| b.started.cmp(&a.started));
    entries.truncate(50);
    entries
}

#[tauri::command]
fn read_terminal_log(filename: String) -> Result<String, String> {
    // Strict filename validation: only alphanumeric, dash, underscore, dot; must end with .log
    if !filename.ends_with(".log")
        || !filename.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err("Invalid filename".to_string());
    }
    let path = app_data_dir().join("inkess").join("terminal-logs").join(&filename);
    if !path.exists() { return Err("Log not found".to_string()); }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    // Strip ANSI escape codes without regex dependency
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip ESC [ ... <letter> sequences
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&nc) = chars.peek() {
                    chars.next();
                    if nc.is_ascii_alphabetic() { break; }
                }
            }
        } else {
            result.push(c);
        }
    }
    Ok(result)
}

// --- Encoding detection ---

fn read_file_with_encoding(path: &PathBuf) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("Cannot read file: {}", e))?;

    // Check for UTF-8 BOM
    let data = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { &bytes[3..] } else { &bytes };

    // Try UTF-8 first
    let (cow, encoding, had_errors) = UTF_8.decode(data);
    if !had_errors && encoding == UTF_8 {
        return Ok(cow.into_owned());
    }

    // Try GBK (covers GB2312 and most Chinese Windows files)
    let (cow, _, had_errors) = GBK.decode(data);
    if !had_errors {
        return Ok(cow.into_owned());
    }

    // Fallback: lossy UTF-8
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

// --- File commands ---

#[tauri::command]
fn read_file_binary(path: String) -> Result<Vec<u8>, String> {
    let canonical = validate_path(&path)?;
    if !canonical.is_file() {
        return Err("Not a valid file".to_string());
    }
    let meta = fs::metadata(&canonical).map_err(|e| format!("Cannot read file info: {}", e))?;
    if meta.len() > MAX_FILE_SIZE {
        return Err("File too large (over 10MB)".to_string());
    }
    fs::read(&canonical).map_err(|e| format!("Cannot read file: {}", e))
}

#[tauri::command]
fn read_file(path: String) -> Result<String, String> {
    do_read_file(&path)
}

#[tauri::command]
fn get_file_size(path: String) -> Result<u64, String> {
    let canonical = validate_path(&path)?;
    let meta = fs::metadata(&canonical).map_err(|e| format!("Cannot read file info: {}", e))?;
    Ok(meta.len())
}

#[tauri::command]
fn read_file_lines(path: String, line: u32, context: Option<u32>) -> Result<String, String> {
    let canonical = validate_path(&path)?;
    if !canonical.is_file() {
        return Err("Not a valid file".to_string());
    }
    let meta = fs::metadata(&canonical).map_err(|e| format!("Cannot read file info: {}", e))?;
    if meta.len() > MAX_FILE_SIZE {
        return Err("File too large for preview".to_string());
    }
    let content = read_file_with_encoding(&canonical)?;
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Ok(String::new());
    }
    let ctx = context.unwrap_or(3) as usize;
    let target = if line == 0 { 0 } else { (line as usize).saturating_sub(1).min(lines.len() - 1) };
    let start = target.saturating_sub(ctx);
    let end = (target + ctx + 1).min(lines.len());
    let mut result = String::new();
    for i in start..end {
        result.push_str(&format!("{:>4} | {}\n", i + 1, lines[i]));
    }
    Ok(result)
}

pub fn do_read_file(path: &str) -> Result<String, String> {
    let canonical = validate_path(path)?;
    if !canonical.is_file() {
        return Err("Not a valid file".to_string());
    }
    let meta = fs::metadata(&canonical).map_err(|e| format!("Cannot read file info: {}", e))?;
    if meta.len() > MAX_FILE_SIZE {
        return Err("File too large (over 10MB)".to_string());
    }
    read_file_with_encoding(&canonical)
}

#[tauri::command]
fn save_file(path: String, content: String) -> Result<(), String> {
    let canonical = validate_path(&path)?;
    if !canonical.is_file() {
        return Err("Not a valid file".to_string());
    }
    fs::write(&canonical, content.as_bytes())
        .map_err(|e| format!("Save failed: {}", e))
}

const MAX_DIR_ENTRIES: usize = 500;

#[tauri::command]
fn list_directory(path: String) -> Result<DirectoryListing, String> {
    do_list_directory(&path)
}

pub fn do_list_directory(path: &str) -> Result<DirectoryListing, String> {
    let canonical = validate_path(path)?;
    if !canonical.is_dir() {
        return Err("Not a valid directory".to_string());
    }
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&canonical)
        .map_err(|e| format!("Cannot read directory: {}", e))?;

    for entry in read_dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        entries.push(FileEntry { name, is_dir });
    }

    entries.sort_by(|a, b| {
        if a.is_dir == b.is_dir {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        } else if a.is_dir {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });

    let total = entries.len();
    let truncated = total > MAX_DIR_ENTRIES;
    if truncated {
        entries.truncate(MAX_DIR_ENTRIES);
    }

    Ok(DirectoryListing { entries, truncated, total })
}

#[tauri::command]
fn write_file(path: String, contents: Vec<u8>) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let parent = p.parent().ok_or("Invalid path")?;
    let canonical_parent = parent.canonicalize()
        .map_err(|_| "Cannot access path".to_string())?;
    let file_name = p.file_name().ok_or("Invalid filename")?;
    let canonical = canonical_parent.join(file_name);

    let path_str = canonical.to_string_lossy();
    for blocked in BLOCKED_PATHS {
        if path_str.contains(blocked) {
            return Err("Permission denied".to_string());
        }
    }

    fs::write(&canonical, contents).map_err(|e| format!("Write failed: {}", e))
}

// --- Snapshot commands ---

fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())[..8].to_string()
}

#[tauri::command]
fn create_snapshot(file_path: String, content: String) -> Result<bool, String> {
    let canonical = validate_path(&file_path)?;
    let file_path_str = canonical.to_string_lossy().to_string();

    if content.len() > MAX_FILE_SIZE as usize {
        return Err("File too large for snapshot".to_string());
    }

    let hash = content_hash(&content);
    let dir = snapshots_dir().join(path_hash(&file_path_str));
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create snapshot dir: {}", e))?;

    // Dedup: check if latest snapshot has same content hash
    if let Ok(entries) = fs::read_dir(&dir) {
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.ends_with(".snap"))
            .collect();
        names.sort();
        if let Some(last) = names.last() {
            if last.contains(&format!("_{hash}.snap")) {
                return Ok(false);
            }
        }
    }

    let now = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let filename = format!("{}_{}.snap", now, hash);
    fs::write(dir.join(&filename), &content)
        .map_err(|e| format!("Failed to write snapshot: {}", e))?;

    // Trim to 100 per file
    let mut names: Vec<String> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".snap"))
        .collect();
    names.sort();
    if names.len() > 100 {
        for old in &names[..names.len() - 100] {
            let _ = fs::remove_file(dir.join(old));
        }
    }

    Ok(true)
}

#[tauri::command]
fn list_snapshots(file_path: String) -> Result<Vec<SnapshotInfo>, String> {
    let canonical = validate_path(&file_path)?;
    let file_path_str = canonical.to_string_lossy().to_string();
    let dir = snapshots_dir().join(path_hash(&file_path_str));

    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut names: Vec<String> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".snap"))
        .collect();
    names.sort_by(|a, b| b.cmp(a));
    names.truncate(50);

    let snapshots: Vec<SnapshotInfo> = names.iter().map(|n| {
        let id = n.trim_end_matches(".snap").to_string();
        let created_at = if let Some(ts) = id.split('_').next() {
            chrono::NaiveDateTime::parse_from_str(ts, "%Y%m%dT%H%M%S")
                .map(|dt| dt.and_utc().to_rfc3339())
                .unwrap_or_else(|_| id.clone())
        } else {
            id.clone()
        };
        SnapshotInfo { id, created_at }
    }).collect();

    Ok(snapshots)
}

#[tauri::command]
fn get_snapshot_content(file_path: String, snapshot_id: String) -> Result<String, String> {
    // Validate snapshot_id contains no path separators (prevent path traversal)
    if snapshot_id.contains('/') || snapshot_id.contains('\\') || snapshot_id.contains("..") {
        return Err("Invalid snapshot ID".to_string());
    }
    let canonical = validate_path(&file_path)?;
    let file_path_str = canonical.to_string_lossy().to_string();
    let dir = snapshots_dir().join(path_hash(&file_path_str));
    let snap_file = dir.join(format!("{}.snap", snapshot_id));
    fs::read_to_string(&snap_file).map_err(|_| "Snapshot not found".to_string())
}

#[derive(serde::Serialize)]
struct SnapshotStats {
    count: i64,
    size_bytes: i64,
}

#[tauri::command]
fn get_snapshot_stats() -> Result<SnapshotStats, String> {
    let base = snapshots_dir();
    let mut count: i64 = 0;
    let mut size: i64 = 0;
    if let Ok(dirs) = fs::read_dir(&base) {
        for dir_entry in dirs.flatten() {
            if dir_entry.path().is_dir() {
                if let Ok(files) = fs::read_dir(dir_entry.path()) {
                    for f in files.flatten() {
                        if f.path().extension().map(|e| e == "snap").unwrap_or(false) {
                            count += 1;
                            size += f.metadata().map(|m| m.len() as i64).unwrap_or(0);
                        }
                    }
                }
            }
        }
    }
    Ok(SnapshotStats { count, size_bytes: size })
}

#[tauri::command]
fn cleanup_snapshots(retention_days: i64, retention_count: i64) -> Result<i64, String> {
    let base = snapshots_dir();
    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days);
    let cutoff_str = cutoff.format("%Y%m%dT%H%M%S").to_string();
    let mut deleted: i64 = 0;

    if let Ok(dirs) = fs::read_dir(&base) {
        for dir_entry in dirs.flatten() {
            if !dir_entry.path().is_dir() { continue; }
            let mut names: Vec<String> = fs::read_dir(dir_entry.path())
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(".snap"))
                .collect();
            names.sort();

            let date_delete: Vec<String> = names.iter()
                .filter(|n| n.split('_').next().map(|ts| ts < cutoff_str.as_str()).unwrap_or(false))
                .cloned()
                .collect();
            for n in &date_delete {
                if fs::remove_file(dir_entry.path().join(n)).is_ok() {
                    deleted += 1;
                }
            }

            let mut remaining: Vec<String> = fs::read_dir(dir_entry.path())
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(".snap"))
                .collect();
            remaining.sort();
            if remaining.len() as i64 > retention_count {
                let to_remove = remaining.len() as i64 - retention_count;
                for n in &remaining[..to_remove as usize] {
                    if fs::remove_file(dir_entry.path().join(n)).is_ok() {
                        deleted += 1;
                    }
                }
            }

            if fs::read_dir(dir_entry.path()).map(|mut d| d.next().is_none()).unwrap_or(false) {
                let _ = fs::remove_dir(dir_entry.path());
            }
        }
    }

    Ok(deleted)
}

// --- Types ---

#[derive(serde::Serialize)]
pub struct FileEntry { pub name: String, pub is_dir: bool }

#[derive(serde::Serialize)]
pub struct DirectoryListing {
    pub entries: Vec<FileEntry>,
    pub truncated: bool,
    pub total: usize,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SnapshotInfo { id: String, created_at: String }

struct InitialFile(Mutex<Option<String>>);

#[tauri::command]
fn get_initial_file(state: tauri::State<'_, InitialFile>) -> Option<String> {
    state.0.lock().ok()?.take()
}

// --- macOS combined file/directory open dialog ---

#[cfg(target_os = "macos")]
#[tauri::command]
#[allow(deprecated, unexpected_cfgs)]
async fn open_file_or_dir(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel();

    app.run_on_main_thread(move || {
        use cocoa::appkit::NSOpenPanel;
        use cocoa::base::{id, nil, YES, NO};
        use cocoa::foundation::{NSArray, NSString};

        unsafe {
            let panel: id = NSOpenPanel::openPanel(nil);
            let () = msg_send![panel, setCanChooseFiles: YES];
            let () = msg_send![panel, setCanChooseDirectories: YES];
            let () = msg_send![panel, setAllowsMultipleSelection: NO];
            let () = msg_send![panel, setTreatsFilePackagesAsDirectories: NO];

            let result: i64 = msg_send![panel, runModal];
            if result == 1 {
                // NSModalResponseOK
                let urls: id = msg_send![panel, URLs];
                let count: usize = NSArray::count(urls) as usize;
                if count > 0 {
                    let url: id = NSArray::objectAtIndex(urls, 0);
                    let path: id = msg_send![url, path];
                    let cstr: *const std::os::raw::c_char = NSString::UTF8String(path);
                    let s = std::ffi::CStr::from_ptr(cstr).to_string_lossy().into_owned();
                    let _ = tx.send(Some(s));
                    return;
                }
            }
            let _ = tx.send(None);
        }
    }).map_err(|e| e.to_string())?;

    rx.recv().map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
async fn open_file_or_dir(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    // On Windows/Linux, try folder picker first (primary use case)
    let folder = app.dialog()
        .file()
        .set_title("Open Folder")
        .blocking_pick_folder();

    if let Some(path) = folder {
        if let Some(p) = path.as_path() {
            return Ok(Some(p.to_string_lossy().to_string()));
        }
    }

    // If cancelled, try file picker
    let file = app.dialog()
        .file()
        .set_title("Open File")
        .add_filter("Markdown", &["md", "markdown", "mdown", "mkd"])
        .add_filter("Text", &["txt", "log", "csv"])
        .add_filter("All Files", &["*"])
        .blocking_pick_file();

    if let Some(path) = file {
        if let Some(p) = path.as_path() {
            return Ok(Some(p.to_string_lossy().to_string()));
        }
    }

    Ok(None)
}

// --- App entry ---

fn is_supported_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    // Files without extension are treated as text
    if !std::path::Path::new(path).extension().is_some_and(|e| !e.is_empty()) {
        return true;
    }
    let supported = [
        ".md", ".markdown", ".mdown", ".mkd",
        ".txt", ".log", ".csv",
        ".js", ".ts", ".tsx", ".jsx", ".py", ".rs", ".go",
        ".json", ".yaml", ".yml", ".toml", ".xml", ".css", ".html", ".sh", ".sql",
        ".c", ".cpp", ".h", ".java", ".rb",
        ".properties", ".ini", ".conf", ".cfg", ".env",
        ".kt", ".swift", ".dart", ".lua", ".r",
        ".scala", ".groovy", ".gradle",
        ".vue", ".svelte", ".less", ".scss", ".sass",
        ".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".bmp", ".ico",
        ".pdf", ".docx", ".xlsx", ".xls",
    ];
    supported.iter().any(|ext| lower.ends_with(ext))
}

// --- Native menu bar ---

fn setup_menu(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::*;

    let version = env!("CARGO_PKG_VERSION");

    // App menu (macOS only shows this as the app name menu)
    let about = PredefinedMenuItem::about(app, Some("About Inkess"), Some(AboutMetadata {
        name: Some("Inkess".into()),
        version: Some(version.into()),
        copyright: Some("© 2025 Inkess. All rights reserved.".into()),
        website: Some("https://inkess.net".into()),
        website_label: Some("inkess.net".into()),
        ..Default::default()
    }))?;
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, Some("CmdOrCtrl+,"))?;
    let app_menu = Submenu::with_items(app, "Inkess", true, &[
        &about,
        &PredefinedMenuItem::separator(app)?,
        &settings,
        &PredefinedMenuItem::separator(app)?,
        &PredefinedMenuItem::hide(app, None)?,
        &PredefinedMenuItem::hide_others(app, None)?,
        &PredefinedMenuItem::show_all(app, None)?,
        &PredefinedMenuItem::separator(app)?,
        &PredefinedMenuItem::quit(app, None)?,
    ])?;

    // File menu
    let open = MenuItem::with_id(app, "open", "Open...", true, Some("CmdOrCtrl+O"))?;
    let save = MenuItem::with_id(app, "save", "Save", true, Some("CmdOrCtrl+S"))?;
    let close_window = PredefinedMenuItem::close_window(app, None)?;
    let file_menu = Submenu::with_items(app, "File", true, &[
        &open, &save,
        &PredefinedMenuItem::separator(app)?,
        &close_window,
    ])?;

    // Edit menu
    let edit_menu = Submenu::with_items(app, "Edit", true, &[
        &PredefinedMenuItem::undo(app, None)?,
        &PredefinedMenuItem::redo(app, None)?,
        &PredefinedMenuItem::separator(app)?,
        &PredefinedMenuItem::cut(app, None)?,
        &PredefinedMenuItem::copy(app, None)?,
        &PredefinedMenuItem::paste(app, None)?,
        &PredefinedMenuItem::select_all(app, None)?,
    ])?;

    // View menu
    let find = MenuItem::with_id(app, "find", "Find in Document", true, Some("CmdOrCtrl+F"))?;
    let toggle_edit = MenuItem::with_id(app, "toggle_edit", "Toggle Edit Mode", true, Some("CmdOrCtrl+E"))?;
    let dev_mode = MenuItem::with_id(app, "dev_mode", "Developer Mode", true, Some("CmdOrCtrl+D"))?;
    let view_menu = Submenu::with_items(app, "View", true, &[
        &find, &toggle_edit,
        &PredefinedMenuItem::separator(app)?,
        &dev_mode,
        &PredefinedMenuItem::separator(app)?,
        &PredefinedMenuItem::fullscreen(app, None)?,
    ])?;

    // Window menu
    let window_menu = Submenu::with_items(app, "Window", true, &[
        &PredefinedMenuItem::minimize(app, None)?,
        &PredefinedMenuItem::maximize(app, None)?,
        &PredefinedMenuItem::separator(app)?,
        &PredefinedMenuItem::close_window(app, None)?,
    ])?;

    // Help menu
    let website = MenuItem::with_id(app, "website", "Inkess Website", true, None::<&str>)?;
    let feedback = MenuItem::with_id(app, "feedback", "Send Feedback...", true, None::<&str>)?;
    let shortcuts = MenuItem::with_id(app, "shortcuts", "Keyboard Shortcuts", true, Some("CmdOrCtrl+/"))?;
    let help_menu = Submenu::with_items(app, "Help", true, &[
        &website, &feedback,
        &PredefinedMenuItem::separator(app)?,
        &shortcuts,
    ])?;

    let menu = Menu::with_items(app, &[&app_menu, &file_menu, &edit_menu, &view_menu, &window_menu, &help_menu])?;
    app.set_menu(menu)?;

    // Handle custom menu events
    app.on_menu_event(move |app_handle, event| {
        match event.id().as_ref() {
            "settings" | "open" | "save" | "find" | "toggle_edit" | "dev_mode" | "shortcuts" => {
                let _ = app_handle.emit("menu-action", event.id().as_ref());
            }
            "website" => {
                #[cfg(target_os = "macos")]
                let _ = std::process::Command::new("open").arg("https://inkess.net").spawn();
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("explorer.exe").arg("https://inkess.net").spawn();
                #[cfg(target_os = "linux")]
                let _ = std::process::Command::new("xdg-open").arg("https://inkess.net").spawn();
            }
            "feedback" => {
                let mailto = "mailto:gezhigang@foxmail.com?subject=Inkess%20Feedback";
                #[cfg(target_os = "macos")]
                let _ = std::process::Command::new("open").arg(mailto).spawn();
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("explorer.exe").arg(mailto).spawn();
                #[cfg(target_os = "linux")]
                let _ = std::process::Command::new("xdg-open").arg(mailto).spawn();
            }
            _ => {}
        }
    });

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_file: Option<String> = std::env::args()
        .skip(1)
        .find(|arg| {
            if arg.starts_with('-') { return false; }
            // Support both files and directories
            let p = std::path::Path::new(arg);
            p.is_dir() || is_supported_file(arg)
        });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            setup_menu(app)?;
            ai::cleanup_decay_cache();
            Ok(())
        })
        .manage(InitialFile(Mutex::new(initial_file)))
        .manage(watcher::WatcherState {
            watcher: Mutex::new(None),
            watched_path: Mutex::new(None),
        })
        .manage(terminal::pty::PtyState {
            sessions: Mutex::new(std::collections::HashMap::new()),
        })
        .manage(mcp::McpState {
            registry: std::sync::Arc::new(tokio::sync::Mutex::new(mcp::registry::McpRegistry::new())),
            health_check_handle: std::sync::Mutex::new(None),
        })
        .manage(ai::AiCancelRegistry::new())
        .manage(ai::ShellConfirmState {
            sender: std::sync::Mutex::new(None),
        })
        .manage({
            let ai_tool_registry = ai::tool::registry::ToolRegistry::new();
            tauri::async_runtime::block_on(async {
                ai::tools::register_builtin_tools(&ai_tool_registry).await;
            });
            ai::AiToolRegistryState { registry: ai_tool_registry }
        })
        .manage({
            let ai_skill_registry = ai::skill::registry::SkillRegistry::new("default");
            tauri::async_runtime::block_on(async {
                ai::skills::register_builtin_skills(&ai_skill_registry).await;
            });
            ai::AiSkillRegistryState { registry: ai_skill_registry }
        })
        .manage({
            let memory_dir = app_data_dir().join("inkess").join("memories");
            let memory_store = ai::memory::FileMemoryStore::new(memory_dir.clone())
                .unwrap_or_else(|e| {
                    safe_eprintln!("[memory] Failed to initialize memory store at {:?}: {}. Trying temp fallback.", memory_dir, e);
                    let fallback_dir = std::env::temp_dir().join("inkess-memories");
                    ai::memory::FileMemoryStore::new(fallback_dir.clone())
                        .unwrap_or_else(|e2| {
                            safe_eprintln!("[memory] Fallback also failed at {:?}: {}. Using empty store.", fallback_dir, e2);
                            // Last resort: create in-place with a known-good temp dir
                            let last_resort = std::env::temp_dir().join(format!("inkess-mem-{}", std::process::id()));
                            ai::memory::FileMemoryStore::new(last_resort)
                                .expect("Cannot create memory store even in temp directory")
                        })
                });
            ai::MemoryStoreState {
                store: std::sync::Arc::new(memory_store),
            }
        })
        .manage(bm25::Bm25State {
            index: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            read_file, read_file_binary, read_file_lines, save_file, list_directory, write_file, get_file_size,
            create_snapshot, list_snapshots, get_snapshot_content,
            get_snapshot_stats, cleanup_snapshots,
            get_initial_file, open_file_or_dir,
            fileops::create_file, fileops::create_directory,
            fileops::rename_entry, fileops::delete_to_trash, fileops::search_files, fileops::copy_file_to_dir,
            watcher::watch_directory, watcher::unwatch_directory,
            terminal::pty::pty_spawn, terminal::pty::pty_write, terminal::pty::pty_resize, terminal::pty::pty_kill,
            git::git_status, git::git_init, git::git_stage, git::git_unstage,
            git::git_commit, git::git_push, git::git_pull,
            git::git_remote_add, git::git_remote_list, git::git_log,
            git::git_config_user, git::setup_ssh_key,
            ai::ai_save_config, ai::ai_load_config, ai::ai_test_connection, ai::ai_test_search, ai::ai_chat,
            ai::ai_save_memory, ai::ai_load_memories, ai::ai_cancel_chat,
            ai::shell_confirm_response, ai::sync_mcp_tools,
            license::license_load, license::license_activate, license::license_deactivate, license::open_external_url,
            python_setup::check_python_env,
            python_setup::preload_python_env,
            save_settings, load_settings,
            bm25::bm25_init, bm25::bm25_search,
            mcp::mcp_add_server, mcp::mcp_remove_server, mcp::mcp_restart_server,
            mcp::mcp_list_servers, mcp::mcp_list_tools, mcp::mcp_tool_logs,
            get_debug_logs, clear_debug_logs,
            list_terminal_logs, read_terminal_log, delete_terminal_log,
            get_system_env_vars, get_shell_env_vars, parse_shell_functions,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, _event| {
            // Auto-connect enabled MCP servers on first launch
            #[cfg(not(any(test)))]
            {
                use tauri::Manager;
                if let tauri::RunEvent::Ready = &_event {
                    let mcp_state = _app.state::<mcp::McpState>();
                    let registry = mcp_state.registry.clone();
                    let registry2 = mcp_state.registry.clone();
                    let app_handle = _app.clone();
                    tauri::async_runtime::spawn(async move {
                        {
                            let mut reg = registry.lock().await;
                            reg.connect_all_enabled().await;
                        }
                        // Sync MCP tools into ToolRegistry after connecting
                        let tool_registry_state = app_handle.state::<ai::AiToolRegistryState>();
                        let mcp_state = app_handle.state::<mcp::McpState>();
                        ai::tools::mcp_bridge::sync_mcp_tools(
                            &tool_registry_state.registry,
                            &mcp_state.registry,
                        ).await;
                    });
                    // Start MCP health check background task
                    let handle = mcp::start_health_check(registry2);
                    // Store handle for cleanup on exit
                    let mcp_state2 = _app.state::<mcp::McpState>();
                    let mcp_hc = mcp_state2.health_check_handle.lock();
                    if let Ok(mut guard) = mcp_hc {
                        *guard = Some(handle);
                    }
                }
                // Clean up all resources on app exit
                if let tauri::RunEvent::ExitRequested { .. } = &_event {
                    // Abort MCP health check task
                    let mcp_state = _app.state::<mcp::McpState>();
                    if let Ok(mut guard) = mcp_state.health_check_handle.lock() {
                        if let Some(handle) = guard.take() {
                            handle.abort();
                        }
                    }
                    // Kill all PTY sessions
                    let pty_state = _app.state::<terminal::pty::PtyState>();
                    if let Ok(mut sessions) = pty_state.sessions.lock() {
                        for (sid, mut session) in sessions.drain() {
                            safe_eprintln!("[cleanup] killing PTY session: {}", sid);
                            if let Err(e) = session.child.kill() {
                                safe_eprintln!("[cleanup] kill failed for {}: {}", sid, e);
                            }
                            // Use try_wait with timeout to avoid blocking indefinitely
                            let start = std::time::Instant::now();
                            loop {
                                match session.child.try_wait() {
                                    Ok(Some(_)) => break,
                                    Ok(None) => {
                                        if start.elapsed() > std::time::Duration::from_secs(2) {
                                            safe_eprintln!("[cleanup] wait timed out for PTY {}", sid);
                                            break;
                                        }
                                        std::thread::sleep(std::time::Duration::from_millis(50));
                                    }
                                    Err(e) => {
                                        safe_eprintln!("[cleanup] wait failed for {}: {}", sid, e);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    // Disconnect all MCP servers
                    let mcp_state = _app.state::<mcp::McpState>();
                    let registry = mcp_state.registry.clone();
                    tauri::async_runtime::spawn(async move {
                        let mut reg = registry.lock().await;
                        reg.disconnect_all().await;
                    });
                }
            }
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = &_event {
                for url in urls {
                    if let Ok(path) = url.to_file_path() {
                        if let Some(path_str) = path.to_str() {
                            if is_supported_file(path_str) {
                                let _ = _app.emit("file-open", path_str.to_string());
                            }
                        }
                    }
                }
            }
        });
}
