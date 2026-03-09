use crate::ai::skill::{Skill, SkillState};
use crate::ai::tool::registry::ToolFilter;

pub struct CodeAnalysisSkill;

impl Skill for CodeAnalysisSkill {
    fn id(&self) -> &str { "code_analysis" }
    fn display_name(&self) -> &str { "Code Analysis" }
    fn description(&self) -> &str { "Code understanding, analysis, and modification" }

    fn should_activate(&self, message: &str, _has_files: bool, _current: &str) -> bool {
        let lower = message.to_lowercase();
        let keywords = ["这段代码", "分析代码", "代码分析", "analyze code", "code review",
            "debug", "调试", "重构", "refactor", "explain code", "解释代码",
            "find bug", "找bug", "fix bug", "修复"];
        keywords.iter().any(|k| lower.contains(k))
    }

    fn priority(&self) -> u32 { 5 }

    fn system_prompt(&self, _state: &SkillState) -> String {
        r#"You are in Code Analysis mode. Follow this approach:
1. SEARCH FIRST: Use search_knowledge and grep_files to find relevant code
2. READ CONTEXT: Read related files to understand the full picture
3. ANALYZE: Look for patterns, bugs, performance issues, and security concerns
4. SUGGEST: Provide specific, actionable improvements with code examples
5. MODIFY: Use edit_file for precise changes, or write_file for new files
6. VERIFY: Use run_shell or run_python to run tests after changes"#.to_string()
    }

    fn tool_filter(&self, _state: &SkillState) -> ToolFilter {
        ToolFilter::All
    }

    fn max_iterations(&self, _state: &SkillState) -> usize { 20 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_returns_code_analysis() {
        let skill = CodeAnalysisSkill;
        assert_eq!(skill.id(), "code_analysis");
    }

    #[test]
    fn should_activate_on_code_keywords() {
        let skill = CodeAnalysisSkill;
        let triggers = [
            "analyze code in this file",
            "please do a code review",
            "debug this issue",
            "refactor this function",
            "explain code here",
            "find bug in this",
            "fix bug please",
            "这段代码有问题",
            "分析代码",
            "代码分析",
            "调试一下",
            "重构这个函数",
            "解释代码",
            "找bug",
            "修复这个问题",
        ];
        for msg in &triggers {
            assert!(skill.should_activate(msg, false, "default"),
                "should activate for: {}", msg);
        }
    }

    #[test]
    fn should_not_activate_on_unrelated_messages() {
        let skill = CodeAnalysisSkill;
        let non_triggers = [
            "hello world",
            "convert file to pdf",
            "research this topic",
            "what is the weather",
            "batch rename files",
        ];
        for msg in &non_triggers {
            assert!(!skill.should_activate(msg, false, "default"),
                "should NOT activate for: {}", msg);
        }
    }

    #[test]
    fn tool_filter_is_all() {
        let skill = CodeAnalysisSkill;
        let state = SkillState::default();
        match skill.tool_filter(&state) {
            ToolFilter::All => {} // expected
            _ => panic!("CodeAnalysisSkill tool_filter should be ToolFilter::All"),
        }
    }

    #[test]
    fn max_iterations_returns_20() {
        let skill = CodeAnalysisSkill;
        let state = SkillState::default();
        assert_eq!(skill.max_iterations(&state), 20);
    }

    #[test]
    fn priority_is_5() {
        let skill = CodeAnalysisSkill;
        assert_eq!(skill.priority(), 5);
    }

    #[test]
    fn system_prompt_contains_code_instructions() {
        let skill = CodeAnalysisSkill;
        let state = SkillState::default();
        let prompt = skill.system_prompt(&state);
        assert!(prompt.contains("Code Analysis"), "prompt should mention Code Analysis");
        assert!(prompt.contains("grep_files"), "prompt should mention grep_files");
        assert!(prompt.contains("edit_file"), "prompt should mention edit_file");
    }
}
