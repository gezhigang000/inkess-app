use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

use super::types::Memory;
use super::MemoryStore;

/// Lightweight index entry kept in memory for fast filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexEntry {
    #[serde(rename = "type")]
    memory_type: String,
    importance: f32,
    workspace: Option<String>,
    tags: Vec<String>,
}

/// On-disk index format.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryIndex {
    version: u32,
    memories: HashMap<String, IndexEntry>,
}

impl MemoryIndex {
    fn new() -> Self {
        Self {
            version: 1,
            memories: HashMap::new(),
        }
    }
}

pub struct FileMemoryStore {
    base_dir: PathBuf,
    index: Mutex<MemoryIndex>,
}

impl FileMemoryStore {
    pub fn new(base_dir: PathBuf) -> Result<Self, String> {
        // Create directory structure
        for subdir in &["core", "episodic", "procedural", "semantic"] {
            fs::create_dir_all(base_dir.join(subdir))
                .map_err(|e| format!("Failed to create memory directory {}: {}", subdir, e))?;
        }

        // Load or rebuild index
        let index = Self::load_or_rebuild_index(&base_dir)?;

        Ok(Self {
            base_dir,
            index: Mutex::new(index),
        })
    }

    fn index_path(base_dir: &PathBuf) -> PathBuf {
        base_dir.join("index.json")
    }

    fn memory_file_path(&self, memory_type: &str, id: &str) -> PathBuf {
        self.base_dir.join(memory_type).join(format!("{}.json", id))
    }

    fn load_or_rebuild_index(base_dir: &PathBuf) -> Result<MemoryIndex, String> {
        let index_path = Self::index_path(base_dir);

        if index_path.exists() {
            match fs::read_to_string(&index_path) {
                Ok(content) => {
                    if let Ok(index) = serde_json::from_str::<MemoryIndex>(&content) {
                        return Ok(index);
                    }
                    // Corrupt index, fall through to rebuild
                }
                Err(_) => {
                    // Can't read, fall through to rebuild
                }
            }
        }

        // Rebuild index by scanning subdirectories
        Self::rebuild_index(base_dir)
    }

    fn rebuild_index(base_dir: &PathBuf) -> Result<MemoryIndex, String> {
        let mut index = MemoryIndex::new();

        for subdir in &["core", "episodic", "procedural", "semantic"] {
            let dir = base_dir.join(subdir);
            if !dir.exists() {
                continue;
            }

            let entries = fs::read_dir(&dir)
                .map_err(|e| format!("Failed to read {} directory: {}", subdir, e))?;

            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }

                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(memory) = serde_json::from_str::<Memory>(&content) {
                        index.memories.insert(
                            memory.id.clone(),
                            IndexEntry {
                                memory_type: memory.memory_type.as_str().to_string(),
                                importance: memory.importance,
                                workspace: memory.metadata.workspace_path.clone(),
                                tags: memory.metadata.tags.clone(),
                            },
                        );
                    }
                }
            }
        }

        // Flush rebuilt index
        Self::write_index_to_disk(base_dir, &index)?;

        Ok(index)
    }

    fn write_index_to_disk(base_dir: &PathBuf, index: &MemoryIndex) -> Result<(), String> {
        let index_path = Self::index_path(base_dir);
        let tmp_path = index_path.with_extension("json.tmp");

        let content = serde_json::to_string_pretty(index)
            .map_err(|e| format!("Failed to serialize index: {}", e))?;

        fs::write(&tmp_path, &content)
            .map_err(|e| format!("Failed to write temp index: {}", e))?;

        fs::rename(&tmp_path, &index_path)
            .map_err(|e| format!("Failed to rename index file: {}", e))?;

        Ok(())
    }

    fn flush_index(&self, index: &MemoryIndex) -> Result<(), String> {
        Self::write_index_to_disk(&self.base_dir, index)
    }

    /// Evict old low-importance episodic memories when total count exceeds the limit.
    /// Keeps all core memories. Deletes episodic memories with importance < 0.3, oldest first.
    fn evict_if_needed(&self, index: &mut MemoryIndex) -> Result<(), String> {
        const MAX_MEMORIES: usize = 500;
        const EVICTION_IMPORTANCE_THRESHOLD: f32 = 0.3;

        if index.memories.len() <= MAX_MEMORIES {
            return Ok(());
        }

        // Collect eviction candidates: episodic memories with low importance
        let mut candidates: Vec<(String, f32)> = index
            .memories
            .iter()
            .filter(|(_, entry)| {
                entry.memory_type == "episodic" && entry.importance < EVICTION_IMPORTANCE_THRESHOLD
            })
            .map(|(id, entry)| (id.clone(), entry.importance))
            .collect();

        if candidates.is_empty() {
            return Ok(());
        }

        // Sort by importance ascending (lowest first)
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Delete enough to get back under the limit
        let to_delete = index.memories.len().saturating_sub(MAX_MEMORIES);
        let to_delete = to_delete.min(candidates.len());

        for (id, _) in candidates.into_iter().take(to_delete) {
            // Remove file (best-effort)
            let path = self.memory_file_path("episodic", &id);
            if path.exists() {
                let _ = fs::remove_file(&path);
            }
            index.memories.remove(&id);
        }

        self.flush_index(index)?;
        Ok(())
    }

    fn load_memory_file(&self, memory_type: &str, id: &str) -> Result<Memory, String> {
        let path = self.memory_file_path(memory_type, id);
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read memory {}: {}", id, e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse memory {}: {}", id, e))
    }

    fn save_memory_file(&self, memory: &Memory) -> Result<(), String> {
        let type_str = memory.memory_type.as_str();
        let path = self.memory_file_path(type_str, &memory.id);

        let content = serde_json::to_string_pretty(memory)
            .map_err(|e| format!("Failed to serialize memory: {}", e))?;

        fs::write(&path, &content)
            .map_err(|e| format!("Failed to write memory file: {}", e))?;

        Ok(())
    }
}

