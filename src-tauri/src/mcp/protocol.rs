use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    pub id: u64,
}

#[derive(Deserialize, Debug)]
pub struct JsonRpcResponse {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
    #[allow(dead_code)]
    pub id: u64,
}

#[derive(Deserialize, Debug)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[allow(dead_code)]
    pub data: Option<Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct McpToolDef {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Deserialize, Debug)]
pub struct McpToolResult {
    pub content: Vec<McpContent>,
    #[serde(rename = "isError")]
    pub is_error: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct McpContent {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub type_: String,
    pub text: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct McpInitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: Value,
    #[serde(rename = "clientInfo")]
    pub client_info: McpClientInfo,
}

#[derive(Serialize, Debug)]
pub struct McpClientInfo {
    pub name: String,
    pub version: String,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC error {}: {}", self.code, self.message)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum McpTransportType {
    Stdio,
    Http,
}

impl Default for McpTransportType {
    fn default() -> Self { McpTransportType::Stdio }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_rpc_request_serialization_with_params() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/list".to_string(),
            params: Some(json!({"cursor": "abc"})),
            id: 42,
        };
        let serialized = serde_json::to_value(&req).unwrap();
        assert_eq!(serialized["jsonrpc"], "2.0");
        assert_eq!(serialized["method"], "tools/list");
        assert_eq!(serialized["params"]["cursor"], "abc");
        assert_eq!(serialized["id"], 42);
    }

    #[test]
    fn test_json_rpc_request_serialization_without_params() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: None,
            id: 1,
        };
        let serialized = serde_json::to_value(&req).unwrap();
        assert!(serialized.get("params").is_none());
    }

    #[test]
    fn test_json_rpc_response_deserialization_with_result() {
        let json_str = r#"{"jsonrpc":"2.0","result":{"tools":[]},"id":1}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
        assert_eq!(resp.id, 1);
    }

    #[test]
    fn test_json_rpc_response_deserialization_with_error() {
        let json_str = r#"{"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"},"id":2}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json_str).unwrap();
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "Method not found");
    }

    #[test]
    fn test_json_rpc_error_display() {
        let err = JsonRpcError {
            code: -32600,
            message: "Invalid Request".to_string(),
            data: None,
        };
        assert_eq!(format!("{}", err), "JSON-RPC error -32600: Invalid Request");
    }

    #[test]
    fn test_json_rpc_error_with_data() {
        let json_str = r#"{"code":-32602,"message":"Invalid params","data":{"detail":"missing field"}}"#;
        let err: JsonRpcError = serde_json::from_str(json_str).unwrap();
        assert_eq!(err.code, -32602);
        assert!(err.data.is_some());
        assert_eq!(err.data.unwrap()["detail"], "missing field");
    }

    #[test]
    fn test_mcp_transport_type_serialization_stdio() {
        let t = McpTransportType::Stdio;
        let s = serde_json::to_string(&t).unwrap();
        assert_eq!(s, r#""stdio""#);
    }

    #[test]
    fn test_mcp_transport_type_serialization_http() {
        let t = McpTransportType::Http;
        let s = serde_json::to_string(&t).unwrap();
        assert_eq!(s, r#""http""#);
    }

    #[test]
    fn test_mcp_transport_type_deserialization() {
        let stdio: McpTransportType = serde_json::from_str(r#""stdio""#).unwrap();
        assert!(matches!(stdio, McpTransportType::Stdio));

        let http: McpTransportType = serde_json::from_str(r#""http""#).unwrap();
        assert!(matches!(http, McpTransportType::Http));
    }

    #[test]
    fn test_mcp_transport_type_default() {
        let t = McpTransportType::default();
        assert!(matches!(t, McpTransportType::Stdio));
    }

    #[test]
    fn test_mcp_transport_type_invalid_deserialization() {
        let result: Result<McpTransportType, _> = serde_json::from_str(r#""grpc""#);
        assert!(result.is_err());
    }

    #[test]
    fn test_mcp_tool_def_serialization() {
        let tool = McpToolDef {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        };
        let serialized = serde_json::to_value(&tool).unwrap();
        assert_eq!(serialized["name"], "read_file");
        assert_eq!(serialized["description"], "Read a file");
        assert_eq!(serialized["inputSchema"]["type"], "object");
    }

    #[test]
    fn test_mcp_tool_def_deserialization_with_input_schema_rename() {
        let json_str = r#"{"name":"test","inputSchema":{"type":"object"}}"#;
        let tool: McpToolDef = serde_json::from_str(json_str).unwrap();
        assert_eq!(tool.name, "test");
        assert_eq!(tool.description, ""); // default
        assert_eq!(tool.input_schema["type"], "object");
    }

    #[test]
    fn test_mcp_tool_def_default_description() {
        let json_str = r#"{"name":"my_tool","inputSchema":{}}"#;
        let tool: McpToolDef = serde_json::from_str(json_str).unwrap();
        assert_eq!(tool.description, "");
    }

    #[test]
    fn test_mcp_tool_result_deserialization() {
        let json_str = r#"{"content":[{"type":"text","text":"hello"}],"isError":false}"#;
        let result: McpToolResult = serde_json::from_str(json_str).unwrap();
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].text.as_deref(), Some("hello"));
        assert_eq!(result.is_error, Some(false));
    }

    #[test]
    fn test_mcp_tool_result_with_error() {
        let json_str = r#"{"content":[{"type":"text","text":"file not found"}],"isError":true}"#;
        let result: McpToolResult = serde_json::from_str(json_str).unwrap();
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_mcp_tool_result_no_is_error() {
        let json_str = r#"{"content":[{"type":"text"}]}"#;
        let result: McpToolResult = serde_json::from_str(json_str).unwrap();
        assert!(result.is_error.is_none());
        assert!(result.content[0].text.is_none());
    }

    #[test]
    fn test_mcp_content_without_text() {
        let json_str = r#"{"type":"image"}"#;
        let content: McpContent = serde_json::from_str(json_str).unwrap();
        assert_eq!(content.type_, "image");
        assert!(content.text.is_none());
    }

    #[test]
    fn test_mcp_initialize_params_serialization() {
        let params = McpInitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: json!({}),
            client_info: McpClientInfo {
                name: "inkess".to_string(),
                version: "1.0.0".to_string(),
            },
        };
        let serialized = serde_json::to_value(&params).unwrap();
        assert_eq!(serialized["protocolVersion"], "2024-11-05");
        assert_eq!(serialized["clientInfo"]["name"], "inkess");
        assert_eq!(serialized["clientInfo"]["version"], "1.0.0");
    }

    #[test]
    fn test_mcp_tool_def_clone() {
        let tool = McpToolDef {
            name: "test".to_string(),
            description: "desc".to_string(),
            input_schema: json!({}),
        };
        let cloned = tool.clone();
        assert_eq!(cloned.name, tool.name);
        assert_eq!(cloned.description, tool.description);
    }
}
