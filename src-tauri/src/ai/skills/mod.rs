pub mod default;
pub mod deep_research;
pub mod file_processing;
pub mod code_analysis;

use std::sync::Arc;
use super::skill::registry::SkillRegistry;

pub async fn register_builtin_skills(registry: &SkillRegistry) {
    registry.register(Arc::new(default::DefaultSkill)).await;
    registry.register(Arc::new(deep_research::DeepResearchSkill)).await;
    registry.register(Arc::new(file_processing::FileProcessingSkill)).await;
    registry.register(Arc::new(code_analysis::CodeAnalysisSkill)).await;
}
