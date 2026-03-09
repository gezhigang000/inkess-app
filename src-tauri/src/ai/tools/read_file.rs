use async_trait::async_trait;
use serde_json::Value;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::sandbox_path;
use crate::do_read_file;

pub struct ReadFileTool;

#[async_trait]
impl ToolPlugin for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    fn description(&self) -> &str { "Read file content" }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "File path" } },
            "required": ["path"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let raw_path = input["path"].as_str().unwrap_or("");
        let path = match sandbox_path(raw_path, &ctx.workspace_path) {
            Some(p) => p,
            None => return Ok(ToolOutput::error(format!("Access denied: path '{}' is outside the current workspace.", raw_path))),
        };
        // Binary files should be read via Python, not as text
        let ext = std::path::Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let binary_exts = ["xlsx", "xls", "pdf", "docx", "doc", "pptx", "ppt",
            "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "svg",
            "zip", "tar", "gz", "rar", "7z", "exe", "dll", "so", "dylib"];
        if binary_exts.contains(&ext.as_str()) {
            return Ok(ToolOutput::error(format!("This file is binary format (.{}), cannot be read as text. Use run_python tool with appropriate libraries (e.g. openpyxl for xlsx, Pillow for images).", ext)));
        }
        match do_read_file(&path) {
            Ok(content) => {
                if content.len() > 8000 {
                    let truncated: String = content.chars().take(8000).collect();
                    Ok(ToolOutput::success(format!("{}...\n\n(file too long, truncated, {} chars total)", truncated, content.len())))
                } else {
                    Ok(ToolOutput::success(content))
                }
            }
            Err(e) => Ok(ToolOutput::success(format!("Error: {}", e))),
        }
    }
}
