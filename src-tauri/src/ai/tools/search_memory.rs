use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::MemoryStoreState;

pub struct SearchMemoryTool;

#[async_trait]
impl ToolPlugin for SearchMemoryTool {
    fn name(&self) -> &str { "search_memory" }

    fn description(&self) -> &str {
        "Search through saved memories to recall relevant information from past conversations and learnings."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query to find relevant memories"
                },
                "limit": {
                    "type": "number",
                    "minimum": 1,
                    "maximum": 20,
                    "description": "Maximum number of results to return (default: 5)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let query = input["query"].as_str()
            .ok_or_else(|| ToolError::MissingArgument("query".to_string()))?;

        let limit = input["limit"].as_u64()
            .unwrap_or(5)
            .min(20) as usize;

        let memory_store_state = ctx.app_handle.state::<MemoryStoreState>();
        match memory_store_state.store.search(query, limit).await {
            Ok(memories) => {
                if memories.is_empty() {
                    Ok(ToolOutput::success("No matching memories found.".to_string()))
                } else {
                    let formatted = memories.iter().enumerate().map(|(i, m)| {
                        let preview = if m.content.chars().count() > 200 {
                            let truncated: String = m.content.chars().take(200).collect();
                            format!("{}...", truncated)
                        } else {
                            m.content.clone()
                        };
                        format!(
                            "[{}] {} (importance: {:.2})\n{}",
                            i + 1,
                            m.memory_type.as_str(),
                            m.importance,
                            preview
                        )
                    }).collect::<Vec<_>>().join("\n\n");

                    Ok(ToolOutput::success(format!(
                        "Found {} matching memories:\n\n{}",
                        memories.len(),
                        formatted
                    )))
                }
            }
            Err(e) => Ok(ToolOutput::error(format!("Memory search failed: {}", e))),
        }
    }
}
