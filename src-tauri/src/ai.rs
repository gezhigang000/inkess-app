use std::fs;
use std::path::PathBuf;

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{do_list_directory, do_read_file};
use crate::fileops::{search_files, grep_files};
use crate::python_setup;
use crate::{app_info};

// --- Data structures ---

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiConfig {
    pub api_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: u32,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub base_prompt: String,
    #[serde(default)]
    pub search_api_key: String,
    #[serde(default)]
    pub search_provider: String,
    #[serde(default)]
    pub provider_keys: std::collections::HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct AiStreamEvent {
    pub session_id: String,
    pub event_type: String,
    pub content: String,
}

// --- Config file path ---

fn config_path() -> PathBuf {
    let data_dir = crate::app_data_dir();
    let dir = data_dir.join("inkess");
    fs::create_dir_all(&dir).ok();
    dir.join("ai-config.json")
}

fn memories_path() -> PathBuf {
    let data_dir = crate::app_data_dir();
    let dir = data_dir.join("inkess");
    fs::create_dir_all(&dir).ok();
    dir.join("ai-memories.json")
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AiMemories {
    pub dirs: std::collections::HashMap<String, Vec<MemoryEntry>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MemoryEntry {
    pub content: String,
    pub created_at: String,
}

#[tauri::command]
pub fn ai_save_config(config: AiConfig) -> Result<(), String> {
    let path = config_path();
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| format!("Failed to save config: {}", e))
}

#[tauri::command]
pub fn ai_load_config() -> Option<AiConfig> {
    let path = config_path();
    let data = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

#[tauri::command]
pub fn ai_save_memory(dir: String, content: String) -> Result<(), String> {
    let path = memories_path();
    let mut memories: AiMemories = fs::read_to_string(&path)
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default();
    let entries = memories.dirs.entry(dir).or_default();
    entries.push(MemoryEntry {
        content,
        created_at: chrono::Utc::now().to_rfc3339(),
    });
    // Keep max 20 memories per directory
    if entries.len() > 20 {
        *entries = entries.split_off(entries.len() - 20);
    }
    let json = serde_json::to_string_pretty(&memories).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| format!("Failed to save memories: {}", e))
}

#[tauri::command]
pub fn ai_load_memories(dir: String) -> Vec<MemoryEntry> {
    let path = memories_path();
    let memories: AiMemories = fs::read_to_string(&path)
        .ok()
        .and_then(|d| serde_json::from_str(&d).ok())
        .unwrap_or_default();
    memories.dirs.get(&dir).cloned().unwrap_or_default()
}

#[tauri::command]
pub async fn ai_test_connection(config: AiConfig) -> Result<String, String> {
    let client = Client::new();
    let url = format!("{}/chat/completions", config.api_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": config.model,
        "messages": [{"role": "user", "content": "Hi"}],
        "max_tokens": 16,
    });
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    if resp.status().is_success() {
        Ok(format!("Connection successful ({})", config.model))
    } else {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Err(format!("Request failed ({}): {}", status, text))
    }
}

#[tauri::command]
pub async fn ai_test_search(provider: String, api_key: String) -> Result<String, String> {
    let result = web_search("test", &provider, &api_key).await;
    if result.contains("failed") || result.contains("error") || result.contains("Error") {
        Err(result)
    } else {
        Ok(format!("Search OK ({})", match provider.as_str() {
            "tavily" => "Tavily",
            "brave" => "Brave Search",
            "serpapi" => "SerpAPI",
            _ => "DuckDuckGo",
        }))
    }
}

// --- Tool definitions ---

fn tool_definitions(mcp_tools: &[(String, crate::mcp::protocol::McpToolDef)]) -> serde_json::Value {
    let mut tools = vec![
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "list_directory",
                "description": "List files and folders in a directory",
                "parameters": {
                    "type": "object",
                    "properties": { "path": { "type": "string", "description": "Directory path" } },
                    "required": ["path"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read file content",
                "parameters": {
                    "type": "object",
                    "properties": { "path": { "type": "string", "description": "File path" } },
                    "required": ["path"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "search_files",
                "description": "Search by filename",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "dir": { "type": "string", "description": "Search directory" },
                        "query": { "type": "string", "description": "Search keyword" }
                    },
                    "required": ["dir", "query"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "grep_files",
                "description": "Search file contents by keyword. Returns matching lines with file path and line number. Use this to find code, text, or patterns inside files.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "dir": { "type": "string", "description": "Search directory" },
                        "pattern": { "type": "string", "description": "Search keyword (case-insensitive)" },
                        "file_pattern": { "type": "string", "description": "Optional filename filter, e.g. *.rs, *.tsx" }
                    },
                    "required": ["dir", "pattern"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the internet for information",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search keyword" }
                    },
                    "required": ["query"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "run_python",
                "description": "Execute Python code (embedded standalone Python, 30s timeout). Pre-installed: numpy, matplotlib, pandas, scipy, sympy, Pillow, openpyxl. Can read/write local files. For large files: read a sample first, then process in chunks across multiple calls.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "code": { "type": "string", "description": "Python code to execute" }
                    },
                    "required": ["code"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "search_knowledge",
                "description": "Search the local knowledge base for relevant content across all indexed files in the current project directory. Use this when the user asks about project content, files, or needs information from their documents.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" }
                    },
                    "required": ["query"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "fetch_url",
                "description": "Fetch and read the text content of a web page. Use after web_search to read full article content from a URL. Returns cleaned text (HTML tags stripped). Only http/https URLs allowed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "url": { "type": "string", "description": "The URL to fetch (http or https)" }
                    },
                    "required": ["url"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Write content to a file in the current workspace. Use to save reports, analysis results, translations, or generated code. Paths are relative to the workspace root. Cannot write outside workspace or to sensitive paths (.env, .git/).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path relative to workspace (e.g. report.md, output/analysis.html)" },
                        "content": { "type": "string", "description": "File content to write" }
                    },
                    "required": ["path", "content"]
                }
            }
        }),
        serde_json::json!({
            "type": "function",
            "function": {
                "name": "open_file",
                "description": "Open a file for the user to view. Markdown/HTML/text files open in Inkess viewer, other files open with system default app. Use after write_file to show generated reports to the user.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path relative to workspace" }
                    },
                    "required": ["path"]
                }
            }
        }),
    ];

    // Append MCP tools with prefixed names: mcp__{serverid}__{toolname}
    for (server_id, tool) in mcp_tools {
        let prefixed_name = format!("mcp__{}__{}",server_id, tool.name);
        tools.push(serde_json::json!({
            "type": "function",
            "function": {
                "name": prefixed_name,
                "description": tool.description,
                "parameters": tool.input_schema,
            }
        }));
    }

    serde_json::Value::Array(tools)
}

