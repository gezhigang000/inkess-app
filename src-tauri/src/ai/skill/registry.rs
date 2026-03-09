use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{Skill, SkillInfo};

pub struct SkillRegistry {
    skills: RwLock<HashMap<String, Arc<dyn Skill>>>,
    default_skill_id: String,
}

impl SkillRegistry {
    pub fn new(default_skill_id: &str) -> Self {
        Self {
            skills: RwLock::new(HashMap::new()),
            default_skill_id: default_skill_id.to_string(),
        }
    }

    pub async fn register(&self, skill: Arc<dyn Skill>) {
        let id = skill.id().to_string();
        let mut skills = self.skills.write().await;
        skills.insert(id, skill);
    }

    /// Detect which skill should activate based on user message.
    /// Returns the skill ID with highest priority among matching skills.
    /// Falls back to default skill if none match.
    pub async fn detect_activation(
        &self,
        message: &str,
        has_files: bool,
        current_skill_id: &str,
    ) -> String {
        let skills = self.skills.read().await;
        let mut best: Option<(u32, String)> = None;

        for skill in skills.values() {
            if skill.should_activate(message, has_files, current_skill_id) {
                let priority = skill.priority();
                if best.as_ref().map_or(true, |(p, _)| priority > *p) {
                    best = Some((priority, skill.id().to_string()));
                }
            }
        }

        best.map(|(_, id)| id).unwrap_or_else(|| self.default_skill_id.clone())
    }

    pub async fn get(&self, id: &str) -> Option<Arc<dyn Skill>> {
        let skills = self.skills.read().await;
        skills.get(id).cloned()
    }

    pub async fn get_default(&self) -> Option<Arc<dyn Skill>> {
        self.get(&self.default_skill_id).await
    }

    /// List all skills for frontend display
    pub async fn list_skills(&self) -> Vec<SkillInfo> {
        let skills = self.skills.read().await;
        skills.values().map(|s| SkillInfo {
            id: s.id().to_string(),
            display_name: s.display_name().to_string(),
            description: s.description().to_string(),
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::skill::{Skill, SkillState};
    use crate::ai::tool::registry::ToolFilter;

    struct MockSkill {
        skill_id: String,
        activate: bool,
        prio: u32,
    }

    impl MockSkill {
        fn new(id: &str, activate: bool, priority: u32) -> Self {
            Self { skill_id: id.to_string(), activate, prio: priority }
        }
    }

    impl Skill for MockSkill {
        fn id(&self) -> &str { &self.skill_id }
        fn display_name(&self) -> &str { &self.skill_id }
        fn description(&self) -> &str { "mock" }
        fn should_activate(&self, _msg: &str, _has_files: bool, _current: &str) -> bool {
            self.activate
        }
        fn priority(&self) -> u32 { self.prio }
        fn system_prompt(&self, _state: &SkillState) -> String { String::new() }
        fn tool_filter(&self, _state: &SkillState) -> ToolFilter { ToolFilter::All }
        fn max_iterations(&self, _state: &SkillState) -> usize { 10 }
    }

    #[tokio::test]
    async fn register_and_get() {
        let reg = SkillRegistry::new("default");
        reg.register(Arc::new(MockSkill::new("alpha", false, 0))).await;
        let skill = reg.get("alpha").await;
        assert!(skill.is_some());
        assert_eq!(skill.unwrap().id(), "alpha");
    }

    #[tokio::test]
    async fn get_returns_none_for_unknown() {
        let reg = SkillRegistry::new("default");
        assert!(reg.get("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn list_skills_returns_all() {
        let reg = SkillRegistry::new("default");
        reg.register(Arc::new(MockSkill::new("a", false, 0))).await;
        reg.register(Arc::new(MockSkill::new("b", false, 0))).await;
        reg.register(Arc::new(MockSkill::new("c", false, 0))).await;
        let list = reg.list_skills().await;
        assert_eq!(list.len(), 3);
        let mut ids: Vec<String> = list.iter().map(|s| s.id.clone()).collect();
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[tokio::test]
    async fn detect_activation_returns_highest_priority() {
        let reg = SkillRegistry::new("default");
        reg.register(Arc::new(MockSkill::new("default", false, 0))).await;
        reg.register(Arc::new(MockSkill::new("low", true, 1))).await;
        reg.register(Arc::new(MockSkill::new("high", true, 10))).await;
        let result = reg.detect_activation("test", false, "default").await;
        assert_eq!(result, "high");
    }

    #[tokio::test]
    async fn detect_activation_falls_back_to_default() {
        let reg = SkillRegistry::new("default");
        reg.register(Arc::new(MockSkill::new("default", false, 0))).await;
        reg.register(Arc::new(MockSkill::new("other", false, 5))).await;
        let result = reg.detect_activation("hello", false, "default").await;
        assert_eq!(result, "default");
    }

    #[tokio::test]
    async fn detect_activation_with_deep_research_keywords() {
        use crate::ai::skills::deep_research::DeepResearchSkill;
        use crate::ai::skills::default::DefaultSkill;

        let reg = SkillRegistry::new("default");
        reg.register(Arc::new(DefaultSkill)).await;
        reg.register(Arc::new(DeepResearchSkill)).await;

        for keyword in &["research", "investigate", "deep dive", "in-depth", "thoroughly", "帮我研究", "深入分析"] {
            let msg = format!("please {} this topic", keyword);
            let result = reg.detect_activation(&msg, false, "default").await;
            assert_eq!(result, "deep_research", "should activate for keyword: {}", keyword);
        }
    }

    #[tokio::test]
    async fn get_default_returns_default_skill() {
        let reg = SkillRegistry::new("default");
        reg.register(Arc::new(MockSkill::new("default", false, 0))).await;
        let skill = reg.get_default().await;
        assert!(skill.is_some());
        assert_eq!(skill.unwrap().id(), "default");
    }
}
