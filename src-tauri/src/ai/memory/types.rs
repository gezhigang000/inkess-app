use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub importance: f32,
    pub metadata: MemoryMetadata,
    pub created_at: i64,
    pub accessed_at: i64,
    pub access_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    Core,       // Persistent facts about user/project
    Episodic,   // Conversation summaries
    Procedural, // How-to knowledge
    Semantic,   // General facts/concepts
}

impl MemoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryType::Core => "core",
            MemoryType::Episodic => "episodic",
            MemoryType::Procedural => "procedural",
            MemoryType::Semantic => "semantic",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "core" => Ok(MemoryType::Core),
            "episodic" => Ok(MemoryType::Episodic),
            "procedural" => Ok(MemoryType::Procedural),
            "semantic" => Ok(MemoryType::Semantic),
            _ => Err(format!("Invalid memory type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetadata {
    pub tags: Vec<String>,
    pub source: String,
    pub workspace_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_type_as_str() {
        assert_eq!(MemoryType::Core.as_str(), "core");
        assert_eq!(MemoryType::Episodic.as_str(), "episodic");
        assert_eq!(MemoryType::Procedural.as_str(), "procedural");
        assert_eq!(MemoryType::Semantic.as_str(), "semantic");
    }

    #[test]
    fn test_memory_type_from_str_lowercase() {
        assert!(matches!(
            MemoryType::from_str("core").unwrap(),
            MemoryType::Core
        ));
        assert!(matches!(
            MemoryType::from_str("episodic").unwrap(),
            MemoryType::Episodic
        ));
        assert!(matches!(
            MemoryType::from_str("procedural").unwrap(),
            MemoryType::Procedural
        ));
        assert!(matches!(
            MemoryType::from_str("semantic").unwrap(),
            MemoryType::Semantic
        ));
    }

    #[test]
    fn test_memory_type_from_str_case_insensitive() {
        assert!(matches!(
            MemoryType::from_str("Core").unwrap(),
            MemoryType::Core
        ));
        assert!(matches!(
            MemoryType::from_str("CORE").unwrap(),
            MemoryType::Core
        ));
        assert!(matches!(
            MemoryType::from_str("Episodic").unwrap(),
            MemoryType::Episodic
        ));
        assert!(matches!(
            MemoryType::from_str("EPISODIC").unwrap(),
            MemoryType::Episodic
        ));
        assert!(matches!(
            MemoryType::from_str("Procedural").unwrap(),
            MemoryType::Procedural
        ));
        assert!(matches!(
            MemoryType::from_str("PROCEDURAL").unwrap(),
            MemoryType::Procedural
        ));
        assert!(matches!(
            MemoryType::from_str("Semantic").unwrap(),
            MemoryType::Semantic
        ));
        assert!(matches!(
            MemoryType::from_str("SEMANTIC").unwrap(),
            MemoryType::Semantic
        ));
    }

    #[test]
    fn test_memory_type_from_str_invalid() {
        assert!(MemoryType::from_str("invalid").is_err());
        assert!(MemoryType::from_str("").is_err());
        assert!(MemoryType::from_str("coreee").is_err());
        assert!(MemoryType::from_str("episodic2").is_err());
    }
}