// --- Path sandboxing ---

/// Resolve a path relative to cwd and ensure it stays within the workspace.
/// Returns None if the resolved path escapes the workspace root.
fn sandbox_path(raw: &str, cwd: &str) -> Option<String> {
    if cwd.is_empty() {
        return Some(raw.to_string());
    }
    let base = std::path::Path::new(cwd).canonicalize().ok()?;
    let target = if std::path::Path::new(raw).is_absolute() {
        std::path::PathBuf::from(raw)
    } else {
        base.join(raw)
    };
    let resolved = target.canonicalize().ok().or_else(|| {
        // File may not exist yet (e.g. write target); check parent
        target.parent().and_then(|p| p.canonicalize().ok()).map(|p| p.join(target.file_name().unwrap_or_default()))
    })?;
    if resolved.starts_with(&base) {
        Some(resolved.to_string_lossy().to_string())
    } else {
        None
    }
}

// --- Execute tool call ---

async fn execute_tool(name: &str, arguments: &str, config: &AiConfig, app: &AppHandle, cwd: &str) -> String {
    app_info!("ai:tool", "execute: {} args={}", name, &arguments[..arguments.len().min(200)]);
    let args: serde_json::Value = serde_json::from_str(arguments).unwrap_or_default();
    match name {
        "list_directory" => {
            let raw_path = args["path"].as_str().unwrap_or(".");
            let path = match sandbox_path(raw_path, cwd) {
                Some(p) => p,
                None => return format!("Access denied: path '{}' is outside the current workspace.", raw_path),
            };
            match do_list_directory(&path) {
                Ok(listing) => {
                    let names: Vec<String> = listing.entries.iter().map(|e| {
                        if e.is_dir { format!("{}/", e.name) } else { e.name.clone() }
                    }).collect();
                    if listing.truncated {
                        format!("(showing {}/{})\n{}", listing.entries.len(), listing.total, names.join("\n"))
                    } else {
                        names.join("\n")
                    }
                }
                Err(e) => format!("Error: {}", e),
            }
        }
        "read_file" => {
            let raw_path = args["path"].as_str().unwrap_or("");
            let path = match sandbox_path(raw_path, cwd) {
                Some(p) => p,
                None => return format!("Access denied: path '{}' is outside the current workspace.", raw_path),
            };
            // Binary files should be read via Python, not as text
            let ext = std::path::Path::new(&path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let binary_exts = ["xlsx", "xls", "pdf", "docx", "doc", "pptx", "ppt",
                "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "svg",
                "zip", "tar", "gz", "rar", "7z", "exe", "dll", "so", "dylib"];
            if binary_exts.contains(&ext.as_str()) {
                return format!("This file is binary format (.{}), cannot be read as text. Use run_python tool with appropriate libraries (e.g. openpyxl for xlsx, Pillow for images).", ext);
            }
            match do_read_file(&path) {
                Ok(content) => {
                    if content.len() > 8000 {
                        format!("{}...\n\n(file too long, truncated, {} chars total)", &content[..8000], content.len())
                    } else {
                        content
                    }
                }
                Err(e) => format!("Error: {}", e),
            }
        }
        "search_files" => {
            let raw_dir = args["dir"].as_str().unwrap_or(".");
            let dir = match sandbox_path(raw_dir, cwd) {
                Some(p) => p,
                None => return format!("Access denied: path '{}' is outside the current workspace.", raw_dir),
            };
            let query = args["query"].as_str().unwrap_or("");
            match search_files(dir, query.to_string()) {
                Ok(results) => {
                    if results.is_empty() {
                        "No matching files found".to_string()
                    } else {
                        results.join("\n")
                    }
                }
                Err(e) => format!("Error: {}", e),
            }
        }
        "grep_files" => {
            let raw_dir = args["dir"].as_str().unwrap_or(".");
            let dir = match sandbox_path(raw_dir, cwd) {
                Some(p) => p,
                None => return format!("Access denied: path '{}' is outside the current workspace.", raw_dir),
            };
            let pattern = args["pattern"].as_str().unwrap_or("");
            let file_pattern = args["file_pattern"].as_str().map(|s| s.to_string());
            match grep_files(dir, pattern.to_string(), file_pattern) {
                Ok(results) => {
                    if results.is_empty() {
                        "No matching content found".to_string()
                    } else {
                        results.join("\n")
                    }
                }
                Err(e) => format!("Error: {}", e),
            }
        }
        "web_search" => {
            let query = args["query"].as_str().unwrap_or("");
            web_search(query, &config.search_provider, &config.search_api_key).await
        }
        "run_python" => {
            let code = args["code"].as_str().unwrap_or("");
            run_python(code, app, cwd).await
        }
        "search_knowledge" => {
            let query = args["query"].as_str().unwrap_or("");
            let rag_state = app.state::<crate::rag::RagState>();
            let mut guard = rag_state.indexer.lock().unwrap();
            match guard.as_mut() {
                Some(indexer) => {
                    match indexer.search(query, 5) {
                        Ok(results) => {
                            if results.is_empty() {
                                "No relevant content found in the knowledge base.".to_string()
                            } else {
                                results.iter().enumerate().map(|(i, r)| {
                                    let heading = r.heading.as_deref().unwrap_or("");
                                    let heading_str = if heading.is_empty() { String::new() } else { format!(" ({})", heading) };
                                    format!("[{}] {}:{}-{}{}\n{}", i + 1, r.path, r.start_line, r.end_line, heading_str, r.content)
                                }).collect::<Vec<_>>().join("\n\n")
                            }
                        }
                        Err(e) => format!("Search error: {}", e),
                    }
                }
                None => "Knowledge base not initialized. Open a directory first.".to_string(),
            }
        }
        "fetch_url" => {
            let url = args["url"].as_str().unwrap_or("");
            fetch_url(url).await
        }
        "write_file" => {
            let raw_path = args["path"].as_str().unwrap_or("");
            let content = args["content"].as_str().unwrap_or("");
            write_file_tool(raw_path, content, cwd)
        }
        "open_file" => {
            let raw_path = args["path"].as_str().unwrap_or("");
            if cwd.is_empty() {
                return "Cannot open files: no workspace directory is open.".to_string();
            }
            let path = match sandbox_path(raw_path, cwd) {
                Some(p) => p,
                None => return format!("Access denied: path '{}' is outside the current workspace.", raw_path),
            };
            let _ = app.emit("open-file-request", serde_json::json!({ "path": path }));
            format!("Opened file: {}", path)
        }
        _ => format!("Unknown tool: {}", name),
    }
}

/// Execute an MCP tool call by parsing the prefixed name and forwarding to the registry
async fn execute_mcp_tool(name: &str, arguments: &str, app: &AppHandle) -> Option<String> {
    // MCP tools are prefixed: mcp__{serverid}__{toolname} (double underscore as delimiter)
    if !name.starts_with("mcp__") {
        return None;
    }
    let rest = &name[5..]; // after "mcp__"
    // Split on "__" (double underscore) to avoid ambiguity with single underscores in names
    let (server_id, tool_name) = match rest.find("__") {
        Some(pos) => (&rest[..pos], &rest[pos + 2..]),
        None => return Some(format!("Invalid MCP tool name: {}", name)),
    };

    let args: serde_json::Value = serde_json::from_str(arguments).unwrap_or_default();
    let mcp_state = app.state::<crate::mcp::McpState>();
    let mut registry = mcp_state.registry.lock().await;
    match registry.call_tool(server_id, tool_name, args).await {
        Ok(result) => {
            let text = result.content.iter()
                .filter_map(|c| c.text.as_deref())
                .collect::<Vec<_>>()
                .join("\n");
            let is_err = result.is_error.unwrap_or(false);
            if is_err {
                Some(format!("MCP tool error: {}", text))
            } else {
                Some(text)
            }
        }
        Err(e) => Some(format!("MCP tool call failed: {}", e)),
    }
}

// --- fetch_url: read web page content ---

async fn fetch_url(url: &str) -> String {
    if url.trim().is_empty() {
        return "Please provide a URL to fetch".to_string();
    }
    // Only allow http/https
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return "Only http and https URLs are allowed".to_string();
    }
    // Block localhost and private IPs (SSRF protection)
    let lower = url.to_lowercase();
    let blocked = [
        "://localhost", "://127.", "://0.0.0.0", "://0/", "://0.",
        "://10.", "://192.168.", "://169.254.",
        "://172.16.", "://172.17.", "://172.18.", "://172.19.",
        "://172.20.", "://172.21.", "://172.22.", "://172.23.",
        "://172.24.", "://172.25.", "://172.26.", "://172.27.",
        "://172.28.", "://172.29.", "://172.30.", "://172.31.",
        "://[::1]", "://[fc", "://[fd", "://[fe80",
    ];
    if blocked.iter().any(|b| lower.contains(b)) {
        return "Access to local/private addresses is not allowed".to_string();
    }

    let client = match Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(c) => c,
        Err(e) => return format!("Failed to create HTTP client: {}", e),
    };

    let resp = match client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (compatible; Inkess/1.0)")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return format!("Fetch failed: {}", e),
    };

    if !resp.status().is_success() {
        return format!("HTTP error: {}", resp.status());
    }

    // Limit response size to 2MB
    let content_length = resp.content_length().unwrap_or(0);
    if content_length > 2 * 1024 * 1024 {
        return format!("Response too large: {} bytes (max 2MB)", content_length);
    }

    let html = match resp.text().await {
        Ok(t) => {
            if t.len() > 2 * 1024 * 1024 {
                return format!("Response too large: {} bytes (max 2MB)", t.len());
            }
            t
        }
        Err(e) => return format!("Failed to read response: {}", e),
    };

    // Extract title
    let title = extract_between(&html, "<title", "</title>")
        .and_then(|t| t.find('>').map(|i| t[i + 1..].to_string()))
        .unwrap_or_default();

    // Strip script, style, nav, header, footer blocks
    let mut cleaned = html;
    for tag in &["script", "style", "nav", "header", "footer", "noscript", "svg"] {
        cleaned = strip_tag_blocks(&cleaned, tag);
    }

    // Strip all remaining HTML tags
    let text = strip_html_tags(&cleaned);

    // Clean whitespace: collapse multiple newlines/spaces
    let text = text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    // Truncate to 15000 chars
    let mut result = String::new();
    if !title.is_empty() {
        result.push_str(&format!("Title: {}\n\n", title.trim()));
    }
    result.push_str(&format!("URL: {}\n\n", url));

    let remaining = 15000 - result.len().min(15000);
    if text.len() > remaining {
        let mut end = remaining;
        while end > 0 && !text.is_char_boundary(end) { end -= 1; }
        result.push_str(&text[..end]);
        result.push_str("\n\n[Content truncated]");
    } else {
        result.push_str(&text);
    }

    result
}

