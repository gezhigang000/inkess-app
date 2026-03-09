use async_trait::async_trait;
use serde_json::Value;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::sandbox_path;
use crate::do_list_directory;

pub struct ListDirectoryTool;

#[async_trait]
impl ToolPlugin for ListDirectoryTool {
    fn name(&self) -> &str { "list_directory" }
    fn description(&self) -> &str { "List files and folders in a directory" }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "Directory path" } },
            "required": ["path"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let raw_path = input["path"].as_str().unwrap_or(".");
        let path = match sandbox_path(raw_path, &ctx.workspace_path) {
            Some(p) => p,
            None => return Ok(ToolOutput::error(format!("Access denied: path '{}' is outside the current workspace.", raw_path))),
        };
        match do_list_directory(&path) {
            Ok(listing) => {
                let names: Vec<String> = listing.entries.iter().map(|e| {
                    if e.is_dir { format!("{}/", e.name) } else { e.name.clone() }
                }).collect();
                if listing.truncated {
                    Ok(ToolOutput::success(format!("(showing {}/{})\n{}", listing.entries.len(), listing.total, names.join("\n"))))
                } else {
                    Ok(ToolOutput::success(names.join("\n")))
                }
            }
            Err(e) => Ok(ToolOutput::success(format!("Error: {}", e))),
        }
    }
}
