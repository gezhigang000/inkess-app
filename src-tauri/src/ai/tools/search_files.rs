use async_trait::async_trait;
use serde_json::Value;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::sandbox_path;
use crate::fileops::search_files;

pub struct SearchFilesTool;

#[async_trait]
impl ToolPlugin for SearchFilesTool {
    fn name(&self) -> &str { "search_files" }
    fn description(&self) -> &str { "Search by filename" }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "dir": { "type": "string", "description": "Search directory" },
                "query": { "type": "string", "description": "Search keyword" }
            },
            "required": ["dir", "query"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let raw_dir = input["dir"].as_str().unwrap_or(".");
        let dir = match sandbox_path(raw_dir, &ctx.workspace_path) {
            Some(p) => p,
            None => return Ok(ToolOutput::error(format!("Access denied: path '{}' is outside the current workspace.", raw_dir))),
        };
        let query = input["query"].as_str().unwrap_or("");
        match search_files(dir, query.to_string()) {
            Ok(results) => {
                if results.is_empty() {
                    Ok(ToolOutput::success("No matching files found".to_string()))
                } else {
                    Ok(ToolOutput::success(results.join("\n")))
                }
            }
            Err(e) => Ok(ToolOutput::success(format!("Error: {}", e))),
        }
    }
}
