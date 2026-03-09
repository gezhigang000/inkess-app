pub mod list_directory;
pub mod read_file;
pub mod search_files;
pub mod grep_files;
pub mod web_search;
pub mod run_python;
pub mod search_knowledge;
pub mod fetch_url;
pub mod write_file;
pub mod open_file;
pub mod edit_file;
pub mod file_info;
pub mod diff_files;
pub mod run_shell;
pub mod save_memory;
pub mod search_memory;
pub mod get_core_memories;
pub mod mcp_bridge;

use std::sync::Arc;
use super::tool::registry::ToolRegistry;

pub async fn register_builtin_tools(registry: &ToolRegistry) {
    // 17 builtin tools
    registry.register(Arc::new(list_directory::ListDirectoryTool)).await;
    registry.register(Arc::new(read_file::ReadFileTool)).await;
    registry.register(Arc::new(search_files::SearchFilesTool)).await;
    registry.register(Arc::new(grep_files::GrepFilesTool)).await;
    registry.register(Arc::new(web_search::WebSearchTool)).await;
    registry.register(Arc::new(run_python::RunPythonTool)).await;
    registry.register(Arc::new(search_knowledge::SearchKnowledgeTool)).await;
    registry.register(Arc::new(fetch_url::FetchUrlTool)).await;
    registry.register(Arc::new(write_file::WriteFileTool)).await;
    registry.register(Arc::new(open_file::OpenFileTool)).await;
    registry.register(Arc::new(edit_file::EditFileTool)).await;
    registry.register(Arc::new(file_info::FileInfoTool)).await;
    registry.register(Arc::new(diff_files::DiffFilesTool)).await;
    registry.register(Arc::new(run_shell::RunShellTool)).await;
    registry.register(Arc::new(save_memory::SaveMemoryTool)).await;
    registry.register(Arc::new(search_memory::SearchMemoryTool)).await;
    registry.register(Arc::new(get_core_memories::GetCoreMemoriesTool)).await;
}