fn extract_between<'a>(html: &'a str, open_tag: &str, close_tag: &str) -> Option<&'a str> {
    let start = html.to_lowercase().find(&open_tag.to_lowercase())?;
    let rest = &html[start + open_tag.len()..];
    let end = rest.to_lowercase().find(&close_tag.to_lowercase())?;
    Some(&rest[..end])
}

fn strip_tag_blocks(html: &str, tag: &str) -> String {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let lower = html.to_lowercase();
    let mut result = String::new();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find(&open) {
        result.push_str(&html[pos..pos + start]);
        let after = pos + start;
        if let Some(end) = lower[after..].find(&close) {
            pos = after + end + close.len();
        } else {
            pos = html.len();
            break;
        }
    }
    result.push_str(&html[pos..]);
    result
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
            result.push(' ');
        } else if !in_tag {
            result.push(ch);
        }
    }
    // Decode common HTML entities
    result.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

// --- write_file: save content to workspace ---

fn write_file_tool(raw_path: &str, content: &str, cwd: &str) -> String {
    if cwd.is_empty() {
        return "Cannot write files: no workspace directory is open.".to_string();
    }
    if raw_path.is_empty() {
        return "Please provide a file path".to_string();
    }
    if content.len() > 1024 * 1024 {
        return format!("Content too large: {} bytes (max 1MB)", content.len());
    }

    // Block sensitive paths
    let lower = raw_path.to_lowercase();
    let sensitive = [".env", ".git/", ".git\\", ".ssh/", ".ssh\\",
        ".bash_history", ".zsh_history", ".npmrc", ".pypirc",
        ".docker/", ".docker\\", ".kube/", ".kube\\",
        ".aws/", ".aws\\", ".config/gh", ".config\\gh",
        ".gnupg/", ".gnupg\\", ".netrc"];
    for s in &sensitive {
        if lower.contains(s) || lower == ".env" {
            return format!("Cannot write to sensitive path: {}", raw_path);
        }
    }
    // Block dotfiles at root
    if raw_path.starts_with('.') && !raw_path.contains('/') && !raw_path.contains('\\') {
        return format!("Cannot write to dotfile: {}", raw_path);
    }

    let path = match sandbox_path(raw_path, cwd) {
        Some(p) => p,
        None => return format!("Access denied: path '{}' is outside the current workspace.", raw_path),
    };

    // Create parent directories
    if let Some(parent) = std::path::Path::new(&path).parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return format!("Failed to create directory: {}", e);
        }
    }

    match fs::write(&path, content) {
        Ok(_) => format!("File written: {} ({} bytes)", path, content.len()),
        Err(e) => format!("Failed to write file: {}", e),
    }
}

