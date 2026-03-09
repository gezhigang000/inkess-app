use crate::ai::skill::{Skill, SkillState};
use crate::ai::tool::registry::ToolFilter;

pub struct DeepResearchSkill;

impl Skill for DeepResearchSkill {
    fn id(&self) -> &str { "deep_research" }
    fn display_name(&self) -> &str { "Deep Research" }
    fn description(&self) -> &str { "In-depth research and analysis with multi-source verification" }

    fn should_activate(&self, message: &str, _has_files: bool, _current: &str) -> bool {
        let lower = message.to_lowercase();
        let keywords = ["帮我研究", "深入分析", "调研", "research", "investigate",
            "深度分析", "详细分析", "全面分析", "comprehensive analysis",
            "deep dive", "in-depth", "thoroughly"];
        keywords.iter().any(|k| lower.contains(k))
    }

    fn priority(&self) -> u32 { 10 }

    fn system_prompt(&self, _state: &SkillState) -> String {
        r#"You are in Deep Research mode. Follow this methodology:
1. SEARCH FIRST: Always start with web_search to understand the landscape and find authoritative sources
2. MULTI-SOURCE: Cross-reference at least 2-3 sources before drawing conclusions
3. READ DEEPLY: Use fetch_url to read full articles, not just search snippets
4. LOCAL CONTEXT: Use search_knowledge and read_file to connect findings with local project context
5. STRUCTURED OUTPUT: Present findings in a clear report format with sections, evidence, and conclusions
6. CITE SOURCES: Always reference where information came from"#.to_string()
    }

    fn tool_filter(&self, _state: &SkillState) -> ToolFilter {
        ToolFilter::All
    }

    fn max_iterations(&self, _state: &SkillState) -> usize { 30 }
    fn token_budget(&self, _state: &SkillState) -> u32 { 8192 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_returns_deep_research() {
        let skill = DeepResearchSkill;
        assert_eq!(skill.id(), "deep_research");
    }

    #[test]
    fn should_activate_on_research_keywords() {
        let skill = DeepResearchSkill;
        let triggers = [
            "帮我研究 this topic",
            "please research this",
            "I need to investigate the issue",
            "let's do a deep dive into this",
            "please analyze this in-depth",
            "thoroughly review this code",
            "深入分析这个问题",
            "详细分析一下",
            "全面分析",
            "comprehensive analysis needed",
            "帮我调研一下",
            "深度分析这段代码",
        ];
        for msg in &triggers {
            assert!(skill.should_activate(msg, false, "default"),
                "should activate for: {}", msg);
        }
    }

    #[test]
    fn should_not_activate_on_simple_messages() {
        let skill = DeepResearchSkill;
        let non_triggers = [
            "hello",
            "what is 2+2?",
            "open the file",
            "convert this to pdf",
            "write some code",
            "fix this bug",
        ];
        for msg in &non_triggers {
            assert!(!skill.should_activate(msg, false, "default"),
                "should NOT activate for: {}", msg);
        }
    }

    #[test]
    fn priority_is_higher_than_default() {
        let skill = DeepResearchSkill;
        assert_eq!(skill.priority(), 10);
        assert!(skill.priority() > 0, "priority should be higher than default (0)");
    }

    #[test]
    fn tool_filter_is_all() {
        let skill = DeepResearchSkill;
        let state = SkillState::default();
        match skill.tool_filter(&state) {
            ToolFilter::All => {} // expected
            _ => panic!("DeepResearchSkill tool_filter should be ToolFilter::All"),
        }
    }

    #[test]
    fn system_prompt_contains_research_instructions() {
        let skill = DeepResearchSkill;
        let state = SkillState::default();
        let prompt = skill.system_prompt(&state);
        assert!(prompt.contains("Deep Research"), "prompt should mention Deep Research");
        assert!(prompt.contains("web_search"), "prompt should mention web_search");
        assert!(prompt.contains("MULTI-SOURCE"), "prompt should mention multi-source");
        assert!(prompt.contains("fetch_url"), "prompt should mention fetch_url");
    }

    #[test]
    fn max_iterations_returns_30() {
        let skill = DeepResearchSkill;
        let state = SkillState::default();
        assert_eq!(skill.max_iterations(&state), 30);
    }

    #[test]
    fn token_budget_returns_8192() {
        let skill = DeepResearchSkill;
        let state = SkillState::default();
        assert_eq!(skill.token_budget(&state), 8192);
    }
}
