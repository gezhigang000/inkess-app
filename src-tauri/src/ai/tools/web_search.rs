use async_trait::async_trait;
use serde_json::Value;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::search;

pub struct WebSearchTool;

#[async_trait]
impl ToolPlugin for WebSearchTool {
    fn name(&self) -> &str { "web_search" }
    fn description(&self) -> &str { "Search the internet for information" }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search keyword" }
            },
            "required": ["query"]
        })
    }
    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let query = input["query"].as_str().unwrap_or("");
        if query.trim().is_empty() {
            return Ok(ToolOutput::error("Please provide search keywords".to_string()));
        }

        let provider = &ctx.ai_config.search_provider;
        let api_key = &ctx.ai_config.search_api_key;

        // Use configured engine, fall back to DuckDuckGo if API key needed but missing
        let engine = search::get_engine(provider);
        let effective_engine = if engine.needs_api_key() && api_key.is_empty() {
            search::get_engine("duckduckgo")
        } else {
            engine
        };

        match effective_engine.search(query, api_key, 8).await {
            Ok(results) => Ok(ToolOutput::success(search::format_results(query, &results))),
            Err(e) => Ok(ToolOutput::error(e)),
        }
    }
}