// --- Web search dispatcher ---

async fn web_search(query: &str, provider: &str, api_key: &str) -> String {
    if query.trim().is_empty() {
        return "Please provide search keywords".to_string();
    }
    match provider {
        "tavily" if !api_key.is_empty() => tavily_search(query, api_key).await,
        "brave" if !api_key.is_empty() => brave_search(query, api_key).await,
        "serpapi" if !api_key.is_empty() => serpapi_search(query, api_key).await,
        _ => duckduckgo_search(query).await,
    }
}

async fn duckduckgo_search(query: &str) -> String {
    let client = Client::new();
    let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoding(query));
    let resp = match client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (compatible; Inkess/1.0)")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return format!("Search request failed: {}", e),
    };
    let html = match resp.text().await {
        Ok(t) => t,
        Err(e) => return format!("Failed to read search results: {}", e),
    };
    // Parse results from DuckDuckGo HTML
    let mut results = Vec::new();
    for part in html.split("class=\"result__a\"") {
        if results.len() >= 8 { break; }
        if let Some(href_start) = part.find("href=\"") {
            let rest = &part[href_start + 6..];
            if let Some(href_end) = rest.find('"') {
                let href = &rest[..href_end];
                // Extract title text (between > and </a>)
                if let Some(tag_end) = rest.find('>') {
                    let after_tag = &rest[tag_end + 1..];
                    if let Some(close) = after_tag.find("</a>") {
                        let title = after_tag[..close]
                            .replace("<b>", "").replace("</b>", "")
                            .replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
                            .replace("&#x27;", "'").replace("&quot;", "\"")
                            .trim().to_string();
                        if !title.is_empty() && !href.is_empty() {
                            results.push(format!("{}. {} - {}", results.len() + 1, title, href));
                        }
                    }
                }
            }
        }
    }
    // Also extract snippets
    let mut snippets = Vec::new();
    for part in html.split("class=\"result__snippet\"") {
        if snippets.len() >= 8 { break; }
        if let Some(tag_end) = part.find('>') {
            let after = &part[tag_end + 1..];
            if let Some(close) = after.find("</a>") {
                let snippet = after[..close]
                    .replace("<b>", "").replace("</b>", "")
                    .replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
                    .replace("&#x27;", "'").replace("&quot;", "\"")
                    .trim().to_string();
                if !snippet.is_empty() {
                    snippets.push(snippet);
                }
            }
        }
    }
    if results.is_empty() {
        return "No search results found".to_string();
    }
    let mut output = format!("Search results for \"{}\":\n\n", query);
    for (i, r) in results.iter().enumerate() {
        output.push_str(r);
        output.push('\n');
        if let Some(s) = snippets.get(i) {
            output.push_str("   ");
            output.push_str(s);
            output.push('\n');
        }
        output.push('\n');
    }
    output
}