#[async_trait]
impl MemoryStore for FileMemoryStore {
    async fn save(&self, memory: Memory) -> Result<String, String> {
        let id = if memory.id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            memory.id.clone()
        };

        let mut mem = memory;
        mem.id = id.clone();

        // Save memory file
        self.save_memory_file(&mem)?;

        // Update index
        let mut index = self.index.lock().map_err(|e| format!("Lock error: {}", e))?;
        index.memories.insert(
            id.clone(),
            IndexEntry {
                memory_type: mem.memory_type.as_str().to_string(),
                importance: mem.importance,
                workspace: mem.metadata.workspace_path.clone(),
                tags: mem.metadata.tags.clone(),
            },
        );
        self.flush_index(&index)?;

        // Evict old low-importance memories if over the limit
        self.evict_if_needed(&mut index)?;

        Ok(id)
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Memory>, String> {
        let query_lower = query.to_lowercase();

        // First pass: collect candidate IDs from index (tag matching)
        let candidates: Vec<(String, String)> = {
            let index = self.index.lock().map_err(|e| format!("Lock error: {}", e))?;
            index
                .memories
                .iter()
                .map(|(id, entry)| (id.clone(), entry.memory_type.clone()))
                .collect()
        };

        // Second pass: load files and check content
        let mut results: Vec<Memory> = Vec::new();
        for (id, mem_type) in &candidates {
            if let Ok(memory) = self.load_memory_file(mem_type, id) {
                if memory.content.to_lowercase().contains(&query_lower) {
                    results.push(memory);
                }
            }
        }

        // Sort by importance DESC, then accessed_at DESC
        results.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.accessed_at.cmp(&a.accessed_at))
        });

        results.truncate(limit);
        Ok(results)
    }

    async fn get_core_memories(&self) -> Result<Vec<Memory>, String> {
        let core_ids: Vec<String> = {
            let index = self.index.lock().map_err(|e| format!("Lock error: {}", e))?;
            index
                .memories
                .iter()
                .filter(|(_, entry)| entry.memory_type == "core")
                .map(|(id, _)| id.clone())
                .collect()
        };

        let mut memories: Vec<Memory> = Vec::new();
        for id in &core_ids {
            if let Ok(memory) = self.load_memory_file("core", id) {
                memories.push(memory);
            }
        }

        memories.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(memories)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Memory>, String> {
        let mem_type = {
            let index = self.index.lock().map_err(|e| format!("Lock error: {}", e))?;
            index.memories.get(id).map(|entry| entry.memory_type.clone())
        };

        match mem_type {
            Some(t) => {
                let memory = self.load_memory_file(&t, id)?;
                Ok(Some(memory))
            }
            None => Ok(None),
        }
    }

    async fn update_importance(&self, id: &str, importance: f32) -> Result<(), String> {
        let mem_type = {
            let index = self.index.lock().map_err(|e| format!("Lock error: {}", e))?;
            index
                .memories
                .get(id)
                .map(|entry| entry.memory_type.clone())
        };

        let mem_type = mem_type.ok_or_else(|| format!("Memory not found: {}", id))?;

        // Load, update, save
        let mut memory = self.load_memory_file(&mem_type, id)?;
        memory.importance = importance;
        self.save_memory_file(&memory)?;

        // Update index
        let mut index = self.index.lock().map_err(|e| format!("Lock error: {}", e))?;
        if let Some(entry) = index.memories.get_mut(id) {
            entry.importance = importance;
        }
        self.flush_index(&index)?;

        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<(), String> {
        let mem_type = {
            let mut index = self.index.lock().map_err(|e| format!("Lock error: {}", e))?;
            let entry = index
                .memories
                .remove(id)
                .ok_or_else(|| format!("Memory not found: {}", id))?;
            self.flush_index(&index)?;
            entry.memory_type
        };

        // Remove file (best-effort, index already updated)
        let path = self.memory_file_path(&mem_type, id);
        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::memory::types::{MemoryType, MemoryMetadata};

    fn test_dir() -> PathBuf {
        std::env::temp_dir().join(format!("inkess-mem-test-{}", Uuid::new_v4()))
    }

    fn make_memory(content: &str, mem_type: MemoryType, importance: f32) -> Memory {
        Memory {
            id: String::new(),
            content: content.to_string(),
            memory_type: mem_type,
            importance,
            metadata: MemoryMetadata {
                tags: vec!["test".into()],
                source: "test".into(),
                workspace_path: None,
            },
            created_at: chrono::Utc::now().timestamp(),
            accessed_at: chrono::Utc::now().timestamp(),
            access_count: 0,
        }
    }

    #[tokio::test]
    async fn test_new_creates_directories() {
        let dir = test_dir();
        let _store = FileMemoryStore::new(dir.clone()).unwrap();
        assert!(dir.join("core").exists());
        assert!(dir.join("episodic").exists());
        assert!(dir.join("procedural").exists());
        assert!(dir.join("semantic").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_save_assigns_id() {
        let dir = test_dir();
        let store = FileMemoryStore::new(dir.clone()).unwrap();
        let mem = make_memory("test content", MemoryType::Core, 0.9);
        let id = store.save(mem).await.unwrap();
        assert!(!id.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_save_and_get_by_id_roundtrip() {
        let dir = test_dir();
        let store = FileMemoryStore::new(dir.clone()).unwrap();
        let mem = make_memory("roundtrip test", MemoryType::Semantic, 0.5);
        let id = store.save(mem).await.unwrap();

        let loaded = store.get_by_id(&id).await.unwrap().unwrap();
        assert_eq!(loaded.content, "roundtrip test");
        assert_eq!(loaded.importance, 0.5);
        assert_eq!(loaded.memory_type.as_str(), "semantic");
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_search_finds_matching() {
        let dir = test_dir();
        let store = FileMemoryStore::new(dir.clone()).unwrap();
        store.save(make_memory("rust programming language", MemoryType::Semantic, 0.5)).await.unwrap();
        store.save(make_memory("python data analysis", MemoryType::Semantic, 0.5)).await.unwrap();

        let results = store.search("rust", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("rust"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_search_respects_limit() {
        let dir = test_dir();
        let store = FileMemoryStore::new(dir.clone()).unwrap();
        for i in 0..5 {
            store.save(make_memory(&format!("item {}", i), MemoryType::Episodic, 0.3)).await.unwrap();
        }
        let results = store.search("item", 2).await.unwrap();
        assert_eq!(results.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_get_core_memories() {
        let dir = test_dir();
        let store = FileMemoryStore::new(dir.clone()).unwrap();
        store.save(make_memory("core fact", MemoryType::Core, 0.9)).await.unwrap();
        store.save(make_memory("episodic event", MemoryType::Episodic, 0.3)).await.unwrap();

        let core = store.get_core_memories().await.unwrap();
        assert_eq!(core.len(), 1);
        assert!(core[0].content.contains("core fact"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_delete() {
        let dir = test_dir();
        let store = FileMemoryStore::new(dir.clone()).unwrap();
        let id = store.save(make_memory("to delete", MemoryType::Episodic, 0.3)).await.unwrap();

        assert!(store.get_by_id(&id).await.unwrap().is_some());
        store.delete(&id).await.unwrap();
        assert!(store.get_by_id(&id).await.unwrap().is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_update_importance() {
        let dir = test_dir();
        let store = FileMemoryStore::new(dir.clone()).unwrap();
        let id = store.save(make_memory("update me", MemoryType::Core, 0.5)).await.unwrap();

        store.update_importance(&id, 0.95).await.unwrap();
        let loaded = store.get_by_id(&id).await.unwrap().unwrap();
        assert!((loaded.importance - 0.95).abs() < f32::EPSILON);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_index_rebuild_on_corruption() {
        let dir = test_dir();
        let store = FileMemoryStore::new(dir.clone()).unwrap();
        let id = store.save(make_memory("survive rebuild", MemoryType::Core, 0.9)).await.unwrap();
        drop(store);

        // Corrupt the index
        fs::write(dir.join("index.json"), "corrupted!").unwrap();

        // Reopen — should rebuild from files
        let store2 = FileMemoryStore::new(dir.clone()).unwrap();
        let loaded = store2.get_by_id(&id).await.unwrap();
        assert!(loaded.is_some());
        assert!(loaded.unwrap().content.contains("survive rebuild"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_get_by_id_not_found() {
        let dir = test_dir();
        let store = FileMemoryStore::new(dir.clone()).unwrap();
        let result = store.get_by_id("nonexistent-id").await.unwrap();
        assert!(result.is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
