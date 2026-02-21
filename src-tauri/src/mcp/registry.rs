use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::client::McpClient;
use super::protocol::{McpToolDef, McpToolResult, McpTransportType};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub transport: McpTransportType,
    #[serde(default)]
    pub url: Option<String>,
}

fn default_true() -> bool { true }

#[derive(Serialize, Clone, Debug)]
pub struct McpServerStatus {
    pub id: String,
    pub name: String,
    pub connected: bool,
    pub tool_count: usize,
    pub error: Option<String>,
    pub transport: String,
    pub last_seen: Option<u64>,
}

#[derive(Serialize, Clone, Debug)]
pub struct McpToolInfo {
    pub server_id: String,
    pub server_name: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Serialize, Clone, Debug)]
pub struct McpToolCallLog {
    pub timestamp: u64,
    pub server_id: String,
    pub tool_name: String,
    pub arguments: String,
    pub result: String,
    pub duration_ms: u64,
    pub is_error: bool,
}

pub struct McpRegistry {
    servers: HashMap<String, McpClient>,
    configs: Vec<McpServerConfig>,
    errors: HashMap<String, String>,
    last_seen: HashMap<String, u64>,
    logs: Vec<McpToolCallLog>,
}

fn config_path() -> PathBuf {
    let data_dir = crate::app_data_dir();
    let dir = data_dir.join("inkess");
    fs::create_dir_all(&dir).ok();
    dir.join("mcp-servers.json")
}

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    // Find a valid char boundary
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

impl McpRegistry {
    pub fn new() -> Self {
        let configs = Self::load_configs();
        Self {
            servers: HashMap::new(),
            configs,
            errors: HashMap::new(),
            last_seen: HashMap::new(),
            logs: Vec::new(),
        }
    }

    fn load_configs() -> Vec<McpServerConfig> {
        let path = config_path();
        if let Ok(data) = fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    fn save_configs(&self) {
        let path = config_path();
        if let Ok(json) = serde_json::to_string_pretty(&self.configs) {
            let _ = fs::write(path, json);
        }
    }

    pub async fn add_server(&mut self, config: McpServerConfig) -> Result<(), String> {
        // Remove existing with same id
        self.configs.retain(|c| c.id != config.id);
        self.configs.push(config.clone());
        self.save_configs();

        if config.enabled {
            self.connect_server(&config.id).await?;
        }
        Ok(())
    }

    pub async fn remove_server(&mut self, id: &str) -> Result<(), String> {
        if let Some(mut client) = self.servers.remove(id) {
            let _ = client.disconnect().await;
        }
        self.configs.retain(|c| c.id != id);
        self.errors.remove(id);
        self.last_seen.remove(id);
        self.save_configs();
        Ok(())
    }

    pub async fn restart_server(&mut self, id: &str) -> Result<(), String> {
        if let Some(mut client) = self.servers.remove(id) {
            let _ = client.disconnect().await;
        }
        self.errors.remove(id);
        self.connect_server(id).await
    }

    async fn connect_server(&mut self, id: &str) -> Result<(), String> {
        let config = self.configs.iter().find(|c| c.id == id)
            .ok_or_else(|| format!("Server '{}' not found", id))?
            .clone();

        match McpClient::connect(&config).await {
            Ok(client) => {
                self.errors.remove(id);
                self.last_seen.insert(id.to_string(), now_ts());
                self.servers.insert(id.to_string(), client);
                Ok(())
            }
            Err(e) => {
                self.errors.insert(id.to_string(), e.clone());
                Err(e)
            }
        }
    }

    pub async fn connect_all_enabled(&mut self) {
        let enabled_ids: Vec<String> = self.configs.iter()
            .filter(|c| c.enabled)
            .map(|c| c.id.clone())
            .collect();
        for id in enabled_ids {
            let _ = self.connect_server(&id).await;
        }
    }

    pub async fn disconnect_all(&mut self) {
        let ids: Vec<String> = self.servers.keys().cloned().collect();
        for id in ids {
            if let Some(mut client) = self.servers.remove(&id) {
                let _ = client.disconnect().await;
            }
        }
    }

    pub fn all_tools(&self) -> Vec<(String, McpToolDef)> {
        let mut result = Vec::new();
        for (server_id, client) in &self.servers {
            for tool in client.tools() {
                result.push((server_id.clone(), tool.clone()));
            }
        }
        result
    }

    pub async fn call_tool(&mut self, server_id: &str, tool_name: &str, args: Value) -> Result<McpToolResult, String> {
        let start = std::time::Instant::now();
        let args_str = serde_json::to_string(&args).unwrap_or_default();

        let client = self.servers.get_mut(server_id)
            .ok_or_else(|| format!("Server '{}' not connected", server_id))?;
        let result = client.call_tool(tool_name, args).await;

        let duration_ms = start.elapsed().as_millis() as u64;
        self.last_seen.insert(server_id.to_string(), now_ts());

        // Log the call
        let (result_str, is_error) = match &result {
            Ok(r) => {
                let text: String = r.content.iter()
                    .filter_map(|c| c.text.as_deref())
                    .collect::<Vec<_>>()
                    .join("\n");
                (truncate_str(&text, 2000), r.is_error.unwrap_or(false))
            }
            Err(e) => (e.clone(), true),
        };
        self.logs.push(McpToolCallLog {
            timestamp: now_ts(),
            server_id: server_id.to_string(),
            tool_name: tool_name.to_string(),
            arguments: truncate_str(&args_str, 2000),
            result: result_str,
            duration_ms,
            is_error,
        });
        // Keep only last 100 logs
        if self.logs.len() > 100 {
            self.logs.drain(..self.logs.len() - 100);
        }

        result
    }

    pub fn server_statuses(&self) -> Vec<McpServerStatus> {
        self.configs.iter().map(|config| {
            let connected = self.servers.contains_key(&config.id);
            let tool_count = self.servers.get(&config.id)
                .map(|c| c.tools().len())
                .unwrap_or(0);
            let error = self.errors.get(&config.id).cloned();
            let transport = match config.transport {
                McpTransportType::Stdio => "stdio",
                McpTransportType::Http => "http",
            };
            McpServerStatus {
                id: config.id.clone(),
                name: config.name.clone(),
                connected,
                tool_count,
                error,
                transport: transport.to_string(),
                last_seen: self.last_seen.get(&config.id).copied(),
            }
        }).collect()
    }

    pub async fn health_check(&mut self) {
        let ids: Vec<String> = self.servers.keys().cloned().collect();
        let mut dead_ids = Vec::new();
        for id in &ids {
            if let Some(client) = self.servers.get_mut(id) {
                if !client.is_connected() {
                    dead_ids.push(id.clone());
                }
            }
        }
        for id in dead_ids {
            // Try reconnect
            if let Some(mut client) = self.servers.remove(&id) {
                let _ = client.disconnect().await;
            }
            let _ = self.connect_server(&id).await;
        }
    }

    pub fn tool_logs(&self) -> &[McpToolCallLog] {
        &self.logs
    }

    #[allow(dead_code)]
    pub fn configs(&self) -> &[McpServerConfig] {
        &self.configs
    }
}
