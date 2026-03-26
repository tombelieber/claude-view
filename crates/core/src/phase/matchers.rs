// crates/core/src/phase/matchers.rs
//! Skill, agent type, and bash command pattern matchers for phase classification.

/// Check if a skill name matches planning skills.
pub fn is_plan_skill(skill: &str) -> bool {
    let sl = skill.to_lowercase();
    matches!(
        sl.as_str(),
        "plan"
            | "office-hours"
            | "brainstorm"
            | "plan-ceo-review"
            | "plan-eng-review"
            | "plan-design-review"
            | "design-consultation"
            | "autoplan"
    ) || sl.contains("brainstorming")
        || sl.contains("writing-plans")
}

/// Check if a skill name matches review skills.
pub fn is_review_skill(skill: &str) -> bool {
    let sl = skill.to_lowercase();
    sl.contains("review")
        || sl.contains("audit")
        || sl.contains("shippable")
        || sl.contains("prove-it")
}

/// Check if a skill name matches test/QA skills.
pub fn is_test_skill(skill: &str) -> bool {
    let sl = skill.to_lowercase();
    sl.contains("qa") || sl.contains("test") || sl.contains("verification")
}

/// Check if a skill name matches ship/release skills.
pub fn is_ship_skill(skill: &str) -> bool {
    let sl = skill.to_lowercase();
    sl.contains("ship")
        || sl.contains("release")
        || sl.contains("deploy")
        || sl.contains("land-and")
}

/// Check if a skill name matches debug skills.
pub fn is_debug_skill(skill: &str) -> bool {
    let sl = skill.to_lowercase();
    sl.contains("debug") || sl.contains("investigat")
}

/// Check if a skill name matches config skills.
pub fn is_config_skill(skill: &str) -> bool {
    let sl = skill.to_lowercase();
    sl.contains("config") || sl.contains("setup")
}

/// Check if a skill name matches implementation skills.
pub fn is_impl_skill(skill: &str) -> bool {
    let sl = skill.to_lowercase();
    sl.contains("executing") || sl.contains("feature-dev") || sl.contains("subagent-driven")
}

/// Check if a skill name matches explore/status skills.
pub fn is_explore_skill(skill: &str) -> bool {
    let sl = skill.to_lowercase();
    sl.contains("landscape") || sl.contains("wtf") || sl.contains("pm-status")
}

/// Classify a file path into a semantic category for phase signals.
pub fn classify_file(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    let ext = name.rsplit('.').next().unwrap_or("");

    // CI/CD files (check before config — .github/ YAML is CI, not generic config)
    if lower.contains(".github/") || name.starts_with("dockerfile") || name.starts_with("docker-compose") {
        return "ci";
    }
    // Config files
    if matches!(
        ext,
        "toml" | "yml" | "yaml" | "json" | "lock" | "config" | "env" | "ini" | "cfg"
    ) || matches!(
        name.as_str(),
        "cargo.toml" | "package.json" | "tsconfig.json" | "biome.json" | ".gitignore"
    ) {
        return "config";
    }
    // Test files
    if name.contains(".test.") || name.contains(".spec.") || name.starts_with("test_") || lower.contains("/tests/") {
        return "test";
    }
    // Plan/design files (check before doc — plan.md is a plan, not generic doc)
    if name.contains("plan") || name.contains("design") || name.contains("spec") || name.contains("rfc") || name.contains("proposal") {
        return "plan";
    }
    // Doc files
    if matches!(ext, "md" | "mdx" | "txt" | "rst") {
        return "doc";
    }
    // Script files
    if matches!(ext, "sh" | "bash") {
        return "script";
    }
    // Migration files
    if name.contains("migration") || name.contains("migrate") {
        return "migration";
    }
    "source"
}

/// Check if a bash command contains test patterns.
pub fn bash_has_test_cmd(cmd: &str) -> bool {
    let cl = cmd.to_lowercase();
    cl.contains("cargo test")
        || cl.contains("bun test")
        || cl.contains("vitest")
        || cl.contains("jest")
        || cl.contains("pytest")
        || cl.contains("npm test")
        || cl.contains("go test")
}

/// Check if a bash command contains git push / PR creation patterns.
pub fn bash_has_git_push(cmd: &str) -> bool {
    let cl = cmd.to_lowercase();
    cl.contains("git push") || cl.contains("gh pr create") || cl.contains("gh pr merge")
}

/// Check if a bash command contains publish patterns.
pub fn bash_has_publish(cmd: &str) -> bool {
    let cl = cmd.to_lowercase();
    cl.contains("npm publish") || cl.contains("cargo publish") || cl.contains("gh release create")
}

/// Check if a bash command contains deploy patterns.
pub fn bash_has_deploy(cmd: &str) -> bool {
    let cl = cmd.to_lowercase();
    cl.contains("fly deploy")
        || cl.contains("vercel")
        || cl.contains("netlify deploy")
        || cl.contains("wrangler deploy")
}

/// Check if a bash command contains install patterns.
pub fn bash_has_install(cmd: &str) -> bool {
    let cl = cmd.to_lowercase();
    cl.contains("cargo add")
        || cl.contains("bun add")
        || cl.contains("bun install")
        || cl.contains("npm install")
        || cl.contains("pip install")
}

/// Check if a bash command contains git commit patterns.
pub fn bash_has_git_commit(cmd: &str) -> bool {
    cmd.to_lowercase().contains("git commit")
}

/// Check if a bash command contains git inspection patterns (diff, log, status, show).
pub fn bash_has_git_diff(cmd: &str) -> bool {
    let cl = cmd.to_lowercase();
    cl.contains("git diff")
        || cl.contains("git log")
        || cl.contains("git status")
        || cl.contains("git show")
}

/// Check if a bash command contains docker patterns.
pub fn bash_has_docker(cmd: &str) -> bool {
    let cl = cmd.to_lowercase();
    cl.contains("docker build")
        || cl.contains("docker run")
        || cl.contains("docker-compose")
        || cl.contains("docker compose")
}

/// Check if a bash command contains lint/format patterns.
pub fn bash_has_lint(cmd: &str) -> bool {
    let cl = cmd.to_lowercase();
    cl.contains("biome")
        || cl.contains("eslint")
        || cl.contains("clippy")
        || cl.contains("cargo fmt")
        || cl.contains("prettier")
}

/// Check if a bash command contains build patterns.
pub fn bash_has_build(cmd: &str) -> bool {
    let cl = cmd.to_lowercase();
    cl.contains("cargo build")
        || cl.contains("cargo check")
        || cl.contains("bun build")
        || cl.contains("npm run build")
        || cl.contains("vite build")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_matchers() {
        assert!(is_plan_skill("office-hours"));
        assert!(is_plan_skill("superpowers:brainstorming"));
        assert!(is_review_skill("code-review"));
        assert!(is_review_skill("pr-review-toolkit:review-pr"));
        assert!(is_ship_skill("claude-view-release"));
        assert!(is_debug_skill("investigate"));
        assert!(is_impl_skill("superpowers:executing-plans"));
        assert!(is_test_skill("qa-only"));
    }

    #[test]
    fn test_bash_matchers() {
        assert!(bash_has_test_cmd("cargo test --release"));
        assert!(bash_has_test_cmd("bun test src/"));
        assert!(bash_has_git_push("git push origin main"));
        assert!(bash_has_git_push("gh pr create --title 'feat'"));
        assert!(bash_has_publish("npm publish"));
        assert!(bash_has_deploy("fly deploy"));
        assert!(bash_has_install("bun add react"));
        assert!(bash_has_build("cargo build --release"));
    }
}
