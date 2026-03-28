//! Bridge from `LiveLine` → `StepSignals` for phase classification.
//!
//! Extracts the signal vector that the phase classifier needs from
//! the parsed JSONL line data already available in the accumulator.

use crate::live_parser::{LineType, LiveLine};
use crate::phase::matchers::*;
use crate::phase::StepSignals;

/// Extract a `StepSignals` from a parsed JSONL line.
///
/// Returns `Some(signals)` for assistant lines (tool-use steps) and
/// user lines (prompt keyword steps). Returns `None` for system, progress,
/// or other line types that don't contribute to phase classification.
pub fn extract_step_signals(line: &LiveLine) -> Option<StepSignals> {
    match line.line_type {
        LineType::Assistant => Some(extract_assistant_signals(line)),
        LineType::User if !line.is_meta && !line.is_tool_result_continuation => {
            Some(extract_user_signals(line))
        }
        _ => None,
    }
}

/// Extract signals from an assistant line (tool-use step).
fn extract_assistant_signals(line: &LiveLine) -> StepSignals {
    let mut s = StepSignals::default();

    // Tool counts from tool_names
    for name in &line.tool_names {
        match name.as_str() {
            "Edit" => s.edit_count += 1,
            "Write" => s.write_count += 1,
            "Read" => s.read_count += 1,
            "Glob" | "LS" => s.glob_count += 1,
            "Grep" => s.grep_count += 1,
            "Bash" => s.bash_count += 1,
            "Agent" | "Task" => s.agent_count += 1,
            "Skill" => s.skill_count += 1,
            "TodoWrite" => s.todo_count += 1,
            _ => {}
        }
    }

    // Skill classification
    for skill in &line.skill_names {
        if is_plan_skill(skill) {
            s.has_plan_skill = true;
        }
        if is_review_skill(skill) {
            s.has_review_skill = true;
        }
        if is_test_skill(skill) {
            s.has_test_skill = true;
        }
        if is_ship_skill(skill) {
            s.has_ship_skill = true;
        }
        if is_debug_skill(skill) {
            s.has_debug_skill = true;
        }
        if is_config_skill(skill) {
            s.has_config_skill = true;
        }
        if is_impl_skill(skill) {
            s.has_impl_skill = true;
        }
        if is_explore_skill(skill) {
            s.has_explore_skill = true;
        }
    }

    // Agent type classification from sub_agent_spawns
    for spawn in &line.sub_agent_spawns {
        let at = spawn.agent_type.to_lowercase();
        if at == "plan" {
            s.has_plan_agent = true;
        } else if at.contains("review") || at.contains("analyzer") || at.contains("silent-failure")
        {
            s.has_review_agent = true;
        } else if at.contains("explore") || at.contains("architect") {
            s.has_explore_agent = true;
        }
    }

    // Bash command classification
    for cmd in &line.bash_commands {
        if bash_has_test_cmd(cmd) {
            s.has_test_cmd = true;
        }
        if bash_has_build(cmd) {
            s.has_build_cmd = true;
        }
        if bash_has_git_push(cmd) {
            s.has_git_push = true;
        }
        if bash_has_publish(cmd) {
            s.has_publish_cmd = true;
        }
        if bash_has_deploy(cmd) {
            s.has_deploy_cmd = true;
        }
        if bash_has_install(cmd) {
            s.has_install_cmd = true;
        }
        if bash_has_git_commit(cmd) {
            s.has_git_commit = true;
        }
        if bash_has_git_diff(cmd) {
            s.has_git_diff = true;
        }
        if bash_has_docker(cmd) {
            s.has_docker_cmd = true;
        }
        if bash_has_lint(cmd) {
            s.has_lint_cmd = true;
        }
    }

    // Detect skill combos
    detect_skill_combos(&line.skill_names, &mut s);

    // Assistant text keyword signals (from content_preview)
    if !line.content_preview.is_empty() {
        let text = line.content_preview.to_lowercase();
        s.assistant_plan_kw = has_plan_keyword(&text);
        s.assistant_impl_kw = has_impl_keyword(&text);
        s.assistant_fix_kw = has_fix_keyword(&text);
        s.assistant_review_kw = has_review_keyword(&text);
        s.assistant_test_kw = has_test_keyword(&text);
        s.assistant_explore_kw = has_explore_keyword(&text);
    }

    // Edited file classification
    for fp in &line.edited_files {
        match classify_file(fp) {
            "config" => s.config_files_edited += 1,
            "test" => s.test_files_edited += 1,
            "doc" => s.doc_files_edited += 1,
            "plan" => s.plan_files_edited += 1,
            "script" => s.script_files_edited += 1,
            "ci" => s.ci_files_edited += 1,
            "migration" => s.migration_files_edited += 1,
            _ => {}
        }
    }

    s
}

