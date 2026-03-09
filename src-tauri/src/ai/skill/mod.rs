pub mod registry;

use serde::Serialize;
use crate::ai::tool::registry::ToolFilter;

/// State passed to Skill methods, persisted per conversation
#[derive(Clone, Debug, Default)]
pub struct SkillState {
    pub skill_id: String,
}

/// The core trait every skill must implement
pub trait Skill: Send + Sync + 'static {
    /// Unique skill identifier
    fn id(&self) -> &str;

    /// Display name for UI
    fn display_name(&self) -> &str;

    /// Short description
    fn description(&self) -> &str;

    /// Whether this skill should activate based on user message
    /// Returns true if the message matches this skill's activation criteria
    fn should_activate(&self, message: &str, has_files: bool, current_skill: &str) -> bool;

    /// Priority (higher wins when multiple skills match)
    fn priority(&self) -> u32 { 0 }

    /// System prompt to use when this skill is active
    fn system_prompt(&self, state: &SkillState) -> String;

    /// Tool filter controlling which tools LLM can see
    fn tool_filter(&self, state: &SkillState) -> ToolFilter;

    /// Maximum tool call iterations
    fn max_iterations(&self, state: &SkillState) -> usize;

    /// Output token budget hint
    fn token_budget(&self, _state: &SkillState) -> u32 { 4096 }
}

/// Skill info for frontend display
#[derive(Serialize, Clone, Debug)]
pub struct SkillInfo {
    pub id: String,
    pub display_name: String,
    pub description: String,
}