async fn tavily_search(query: &str, api_key: &str) -> String {
    let client = Client::new();
    let body = serde_json::json!({
        "api_key": api_key,
        "query": query,
        "max_results": 8,
    });
    let resp = match client
        .post("https://api.tavily.com/search")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return format!("Tavily search request failed: {}", e),
    };
    let json: serde_json::Value = match resp.json().await {
        Ok(j) => j,
        Err(e) => return format!("Failed to parse Tavily results: {}", e),
    };
    let results = match json["results"].as_array() {
        Some(arr) => arr,
        None => return "Tavily returned no results".to_string(),
    };
    if results.is_empty() {
        return "No search results found".to_string();
    }
    let mut output = format!("Search results for \"{}\":\n\n", query);
    for (i, r) in results.iter().enumerate() {
        let title = r["title"].as_str().unwrap_or("");
        let url = r["url"].as_str().unwrap_or("");
        let content = r["content"].as_str().unwrap_or("");
        output.push_str(&format!("{}. {} - {}\n", i + 1, title, url));
        if !content.is_empty() {
            output.push_str(&format!("   {}\n", content));
        }
        output.push('\n');
    }
    output
}

async fn brave_search(query: &str, api_key: &str) -> String {
    let client = Client::new();
    let url = format!("https://api.search.brave.com/res/v1/web/search?q={}&count=8", urlencoding(query));
    let resp = match client
        .get(&url)
        .header("Accept", "application/json")
        .header("X-Subscription-Token", api_key)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return format!("Brave search request failed: {}", e),
    };
    let json: serde_json::Value = match resp.json().await {
        Ok(j) => j,
        Err(e) => return format!("Failed to parse Brave results: {}", e),
    };
    let results = match json["web"]["results"].as_array() {
        Some(arr) => arr,
        None => return "Brave returned no results".to_string(),
    };
    if results.is_empty() {
        return "No search results found".to_string();
    }
    let mut output = format!("Search results for \"{}\":\n\n", query);
    for (i, r) in results.iter().enumerate() {
        let title = r["title"].as_str().unwrap_or("");
        let url = r["url"].as_str().unwrap_or("");
        let desc = r["description"].as_str().unwrap_or("");
        output.push_str(&format!("{}. {} - {}\n", i + 1, title, url));
        if !desc.is_empty() {
            output.push_str(&format!("   {}\n", desc));
        }
        output.push('\n');
    }
    output
}

