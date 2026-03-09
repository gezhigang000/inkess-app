use serde::Deserialize;
use reqwest::Client;
use crate::ai::{AiConfig, ChatMessage};
use super::{Memory, MemoryMetadata, MemoryType};

/// Minimum number of messages before auto-distill triggers
pub const DISTILL_THRESHOLD: usize = 50;

const DISTILL_TIMEOUT_SECS: u64 = 15;
const MAX_CONVERSATION_CHARS: usize = 50_000; // Limit conversation text to avoid huge prompts

#[derive(Deserialize)]
struct DistillResult {
    content: String,
    memory_type: String,
    importance: f32,
    tags: Vec<String>,
}

/// Format conversation messages into readable text for distillation
fn format_conversation(messages: &[ChatMessage]) -> String {
    let mut lines = Vec::new();

    for msg in messages {
        match msg.role.as_str() {
            "user" => {
                if let Some(content) = &msg.content {
                    lines.push(format!("User: {}", content));
                }
            }
            "assistant" => {
                if let Some(content) = &msg.content {
                    if !content.trim().is_empty() {
                        lines.push(format!("Assistant: {}", content));
                    }
                }
                // Include tool calls for context
                if let Some(tool_calls) = &msg.tool_calls {
                    for tc in tool_calls {
                        lines.push(format!("  [Tool: {}]", tc.function.name));
                    }
                }
            }
            "tool" => {
                // Skip tool results to keep it concise
            }
            _ => {}
        }
    }

    let full_text = lines.join("\n");

    // Truncate if too long (char-safe to avoid panic on CJK/emoji)
    if full_text.len() > MAX_CONVERSATION_CHARS {
        let truncated: String = full_text.chars().take(MAX_CONVERSATION_CHARS).collect();
        format!("{}...\n[conversation truncated]", truncated)
    } else {
        full_text
    }
}

/// Call LLM to distill conversation into structured memory
async fn call_llm_for_distill(
    conversation_text: String,
    ai_config: &AiConfig,
) -> Result<String, String> {
    let distill_prompt = format!(
        r#"Analyze this conversation and extract the most important learning or fact.
Output ONLY valid JSON in this exact format (no markdown, no code blocks):

{{
  "content": "A concise summary of the key learning (1-2 sentences)",
  "memory_type": "core|episodic|procedural|semantic",
  "importance": 0.8,
  "tags": ["tag1", "tag2"]
}}

Guidelines:
- core: Persistent facts about user/project (importance 0.8-1.0)
- episodic: What happened in this conversation (importance 0.5-0.7)
- procedural: How to do something (importance 0.6-0.9)
- semantic: General knowledge/concepts (importance 0.4-0.7)
- Extract 2-4 relevant tags
- Focus on actionable or memorable insights

Conversation:
{}"#,
        conversation_text
    );

    let client = Client::new();
    let url = format!("{}/chat/completions", ai_config.api_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": ai_config.model,
        "messages": [
            {
                "role": "user",
                "content": distill_prompt
            }
        ],
        "temperature": 0.3, // Lower temperature for more consistent JSON output
        "max_tokens": 500,
        "stream": false,
    });

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", ai_config.api_key))
        .header("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(DISTILL_TIMEOUT_SECS))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Distill request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Distill API error ({}): {}", status, text));
    }

    #[derive(Deserialize)]
    struct CompletionResponse {
        choices: Vec<CompletionChoice>,
    }

    #[derive(Deserialize)]
    struct CompletionChoice {
        message: CompletionMessage,
    }

    #[derive(Deserialize)]
    struct CompletionMessage {
        content: String,
    }

    let response: CompletionResponse = resp.json().await
        .map_err(|e| format!("Failed to parse distill response: {}", e))?;

    response.choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| "No content in distill response".to_string())
}

/// Distill a conversation into a Memory
pub async fn distill_conversation(
    messages: &[ChatMessage],
    ai_config: &AiConfig,
    workspace_path: Option<String>,
) -> Result<Memory, String> {
    // Format conversation
    let conversation_text = format_conversation(messages);

    if conversation_text.trim().is_empty() {
        return Err("Empty conversation, nothing to distill".to_string());
    }

    // Call LLM
    let response = call_llm_for_distill(conversation_text, ai_config).await?;

    // Parse JSON response (strip markdown code blocks if present)
    let json_str = response.trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let parsed: DistillResult = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse distill JSON: {}. Response: {}", e, json_str))?;

    // Validate and clamp importance
    let importance = parsed.importance.clamp(0.0, 1.0);

    // Parse memory type
    let memory_type = MemoryType::from_str(&parsed.memory_type)?;

    // Create Memory
    let now = chrono::Utc::now().timestamp();
    Ok(Memory {
        id: uuid::Uuid::new_v4().to_string(),
        content: parsed.content,
        memory_type,
        importance,
        metadata: MemoryMetadata {
            tags: parsed.tags,
            source: "auto_distill".to_string(),
            workspace_path,
        },
        created_at: now,
        accessed_at: now,
        access_count: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_conversation() {
        let messages = vec![
            ChatMessage {
                role: "user".to_string(),
                content: Some("How do I use Rust async?".to_string()),
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: Some("You need to use tokio runtime.".to_string()),
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let formatted = format_conversation(&messages);
        assert!(formatted.contains("User: How do I use Rust async?"));
        assert!(formatted.contains("Assistant: You need to use tokio runtime."));
    }

    #[test]
    fn test_format_conversation_truncates() {
        let long_content = "a".repeat(60_000);
        let messages = vec![
            ChatMessage {
                role: "user".to_string(),
                content: Some(long_content),
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let formatted = format_conversation(&messages);
        assert!(formatted.len() <= MAX_CONVERSATION_CHARS + 100); // +100 for truncation message
        assert!(formatted.contains("[conversation truncated]"));
    }
}
