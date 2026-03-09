use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;

use crate::ai::tool::{ToolContext, ToolError, ToolOutput, ToolPlugin};
use crate::ai::tool::registry::ToolRegistry;
use crate::mcp::McpState;

/// MCP tool name prefix used in ToolRegistry to avoid collisions with builtin tools.
/// Format: mcp__{server_id}__{tool_name}
pub const MCP_TOOL_PREFIX: &str = "mcp__";

/// A bridge that wraps an MCP tool as a ToolPlugin so it participates
/// in ToolRegistry-based skill filtering and unified tool execution.
pub struct McpBridgeTool {
    /// Prefixed name: mcp__{server_id}__{tool_name}
    prefixed_name: String,
    /// MCP server ID
    server_id: String,
    /// Original MCP tool name (without prefix)
    original_name: String,
    /// Tool description from MCP server
    tool_description: String,
    /// JSON Schema for input parameters
    schema: Value,
}

impl McpBridgeTool {
    pub fn new(
        server_id: String,
        tool_name: String,
        description: String,
        schema: Value,
    ) -> Self {
        let prefixed_name = format!("mcp__{}__{}",server_id, tool_name);
        Self {
            prefixed_name,
            server_id,
            original_name: tool_name,
            tool_description: description,
            schema,
        }
    }
}

#[async_trait]
impl ToolPlugin for McpBridgeTool {
    fn name(&self) -> &str {
        &self.prefixed_name
    }

    fn description(&self) -> &str {
        &self.tool_description
    }

    fn input_schema(&self) -> Value {
        self.schema.clone()
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let mcp_state = ctx.app_handle.try_state::<McpState>()
            .ok_or_else(|| ToolError::ExecutionFailed("MCP state not available".into()))?;

        let mut registry = mcp_state.registry.lock().await;
        match registry.call_tool(&self.server_id, &self.original_name, input).await {
            Ok(result) => {
                let text = result.content.iter()
                    .filter_map(|c| c.text.as_deref())
                    .collect::<Vec<_>>()
                    .join("\n");
                let is_err = result.is_error.unwrap_or(false);
                if is_err {
                    Ok(ToolOutput::error(format!("MCP tool error: {}", text)))
                } else {
                    Ok(ToolOutput::success(text))
                }
            }
            Err(e) => Ok(ToolOutput::error(format!("MCP tool call failed: {}", e))),
        }
    }
}

/// Synchronize MCP tools into the ToolRegistry.
///
/// 1. Removes all existing MCP bridge tools (by prefix)
/// 2. Reads current tools from McpRegistry
/// 3. Creates McpBridgeTool for each and registers in ToolRegistry
///
/// This function is idempotent — safe to call repeatedly.
pub async fn sync_mcp_tools(
    tool_registry: &ToolRegistry,
    mcp_registry: &tokio::sync::Mutex<crate::mcp::registry::McpRegistry>,
) {
    // 1. Remove all existing MCP tools from ToolRegistry
    tool_registry.remove_by_prefix(MCP_TOOL_PREFIX).await;

    // 2. Get current MCP tools
    let mcp_tools = {
        let registry = mcp_registry.lock().await;
        registry.all_tools()
    };

    // 3. Register each as McpBridgeTool
    for (server_id, tool_def) in mcp_tools {
        let bridge = McpBridgeTool::new(
            server_id,
            tool_def.name,
            tool_def.description,
            tool_def.input_schema,
        );
        tool_registry.register(std::sync::Arc::new(bridge)).await;
    }
}
