use std::fs;
use std::path::PathBuf;
use reqwest::Client;
use serde::{Deserialize, Serialize};

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
    let engine = super::search::get_engine(&provider);
    match engine.search("test", &api_key, 3).await {
        Ok(results) if !results.is_empty() => Ok(format!("Search OK ({})", engine.name())),
        Ok(_) => Err("No results returned".to_string()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    // --- AiConfig tests ---

    #[test]
    fn ai_config_serialize_deserialize_roundtrip() {
        let config = AiConfig {
            api_url: "https://api.openai.com/v1".to_string(),
            api_key: "sk-test-key".to_string(),
            model: "gpt-4".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            system_prompt: "You are helpful.".to_string(),
            base_prompt: "Be concise.".to_string(),
            search_api_key: "tvly-xxx".to_string(),
            search_provider: "tavily".to_string(),
            provider_keys: {
                let mut m = std::collections::HashMap::new();
                m.insert("https://api.openai.com/v1".to_string(), "sk-key1".to_string());
                m.insert("https://api.anthropic.com".to_string(), "sk-key2".to_string());
                m
            },
        };
        let json_str = serde_json::to_string(&config).unwrap();
        let restored: AiConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(restored.api_url, config.api_url);
        assert_eq!(restored.api_key, config.api_key);
        assert_eq!(restored.model, config.model);
        assert!((restored.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(restored.max_tokens, 4096);
        assert_eq!(restored.system_prompt, "You are helpful.");
        assert_eq!(restored.base_prompt, "Be concise.");
        assert_eq!(restored.search_api_key, "tvly-xxx");
        assert_eq!(restored.search_provider, "tavily");
        assert_eq!(restored.provider_keys.len(), 2);
        assert_eq!(restored.provider_keys.get("https://api.openai.com/v1").unwrap(), "sk-key1");
    }

    #[test]
    fn ai_config_deserialize_with_defaults() {
        // Fields with #[serde(default)] should default to empty string / empty map
        let json = r#"{
            "api_url": "https://api.example.com",
            "api_key": "key",
            "model": "gpt-3.5",
            "temperature": 0.5,
            "max_tokens": 1024
        }"#;
        let config: AiConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.api_url, "https://api.example.com");
        assert_eq!(config.model, "gpt-3.5");
        assert_eq!(config.system_prompt, ""); // default
        assert_eq!(config.base_prompt, ""); // default
        assert_eq!(config.search_api_key, ""); // default
        assert_eq!(config.search_provider, ""); // default
        assert!(config.provider_keys.is_empty()); // default
    }

    #[test]
    fn ai_config_temperature_precision() {
        let json = r#"{
            "api_url": "u",
            "api_key": "k",
            "model": "m",
            "temperature": 0.123456789,
            "max_tokens": 100
        }"#;
        let config: AiConfig = serde_json::from_str(json).unwrap();
        assert!((config.temperature - 0.123456789).abs() < 1e-9);
    }

    #[test]
    fn ai_config_serialization_includes_all_fields() {
        let config = AiConfig {
            api_url: "u".to_string(),
            api_key: "k".to_string(),
            model: "m".to_string(),
            temperature: 0.0,
            max_tokens: 1,
            system_prompt: "".to_string(),
            base_prompt: "".to_string(),
            search_api_key: "".to_string(),
            search_provider: "".to_string(),
            provider_keys: std::collections::HashMap::new(),
        };
        let json = serde_json::to_value(&config).unwrap();
        // All fields present even if empty
        assert!(json.get("api_url").is_some());
        assert!(json.get("api_key").is_some());
        assert!(json.get("model").is_some());
        assert!(json.get("temperature").is_some());
        assert!(json.get("max_tokens").is_some());
        assert!(json.get("system_prompt").is_some());
        assert!(json.get("base_prompt").is_some());
        assert!(json.get("search_api_key").is_some());
        assert!(json.get("search_provider").is_some());
        assert!(json.get("provider_keys").is_some());
    }

    // --- AiMemories tests ---

    #[test]
    fn ai_memories_default_is_empty() {
        let mem = AiMemories::default();
        assert!(mem.dirs.is_empty());
    }

    #[test]
    fn ai_memories_roundtrip() {
        let mut mem = AiMemories::default();
        mem.dirs.insert("/project".to_string(), vec![
            MemoryEntry {
                content: "Uses React 19".to_string(),
                created_at: "2025-01-01T00:00:00Z".to_string(),
            },
        ]);
        let json = serde_json::to_string(&mem).unwrap();
        let restored: AiMemories = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.dirs.len(), 1);
        let entries = restored.dirs.get("/project").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "Uses React 19");
        assert_eq!(entries[0].created_at, "2025-01-01T00:00:00Z");
    }

    #[test]
    fn memory_entry_serialize_deserialize() {
        let entry = MemoryEntry {
            content: "Important fact".to_string(),
            created_at: "2025-06-15T12:00:00+00:00".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let restored: MemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.content, "Important fact");
        assert_eq!(restored.created_at, "2025-06-15T12:00:00+00:00");
    }

    #[test]
    fn ai_memories_multiple_dirs() {
        let mut mem = AiMemories::default();
        mem.dirs.insert("/a".to_string(), vec![
            MemoryEntry { content: "a1".to_string(), created_at: "t1".to_string() },
            MemoryEntry { content: "a2".to_string(), created_at: "t2".to_string() },
        ]);
        mem.dirs.insert("/b".to_string(), vec![
            MemoryEntry { content: "b1".to_string(), created_at: "t3".to_string() },
        ]);
        let json = serde_json::to_string(&mem).unwrap();
        let restored: AiMemories = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.dirs.len(), 2);
        assert_eq!(restored.dirs["/a"].len(), 2);
        assert_eq!(restored.dirs["/b"].len(), 1);
    }
}
