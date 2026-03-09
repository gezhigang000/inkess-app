pub mod registry;

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tauri::AppHandle;
use super::config::AiConfig;
use super::memory::MemoryStore;

/// Shared context injected into every tool execution
#[derive(Clone)]
pub struct ToolContext {
    pub workspace_path: String,
    pub app_handle: AppHandle,
    pub ai_config: AiConfig,
    pub memory_store: Arc<dyn MemoryStore>,
}

/// Structured output from tool execution
pub struct ToolOutput {
    pub content: String,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn success(content: String) -> Self {
        Self { content, is_error: false }
    }
    pub fn error(content: String) -> Self {
        Self { content, is_error: true }
    }
}

/// Tool execution errors
#[derive(Debug)]
pub enum ToolError {
    MissingArgument(String),
    InvalidArgument(String),
    ExecutionFailed(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolError::MissingArgument(s) => write!(f, "Missing argument: {}", s),
            ToolError::InvalidArgument(s) => write!(f, "Invalid argument: {}", s),
            ToolError::ExecutionFailed(s) => write!(f, "Execution failed: {}", s),
        }
    }
}

/// The core trait every tool must implement
#[async_trait]
pub trait ToolPlugin: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError>;
}
