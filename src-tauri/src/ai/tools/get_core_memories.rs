use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::MemoryStoreState;

pub struct GetCoreMemoriesTool;

#[async_trait]
impl ToolPlugin for GetCoreMemoriesTool {
    fn name(&self) -> &str { "get_core_memories" }

    fn description(&self) -> &str {
        "Retrieve all core memories - persistent facts about the user, project, and important context that should always be remembered."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, ctx: &ToolContext, _input: Value) -> Result<ToolOutput, ToolError> {
        let memory_store_state = ctx.app_handle.state::<MemoryStoreState>();
        match memory_store_state.store.get_core_memories().await {
            Ok(memories) => {
                if memories.is_empty() {
                    Ok(ToolOutput::success("No core memories found.".to_string()))
                } else {
                    let formatted = memories.iter().enumerate().map(|(i, m)| {
                        format!(
                            "[{}] (importance: {:.2})\n{}",
                            i + 1,
                            m.importance,
                            m.content
                        )
                    }).collect::<Vec<_>>().join("\n\n");

                    Ok(ToolOutput::success(format!(
                        "Core memories ({} total):\n\n{}",
                        memories.len(),
                        formatted
                    )))
                }
            }
            Err(e) => Ok(ToolOutput::error(format!("Failed to retrieve core memories: {}", e))),
        }
    }
}
