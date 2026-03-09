use crate::ai::skill::{Skill, SkillState};
use crate::ai::tool::registry::ToolFilter;

pub struct DefaultSkill;

impl Skill for DefaultSkill {
    fn id(&self) -> &str { "default" }
    fn display_name(&self) -> &str { "Default" }
    fn description(&self) -> &str { "General-purpose assistant" }

    fn should_activate(&self, _message: &str, _has_files: bool, _current: &str) -> bool {
        false // Fallback only - never actively matches
    }

    fn system_prompt(&self, _state: &SkillState) -> String {
        String::new() // Uses the user-configured base_prompt from AiConfig
    }

    fn tool_filter(&self, _state: &SkillState) -> ToolFilter {
        ToolFilter::All
    }

    fn max_iterations(&self, _state: &SkillState) -> usize { 20 }
    fn token_budget(&self, _state: &SkillState) -> u32 { 4096 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_returns_default() {
        let skill = DefaultSkill;
        assert_eq!(skill.id(), "default");
    }

    #[test]
    fn should_activate_returns_false() {
        let skill = DefaultSkill;
        // DefaultSkill never actively matches; it's a fallback only
        assert!(!skill.should_activate("anything", false, "default"));
        assert!(!skill.should_activate("research this", true, "other"));
        assert!(!skill.should_activate("", false, ""));
    }

    #[test]
    fn priority_is_zero() {
        let skill = DefaultSkill;
        assert_eq!(skill.priority(), 0);
    }

    #[test]
    fn tool_filter_returns_all() {
        let skill = DefaultSkill;
        let state = SkillState::default();
        match skill.tool_filter(&state) {
            ToolFilter::All => {} // expected
            _ => panic!("DefaultSkill tool_filter should be ToolFilter::All"),
        }
    }

    #[test]
    fn max_iterations_returns_20() {
        let skill = DefaultSkill;
        let state = SkillState::default();
        assert_eq!(skill.max_iterations(&state), 20);
    }

    #[test]
    fn system_prompt_is_empty() {
        let skill = DefaultSkill;
        let state = SkillState::default();
        assert!(skill.system_prompt(&state).is_empty());
    }

    #[test]
    fn token_budget_returns_4096() {
        let skill = DefaultSkill;
        let state = SkillState::default();
        assert_eq!(skill.token_budget(&state), 4096);
    }
}
