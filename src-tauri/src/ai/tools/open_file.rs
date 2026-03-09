use async_trait::async_trait;
use serde_json::Value;
use tauri::Emitter;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::sandbox_path;

pub struct OpenFileTool;

#[async_trait]
impl ToolPlugin for OpenFileTool {
    fn name(&self) -> &str { "open_file" }
    fn description(&self) -> &str { "Open a file for the user to view. Markdown/HTML/text files open in Inkess viewer, other files open with system default app. Use after write_file to show generated reports to the user." }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path relative to workspace" }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let raw_path = input["path"].as_str().unwrap_or("");
        if ctx.workspace_path.is_empty() {
            return Ok(ToolOutput::error("Cannot open files: no workspace directory is open.".to_string()));
        }
        let path = match sandbox_path(raw_path, &ctx.workspace_path) {
            Some(p) => p,
            None => return Ok(ToolOutput::error(format!("Access denied: path '{}' is outside the current workspace.", raw_path))),
        };
        let _ = ctx.app_handle.emit("open-file-request", serde_json::json!({ "path": path }));
        Ok(ToolOutput::success(format!("Opened file: {}", path)))
    }
}
