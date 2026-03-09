use std::fs;
use std::path::Path;
use async_trait::async_trait;
use serde_json::Value;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::sandbox_path;

pub struct FileInfoTool;

#[async_trait]
impl ToolPlugin for FileInfoTool {
    fn name(&self) -> &str { "file_info" }
    fn description(&self) -> &str {
        "Get file or directory metadata: size, line count, last modified time, file type. Use this instead of run_python for basic file information."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File or directory path" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let raw_path = input["path"].as_str().unwrap_or("");
        let path = match sandbox_path(raw_path, &ctx.workspace_path) {
            Some(p) => p,
            None => return Ok(ToolOutput::error(format!(
                "Access denied: path '{}' is outside the current workspace.", raw_path
            ))),
        };

        let p = Path::new(&path);
        let meta = match fs::metadata(p) {
            Ok(m) => m,
            Err(e) => return Ok(ToolOutput::error(format!("Failed to read metadata: {}", e))),
        };

        let mut info = Vec::new();
        info.push(format!("Path: {}", path));

        if meta.is_dir() {
            info.push("Type: Directory".to_string());
            let count = match fs::read_dir(p) {
                Ok(entries) => entries.count(),
                Err(_) => 0,
            };
            info.push(format!("Children: {}", count));
        } else {
            let ext = p.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let file_type = detect_file_type(&ext, p);
            info.push(format!("Type: {}", file_type));

            let size = meta.len();
            info.push(format!("Size: {} ({} bytes)", format_size(size), size));

            // Count lines for text files
            let binary_exts = ["xlsx", "xls", "pdf", "docx", "doc", "pptx", "ppt",
                "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico",
                "zip", "tar", "gz", "rar", "7z", "exe", "dll", "so", "dylib",
                "woff", "woff2", "ttf", "otf", "eot", "mp3", "mp4", "wav", "avi"];
            if !binary_exts.contains(&ext.as_str()) {
                if let Ok(content) = fs::read_to_string(p) {
                    let line_count = content.lines().count();
                    info.push(format!("Lines: {}", line_count));
                }
            }
        }

        // Modified time
        if let Ok(modified) = meta.modified() {
            let datetime: chrono::DateTime<chrono::Utc> = modified.into();
            info.push(format!("Modified: {}", datetime.format("%Y-%m-%d %H:%M:%S UTC")));
        }

        // Permissions (unix)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = meta.permissions().mode();
            info.push(format!("Permissions: {:o}", mode & 0o7777));
        }
        if meta.permissions().readonly() {
            info.push("Readonly: yes".to_string());
        }

        Ok(ToolOutput::success(info.join("\n")))
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn detect_file_type(ext: &str, path: &Path) -> String {
    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    match ext {
        "rs" => "Rust source",
        "ts" | "tsx" => "TypeScript source",
        "js" | "jsx" => "JavaScript source",
        "py" => "Python source",
        "go" => "Go source",
        "java" => "Java source",
        "c" => "C source",
        "cpp" | "cc" | "cxx" => "C++ source",
        "h" | "hpp" => "C/C++ header",
        "rb" => "Ruby source",
        "php" => "PHP source",
        "swift" => "Swift source",
        "kt" => "Kotlin source",
        "dart" => "Dart source",
        "vue" => "Vue component",
        "svelte" => "Svelte component",
        "html" | "htm" => "HTML document",
        "css" | "scss" | "sass" | "less" => "Stylesheet",
        "json" => "JSON data",
        "yaml" | "yml" => "YAML data",
        "toml" => "TOML config",
        "xml" => "XML document",
        "md" | "markdown" => "Markdown document",
        "txt" => "Plain text",
        "sh" | "bash" | "zsh" => "Shell script",
        "sql" => "SQL script",
        "png" => "PNG image",
        "jpg" | "jpeg" => "JPEG image",
        "gif" => "GIF image",
        "svg" => "SVG image",
        "webp" => "WebP image",
        "pdf" => "PDF document",
        "docx" | "doc" => "Word document",
        "xlsx" | "xls" => "Excel spreadsheet",
        "pptx" | "ppt" => "PowerPoint presentation",
        "zip" => "ZIP archive",
        "tar" => "TAR archive",
        "gz" => "Gzip archive",
        "exe" => "Windows executable",
        "dll" => "Windows library",
        "so" => "Shared library",
        "dylib" => "macOS dynamic library",
        "lock" => "Lock file",
        "log" => "Log file",
        "env" => "Environment config",
        "conf" | "cfg" | "ini" => "Configuration file",
        "properties" => "Properties file",
        "" => {
            // Check well-known filenames
            match name {
                "Makefile" | "makefile" => "Makefile",
                "Dockerfile" => "Dockerfile",
                "Cargo.toml" => "Cargo manifest",
                "package.json" => "Node.js manifest",
                ".gitignore" => "Git ignore rules",
                _ => "File",
            }
        }
        _ => "File",
    }.to_string()
}
