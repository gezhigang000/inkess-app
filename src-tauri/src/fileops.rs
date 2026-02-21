use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use crate::BLOCKED_PATHS;

const SEARCH_MAX_RESULTS: usize = 50;
const SEARCH_MAX_DEPTH: usize = 8;

fn validate_parent(path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    let parent = p.parent().ok_or("Invalid path".to_string())?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| "Cannot access parent directory".to_string())?;
    let path_str = canonical_parent.to_string_lossy();
    for blocked in BLOCKED_PATHS {
        if path_str.contains(blocked) {
            return Err("Permission denied".to_string());
        }
    }
    let file_name = p
        .file_name()
        .ok_or("Invalid filename".to_string())?;
    Ok(canonical_parent.join(file_name))
}

#[tauri::command]
pub fn create_file(path: String, template: String) -> Result<(), String> {
    let target = validate_parent(&path)?;
    if target.exists() {
        return Err("File already exists".to_string());
    }
    fs::write(&target, template.as_bytes())
        .map_err(|e| format!("Failed to create file: {}", e))
}

#[tauri::command]
pub fn create_directory(path: String) -> Result<(), String> {
    let target = validate_parent(&path)?;
    if target.exists() {
        return Err("Directory already exists".to_string());
    }
    fs::create_dir(&target)
        .map_err(|e| format!("Failed to create directory: {}", e))
}

#[tauri::command]
pub fn rename_entry(old_path: String, new_path: String) -> Result<(), String> {
    let old = PathBuf::from(&old_path)
        .canonicalize()
        .map_err(|_| "Source path does not exist".to_string())?;
    let new = validate_parent(&new_path)?;
    let old_str = old.to_string_lossy();
    for blocked in BLOCKED_PATHS {
        if old_str.contains(blocked) {
            return Err("Permission denied".to_string());
        }
    }
    if new.exists() {
        return Err("Target path already exists".to_string());
    }
    fs::rename(&old, &new).map_err(|e| format!("Rename failed: {}", e))
}

#[tauri::command]
pub fn delete_to_trash(path: String) -> Result<(), String> {
    let canonical = PathBuf::from(&path)
        .canonicalize()
        .map_err(|_| "Path does not exist".to_string())?;
    let path_str = canonical.to_string_lossy();
    for blocked in BLOCKED_PATHS {
        if path_str.contains(blocked) {
            return Err("Permission denied".to_string());
        }
    }
    trash::delete(&canonical).map_err(|e| format!("Delete failed: {}", e))
}

#[tauri::command]
pub fn search_files(dir: String, query: String) -> Result<Vec<String>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    let root = PathBuf::from(&dir)
        .canonicalize()
        .map_err(|_| "Directory does not exist".to_string())?;
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();
    search_recursive(&root, &root, &query_lower, 0, &mut results);
    Ok(results)
}

fn search_recursive(
    root: &PathBuf,
    dir: &PathBuf,
    query: &str,
    depth: usize,
    results: &mut Vec<String>,
) {
    if depth > SEARCH_MAX_DEPTH || results.len() >= SEARCH_MAX_RESULTS {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        if results.len() >= SEARCH_MAX_RESULTS {
            return;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files/dirs
        if name.starts_with('.') {
            continue;
        }
        if name.to_lowercase().contains(query) {
            if let Ok(rel) = path.strip_prefix(root) {
                results.push(rel.to_string_lossy().replace('\\', "/"));
            } else {
                results.push(path.to_string_lossy().to_string());
            }
        }
        if path.is_dir() {
            search_recursive(root, &path, query, depth + 1, results);
        }
    }
}

const GREP_MAX_LINE_LEN: usize = 500;

fn truncate_line(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

pub fn grep_files(dir: String, pattern: String, file_pattern: Option<String>) -> Result<Vec<String>, String> {
    if pattern.trim().is_empty() {
        return Ok(vec![]);
    }
    let root = PathBuf::from(&dir)
        .canonicalize()
        .map_err(|_| "Directory does not exist".to_string())?;
    let pattern_lower = pattern.to_lowercase();
    let mut results = Vec::new();
    grep_recursive(&root, &root, &pattern_lower, file_pattern.as_deref(), 0, &mut results);
    Ok(results)
}

fn matches_file_pattern(name: &str, pattern: &str) -> bool {
    // Simple glob: *.ext or exact match
    if let Some(ext) = pattern.strip_prefix("*.") {
        name.to_lowercase().ends_with(&format!(".{}", ext.to_lowercase()))
    } else {
        name.to_lowercase() == pattern.to_lowercase()
    }
}

fn is_binary(path: &PathBuf) -> bool {
    if let Ok(f) = fs::File::open(path) {
        let mut reader = BufReader::new(f);
        let mut buf = [0u8; 512];
        if let Ok(n) = std::io::Read::read(&mut reader, &mut buf) {
            return buf[..n].contains(&0);
        }
    }
    false
}

fn grep_recursive(
    root: &PathBuf,
    dir: &PathBuf,
    pattern: &str,
    file_pattern: Option<&str>,
    depth: usize,
    results: &mut Vec<String>,
) {
    if depth > SEARCH_MAX_DEPTH || results.len() >= SEARCH_MAX_RESULTS {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        if results.len() >= SEARCH_MAX_RESULTS {
            return;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            grep_recursive(root, &path, pattern, file_pattern, depth + 1, results);
        } else if path.is_file() {
            if let Some(fp) = file_pattern {
                if !matches_file_pattern(&name, fp) {
                    continue;
                }
            }
            if is_binary(&path) {
                continue;
            }
            if let Ok(file) = fs::File::open(&path) {
                let reader = BufReader::new(file);
                let rel = path.strip_prefix(root)
                    .map(|r| r.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| path.to_string_lossy().to_string());
                for (line_num, line) in reader.lines().enumerate() {
                    if results.len() >= SEARCH_MAX_RESULTS {
                        return;
                    }
                    if let Ok(line) = line {
                        if line.to_lowercase().contains(pattern) {
                            let display = truncate_line(&line, GREP_MAX_LINE_LEN);
                            results.push(format!("{}:{}: {}", rel, line_num + 1, display));
                        }
                    }
                }
            }
        }
    }
}
