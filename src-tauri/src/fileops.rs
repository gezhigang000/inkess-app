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
        // Skip symlinks to prevent traversal attacks and infinite loops
        if let Ok(ft) = entry.file_type() {
            if ft.is_symlink() { continue; }
        }
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

#[tauri::command]
pub fn copy_file_to_dir(src: String, dest_dir: String) -> Result<String, String> {
    let src_path = PathBuf::from(&src)
        .canonicalize()
        .map_err(|_| "Source file does not exist".to_string())?;
    if !src_path.is_file() {
        return Err("Source is not a file".to_string());
    }
    let dest_path = PathBuf::from(&dest_dir)
        .canonicalize()
        .map_err(|_| "Destination directory does not exist".to_string())?;
    if !dest_path.is_dir() {
        return Err("Destination is not a directory".to_string());
    }
    // Validate both paths against blocked paths
    let src_str = src_path.to_string_lossy();
    let dest_str = dest_path.to_string_lossy();
    for blocked in BLOCKED_PATHS {
        if src_str.contains(blocked) || dest_str.contains(blocked) {
            return Err("Permission denied".to_string());
        }
    }
    let file_name = src_path
        .file_name()
        .ok_or("Invalid source filename".to_string())?
        .to_string_lossy()
        .to_string();
    let stem = src_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = src_path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    let mut target = dest_path.join(&file_name);
    let mut counter = 1;
    while target.exists() {
        target = dest_path.join(format!("{}-{}{}", stem, counter, ext));
        counter += 1;
    }
    fs::copy(&src_path, &target)
        .map_err(|e| format!("Copy failed: {}", e))?;
    let final_name = target
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    Ok(final_name)
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
    if end == 0 {
        return "[truncated]".to_string();
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
        // Skip symlinks to prevent traversal attacks and infinite loops
        if let Ok(ft) = entry.file_type() {
            if ft.is_symlink() { continue; }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // --- truncate_line tests ---

    #[test]
    fn truncate_line_short_string() {
        assert_eq!(truncate_line("hello", 500), "hello");
    }

    #[test]
    fn truncate_line_exact_limit() {
        let s = "a".repeat(500);
        assert_eq!(truncate_line(&s, 500), s);
    }

    #[test]
    fn truncate_line_exceeds_limit() {
        let s = "a".repeat(510);
        let result = truncate_line(&s, 500);
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), 503); // 500 + "..."
    }

    #[test]
    fn truncate_line_multibyte_boundary() {
        // Chinese chars are 3 bytes each
        let s = "\u{4e2d}".repeat(200); // 600 bytes total
        let result = truncate_line(&s, 500);
        assert!(result.ends_with("..."));
        // Should truncate at a valid char boundary (multiple of 3)
        let without_dots = &result[..result.len() - 3];
        assert!(without_dots.len() <= 500);
        // Verify it's valid UTF-8 by checking we can iterate chars
        assert!(without_dots.chars().count() > 0);
    }

    #[test]
    fn truncate_line_empty_string() {
        assert_eq!(truncate_line("", 500), "");
    }

    // --- matches_file_pattern tests ---

    #[test]
    fn matches_file_pattern_glob_ext() {
        assert!(matches_file_pattern("foo.rs", "*.rs"));
        assert!(matches_file_pattern("FOO.RS", "*.rs"));
        assert!(!matches_file_pattern("foo.py", "*.rs"));
    }

    #[test]
    fn matches_file_pattern_exact() {
        assert!(matches_file_pattern("Cargo.toml", "cargo.toml"));
        assert!(!matches_file_pattern("cargo.lock", "cargo.toml"));
    }

    // --- is_binary tests ---

    #[test]
    fn is_binary_text_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "hello world").unwrap();
        assert!(!is_binary(&path.to_path_buf()));
    }

    #[test]
    fn is_binary_with_null_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        fs::write(&path, b"hello\x00world").unwrap();
        assert!(is_binary(&path.to_path_buf()));
    }

    #[test]
    fn is_binary_nonexistent() {
        let path = PathBuf::from("/tmp/nonexistent_file_12345");
        assert!(!is_binary(&path));
    }

    // --- search_recursive tests ---

    #[test]
    fn search_files_empty_query_returns_empty() {
        let result = search_files("/tmp".to_string(), "  ".to_string());
        assert_eq!(result.unwrap(), Vec::<String>::new());
    }

    #[test]
    fn search_files_finds_matching_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("hello.txt"), "content").unwrap();
        fs::write(root.join("world.md"), "content").unwrap();
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("sub").join("hello.rs"), "code").unwrap();

        let result = search_files(root.to_string_lossy().to_string(), "hello".to_string()).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|r| r.contains("hello.txt")));
        assert!(result.iter().any(|r| r.contains("hello.rs")));
    }

    #[test]
    fn search_files_skips_hidden_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join(".hidden")).unwrap();
        fs::write(root.join(".hidden").join("secret.txt"), "data").unwrap();
        fs::write(root.join("visible.txt"), "data").unwrap();

        let result = search_files(root.to_string_lossy().to_string(), "txt".to_string()).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("visible"));
    }

    #[test]
    fn search_files_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("README.md"), "content").unwrap();

        let result = search_files(root.to_string_lossy().to_string(), "readme".to_string()).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn search_files_respects_max_results() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for i in 0..60 {
            fs::write(root.join(format!("file{}.txt", i)), "data").unwrap();
        }

        let result = search_files(root.to_string_lossy().to_string(), "file".to_string()).unwrap();
        assert!(result.len() <= SEARCH_MAX_RESULTS);
    }

    // --- copy_file_to_dir naming collision tests ---

    #[test]
    fn copy_file_to_dir_basic() {
        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();
        let src_file = src_dir.path().join("image.png");
        fs::write(&src_file, b"fake image data").unwrap();

        let result = copy_file_to_dir(
            src_file.to_string_lossy().to_string(),
            dest_dir.path().to_string_lossy().to_string(),
        ).unwrap();
        assert_eq!(result, "image.png");
        assert!(dest_dir.path().join("image.png").exists());
    }

    #[test]
    fn copy_file_to_dir_collision_numbering() {
        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();
        let src_file = src_dir.path().join("photo.jpg");
        fs::write(&src_file, b"data").unwrap();

        // Create existing files in destination to trigger collision
        fs::write(dest_dir.path().join("photo.jpg"), b"existing").unwrap();
        fs::write(dest_dir.path().join("photo-1.jpg"), b"existing").unwrap();

        let result = copy_file_to_dir(
            src_file.to_string_lossy().to_string(),
            dest_dir.path().to_string_lossy().to_string(),
        ).unwrap();
        assert_eq!(result, "photo-2.jpg");
        assert!(dest_dir.path().join("photo-2.jpg").exists());
    }

    #[test]
    fn copy_file_to_dir_no_extension() {
        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();
        let src_file = src_dir.path().join("Makefile");
        fs::write(&src_file, b"all:").unwrap();
        fs::write(dest_dir.path().join("Makefile"), b"existing").unwrap();

        let result = copy_file_to_dir(
            src_file.to_string_lossy().to_string(),
            dest_dir.path().to_string_lossy().to_string(),
        ).unwrap();
        assert_eq!(result, "Makefile-1");
    }

    #[test]
    fn copy_file_to_dir_src_not_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = copy_file_to_dir(
            dir.path().to_string_lossy().to_string(),
            "/tmp".to_string(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn copy_file_to_dir_dest_not_dir() {
        let src_dir = tempfile::tempdir().unwrap();
        let src_file = src_dir.path().join("test.txt");
        fs::write(&src_file, b"data").unwrap();

        let result = copy_file_to_dir(
            src_file.to_string_lossy().to_string(),
            "/tmp/nonexistent_dir_xyz_12345".to_string(),
        );
        assert!(result.is_err());
    }

    // --- grep_files tests ---

    #[test]
    fn grep_files_empty_pattern() {
        let result = grep_files("/tmp".to_string(), "  ".to_string(), None);
        assert_eq!(result.unwrap(), Vec::<String>::new());
    }

    #[test]
    fn grep_files_finds_content() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("test.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

        let result = grep_files(
            root.to_string_lossy().to_string(),
            "println".to_string(),
            None,
        ).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("test.rs:2:"));
        assert!(result[0].contains("println"));
    }

    #[test]
    fn grep_files_with_file_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("code.rs"), "let x = 1;\n").unwrap();
        fs::write(root.join("code.py"), "x = 1\n").unwrap();

        let result = grep_files(
            root.to_string_lossy().to_string(),
            "x".to_string(),
            Some("*.rs".to_string()),
        ).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("code.rs"));
    }

    #[test]
    fn grep_files_skips_binary() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("binary.dat"), b"hello\x00world\n").unwrap();
        fs::write(root.join("text.txt"), "hello world\n").unwrap();

        let result = grep_files(
            root.to_string_lossy().to_string(),
            "hello".to_string(),
            None,
        ).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("text.txt"));
    }

    // --- validate_parent tests (indirect via create_file) ---

    #[test]
    fn create_file_in_valid_dir() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("new_file.txt");
        let result = create_file(file_path.to_string_lossy().to_string(), "content".to_string());
        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "content");
    }

    #[test]
    fn create_file_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("existing.txt");
        fs::write(&file_path, "old").unwrap();
        let result = create_file(file_path.to_string_lossy().to_string(), "new".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }
}
