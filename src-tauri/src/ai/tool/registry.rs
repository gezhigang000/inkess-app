use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::Value;
use super::{ToolPlugin, ToolContext, ToolOutput, ToolError};

pub enum ToolFilter {
    All,
    Only(Vec<String>),
    Exclude(Vec<String>),
}

pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn ToolPlugin>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: RwLock::new(HashMap::new()) }
    }

    pub async fn register(&self, tool: Arc<dyn ToolPlugin>) {
        let name = tool.name().to_string();
        let mut tools = self.tools.write().await;
        tools.insert(name, tool);
    }

    pub async fn get_all_schemas(&self) -> Vec<Value> {
        let tools = self.tools.read().await;
        tools.values().map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name(),
                    "description": t.description(),
                    "parameters": t.input_schema(),
                }
            })
        }).collect()
    }

    pub async fn get_schemas_filtered(&self, filter: &ToolFilter) -> Vec<Value> {
        let tools = self.tools.read().await;
        tools.values()
            .filter(|t| match filter {
                ToolFilter::All => true,
                ToolFilter::Only(names) => names.iter().any(|n| n == t.name()),
                ToolFilter::Exclude(names) => !names.iter().any(|n| n == t.name()),
            })
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name(),
                        "description": t.description(),
                        "parameters": t.input_schema(),
                    }
                })
            })
            .collect()
    }

    /// Execute a tool by name. Releases read lock before calling execute().
    pub async fn execute(
        &self,
        name: &str,
        ctx: &ToolContext,
        input: Value,
    ) -> Result<ToolOutput, ToolError> {
        let tool = {
            let tools = self.tools.read().await;
            tools.get(name).cloned()
        };
        match tool {
            Some(t) => t.execute(ctx, input).await,
            None => Err(ToolError::ExecutionFailed(format!("Unknown tool: {}", name))),
        }
    }

    pub async fn has_tool(&self, name: &str) -> bool {
        let tools = self.tools.read().await;
        tools.contains_key(name)
    }

    /// Remove a tool by name
    pub async fn unregister(&self, name: &str) {
        let mut tools = self.tools.write().await;
        tools.remove(name);
    }

    /// Remove all tools whose names start with the given prefix
    pub async fn remove_by_prefix(&self, prefix: &str) {
        let mut tools = self.tools.write().await;
        tools.retain(|name, _| !name.starts_with(prefix));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct DummyTool {
        tool_name: String,
    }

    impl DummyTool {
        fn new(name: &str) -> Self {
            Self { tool_name: name.to_string() }
        }
    }

    #[async_trait]
    impl ToolPlugin for DummyTool {
        fn name(&self) -> &str { &self.tool_name }
        fn description(&self) -> &str { "A dummy tool for testing" }
        fn input_schema(&self) -> Value {
            serde_json::json!({ "type": "object", "properties": {} })
        }
        async fn execute(&self, _ctx: &ToolContext, _input: Value) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::success(format!("executed {}", self.tool_name)))
        }
    }

    #[tokio::test]
    async fn test_register_and_get_schemas() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(DummyTool::new("tool_a"))).await;

        let schemas = registry.get_all_schemas().await;
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0]["function"]["name"], "tool_a");
    }

    #[tokio::test]
    async fn test_has_tool() {
        let registry = ToolRegistry::new();
        assert!(!registry.has_tool("foo").await);
        registry.register(Arc::new(DummyTool::new("foo"))).await;
        assert!(registry.has_tool("foo").await);
    }

    #[tokio::test]
    async fn test_unregister() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(DummyTool::new("bar"))).await;
        assert!(registry.has_tool("bar").await);
        registry.unregister("bar").await;
        assert!(!registry.has_tool("bar").await);
    }

    #[tokio::test]
    async fn test_filter_all() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(DummyTool::new("a"))).await;
        registry.register(Arc::new(DummyTool::new("b"))).await;

        let schemas = registry.get_schemas_filtered(&ToolFilter::All).await;
        assert_eq!(schemas.len(), 2);
    }

    #[tokio::test]
    async fn test_filter_only() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(DummyTool::new("a"))).await;
        registry.register(Arc::new(DummyTool::new("b"))).await;
        registry.register(Arc::new(DummyTool::new("c"))).await;

        let filter = ToolFilter::Only(vec!["a".into(), "c".into()]);
        let schemas = registry.get_schemas_filtered(&filter).await;
        assert_eq!(schemas.len(), 2);
        let names: Vec<&str> = schemas.iter().filter_map(|s| s["function"]["name"].as_str()).collect();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"c"));
        assert!(!names.contains(&"b"));
    }

    #[tokio::test]
    async fn test_filter_exclude() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(DummyTool::new("a"))).await;
        registry.register(Arc::new(DummyTool::new("b"))).await;

        let filter = ToolFilter::Exclude(vec!["a".into()]);
        let schemas = registry.get_schemas_filtered(&filter).await;
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0]["function"]["name"], "b");
    }

    #[tokio::test]
    async fn test_remove_by_prefix() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(DummyTool::new("mcp__server__tool1"))).await;
        registry.register(Arc::new(DummyTool::new("mcp__server__tool2"))).await;
        registry.register(Arc::new(DummyTool::new("builtin_read"))).await;

        assert_eq!(registry.get_all_schemas().await.len(), 3);
        registry.remove_by_prefix("mcp__").await;
        assert_eq!(registry.get_all_schemas().await.len(), 1);
        assert!(registry.has_tool("builtin_read").await);
    }

    #[tokio::test]
    async fn test_register_overwrites() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(DummyTool::new("x"))).await;
        registry.register(Arc::new(DummyTool::new("x"))).await;
        // Should still be 1, not 2
        assert_eq!(registry.get_all_schemas().await.len(), 1);
    }
}
