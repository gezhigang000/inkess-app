use async_trait::async_trait;

pub mod distill;
pub mod store;
pub mod types;

pub use store::FileMemoryStore;
pub use types::{Memory, MemoryMetadata, MemoryType};

#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn save(&self, memory: Memory) -> Result<String, String>;
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Memory>, String>;
    async fn get_core_memories(&self) -> Result<Vec<Memory>, String>;
    async fn get_by_id(&self, id: &str) -> Result<Option<Memory>, String>;
    async fn update_importance(&self, id: &str, importance: f32) -> Result<(), String>;
    async fn delete(&self, id: &str) -> Result<(), String>;
}
