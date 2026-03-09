pub mod config;
pub mod streaming;
pub mod gateway;
pub mod tool;
pub mod tools;
pub mod skill;
pub mod skills;
pub mod search;
pub mod sandbox;
pub mod memory;

pub use config::*;
pub use streaming::*;
pub use memory::MemoryStore;

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::collections::HashMap;

use futures_util::StreamExt;
use reqwest::Client;
use tauri::{AppHandle, Emitter, Manager};

use crate::app_info;

use self::streaming::SseChunk;

// --- ToolRegistry as Tauri managed state ---

pub struct AiToolRegistryState {
    pub registry: tool::registry::ToolRegistry,
}

// --- SkillRegistry as Tauri managed state ---

pub struct AiSkillRegistryState {
    pub registry: skill::registry::SkillRegistry,
}

// --- MemoryStore as Tauri managed state ---

pub struct MemoryStoreState {
    pub store: Arc<dyn MemoryStore>,
}

// --- Shell confirm state for run_shell tool ---
// NOTE: Known limitation — single-slot design. If multiple concurrent tool calls
// require confirmation, later senders will overwrite earlier ones. This is acceptable
// because the LLM processes tool calls sequentially in the current architecture.
pub struct ShellConfirmState {
    pub sender: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<bool>>>,
}

#[tauri::command]
pub async fn shell_confirm_response(app: AppHandle, approved: bool) -> Result<(), String> {
    let state = app.state::<ShellConfirmState>();
    let sender = state.sender.lock().map_err(|e| e.to_string())?.take();
    if let Some(tx) = sender {
        let _ = tx.send(approved);
    }
    Ok(())
}

