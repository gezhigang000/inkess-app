use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};

pub struct SearchKnowledgeTool;

#[async_trait]
impl ToolPlugin for SearchKnowledgeTool {
    fn name(&self) -> &str { "search_knowledge" }
    fn description(&self) -> &str { "Search for relevant content across all files in the current project using full-text search. Use this when the user asks about project content, code, or documentation." }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search keywords" }
            },
            "required": ["query"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let query = input["query"].as_str().unwrap_or("");
        let bm25_state = ctx.app_handle.state::<crate::bm25::Bm25State>();
        let guard = bm25_state.index.lock().map_err(|e|
            ToolError::ExecutionFailed(format!("BM25 index lock poisoned: {}", e))
        )?;
        match guard.as_ref() {
            Some(index) => {
                let results = index.search(query, 5);
                if results.is_empty() {
                    Ok(ToolOutput::success("No relevant content found.".to_string()))
                } else {
                    let text = results.iter().enumerate().map(|(i, r)| {
                        format!("[{}] {}:{}-{}\n{}", i + 1, r.path, r.start_line, r.end_line, r.content)
                    }).collect::<Vec<_>>().join("\n\n");
                    Ok(ToolOutput::success(text))
                }
            }
            None => Ok(ToolOutput::success("Search index not initialized. Open a directory first.".to_string())),
        }
    }
}