/// Extract signals from a user prompt line (keyword matching).
fn extract_user_signals(line: &LiveLine) -> StepSignals {
    let mut s = StepSignals::default();
    s.is_user_prompt = true;

    let text = line.content_preview.to_lowercase();
    s.prompt_plan_kw = has_plan_keyword(&text);
    s.prompt_impl_kw = has_impl_keyword(&text);
    s.prompt_fix_kw = has_fix_keyword(&text);
    s.prompt_review_kw = has_review_keyword(&text);
    s.prompt_test_kw = has_test_keyword(&text);
    s.prompt_release_kw = has_release_keyword(&text);
    s.prompt_config_kw = has_config_keyword(&text);
    s.prompt_explore_kw = has_explore_keyword(&text);

    s
}

/// Detect meaningful skill combinations.
fn detect_skill_combos(skills: &[String], s: &mut StepSignals) {
    let has_shippable = skills.iter().any(|sk| sk.contains("shippable"));
    let has_audit = skills
        .iter()
        .any(|sk| sk.contains("audit") || sk.contains("prove-it"));
    if has_shippable && has_audit {
        s.has_review_combo = true;
    }

    let has_executing = skills.iter().any(|sk| sk.contains("executing"));
    let has_explorer = skills.iter().any(|sk| sk.contains("explorer"));
    if has_executing && has_explorer {
        s.has_plan_execute_combo = true;
    }

    let has_tdd = skills.iter().any(|sk| sk.contains("test-driven"));
    let has_impl = skills.iter().any(|sk| is_impl_skill(sk));
    if has_tdd && has_impl {
        s.has_tdd_combo = true;
    }
}

// Prompt keyword matchers (simple substring checks on lowercased text)

fn has_plan_keyword(text: &str) -> bool {
    text.contains("plan")
        || text.contains("design")
        || text.contains("brainstorm")
        || text.contains("scope")
        || text.contains("architect")
        || text.contains("strategy")
        || text.contains("rethink")
}

fn has_impl_keyword(text: &str) -> bool {
    text.contains("implement")
        || text.contains("build")
        || text.contains("create")
        || text.contains("write")
        || text.contains("scaffold")
        || text.contains("wire up")
}

fn has_fix_keyword(text: &str) -> bool {
    text.contains("fix")
        || text.contains("bug")
        || text.contains("broken")
        || text.contains("error")
        || text.contains("crash")
        || text.contains("wrong")
        || text.contains("fails")
        || text.contains("regression")
}

fn has_review_keyword(text: &str) -> bool {
    text.contains("review") || text.contains("audit") || text.contains("diff")
}

fn has_test_keyword(text: &str) -> bool {
    text.contains("test")
        || text.contains("verify")
        || text.contains("validate")
        || text.contains("assert")
        || text.contains("benchmark")
}

fn has_release_keyword(text: &str) -> bool {
    text.contains("ship")
        || text.contains("release")
        || text.contains("deploy")
        || text.contains("publish")
        || text.contains("changelog")
}

fn has_config_keyword(text: &str) -> bool {
    text.contains("setup")
        || text.contains("configure")
        || text.contains("install")
        || text.contains("pipeline")
        || text.contains("docker")
}