// --- Cancel registry for active sessions ---
pub struct AiCancelRegistry {
    pub flags: std::sync::Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl AiCancelRegistry {
    pub fn new() -> Self {
        Self { flags: std::sync::Mutex::new(HashMap::new()) }
    }
}

#[tauri::command]
pub async fn ai_cancel_chat(app: AppHandle, session_id: String) -> Result<(), String> {
    let registry = app.state::<AiCancelRegistry>();
    let flags = registry.flags.lock().map_err(|e| e.to_string())?;
    if let Some(flag) = flags.get(&session_id) {
        flag.store(true, Ordering::Relaxed);
        app_info!("ai", "cancel requested for session {}", session_id);
    }
    Ok(())
}

// --- Path sandboxing ---

/// Resolve a path relative to cwd and ensure it stays within the workspace.
/// Returns None if the resolved path escapes the workspace root.
pub(crate) fn sandbox_path(raw: &str, cwd: &str) -> Option<String> {
    if cwd.is_empty() {
        return None; // Reject all paths when no workspace is open
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

/// Synchronize MCP tools into the ToolRegistry so they participate in
/// skill-based filtering and unified tool execution.
/// Called automatically after MCP servers connect, and can be called
/// from the frontend after MCP configuration changes.
#[tauri::command]
pub async fn sync_mcp_tools(app: AppHandle) -> Result<(), String> {
    let tool_registry_state = app.state::<AiToolRegistryState>();
    let mcp_state = app.state::<crate::mcp::McpState>();
    tools::mcp_bridge::sync_mcp_tools(
        &tool_registry_state.registry,
        &mcp_state.registry,
    ).await;
    app_info!("ai", "MCP tools synced to ToolRegistry");
    Ok(())
}

// --- Main chat command ---

/// RAII guard to clean up cancel flag when ai_chat exits (normal, error, or panic)
struct CancelGuard {
    app: AppHandle,
    session_id: String,
}
impl Drop for CancelGuard {
    fn drop(&mut self) {
        if let Some(registry) = self.app.try_state::<AiCancelRegistry>() {
            if let Ok(mut flags) = registry.flags.lock() { flags.remove(&self.session_id); }
        }
    }
}

#[tauri::command]
pub async fn ai_chat(
    app: AppHandle,
    session_id: String,
    messages: Vec<ChatMessage>,
    config: AiConfig,
    deep_mode: Option<bool>,
    cwd: Option<String>,
    current_skill_id: Option<String>,
) -> Result<(), String> {
    let client = Client::new();
    let url = format!("{}/chat/completions", config.api_url.trim_end_matches('/'));
    let mut conversation = messages.clone();
    let is_deep = deep_mode.unwrap_or(false);

    // Skill detection and activation
    let skill_registry_state = app.state::<AiSkillRegistryState>();
    let has_files = cwd.is_some();
    let user_message = messages.last()
        .and_then(|m| m.content.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("");
    let prev_skill_id = current_skill_id.as_deref().unwrap_or("default");

    let activated_skill_id = skill_registry_state.registry
        .detect_activation(user_message, has_files, prev_skill_id)
        .await;

    let skill = skill_registry_state.registry
        .get(&activated_skill_id)
        .await
        .ok_or_else(|| format!("Skill not found: {}", activated_skill_id))?;

    // Emit skill-changed event if skill switched
    if activated_skill_id != prev_skill_id {
        let _ = app.emit("skill-changed", serde_json::json!({
            "session_id": session_id,
            "skill_id": activated_skill_id,
            "skill_name": skill.display_name(),
        }));
    }

    // Build skill state
    let skill_state = skill::SkillState {
        skill_id: activated_skill_id.clone(),
    };

    // Apply skill's [REDACTED]
    let skill_system_prompt = skill.system_prompt(&skill_state);
    if let Some(first) = conversation.first_mut() {
        if first.role == "system" {
            if let Some(ref mut content) = first.content {
                // Prepend skill prompt to existing system message
                *content = format!("{}\n\n{}", skill_system_prompt, content);
            }
        }
    } else {
        // Insert new system message at the beginning
        conversation.insert(0, ChatMessage {
            role: "system".to_string(),
            content: Some(skill_system_prompt),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    // Get max iterations from skill (overrides deep_mode)
    let max_tool_rounds = skill.max_iterations(&skill_state);

    app_info!("ai", "chat start: model={}, skill={}, deep={}, msgs={}, max_rounds={}, url={}",
        config.model, activated_skill_id, is_deep, messages.len(), max_tool_rounds, url);

    // Register cancel flag for this session
    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let registry = app.state::<AiCancelRegistry>();
        let mut flags = registry.flags.lock().map_err(|e| e.to_string())?;
        flags.insert(session_id.clone(), cancel_flag.clone());
    }
    let _cancel_guard = CancelGuard { app: app.clone(), session_id: session_id.clone() };

    // Deep analysis mode prompt is now injected by the frontend (AIChatPanel.tsx)
    // to keep all prompt logic transparent and user-configurable.

    // Inject relevant memories into context
    {
        let memory_store_state = app.state::<MemoryStoreState>();
        let memory_store = &memory_store_state.store;

        if let Ok(memory_text) = load_relevant_memories(
            memory_store.as_ref(),
            user_message,
            cwd.as_deref(),
        ).await {
            if !memory_text.is_empty() {
                if let Some(first) = conversation.first_mut() {
                    if first.role == "system" {
                        if let Some(ref mut content) = first.content {
                            content.push_str("\n\n");
                            content.push_str(&memory_text);
                        }
                    }
                }
            }
        }
    }

    // Inject search hint if BM25 index is initialized
    {
        let bm25_state = app.state::<crate::bm25::Bm25State>();
        let has_index = bm25_state.index.lock().map(|g| g.is_some()).unwrap_or(false);
        if has_index {
            let hint = "\n\n[Full-Text Search]\nA full-text search index is available for this project. Use the search_knowledge tool to find relevant content across all indexed files when the user asks about project content, code, or documentation.";
            if let Some(first) = conversation.first_mut() {
                if first.role == "system" {
                    if let Some(ref mut content) = first.content {
                        content.push_str(hint);
                    }
                }
            }
        }
    }

    // Get tool schemas from ToolRegistry with skill's filter
    // MCP tools are already registered in ToolRegistry via McpBridgeTool
    let tool_registry_state = app.state::<AiToolRegistryState>();
    let tool_filter = skill.tool_filter(&skill_state);
    let all_tool_schemas = tool_registry_state.registry.get_schemas_filtered(&tool_filter).await;
    let tools_json = serde_json::Value::Array(all_tool_schemas);

    let mut python_fail_count: u32 = 0;
    let mut silent_rounds: u32 = 0; // Rounds with tool calls but no text output

    for _round in 0..max_tool_rounds {
        // Check cancel flag at start of each round
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = app.emit("ai-stream", AiStreamEvent {
                session_id: session_id.clone(),
                event_type: "done".into(),
                content: String::new(),
            });
            return Ok(());
        }

        // Micro-compact: silently replace old tool results with placeholders
        micro_compact(&mut conversation);

        let body = serde_json::json!({
            "model": config.model,
            "messages": conversation,
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
            "stream": true,
            "tools": tools_json,
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
            // Check cancel flag during streaming
            if cancel_flag.load(Ordering::Relaxed) {
                let _ = app.emit("ai-stream", AiStreamEvent {
                    session_id: session_id.clone(),
                    event_type: "done".into(),
                    content: String::new(),
                });
                return Ok(());
            }
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

            // Guard against malformed SSE data causing unbounded buffer growth
            if buffer.len() + chunk.len() > MAX_SSE_BUFFER {
                safe_eprintln!("[ai] SSE buffer would exceed {}KB limit, clearing", MAX_SSE_BUFFER / 1024);
                buffer.clear();
                continue;
            }
            buffer.push_str(&String::from_utf8_lossy(&chunk));

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
                // Check cancel before each tool execution
                if cancel_flag.load(Ordering::Relaxed) {
                    let _ = app.emit("ai-stream", AiStreamEvent {
                        session_id: session_id.clone(),
                        event_type: "done".into(),
                        content: String::new(),
                    });
                    return Ok(());
                }
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

                // All tools (builtin + MCP bridge) go through ToolRegistry
                let args_preview_end = char_boundary(&tc.function.arguments, 200);
                app_info!("ai:tool", "execute: {} args={}", tc.function.name, &tc.function.arguments[..args_preview_end]);
                let args: serde_json::Value = serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                let memory_store_state = app.state::<MemoryStoreState>();
                let tool_ctx = tool::ToolContext {
                    workspace_path: cwd_str.to_string(),
                    app_handle: app.clone(),
                    ai_config: config.clone(),
                    memory_store: memory_store_state.store.clone(),
                };
                let result = match tool_registry_state.registry.execute(&tc.function.name, &tool_ctx, args).await {
                    Ok(output) => output.content,
                    Err(e) => format!("Tool error: {}", e),
                };

                // Auto-decay: save large tool results to file, replace with reference + hint
                const DECAY_THRESHOLD: usize = 32 * 1024; // 32KB
                let result = if result.len() > DECAY_THRESHOLD {
                    let decay_dir = crate::app_data_dir().join("inkess").join("decay-cache");
                    let _ = fs::create_dir_all(&decay_dir);
                    // Sanitize tool name for safe file naming
                    let safe_name: String = tc.function.name.chars()
                        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
                        .collect();
                    let file_name = format!(
                        "decay-{}-{}.txt",
                        &safe_name[..safe_name.len().min(32)],
                        &uuid::Uuid::new_v4().to_string()[..8]
                    );
                    let decay_path = decay_dir.join(&file_name);
                    let original_size = result.len();
                    let decay_path_str = decay_path.display().to_string();

                    // Smart truncation: structure-aware preview based on tool type
                    let (preview, extra_info) = smart_truncate(&result, &tc.function.name);
                    let hint = decay_tool_hint(&tc.function.name, &decay_path_str, original_size, &extra_info);

                    // Memory-aware decay: save brief episodic memory (best-effort)
                    let memory_store_state = app.state::<MemoryStoreState>();
                    save_decay_memory(
                        memory_store_state.store.clone(),
                        tc.function.name.clone(),
                        original_size,
                        extra_info,
                    );

                    match fs::write(&decay_path, &result) {
                        Ok(_) => format!(
                            "{}\n\n[Output too large ({:.0}KB) — full content saved to: {}]\n{}",
                            preview,
                            original_size as f64 / 1024.0,
                            decay_path_str,
                            hint
                        ),
                        Err(_) => {
                            // Fallback: simple truncation if file write fails
                            let (fallback_preview, _) = default_truncate(&result);
                            format!(
                                "{}\n[Truncated: result was {} bytes]\n{}",
                                fallback_preview, original_size, hint
                            )
                        }
                    }
                } else {
                    result
                };

                // Track consecutive Python failures
                let mut result = result;
                if tc.function.name == "run_python" {
                    if result.contains("Python execution failed") || result.contains("Code blocked for security") {
                        python_fail_count += 1;
                        if python_fail_count >= 2 {
                            result.push_str("\n\nPython execution failed twice consecutively. Please review the approach or ask the user for guidance.");
                        }
                    } else {
                        python_fail_count = 0;
                    }
                }

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

            // Track silent rounds (tool calls without user-visible text)
            if full_content.trim().is_empty() {
                silent_rounds += 1;
            } else {
                silent_rounds = 0;
            }

            // Nag reminder: after 3+ silent rounds, nudge the LLM to provide progress
            if silent_rounds >= 3 {
                conversation.push(ChatMessage {
                    role: "system".into(),
                    content: Some(
                        "[Progress reminder] You've been working silently for several rounds. Please briefly update the user on your progress and what you're doing next.".to_string()
                    ),
                    tool_calls: None,
                    tool_call_id: None,
                });
                silent_rounds = 0; // Reset after reminder
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

        // Save transcript for recoverability
        save_transcript(&session_id, &conversation);

        // Auto-distill: compress long conversations into persistent memories
        maybe_spawn_distill(&app, &conversation, &config, cwd.as_deref());

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

    // Save transcript for recoverability
    save_transcript(&session_id, &conversation);

    // Auto-distill for max-rounds exit too
    maybe_spawn_distill(&app, &conversation, &config, cwd.as_deref());

    Ok(())
}

/// Spawn a background task to distill long conversations into persistent memories.
/// Only triggers when the conversation exceeds DISTILL_THRESHOLD messages.
/// Failures are logged but never crash the application.
fn maybe_spawn_distill(
    app: &AppHandle,
    conversation: &[ChatMessage],
    config: &AiConfig,
    workspace_path: Option<&str>,
) {
    use memory::distill::DISTILL_THRESHOLD;

    if conversation.len() < DISTILL_THRESHOLD {
        return;
    }

    let memory_store = app.state::<MemoryStoreState>().store.clone();
    let messages = conversation.to_vec();
    let ai_config = config.clone();
    let ws = workspace_path.map(|s| s.to_string());

    tokio::spawn(async move {
        app_info!("ai:distill", "starting auto-distill ({} messages)", messages.len());
        match memory::distill::distill_conversation(&messages, &ai_config, ws).await {
            Ok(mem) => {
                let content_preview: String = mem.content.chars().take(80).collect();
                match memory_store.save(mem).await {
                    Ok(id) => {
                        app_info!("ai:distill", "saved memory {}: {}...", id, content_preview);
                    }
                    Err(e) => {
                        safe_eprintln!("[ai:distill] failed to save memory: {}", e);
                    }
                }
            }
            Err(e) => {
                safe_eprintln!("[ai:distill] distillation failed: {}", e);
            }
        }
    });
}

/// Generate a tool-specific recovery hint for decayed content.
/// `decay_file` is the path where full content was saved; `extra` holds
/// tool-specific stats (match count, entry count, etc.).
fn decay_tool_hint(tool_name: &str, decay_file: &str, original_size: usize, extra: &str) -> String {
    match tool_name {
        "read_file" => format!(
            "Full file content saved to {}. Use read_file to re-read specific sections.",
            decay_file
        ),
        "grep_files" => format!(
            "{}Full results in {}.",
            if extra.is_empty() { String::new() } else { format!("{} ", extra) },
            decay_file
        ),
        "list_directory" => format!(
            "{}Full listing in {}.",
            if extra.is_empty() { String::new() } else { format!("{} ", extra) },
            decay_file
        ),
        "run_python" => format!(
            "Python output was {:.0}KB. Full output in {}.",
            original_size as f64 / 1024.0,
            decay_file
        ),
        "web_search" | "fetch_url" => format!(
            "Fetched content was {:.0}KB. Full content in {}.",
            original_size as f64 / 1024.0,
            decay_file
        ),
        "search_knowledge" => format!(
            "{}Full search results in {}.",
            if extra.is_empty() { String::new() } else { format!("{} ", extra) },
            decay_file
        ),
        _ => format!(
            "Large result ({:.0}KB) saved to {}.",
            original_size as f64 / 1024.0,
            decay_file
        ),
    }
}

/// Structure-aware truncation of large tool results.
/// Returns a shortened version with meaningful content preserved based on tool type.
fn smart_truncate(content: &str, tool_name: &str) -> (String, String) {
    // Returns (truncated_preview, extra_info_for_hint)
    match tool_name {
        "read_file" | "run_python" => {
            // Code-like output: keep first 20 lines + last 10 lines
            let lines: Vec<&str> = content.lines().collect();
            let total = lines.len();
            if total <= 35 {
                return (content.to_string(), String::new());
            }
            let head: Vec<&str> = lines[..20].to_vec();
            let tail: Vec<&str> = lines[total.saturating_sub(10)..].to_vec();
            let omitted = total - 30;
            let preview = format!(
                "{}\n\n... [{} lines omitted] ...\n\n{}",
                head.join("\n"),
                omitted,
                tail.join("\n")
            );
            (preview, String::new())
        }
        "grep_files" | "search_knowledge" => {
            // Search results: keep first 3 result blocks, count the rest
            let lines: Vec<&str> = content.lines().collect();
            let total = lines.len();
            // Heuristic: each result block is separated by empty lines or starts with a path
            let mut block_count = 0;
            let mut cut_line = total;
            for (i, line) in lines.iter().enumerate() {
                if line.is_empty() || line.starts_with('/') || line.starts_with("File:") || line.starts_with("Match") {
                    if i > 0 && !lines[i - 1].is_empty() {
                        block_count += 1;
                    }
                }
                if block_count >= 3 && cut_line == total {
                    cut_line = i;
                }
            }
            if cut_line >= total {
                // Fallback: first 1500 chars + last 500
                let (preview, _) = default_truncate(content);
                let extra = format!("Found results across {} lines.", total);
                return (preview, extra);
            }
            let kept = lines[..cut_line].join("\n");
            let remaining_lines = total - cut_line;
            let preview = format!(
                "{}\n\n... [{} more lines with additional matches] ...",
                kept, remaining_lines
            );
            let match_count = lines.iter().filter(|l| !l.is_empty()).count();
            let extra = format!("Found ~{} matches.", match_count);
            (preview, extra)
        }
        "list_directory" => {
            // Directory listing: keep first 30 entries, count remaining
            let lines: Vec<&str> = content.lines().collect();
            let total = lines.len();
            if total <= 35 {
                return (content.to_string(), String::new());
            }
            let kept = lines[..30].join("\n");
            let remaining = total - 30;
            let preview = format!(
                "{}\n\n... [{} more entries] ...",
                kept, remaining
            );
            let extra = format!("Directory has {} entries.", total);
            (preview, extra)
        }
        _ => {
            let (preview, _) = default_truncate(content);
            (preview, String::new())
        }
    }
}

/// Default truncation: first 1500 chars + last 500 chars with omission marker.
fn default_truncate(content: &str) -> (String, String) {
    let head_end = char_boundary(content, 1500);
    let tail_start = char_boundary_back(content, 500);

    if tail_start <= head_end {
        // Content is short enough, shouldn't happen but handle gracefully
        return (content.to_string(), String::new());
    }
    let omitted = tail_start - head_end;
    let preview = format!(
        "{}\n\n[... {} chars omitted ...]\n\n{}",
        &content[..head_end],
        omitted,
        &content[tail_start..]
    );
    (preview, String::new())
}

/// Find the largest valid char boundary <= target position.
fn char_boundary(s: &str, target: usize) -> usize {
    let mut pos = target.min(s.len());
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

/// Find the smallest valid char boundary for the last `n` bytes of the string.
fn char_boundary_back(s: &str, n: usize) -> usize {
    if n >= s.len() {
        return 0;
    }
    let mut pos = s.len() - n;
    while pos < s.len() && !s.is_char_boundary(pos) {
        pos += 1;
    }
    pos
}

/// Save a brief episodic memory when decaying a large tool result (best-effort).
fn save_decay_memory(
    memory_store: Arc<dyn MemoryStore>,
    tool_name: String,
    content_len: usize,
    extra_info: String,
) {
    tokio::spawn(async move {
        let now = chrono::Utc::now().timestamp();
        let summary = if extra_info.is_empty() {
            format!("Tool '{}' produced large output ({:.0}KB), saved to decay cache.",
                tool_name, content_len as f64 / 1024.0)
        } else {
            format!("Tool '{}': {} Output was {:.0}KB, saved to decay cache.",
                tool_name, extra_info, content_len as f64 / 1024.0)
        };

        let mem = memory::Memory {
            id: String::new(),
            content: summary,
            memory_type: memory::MemoryType::Episodic,
            importance: 0.3,
            metadata: memory::MemoryMetadata {
                tags: vec![format!("decay:{}", tool_name), "auto-decay".to_string()],
                source: "auto-decay".to_string(),
                workspace_path: None,
            },
            created_at: now,
            accessed_at: now,
            access_count: 0,
        };
        if let Err(e) = memory_store.save(mem).await {
            safe_eprintln!("[ai:decay] failed to save decay memory: {}", e);
        }
    });
}

/// Load relevant memories for AI context injection
async fn load_relevant_memories(
    memory_store: &dyn MemoryStore,
    user_message: &str,
    workspace_path: Option<&str>,
) -> Result<String, String> {
    let mut relevant_memories = Vec::new();

    // 1. Get core memories (top 5 by importance)
    if let Ok(mut core) = memory_store.get_core_memories().await {
        // Filter by workspace if provided
        if let Some(ws) = workspace_path {
            core.retain(|m| {
                m.metadata.workspace_path.as_deref() == Some(ws) ||
                m.metadata.workspace_path.is_none() // Include global memories
            });
        }
        // Sort by importance descending
        core.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap_or(std::cmp::Ordering::Equal));
        relevant_memories.extend(core.into_iter().take(5));
    }

    // 2. Search for contextually relevant memories
    let search_query: String = user_message.chars().take(200).collect();
    if !search_query.trim().is_empty() {
        if let Ok(mut searched) = memory_store.search(&search_query, 5).await {
            // Filter by workspace if provided
            if let Some(ws) = workspace_path {
                searched.retain(|m| {
                    m.metadata.workspace_path.as_deref() == Some(ws) ||
                    m.metadata.workspace_path.is_none()
                });
            }
            relevant_memories.extend(searched.into_iter().take(3));
        }
    }

    // 3. Deduplicate by ID
    relevant_memories.sort_by(|a, b| a.id.cmp(&b.id));
    relevant_memories.dedup_by(|a, b| a.id == b.id);

    // 4. Format as markdown section
    if relevant_memories.is_empty() {
        return Ok(String::new());
    }

    let formatted = format_memories_for_context(&relevant_memories);

    // 5. Truncate if too long (max 1000 chars)
    if formatted.len() > 1000 {
        let truncated: String = formatted.chars().take(997).collect();
        Ok(format!("{}...", truncated))
    } else {
        Ok(formatted)
    }
}

/// Format memories as markdown for LLM context
fn format_memories_for_context(memories: &[memory::Memory]) -> String {
    use std::collections::HashMap;

    // Group by memory type
    let mut grouped: HashMap<&str, Vec<&memory::Memory>> = HashMap::new();
    for mem in memories {
        grouped.entry(mem.memory_type.as_str()).or_default().push(mem);
    }

    let mut sections = Vec::new();

    // Core memories first (most important)
    if let Some(core_mems) = grouped.get("core") {
        let mut lines = vec!["### Core Facts".to_string()];
        for mem in core_mems {
            lines.push(format!("- [Core] {} (importance: {:.1})", mem.content, mem.importance));
        }
        sections.push(lines.join("\n"));
    }

    // Procedural memories (how-to knowledge)
    if let Some(proc_mems) = grouped.get("procedural") {
        let mut lines = vec!["### Procedures".to_string()];
        for mem in proc_mems {
            lines.push(format!("- [Procedural] {}", mem.content));
        }
        sections.push(lines.join("\n"));
    }

    // Episodic memories (conversation history)
    if let Some(epi_mems) = grouped.get("episodic") {
        let mut lines = vec!["### Related Context".to_string()];
        for mem in epi_mems {
            lines.push(format!("- [Episodic] {}", mem.content));
        }
        sections.push(lines.join("\n"));
    }

    // Semantic memories (general facts)
    if let Some(sem_mems) = grouped.get("semantic") {
        let mut lines = vec!["### General Knowledge".to_string()];
        for mem in sem_mems {
            lines.push(format!("- [Semantic] {}", mem.content));
        }
        sections.push(lines.join("\n"));
    }

    if sections.is_empty() {
        return String::new();
    }

    format!("## Relevant Memories\n\n{}", sections.join("\n\n"))
}

/// Micro-compact: silently replace old tool results with placeholders to save context.
/// Keeps the most recent `keep_recent` tool messages intact; for older ones, if the
/// content exceeds `COMPACT_THRESHOLD` bytes, replace with a brief placeholder.
/// This is zero-LLM-cost — pure in-memory string replacement each round.
const MICRO_COMPACT_KEEP_RECENT: usize = 3;
const MICRO_COMPACT_THRESHOLD: usize = 200;

fn micro_compact(conversation: &mut Vec<ChatMessage>) {
    // Collect indices of all tool-role messages (in order)
    let tool_indices: Vec<usize> = conversation
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == "tool")
        .map(|(i, _)| i)
        .collect();

    if tool_indices.len() <= MICRO_COMPACT_KEEP_RECENT {
        return; // Not enough tool messages to compact
    }

    // Compact all but the last KEEP_RECENT tool messages
    let compact_count = tool_indices.len() - MICRO_COMPACT_KEEP_RECENT;
    for &idx in &tool_indices[..compact_count] {
        if let Some(ref content) = conversation[idx].content {
            if content.len() > MICRO_COMPACT_THRESHOLD {
                conversation[idx].content = Some(
                    "[Previous tool result omitted for context efficiency]".to_string()
                );
            }
        }
    }
}

/// Save the full conversation transcript to disk for recoverability.
/// Runs in background (tokio::spawn) to avoid blocking the response.
/// Transcripts are saved to `<APP_DATA>/inkess/transcripts/` with 7-day auto-cleanup.
fn save_transcript(session_id: &str, conversation: &[ChatMessage]) {
    let transcript_dir = crate::app_data_dir().join("inkess").join("transcripts");
    let session = session_id.to_string();
    let messages = conversation.to_vec();

    tokio::spawn(async move {
        if let Err(e) = fs::create_dir_all(&transcript_dir) {
            safe_eprintln!("[ai:transcript] failed to create dir: {}", e);
            return;
        }

        // Cleanup old transcripts (>7 days)
        if let Ok(entries) = fs::read_dir(&transcript_dir) {
            let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(7 * 24 * 3600);
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.modified().map(|t| t < cutoff).unwrap_or(false) {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }

        // Save current transcript
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let safe_session: String = session.chars()
            .take(32)
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
            .collect();
        let filename = format!("{}_{}.json", timestamp, safe_session);
        let path = transcript_dir.join(&filename);

        match serde_json::to_string(&messages) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, &json) {
                    safe_eprintln!("[ai:transcript] failed to write: {}", e);
                }
            }
            Err(e) => {
                safe_eprintln!("[ai:transcript] failed to serialize: {}", e);
            }
        }
    });
}

pub fn cleanup_decay_cache() {
    let decay_dir = crate::app_data_dir().join("inkess").join("decay-cache");
    if let Ok(entries) = fs::read_dir(&decay_dir) {
        let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(24 * 3600);
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.modified().map(|t| t < cutoff).unwrap_or(false) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // =========================================================================
    // sandbox_path tests
    // =========================================================================

    #[test]
    fn sandbox_path_empty_cwd_returns_none() {
        assert_eq!(sandbox_path("somefile.txt", ""), None);
    }

    #[test]
    fn sandbox_path_relative_within_workspace() {
        let tmp = std::env::temp_dir().join("inkess_test_sandbox_rel");
        let _ = fs::create_dir_all(tmp.join("subdir"));
        // Create a file so canonicalize succeeds
        let _ = fs::write(tmp.join("subdir/test.txt"), "hello");

        let result = sandbox_path("subdir/test.txt", tmp.to_str().unwrap());
        assert!(result.is_some());
        let resolved = result.unwrap();
        assert!(resolved.contains("subdir"));
        assert!(resolved.contains("test.txt"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sandbox_path_absolute_within_workspace() {
        let tmp = std::env::temp_dir().join("inkess_test_sandbox_abs");
        let _ = fs::create_dir_all(&tmp);
        let file_path = tmp.join("inside.txt");
        let _ = fs::write(&file_path, "data");

        let result = sandbox_path(file_path.to_str().unwrap(), tmp.to_str().unwrap());
        assert!(result.is_some());
        let resolved = result.unwrap();
        assert!(resolved.contains("inside.txt"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sandbox_path_escaping_workspace_returns_none() {
        let tmp = std::env::temp_dir().join("inkess_test_sandbox_esc");
        let _ = fs::create_dir_all(&tmp);

        // Attempting to escape via ../../etc should return None
        let result = sandbox_path("../../etc/passwd", tmp.to_str().unwrap());
        assert!(result.is_none());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sandbox_path_nonexistent_file_existing_parent() {
        let tmp = std::env::temp_dir().join("inkess_test_sandbox_noexist");
        let _ = fs::create_dir_all(tmp.join("subdir"));

        // File does not exist but parent directory does
        let result = sandbox_path("subdir/new_file.txt", tmp.to_str().unwrap());
        assert!(result.is_some());
        let resolved = result.unwrap();
        assert!(resolved.contains("new_file.txt"));

        let _ = fs::remove_dir_all(&tmp);
    }

    // =========================================================================
    // char_boundary tests
    // =========================================================================

    #[test]
    fn char_boundary_ascii_returns_target() {
        let s = "hello world";
        assert_eq!(char_boundary(s, 5), 5);
    }

    #[test]
    fn char_boundary_cjk_finds_valid_boundary() {
        // Each CJK character is 3 bytes in UTF-8
        let s = "你好世界"; // 12 bytes total
        let pos = char_boundary(s, 4); // 4 is mid-character (byte 3-5 is '好')
        assert!(s.is_char_boundary(pos));
        assert!(pos <= 4);
        // Should snap back to 3 (end of '你')
        assert_eq!(pos, 3);
    }

    #[test]
    fn char_boundary_beyond_len_returns_len() {
        let s = "abc";
        assert_eq!(char_boundary(s, 100), 3);
    }

    #[test]
    fn char_boundary_zero_returns_zero() {
        let s = "hello";
        assert_eq!(char_boundary(s, 0), 0);
    }

    // =========================================================================
    // char_boundary_back tests
    // =========================================================================

    #[test]
    fn char_boundary_back_n_ge_len_returns_zero() {
        let s = "hello";
        assert_eq!(char_boundary_back(s, 5), 0);
        assert_eq!(char_boundary_back(s, 100), 0);
    }

    #[test]
    fn char_boundary_back_ascii() {
        let s = "hello world"; // 11 bytes
        // Last 5 bytes => start at index 6
        assert_eq!(char_boundary_back(s, 5), 6);
    }

    #[test]
    fn char_boundary_back_cjk_finds_valid_boundary() {
        let s = "你好世界"; // 12 bytes
        // Last 4 bytes: 12-4=8, but byte 8 is mid-char, so should advance to 9
        let pos = char_boundary_back(s, 4);
        assert!(s.is_char_boundary(pos));
        assert_eq!(pos, 9); // start of '界'
    }

    // =========================================================================
    // default_truncate tests
    // =========================================================================

    #[test]
    fn default_truncate_short_content_returns_as_is() {
        let content = "short content";
        let (result, extra) = default_truncate(content);
        assert_eq!(result, content);
        assert!(extra.is_empty());
    }

    #[test]
    fn default_truncate_long_content_truncates() {
        // Create content longer than 1500 + 500 = 2000 chars
        let content: String = "a".repeat(3000);
        let (result, _) = default_truncate(&content);
        assert!(result.contains("[..."));
        assert!(result.contains("chars omitted"));
        assert!(result.len() < content.len());
    }

    #[test]
    fn default_truncate_cjk_no_split_mid_char() {
        // Create CJK content long enough to trigger truncation
        // Each char is 3 bytes, need > 2000 bytes total
        let content: String = "中".repeat(1000); // 3000 bytes
        let (result, _) = default_truncate(&content);
        // Verify the result is valid UTF-8 (would panic if not)
        assert!(result.len() > 0);
        // Verify we can iterate chars without issues
        let _: Vec<char> = result.chars().collect();
    }

    // =========================================================================
    // smart_truncate tests
    // =========================================================================

    #[test]
    fn smart_truncate_read_file_keeps_head_and_tail() {
        let lines: Vec<String> = (0..100).map(|i| format!("line {}", i)).collect();
        let content = lines.join("\n");

        let (result, _) = smart_truncate(&content, "read_file");
        // Should contain first line and last line
        assert!(result.contains("line 0"));
        assert!(result.contains("line 19")); // last of head (20 lines)
        assert!(result.contains("line 99")); // last line
        assert!(result.contains("lines omitted"));
    }

    #[test]
    fn smart_truncate_list_directory_keeps_first_30() {
        let lines: Vec<String> = (0..100).map(|i| format!("entry_{}.txt", i)).collect();
        let content = lines.join("\n");

        let (result, extra) = smart_truncate(&content, "list_directory");
        assert!(result.contains("entry_0.txt"));
        assert!(result.contains("entry_29.txt"));
        assert!(result.contains("more entries"));
        assert!(extra.contains("100"));
    }

    #[test]
    fn smart_truncate_unknown_tool_uses_default() {
        let content: String = "x".repeat(3000);
        let (result, extra) = smart_truncate(&content, "unknown_tool");
        assert!(result.contains("chars omitted"));
        assert!(extra.is_empty());
    }

    #[test]
    fn smart_truncate_short_content_returns_as_is() {
        let content = "just a few lines\nof content\nhere";
        let (result, _) = smart_truncate(content, "read_file");
        assert_eq!(result, content);
    }

    // =========================================================================
    // decay_tool_hint tests
    // =========================================================================

    #[test]
    fn decay_tool_hint_read_file() {
        let hint = decay_tool_hint("read_file", "/tmp/decay.txt", 50000, "");
        assert!(hint.contains("read_file"));
        assert!(hint.contains("/tmp/decay.txt"));
    }

    #[test]
    fn decay_tool_hint_grep_files() {
        let hint = decay_tool_hint("grep_files", "/tmp/decay.txt", 50000, "Found ~42 matches.");
        assert!(hint.contains("Found ~42 matches."));
        assert!(hint.contains("/tmp/decay.txt"));
    }

    #[test]
    fn decay_tool_hint_list_directory() {
        let hint = decay_tool_hint("list_directory", "/tmp/decay.txt", 50000, "Directory has 500 entries.");
        assert!(hint.contains("Directory has 500 entries."));
    }

    #[test]
    fn decay_tool_hint_run_python() {
        let hint = decay_tool_hint("run_python", "/tmp/decay.txt", 51200, "");
        assert!(hint.contains("Python output"));
        assert!(hint.contains("50")); // 50KB
    }

    #[test]
    fn decay_tool_hint_web_search() {
        let hint = decay_tool_hint("web_search", "/tmp/decay.txt", 10240, "");
        assert!(hint.contains("Fetched content"));
    }

    #[test]
    fn decay_tool_hint_fetch_url() {
        let hint = decay_tool_hint("fetch_url", "/tmp/decay.txt", 10240, "");
        assert!(hint.contains("Fetched content"));
    }

    #[test]
    fn decay_tool_hint_search_knowledge() {
        let hint = decay_tool_hint("search_knowledge", "/tmp/decay.txt", 50000, "Found 15 results.");
        assert!(hint.contains("Found 15 results."));
    }

    #[test]
    fn decay_tool_hint_unknown_tool() {
        let hint = decay_tool_hint("some_other_tool", "/tmp/decay.txt", 102400, "");
        assert!(hint.contains("Large result"));
        assert!(hint.contains("100")); // ~100KB
        assert!(hint.contains("/tmp/decay.txt"));
    }

    // =========================================================================
    // micro_compact tests
    // =========================================================================

    fn make_msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.into(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: if role == "tool" { Some("tc1".into()) } else { None },
        }
    }

    #[test]
    fn micro_compact_no_tool_messages() {
        let mut conv = vec![
            make_msg("system", "You are helpful"),
            make_msg("user", "Hello"),
            make_msg("assistant", "Hi there"),
        ];
        micro_compact(&mut conv);
        assert_eq!(conv[0].content.as_deref(), Some("You are helpful"));
        assert_eq!(conv[1].content.as_deref(), Some("Hello"));
        assert_eq!(conv[2].content.as_deref(), Some("Hi there"));
    }

    #[test]
    fn micro_compact_fewer_than_keep_recent() {
        let mut conv = vec![
            make_msg("user", "Hello"),
            make_msg("tool", "a".repeat(500).as_str()),
            make_msg("tool", "b".repeat(500).as_str()),
        ];
        micro_compact(&mut conv);
        // Only 2 tool messages <= KEEP_RECENT(3), nothing compacted
        assert!(conv[1].content.as_ref().unwrap().len() == 500);
        assert!(conv[2].content.as_ref().unwrap().len() == 500);
    }

    #[test]
    fn micro_compact_compacts_old_large_tool_results() {
        let large = "x".repeat(300);
        let mut conv = vec![
            make_msg("user", "q1"),
            make_msg("tool", &large),      // old — should be compacted
            make_msg("tool", &large),      // old — should be compacted
            make_msg("tool", "small"),     // recent 3rd from end — kept
            make_msg("tool", &large),      // recent 2nd — kept
            make_msg("tool", &large),      // recent 1st — kept
        ];
        micro_compact(&mut conv);
        // First two tool messages (idx 1,2) should be compacted
        assert_eq!(conv[1].content.as_deref(), Some("[Previous tool result omitted for context efficiency]"));
        assert_eq!(conv[2].content.as_deref(), Some("[Previous tool result omitted for context efficiency]"));
        // Last three tool messages (idx 3,4,5) should remain intact
        assert_eq!(conv[3].content.as_deref(), Some("small"));
        assert_eq!(conv[4].content.as_ref().unwrap().len(), 300);
        assert_eq!(conv[5].content.as_ref().unwrap().len(), 300);
    }

    #[test]
    fn micro_compact_preserves_small_old_results() {
        let mut conv = vec![
            make_msg("user", "q1"),
            make_msg("tool", "short"),     // old but small — kept
            make_msg("tool", "also short"), // old but small — kept
            make_msg("tool", "r3"),
            make_msg("tool", "r4"),
            make_msg("tool", "r5"),
        ];
        micro_compact(&mut conv);
        // Old but under threshold — preserved
        assert_eq!(conv[1].content.as_deref(), Some("short"));
        assert_eq!(conv[2].content.as_deref(), Some("also short"));
    }

    #[test]
    fn micro_compact_idempotent() {
        let large = "x".repeat(300);
        let mut conv = vec![
            make_msg("user", "q1"),
            make_msg("tool", &large),
            make_msg("tool", "r2"),
            make_msg("tool", "r3"),
            make_msg("tool", "r4"),
        ];
        micro_compact(&mut conv);
        let after_first = conv[1].content.clone();
        micro_compact(&mut conv);
        // Should be idempotent — placeholder is under threshold so won't be re-compacted
        assert_eq!(conv[1].content, after_first);
    }
}
