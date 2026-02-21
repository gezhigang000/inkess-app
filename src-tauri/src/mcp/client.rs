use serde_json::Value;
use super::protocol::{McpClientInfo, McpInitializeParams, McpToolDef, McpToolResult, McpContent, McpTransportType};
use super::transport::{McpTransport, StdioTransport, HttpTransport};
use super::registry::McpServerConfig;

pub struct McpClient {
    transport: McpTransport,
    #[allow(dead_code)]
    server_info: Option<Value>,
    tools: Vec<McpToolDef>,
    config: McpServerConfig,
}

impl McpClient {
    pub async fn connect(config: &McpServerConfig) -> Result<Self, String> {
        let mut transport = Self::create_transport(config).await?;

        // Initialize
        let init_params = McpInitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: serde_json::json!({}),
            client_info: McpClientInfo {
                name: "Inkess".to_string(),
                version: "1.0.0".to_string(),
            },
        };

        let server_info = transport
            .send_request("initialize", Some(serde_json::to_value(&init_params).unwrap()))
            .await?;

        // Send "initialized" notification
        transport.send_notification("notifications/initialized").await?;

        // List tools
        let tools_result = transport
            .send_request("tools/list", Some(serde_json::json!({})))
            .await?;

        let tools: Vec<McpToolDef> = if let Some(tools_arr) = tools_result.get("tools") {
            serde_json::from_value(tools_arr.clone()).unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Self {
            transport,
            server_info: Some(server_info),
            tools,
            config: config.clone(),
        })
    }

    async fn create_transport(config: &McpServerConfig) -> Result<McpTransport, String> {
        match config.transport {
            McpTransportType::Http => {
                let url = config.url.as_deref().ok_or("HTTP transport requires a URL")?;
                Ok(McpTransport::Http(HttpTransport::new(url)))
            }
            McpTransportType::Stdio => {
                let t = StdioTransport::spawn(
                    &config.command,
                    &config.args,
                    &config.env,
                    None,
                ).await?;
                Ok(McpTransport::Stdio(t))
            }
        }
    }

    pub fn tools(&self) -> &[McpToolDef] {
        &self.tools
    }

    pub fn is_connected(&mut self) -> bool {
        self.transport.is_alive()
    }

    pub async fn call_tool(&mut self, name: &str, args: Value) -> Result<McpToolResult, String> {
        let params = serde_json::json!({
            "name": name,
            "arguments": args,
        });

        let result = match self.transport.send_request("tools/call", Some(params.clone())).await {
            Ok(r) => r,
            Err(e) => {
                // If transport is dead, try reconnect once
                if !self.transport.is_alive() {
                    self.reconnect().await?;
                    self.transport.send_request("tools/call", Some(params)).await?
                } else {
                    return Err(e);
                }
            }
        };

        // Parse result
        let tool_result: McpToolResult = serde_json::from_value(result.clone()).unwrap_or(McpToolResult {
            content: vec![McpContent {
                type_: "text".to_string(),
                text: Some(result.to_string()),
            }],
            is_error: None,
        });

        Ok(tool_result)
    }

    async fn reconnect(&mut self) -> Result<(), String> {
        let _ = self.transport.close().await;
        let mut transport = Self::create_transport(&self.config).await?;

        let init_params = McpInitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: serde_json::json!({}),
            client_info: McpClientInfo {
                name: "Inkess".to_string(),
                version: "1.0.0".to_string(),
            },
        };
        transport.send_request("initialize", Some(serde_json::to_value(&init_params).unwrap())).await?;
        transport.send_notification("notifications/initialized").await?;

        let tools_result = transport.send_request("tools/list", Some(serde_json::json!({}))).await?;
        self.tools = if let Some(tools_arr) = tools_result.get("tools") {
            serde_json::from_value(tools_arr.clone()).unwrap_or_default()
        } else {
            Vec::new()
        };

        self.transport = transport;
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<(), String> {
        self.transport.close().await
    }
}
