use std::fs;
use async_trait::async_trait;
use serde_json::Value;
use crate::ai::tool::{ToolPlugin, ToolContext, ToolOutput, ToolError};
use crate::ai::sandbox_path;

pub struct EditFileTool;

#[async_trait]
impl ToolPlugin for EditFileTool {
    fn name(&self) -> &str { "edit_file" }
    fn description(&self) -> &str {
        "Edit a file by applying precise line-based operations (replace, insert, delete). More efficient than write_file for small changes."
    }
    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path relative to workspace" },
                "edits": {
                    "type": "array",
                    "description": "Array of edit operations to apply",
                    "items": {
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["replace", "insert", "delete"], "description": "Edit action type" },
                            "line_start": { "type": "number", "description": "Start line number (1-based)" },
                            "line_end": { "type": "number", "description": "End line number (1-based, inclusive). Required for replace and delete." },
                            "content": { "type": "string", "description": "New content. Required for replace and insert." }
                        },
                        "required": ["action", "line_start"]
                    }
                }
            },
            "required": ["path", "edits"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<ToolOutput, ToolError> {
        let raw_path = input["path"].as_str().unwrap_or("");

        // Block sensitive paths (same as write_file)
        let lower = raw_path.to_lowercase();
        let sensitive = [".env", ".git/", ".git\\", ".ssh/", ".ssh\\",
            ".bash_history", ".zsh_history", ".npmrc", ".pypirc",
            ".docker/", ".docker\\", ".kube/", ".kube\\",
            ".aws/", ".aws\\", ".config/gh", ".config\\gh",
            ".gnupg/", ".gnupg\\", ".netrc"];
        for s in &sensitive {
            if lower.contains(s) || lower == ".env" {
                return Ok(ToolOutput::error(format!("Cannot edit sensitive path: {}", raw_path)));
            }
        }

        let path = match sandbox_path(raw_path, &ctx.workspace_path) {
            Some(p) => p,
            None => return Ok(ToolOutput::error(format!(
                "Access denied: path '{}' is outside the current workspace.", raw_path
            ))),
        };

        // Read file
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => return Ok(ToolOutput::error(format!("Failed to read file: {}", e))),
        };
        // Detect line ending style (CRLF vs LF) before .lines() strips them
        let line_ending = if content.contains("\r\n") { "\r\n" } else { "\n" };
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let original_count = lines.len();

        // Parse edits
        let edits = match input["edits"].as_array() {
            Some(arr) => arr,
            None => return Ok(ToolOutput::error("'edits' must be an array".to_string())),
        };

        if edits.is_empty() {
            return Ok(ToolOutput::error("No edits provided".to_string()));
        }

        // Parse and validate each edit
        struct EditOp {
            action: String,
            line_start: usize,
            line_end: usize,
            content: String,
        }

        let mut ops: Vec<EditOp> = Vec::new();
        for (i, edit) in edits.iter().enumerate() {
            let action = edit["action"].as_str().unwrap_or("").to_string();
            if !["replace", "insert", "delete"].contains(&action.as_str()) {
                return Ok(ToolOutput::error(format!(
                    "Edit {}: invalid action '{}'. Must be replace, insert, or delete.", i, action
                )));
            }
            let line_start = edit["line_start"].as_u64().unwrap_or(0) as usize;
            if line_start == 0 {
                return Ok(ToolOutput::error(format!(
                    "Edit {}: line_start must be a positive integer.", i
                )));
            }

            let line_end = if action == "insert" {
                line_start // insert doesn't use line_end
            } else {
                let end = edit["line_end"].as_u64().unwrap_or(0) as usize;
                if end == 0 {
                    return Ok(ToolOutput::error(format!(
                        "Edit {}: line_end is required for {} action.", i, action
                    )));
                }
                if end < line_start {
                    return Ok(ToolOutput::error(format!(
                        "Edit {}: line_end ({}) must be >= line_start ({}).", i, end, line_start
                    )));
                }
                end
            };

            let content_str = edit["content"].as_str().unwrap_or("").to_string();
            if (action == "replace" || action == "insert") && content_str.is_empty() && edit["content"].is_null() {
                return Ok(ToolOutput::error(format!(
                    "Edit {}: content is required for {} action.", i, action
                )));
            }

            ops.push(EditOp { action, line_start, line_end, content: content_str });
        }

        // Sort by line_start descending so edits from bottom don't shift upper line numbers
        ops.sort_by(|a, b| b.line_start.cmp(&a.line_start));

        // Apply edits
        let mut summary: Vec<String> = Vec::new();
        for op in &ops {
            match op.action.as_str() {
                "replace" => {
                    if op.line_end > lines.len() {
                        return Ok(ToolOutput::error(format!(
                            "Line range {}-{} exceeds file length ({} lines).", op.line_start, op.line_end, lines.len()
                        )));
                    }
                    let new_lines: Vec<String> = op.content.lines().map(|l| l.to_string()).collect();
                    let removed = op.line_end - op.line_start + 1;
                    lines.splice((op.line_start - 1)..op.line_end, new_lines.iter().cloned());
                    summary.push(format!("Replaced lines {}-{} ({} lines -> {} lines)", op.line_start, op.line_end, removed, new_lines.len()));
                }
                "insert" => {
                    if op.line_start > lines.len() + 1 {
                        return Ok(ToolOutput::error(format!(
                            "Insert line {} exceeds file length ({} lines). Max insert position is {}.",
                            op.line_start, lines.len(), lines.len() + 1
                        )));
                    }
                    let new_lines: Vec<String> = op.content.lines().map(|l| l.to_string()).collect();
                    let count = new_lines.len();
                    let insert_idx = op.line_start - 1;
                    for (j, line) in new_lines.into_iter().enumerate() {
                        lines.insert(insert_idx + j, line);
                    }
                    summary.push(format!("Inserted {} lines before line {}", count, op.line_start));
                }
                "delete" => {
                    if op.line_end > lines.len() {
                        return Ok(ToolOutput::error(format!(
                            "Line range {}-{} exceeds file length ({} lines).", op.line_start, op.line_end, lines.len()
                        )));
                    }
                    let removed = op.line_end - op.line_start + 1;
                    lines.drain((op.line_start - 1)..op.line_end);
                    summary.push(format!("Deleted lines {}-{} ({} lines)", op.line_start, op.line_end, removed));
                }
                _ => {}
            }
        }

        // Write back — preserve original line ending style
        let result = lines.join(line_ending);
        // Preserve trailing newline if original had one
        let final_content = if content.ends_with('\n') {
            format!("{}{}", result, line_ending)
        } else {
            result
        };

        match fs::write(&path, &final_content) {
            Ok(_) => {
                summary.reverse(); // Show in original order (we reversed for bottom-up application)
                Ok(ToolOutput::success(format!(
                    "File edited: {} ({} -> {} lines)\n{}",
                    path, original_count, lines.len(),
                    summary.join("\n")
                )))
            }
            Err(e) => Ok(ToolOutput::error(format!("Failed to write file: {}", e))),
        }
    }
}