fn has_explore_keyword(text: &str) -> bool {
    text.contains("explain")
        || text.contains("how does")
        || text.contains("what is")
        || text.contains("understand")
        || text.contains("show me")
        || text.contains("explore")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_parser::{LineType, LiveLine};

    fn make_assistant_line(
        tool_names: Vec<&str>,
        bash_commands: Vec<&str>,
        edited_files: Vec<&str>,
    ) -> LiveLine {
        LiveLine {
            line_type: LineType::Assistant,
            role: Some("assistant".to_string()),
            content_preview: String::new(),
            content_extended: String::new(),
            tool_names: tool_names.into_iter().map(String::from).collect(),
            model: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cache_creation_5m_tokens: None,
            cache_creation_1hr_tokens: None,
            timestamp: None,
            stop_reason: None,
            git_branch: None,
            cwd: None,
            is_meta: false,
            is_tool_result_continuation: false,
            has_system_prefix: false,
            sub_agent_spawns: Vec::new(),
            sub_agent_result: None,
            sub_agent_progress: None,
            sub_agent_notification: None,
            todo_write: None,
            task_creates: Vec::new(),
            task_updates: Vec::new(),
            task_id_assignments: Vec::new(),
            skill_names: Vec::new(),
            bash_commands: bash_commands.into_iter().map(String::from).collect(),
            edited_files: edited_files.into_iter().map(String::from).collect(),
            is_compact_boundary: false,
            ide_file: None,
            message_id: None,
            request_id: None,
            hook_progress: None,
            slug: None,
            team_name: None,
            at_files: Vec::new(),
            pasted_paths: Vec::new(),
        }
    }

    #[test]
    fn test_tool_counts() {
        let line = make_assistant_line(
            vec!["Edit", "Edit", "Write", "Bash", "Read", "Grep"],
            vec!["cargo test"],
            vec!["/src/main.rs", "/src/lib.rs", "/tests/foo.rs"],
        );
        let s = extract_step_signals(&line).unwrap();
        assert_eq!(s.edit_count, 2);
        assert_eq!(s.write_count, 1);
        assert_eq!(s.bash_count, 1);
        assert_eq!(s.read_count, 1);
        assert_eq!(s.grep_count, 1);
        assert!(s.has_test_cmd);
        assert_eq!(s.test_files_edited, 1); // tests/foo.rs
    }

    #[test]
    fn test_user_prompt_signals() {
        let mut line = make_assistant_line(vec![], vec![], vec![]);
        line.line_type = LineType::User;
        line.content_preview = "please review the PR and run tests".to_string();
        line.is_meta = false;
        line.is_tool_result_continuation = false;

        let s = extract_step_signals(&line).unwrap();
        assert!(s.is_user_prompt);
        assert!(s.prompt_review_kw);
        assert!(s.prompt_test_kw);
    }

    #[test]
    fn test_skips_system_lines() {
        let mut line = make_assistant_line(vec![], vec![], vec![]);
        line.line_type = LineType::System;
        assert!(extract_step_signals(&line).is_none());
    }

    #[test]
    fn test_skips_tool_result_continuation() {
        let mut line = make_assistant_line(vec![], vec![], vec![]);
        line.line_type = LineType::User;
        line.is_tool_result_continuation = true;
        assert!(extract_step_signals(&line).is_none());
    }

    #[test]
    fn test_file_classification() {
        let line = make_assistant_line(
            vec!["Edit", "Edit", "Write", "Edit", "Write"],
            vec![],
            vec![
                "/src/app.rs",               // source
                "/tests/integration.rs",     // test
                "/.github/workflows/ci.yml", // ci
                "/docs/plan.md",             // plan (not doc!)
                "/README.md",                // doc
            ],
        );
        let s = extract_step_signals(&line).unwrap();
        assert_eq!(s.test_files_edited, 1);
        assert_eq!(s.ci_files_edited, 1);
        assert_eq!(s.plan_files_edited, 1);
        assert_eq!(s.doc_files_edited, 1);
    }
}