async fn serpapi_search(query: &str, api_key: &str) -> String {
    let client = Client::new();
    let url = format!(
        "https://serpapi.com/search.json?q={}&api_key={}&num=8",
        urlencoding(query), urlencoding(api_key)
    );
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return format!("SerpAPI search request failed: {}", e),
    };
    let json: serde_json::Value = match resp.json().await {
        Ok(j) => j,
        Err(e) => return format!("Failed to parse SerpAPI results: {}", e),
    };
    let results = match json["organic_results"].as_array() {
        Some(arr) => arr,
        None => return "SerpAPI returned no results".to_string(),
    };
    if results.is_empty() {
        return "No search results found".to_string();
    }
    let mut output = format!("Search results for \"{}\":\n\n", query);
    for (i, r) in results.iter().enumerate() {
        let title = r["title"].as_str().unwrap_or("");
        let link = r["link"].as_str().unwrap_or("");
        let snippet = r["snippet"].as_str().unwrap_or("");
        output.push_str(&format!("{}. {} - {}\n", i + 1, title, link));
        if !snippet.is_empty() {
            output.push_str(&format!("   {}\n", snippet));
        }
        output.push('\n');
    }
    output
}

fn urlencoding(s: &str) -> String {
    let mut result = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            b' ' => result.push('+'),
            _ => {
                result.push_str(&format!("%{:02X}", b));
            }
        }
    }
    result
}

// --- Embedded Python execution ---

fn find_python() -> Option<PathBuf> {
    let p = python_setup::python_bin_path();
    if p.exists() { Some(p) } else { None }
}

async fn run_python(code: &str, app: &AppHandle, cwd: &str) -> String {
    if code.trim().is_empty() {
        return "Please provide Python code to execute".to_string();
    }

    let python_path = match find_python() {
        Some(p) => p,
        None => {
            // Auto-trigger Python environment setup
            match python_setup::setup_python_env(app).await {
                Ok(p) => p,
                Err(e) => return format!("Python environment setup failed: {}", e),
            }
        }
    };

    // Write code to a temp file
    let tmp_dir = std::env::temp_dir();
    let tmp_file = tmp_dir.join(format!("inkess_py_{}.py", uuid::Uuid::new_v4()));
    if let Err(e) = fs::write(&tmp_file, code) {
        return format!("Failed to write temp file: {}", e);
    }

    // Execute with 30s timeout — spawn explicitly so we can kill on timeout
    let mut child = {
        let mut cmd = tokio::process::Command::new(&python_path);
        cmd.arg(&tmp_file);
        cmd.env("PYTHONIOENCODING", "utf-8");
        cmd.env("PYTHONUNBUFFERED", "1");
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        if !cwd.is_empty() {
            cmd.current_dir(cwd);
        }
        match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = fs::remove_file(&tmp_file);
                return format!("Failed to start Python process: {}", e);
            }
        }
    };

    // Take stdout/stderr before waiting — read concurrently to avoid pipe buffer deadlock
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    // Spawn tasks to drain stdout/stderr concurrently with child.wait()
    let stdout_task = tokio::spawn(async move {
        if let Some(mut h) = stdout_handle {
            let mut buf = Vec::new();
            let _ = tokio::io::AsyncReadExt::read_to_end(&mut h, &mut buf).await;
            String::from_utf8_lossy(&buf).to_string()
        } else {
            String::new()
        }
    });
    let stderr_task = tokio::spawn(async move {
        if let Some(mut h) = stderr_handle {
            let mut buf = Vec::new();
            let _ = tokio::io::AsyncReadExt::read_to_end(&mut h, &mut buf).await;
            String::from_utf8_lossy(&buf).to_string()
        } else {
            String::new()
        }
    });

    let wait_result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        child.wait(),
    )
    .await;

    // On timeout, explicitly kill the child process
    if wait_result.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }

    // Clean up temp file
    let _ = fs::remove_file(&tmp_file);

    // Collect output from drain tasks
    let stdout_str = stdout_task.await.unwrap_or_default();
    let stderr_str = stderr_task.await.unwrap_or_default();

    match wait_result {
        Ok(Ok(status)) => {
            // Clean stderr: replace temp file paths with "<script>" for readability
            let stderr = {
                let mut s = stderr_str.clone();
                // Remove full temp file paths like /tmp/inkess_py_xxxx.py
                while let Some(start) = s.find("inkess_py_") {
                    if let Some(end) = s[start..].find(".py") {
                        let prefix_start = s[..start].rfind(|c: char| c == '"' || c == '\'' || c == ' ' || c == '\n').map(|i| i + 1).unwrap_or(0);
                        s = format!("{}<script>{}", &s[..prefix_start], &s[start + end + 3..]);
                    } else {
                        break;
                    }
                }
                // Filter out non-UTF8 replacement chars
                s.replace('\u{FFFD}', "?")
            };
            if status.success() {
                if stdout_str.is_empty() && stderr.is_empty() {
                    "(execution successful, no output)".to_string()
                } else if stderr.is_empty() {
                    stdout_str
                } else {
                    format!("{}\n[stderr]: {}", stdout_str, stderr)
                }
            } else {
                if stderr.is_empty() {
                    format!("Python execution failed (exit code: {:?})\n{}", status.code(), stdout_str)
                } else {
                    format!("Python execution failed:\n{}", stderr)
                }
            }
        }
        Ok(Err(e)) => format!("Python execution error: {}", e),
        Err(_) => "Python execution timed out (30s limit). The process has been terminated.".to_string(),
    }
}

