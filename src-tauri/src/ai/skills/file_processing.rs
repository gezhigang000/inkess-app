use crate::ai::skill::{Skill, SkillState};
use crate::ai::tool::registry::ToolFilter;

pub struct FileProcessingSkill;

impl Skill for FileProcessingSkill {
    fn id(&self) -> &str { "file_processing" }
    fn display_name(&self) -> &str { "File Processing" }
    fn description(&self) -> &str { "Batch file operations, format conversion, and data extraction" }

    fn should_activate(&self, message: &str, has_files: bool, _current: &str) -> bool {
        let lower = message.to_lowercase();
        let keywords = ["转换", "convert", "批量", "batch", "transform",
            "格式转换", "file format", "extract data", "提取",
            "merge files", "合并文件", "split file", "拆分"];
        let has_keyword = keywords.iter().any(|k| lower.contains(k));
        has_keyword || (has_files && lower.contains("处理"))
    }

    fn priority(&self) -> u32 { 5 }

    fn system_prompt(&self, _state: &SkillState) -> String {
        r#"You are in File Processing mode. Focus on efficient file operations:
1. UNDERSTAND FORMAT: Read a sample of the input file first to understand its structure
2. PLAN STEPS: Break complex conversions into clear steps
3. USE PYTHON: For data transformations, use run_python with pandas/openpyxl
4. WRITE OUTPUT: Save results with write_file, then open_file to show the user
5. VERIFY: Read back a sample of the output to confirm correctness
6. REPORT: Summarize what was processed (file count, rows, any errors)"#.to_string()
    }

    fn tool_filter(&self, _state: &SkillState) -> ToolFilter {
        ToolFilter::Only(vec![
            "read_file".into(), "write_file".into(), "edit_file".into(),
            "run_python".into(), "open_file".into(),
            "list_directory".into(), "search_files".into(),
            "file_info".into(), "diff_files".into(),
        ])
    }

    fn max_iterations(&self, _state: &SkillState) -> usize { 15 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_returns_file_processing() {
        let skill = FileProcessingSkill;
        assert_eq!(skill.id(), "file_processing");
    }

    #[test]
    fn should_activate_on_file_keywords() {
        let skill = FileProcessingSkill;
        let triggers = [
            "convert this file to csv",
            "batch rename these files",
            "transform the data",
            "格式转换一下",
            "extract data from this",
            "merge files together",
            "合并文件",
            "split file into parts",
            "拆分这个文件",
            "转换格式",
            "批量处理",
            "file format change",
            "提取数据",
        ];
        for msg in &triggers {
            assert!(skill.should_activate(msg, false, "default"),
                "should activate for: {}", msg);
        }
    }

    #[test]
    fn should_activate_with_files_and_keyword() {
        let skill = FileProcessingSkill;
        // "处理" with has_files=true should activate
        assert!(skill.should_activate("处理这些", true, "default"));
        // "处理" without files should NOT activate
        assert!(!skill.should_activate("处理这些", false, "default"));
    }

    #[test]
    fn should_not_activate_on_unrelated_messages() {
        let skill = FileProcessingSkill;
        let non_triggers = [
            "hello",
            "what is rust?",
            "research this topic",
            "debug this code",
        ];
        for msg in &non_triggers {
            assert!(!skill.should_activate(msg, false, "default"),
                "should NOT activate for: {}", msg);
        }
    }

    #[test]
    fn tool_filter_returns_only_file_tools() {
        let skill = FileProcessingSkill;
        let state = SkillState::default();
        match skill.tool_filter(&state) {
            ToolFilter::Only(tools) => {
                assert!(tools.contains(&"read_file".to_string()));
                assert!(tools.contains(&"write_file".to_string()));
                assert!(tools.contains(&"run_python".to_string()));
                assert!(tools.contains(&"open_file".to_string()));
                assert!(tools.contains(&"edit_file".to_string()));
                assert!(!tools.contains(&"web_search".to_string()));
            }
            _ => panic!("FileProcessingSkill tool_filter should be ToolFilter::Only"),
        }
    }

    #[test]
    fn max_iterations_returns_15() {
        let skill = FileProcessingSkill;
        let state = SkillState::default();
        assert_eq!(skill.max_iterations(&state), 15);
    }

    #[test]
    fn priority_is_5() {
        let skill = FileProcessingSkill;
        assert_eq!(skill.priority(), 5);
    }

    #[test]
    fn system_prompt_contains_file_instructions() {
        let skill = FileProcessingSkill;
        let state = SkillState::default();
        let prompt = skill.system_prompt(&state);
        assert!(prompt.contains("File Processing"), "prompt should mention File Processing");
        assert!(prompt.contains("run_python"), "prompt should mention run_python");
    }
}
