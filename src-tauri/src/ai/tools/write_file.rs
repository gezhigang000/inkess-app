use std::fs;
use async_trait::async_trait;
use serde_json::Value;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::sandbox_path;

pub struct WriteFileTool;

#[async_trait]
impl ToolPlugin for WriteFileTool {
    fn name(&self) -> &str { "write_file" }
    fn description(&self) -> &str { "Write content to a file in the current workspace. Use to save reports, analysis results, translations, or generated code. Paths are relative to the workspace root. Cannot write outside workspace or to sensitive paths (.env, .git/)." }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path relative to workspace (e.g. report.md, output/analysis.html)" },
                "content": { "type": "string", "description": "File content to write" }
            },
            "required": ["path", "content"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let raw_path = input["path"].as_str().unwrap_or("");
        let content = input["content"].as_str().unwrap_or("");
        let result = write_file_tool(raw_path, content, &ctx.workspace_path);
        Ok(ToolOutput::success(result))
    }
}

fn write_file_tool(raw_path: &str, content: &str, cwd: &str) -> String {
    if cwd.is_empty() {
        return "Cannot write files: no workspace directory is open.".to_string();
    }
    if raw_path.is_empty() {
        return "Please provide a file path".to_string();
    }
    if content.len() > 1024 * 1024 {
        return format!("Content too large: {} bytes (max 1MB)", content.len());
    }

    // Block sensitive paths
    let lower = raw_path.to_lowercase();
    let sensitive = [".env", ".git/", ".git\\", ".ssh/", ".ssh\\",
        ".bash_history", ".zsh_history", ".npmrc", ".pypirc",
        ".docker/", ".docker\\", ".kube/", ".kube\\",
        ".aws/", ".aws\\", ".config/gh", ".config\\gh",
        ".gnupg/", ".gnupg\\", ".netrc"];
    for s in &sensitive {
        if lower.contains(s) || lower == ".env" {
            return format!("Cannot write to sensitive path: {}", raw_path);
        }
    }
    // Block dotfiles at root
    if raw_path.starts_with('.') && !raw_path.contains('/') && !raw_path.contains('\\') {
        return format!("Cannot write to dotfile: {}", raw_path);
    }

    let path = match sandbox_path(raw_path, cwd) {
        Some(p) => p,
        None => return format!("Access denied: path '{}' is outside the current workspace.", raw_path),
    };

    // Create parent directories
    if let Some(parent) = std::path::Path::new(&path).parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return format!("Failed to create directory: {}", e);
        }
    }

    match fs::write(&path, content) {
        Ok(_) => format!("File written: {} ({} bytes)", path, content.len()),
        Err(e) => format!("Failed to write file: {}", e),
    }
}