// --- SSE stream parsing helpers ---

#[derive(Deserialize, Debug)]
struct SseDelta {
    content: Option<String>,
    tool_calls: Option<Vec<SseDeltaToolCall>>,
}

#[derive(Deserialize, Debug)]
struct SseDeltaToolCall {
    index: Option<usize>,
    id: Option<String>,
    #[allow(dead_code)]
    r#type: Option<String>,
    function: Option<SseDeltaFunction>,
}

#[derive(Deserialize, Debug)]
struct SseDeltaFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Deserialize, Debug)]
struct SseChoice {
    delta: Option<SseDelta>,
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct SseChunk {
    choices: Option<Vec<SseChoice>>,
}

// --- Main chat command ---

#[tauri::command]
pub async fn ai_chat(
    app: AppHandle,
    session_id: String,
    messages: Vec<ChatMessage>,
    config: AiConfig,
    deep_mode: Option<bool>,
    cwd: Option<String>,
) -> Result<(), String> {
    let client = Client::new();
    let url = format!("{}/chat/completions", config.api_url.trim_end_matches('/'));
    let mut conversation = messages.clone();
    let is_deep = deep_mode.unwrap_or(false);
    let max_tool_rounds = if is_deep { 30 } else { 20 };
    app_info!("ai", "chat start: model={}, deep={}, msgs={}, url={}", config.model, is_deep, messages.len(), url);

    // Deep analysis mode prompt is now injected by the frontend (AIChatPanel.tsx)
    // to keep all prompt logic transparent and user-configurable.

    // Inject knowledge base hint if RAG is initialized
    {
        let rag_state = app.state::<crate::rag::RagState>();
        let has_rag = rag_state.indexer.lock().map(|g| g.is_some()).unwrap_or(false);
        if has_rag {
            let kb_hint = "\n\n[Knowledge Base]\nA local knowledge base is available for this project. Use the search_knowledge tool to find relevant content across all indexed files when the user asks about project content, code, or documentation.";
            if let Some(first) = conversation.first_mut() {
                if first.role == "system" {
                    if let Some(ref mut content) = first.content {
                        content.push_str(kb_hint);
                    }
                }
            }
        }
    }

    // Gather MCP tools
    let mcp_tools = {
        let mcp_state = app.state::<crate::mcp::McpState>();
        let registry = mcp_state.registry.lock().await;
        registry.all_tools()
    };

    for _round in 0..max_tool_rounds {
        let body = serde_json::json!({
            "model": config.model,
            "messages": conversation,
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
            "stream": true,
            "tools": tool_definitions(&mcp_tools),
        });

        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                let _ = app.emit("ai-stream", AiStreamEvent {
                    session_id: session_id.clone(),
                    event_type: "error".into(),
                    content: format!("Request failed: {}", e),
                });
                format!("Request failed: {}", e)
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let err_msg = format!("API error ({}): {}", status, text);
            let _ = app.emit("ai-stream", AiStreamEvent {
                session_id: session_id.clone(),
                event_type: "error".into(),
                content: err_msg.clone(),
            });
            return Err(err_msg);
        }

        // Parse SSE stream
        let mut stream = resp.bytes_stream();
        let mut full_content = String::new();
        let mut tool_calls_map: std::collections::HashMap<usize, ToolCall> = std::collections::HashMap::new();
        let mut finish_reason: Option<String> = None;
        let mut buffer = String::new();
        const MAX_SSE_BUFFER: usize = 512 * 1024; // 512KB cap for SSE buffer

        while let Some(chunk_result) = stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    let _ = app.emit("ai-stream", AiStreamEvent {
                        session_id: session_id.clone(),
                        event_type: "error".into(),
                        content: format!("Stream read error: {}", e),
                    });
                    return Err(e.to_string());
                }
            };

            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Guard against malformed SSE data causing unbounded buffer growth
            if buffer.len() > MAX_SSE_BUFFER {
                safe_eprintln!("[ai] SSE buffer exceeded {}KB, truncating", MAX_SSE_BUFFER / 1024);
                buffer.clear();
                continue;
            }

            // Process complete SSE lines
            while let Some(pos) = buffer.find('\n') {
                let line = &buffer[..pos];
                let line = line.trim();

                if line.is_empty() || line == "data: [DONE]" {
                    buffer.drain(..pos + 1);
                    continue;
                }
                if !line.starts_with("data: ") {
                    buffer.drain(..pos + 1);
                    continue;
                }
                let json_str = &line[6..];
                let sse_chunk: SseChunk = match serde_json::from_str(json_str) {
                    Ok(c) => c,
                    Err(_) => { buffer.drain(..pos + 1); continue; }
                };

                buffer.drain(..pos + 1);

                if let Some(choices) = &sse_chunk.choices {
                    for choice in choices {
                        if let Some(reason) = &choice.finish_reason {
                            finish_reason = Some(reason.clone());
                        }
                        if let Some(delta) = &choice.delta {
                            // Text content
                            if let Some(text) = &delta.content {
                                full_content.push_str(text);
                                let _ = app.emit("ai-stream", AiStreamEvent {
                                    session_id: session_id.clone(),
                                    event_type: "delta".into(),
                                    content: text.clone(),
                                });
                            }
                            // Tool calls
                            if let Some(tcs) = &delta.tool_calls {
                                for tc in tcs {
                                    let idx = tc.index.unwrap_or(0);
                                    let entry = tool_calls_map.entry(idx).or_insert_with(|| ToolCall {
                                        id: String::new(),
                                        r#type: "function".into(),
                                        function: FunctionCall {
                                            name: String::new(),
                                            arguments: String::new(),
                                        },
                                    });
                                    if let Some(id) = &tc.id {
                                        entry.id = id.clone();
                                    }
                                    if let Some(f) = &tc.function {
                                        if let Some(name) = &f.name {
                                            entry.function.name.push_str(name);
                                        }
                                        if let Some(args) = &f.arguments {
                                            entry.function.arguments.push_str(args);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check if we got tool calls
        if finish_reason.as_deref() == Some("tool_calls") && !tool_calls_map.is_empty() {
            let mut sorted_calls: Vec<(usize, ToolCall)> = tool_calls_map.into_iter().collect();
            sorted_calls.sort_by_key(|(idx, _)| *idx);
            let tool_calls: Vec<ToolCall> = sorted_calls.into_iter().map(|(_, tc)| tc).collect();

            // Add assistant message with tool_calls
            conversation.push(ChatMessage {
                role: "assistant".into(),
                content: if full_content.is_empty() { None } else { Some(full_content.clone()) },
                tool_calls: Some(tool_calls.clone()),
                tool_call_id: None,
            });

            // Execute each tool and add results
            for tc in &tool_calls {
                let _ = app.emit("ai-stream", AiStreamEvent {
                    session_id: session_id.clone(),
                    event_type: "tool_call".into(),
                    content: serde_json::json!({
                        "id": tc.id,
                        "name": tc.function.name,
                        "arguments": tc.function.arguments,
                    }).to_string(),
                });

                let cwd_str = cwd.as_deref().unwrap_or("");
                let result = if let Some(mcp_result) = execute_mcp_tool(&tc.function.name, &tc.function.arguments, &app).await {
                    mcp_result
                } else {
                    execute_tool(&tc.function.name, &tc.function.arguments, &config, &app, cwd_str).await
                };

                // Cap tool result size to prevent conversation memory explosion
                const MAX_TOOL_RESULT: usize = 32 * 1024; // 32KB
                let result = if result.len() > MAX_TOOL_RESULT {
                    let mut end = MAX_TOOL_RESULT;
                    while end > 0 && !result.is_char_boundary(end) { end -= 1; }
                    format!("{}...\n[Truncated: result was {} bytes]", &result[..end], result.len())
                } else {
                    result
                };

                let _ = app.emit("ai-stream", AiStreamEvent {
                    session_id: session_id.clone(),
                    event_type: "tool_result".into(),
                    content: serde_json::json!({
                        "id": tc.id,
                        "name": tc.function.name,
                        "result": result,
                    }).to_string(),
                });

                conversation.push(ChatMessage {
                    role: "tool".into(),
                    content: Some(result),
                    tool_calls: None,
                    tool_call_id: Some(tc.id.clone()),
                });
            }
            // Continue loop to send tool results back to LLM
            continue;
        }

        // No tool calls — we're done
        let _ = app.emit("ai-stream", AiStreamEvent {
            session_id: session_id.clone(),
            event_type: "done".into(),
            content: full_content,
        });
        return Ok(());
    }

    // Exceeded max tool rounds — send accumulated content + friendly notice
    let notice = format!("\n\n---\n⚠️ Reached the maximum of {} tool call rounds. You can continue the conversation to ask for more analysis.", max_tool_rounds);
    let _ = app.emit("ai-stream", AiStreamEvent {
        session_id: session_id.clone(),
        event_type: "delta".into(),
        content: notice,
    });
    let _ = app.emit("ai-stream", AiStreamEvent {
        session_id: session_id.clone(),
        event_type: "done".into(),
        content: String::new(),
    });
    Ok(())
}
