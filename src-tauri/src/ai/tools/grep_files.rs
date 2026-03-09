use async_trait::async_trait;
use serde_json::Value;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::sandbox_path;
use crate::fileops::grep_files;

pub struct GrepFilesTool;

#[async_trait]
impl ToolPlugin for GrepFilesTool {
    fn name(&self) -> &str { "grep_files" }
    fn description(&self) -> &str { "Search file contents by keyword. Returns matching lines with file path and line number. Use this to find code, text, or patterns inside files." }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "dir": { "type": "string", "description": "Search directory" },
                "pattern": { "type": "string", "description": "Search keyword (case-insensitive)" },
                "file_pattern": { "type": "string", "description": "Optional filename filter, e.g. *.rs, *.tsx" }
            },
            "required": ["dir", "pattern"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let raw_dir = input["dir"].as_str().unwrap_or(".");
        let dir = match sandbox_path(raw_dir, &ctx.workspace_path) {
            Some(p) => p,
            None => return Ok(ToolOutput::error(format!("Access denied: path '{}' is outside the current workspace.", raw_dir))),
        };
        let pattern = input["pattern"].as_str().unwrap_or("");
        let file_pattern = input["file_pattern"].as_str().map(|s| s.to_string());
        match grep_files(dir, pattern.to_string(), file_pattern) {
            Ok(results) => {
                if results.is_empty() {
                    Ok(ToolOutput::success("No matching content found".to_string()))
                } else {
                    Ok(ToolOutput::success(results.join("\n")))
                }
            }
            Err(e) => Ok(ToolOutput::success(format!("Error: {}", e))),
        }
    }
}
