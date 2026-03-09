use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::memory::{Memory, MemoryMetadata, MemoryType};
use crate::ai::MemoryStoreState;

pub struct SaveMemoryTool;

#[async_trait]
impl ToolPlugin for SaveMemoryTool {
    fn name(&self) -> &str { "save_memory" }

    fn description(&self) -> &str {
        "Save important information to long-term memory. Use this to remember key facts about the user, project, or learnings from conversations."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The information to remember"
                },
                "memory_type": {
                    "type": "string",
                    "enum": ["Core", "Episodic", "Procedural", "Semantic"],
                    "description": "Type of memory: Core (persistent facts), Episodic (conversation summaries), Procedural (how-to knowledge), Semantic (general facts)"
                },
                "importance": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "description": "Importance score from 0.0 to 1.0"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tags for categorization"
                }
            },
            "required": ["content", "memory_type", "importance"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let content = input["content"].as_str()
            .ok_or_else(|| ToolError::MissingArgument("content".to_string()))?;

        let memory_type_str = input["memory_type"].as_str()
            .ok_or_else(|| ToolError::MissingArgument("memory_type".to_string()))?;

        let memory_type = MemoryType::from_str(memory_type_str)
            .map_err(|e| ToolError::InvalidArgument(e))?;

        let importance = input["importance"].as_f64()
            .ok_or_else(|| ToolError::MissingArgument("importance".to_string()))? as f32;

        // Clamp importance to valid range
        let importance = importance.clamp(0.0, 1.0);

        let tags = input["tags"].as_array()
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>())
            .unwrap_or_default();

        let memory = Memory {
            id: uuid::Uuid::new_v4().to_string(),
            content: content.to_string(),
            memory_type,
            importance,
            metadata: MemoryMetadata {
                tags,
                source: "ai_assistant".to_string(),
                workspace_path: if ctx.workspace_path.is_empty() {
                    None
                } else {
                    Some(ctx.workspace_path.clone())
                },
            },
            created_at: chrono::Utc::now().timestamp(),
            accessed_at: chrono::Utc::now().timestamp(),
            access_count: 0,
        };

        let memory_store_state = ctx.app_handle.state::<MemoryStoreState>();
        match memory_store_state.store.save(memory.clone()).await {
            Ok(id) => Ok(ToolOutput::success(format!(
                "Memory saved successfully (ID: {})\nType: {}\nImportance: {:.2}",
                id, memory_type_str, importance
            ))),
            Err(e) => Ok(ToolOutput::error(format!("Failed to save memory: {}", e))),
        }
    }
}
