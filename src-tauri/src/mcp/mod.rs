pub mod protocol;
pub mod transport;
pub mod client;
pub mod registry;

use std::sync::Arc;
use tokio::sync::Mutex;
use serde::Serialize;
use registry::{McpRegistry, McpServerConfig, McpServerStatus, McpToolInfo, McpToolCallLog};

pub struct McpState {
    pub registry: Arc<Mutex<McpRegistry>>,
    pub health_check_handle: std::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct McpToolInfoResponse {
    server_id: String,
    server_name: String,
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[tauri::command]
pub async fn mcp_add_server(
    state: tauri::State<'_, McpState>,
    config: McpServerConfig,
) -> Result<(), String> {
    let mut registry = state.registry.lock().await;
    registry.add_server(config).await
}

#[tauri::command]
pub async fn mcp_remove_server(
    state: tauri::State<'_, McpState>,
    id: String,
) -> Result<(), String> {
    let mut registry = state.registry.lock().await;
    registry.remove_server(&id).await
}

#[tauri::command]
pub async fn mcp_restart_server(
    state: tauri::State<'_, McpState>,
    id: String,
) -> Result<(), String> {
    let mut registry = state.registry.lock().await;
    registry.restart_server(&id).await
}

#[tauri::command]
pub async fn mcp_list_servers(
    state: tauri::State<'_, McpState>,
) -> Result<Vec<McpServerStatus>, String> {
    let registry = state.registry.lock().await;
    Ok(registry.server_statuses())
}

#[tauri::command]
pub async fn mcp_list_tools(
    state: tauri::State<'_, McpState>,
) -> Result<Vec<McpToolInfo>, String> {
    let registry = state.registry.lock().await;
    let tools = registry.all_tools();
    let statuses = registry.server_statuses();
    let name_map: std::collections::HashMap<String, String> = statuses.into_iter()
        .map(|s| (s.id, s.name))
        .collect();
    Ok(tools.into_iter().map(|(server_id, tool)| {
        McpToolInfo {
            server_name: name_map.get(&server_id).cloned().unwrap_or_default(),
            server_id,
            name: tool.name,
            description: tool.description,
            input_schema: tool.input_schema,
        }
    }).collect())
}

#[tauri::command]
pub async fn mcp_tool_logs(
    state: tauri::State<'_, McpState>,
) -> Result<Vec<McpToolCallLog>, String> {
    let registry = state.registry.lock().await;
    Ok(registry.tool_logs().to_vec())
}

pub fn start_health_check(registry: Arc<Mutex<McpRegistry>>) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let mut reg = registry.lock().await;
            reg.health_check().await;
        }
    })
}
