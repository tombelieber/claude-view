# Custom Skill Registry Fix — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make user-level custom skills (`~/.claude/skills/`) appear in the Top Skills analytics dashboard.

**Architecture:** Add a `scan_user_skills()` helper to `crates/core/src/registry.rs` that scans `{claude_dir}/skills/*/SKILL.md`, then call it inside `build_registry()` after plugin scanning. No signature change, no frontend change, no new dependencies.

**Tech Stack:** Rust (existing `crates/core` crate), existing `std::fs` patterns already used in the file.

---

### Task 1: Write failing test for user-level skill scanning

**Files:**
- Modify: `crates/core/src/registry.rs` (test module, before the closing `}` of `mod tests` at line 975)

**Step 1: Write the failing test**

Add this test at the end of the `mod tests` block in `registry.rs`:

```rust
#[tokio::test]
async fn test_user_level_skills_registered() {
    let tmp = TempDir::new().unwrap();
    let claude_dir = tmp.path();

    // Create user-level skill: {claude_dir}/skills/prove-it/SKILL.md
    let skill_dir = claude_dir.join("skills/prove-it");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "# Prove It\nAudit proposed fixes.").unwrap();

    // Create another user-level skill
    let skill_dir2 = claude_dir.join("skills/shippable");
    fs::create_dir_all(&skill_dir2).unwrap();
    fs::write(skill_dir2.join("SKILL.md"), "# Shippable\nPost-implementation audit.").unwrap();

    let registry = build_registry(claude_dir).await;

    // User skills should be found by qualified name
    let prove_it = registry.lookup("user:prove-it");
    assert!(prove_it.is_some(), "User skill 'user:prove-it' not found");
    assert_eq!(prove_it.unwrap().kind, InvocableKind::Skill);
    assert_eq!(prove_it.unwrap().name, "prove-it");
    assert!(prove_it.unwrap().plugin_name.is_none(), "User skills should have no plugin_name");
    assert_eq!(prove_it.unwrap().description, "Audit proposed fixes.");

    // User skills should also be found by bare name
    let bare = registry.lookup("prove-it");
    assert!(bare.is_some(), "Bare name 'prove-it' not found");
    assert_eq!(bare.unwrap().id, "user:prove-it");

    // Second skill should also be present
    let shippable = registry.lookup("user:shippable");
    assert!(shippable.is_some(), "User skill 'user:shippable' not found");

    // Total: 2 user skills + builtins
    assert_eq!(registry.len(), 2 + num_builtins());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-core test_user_level_skills_registered -- --nocapture`

Expected: FAIL — the test asserts `user:prove-it` exists but `build_registry` doesn't scan `{claude_dir}/skills/` yet.

**Step 3: Commit**

```bash
git add crates/core/src/registry.rs
git commit -m "test: add failing test for user-level custom skill registry scanning"
```

---

### Task 2: Write failing test for plugin-vs-user dedup

**Files:**
- Modify: `crates/core/src/registry.rs` (test module)

**Step 1: Write the failing test**

Add this test after the previous one:

```rust
#[tokio::test]
async fn test_plugin_skills_take_precedence_over_user_skills() {
    let tmp = TempDir::new().unwrap();
    let claude_dir = tmp.path();

    // Create a plugin skill named "brainstorming"
    let install_path = claude_dir.join("plugins/cache/superpowers/1.0.0");
    fs::create_dir_all(&install_path).unwrap();
    fs::write(
        install_path.join("plugin.json"),
        r#"{"name": "superpowers", "description": "test"}"#,
    ).unwrap();
    let plugin_skill = install_path.join("brainstorming");
    fs::create_dir_all(&plugin_skill).unwrap();
    fs::write(plugin_skill.join("SKILL.md"), "# Brainstorming\nFrom plugin.").unwrap();

    let plugins_dir = claude_dir.join("plugins");
    fs::write(
        plugins_dir.join("installed_plugins.json"),
        serde_json::json!({
            "version": 2,
            "plugins": {
                "superpowers@marketplace": [{
                    "scope": "user",
                    "installPath": install_path.to_str().unwrap(),
                    "version": "1.0.0",
                    "installedAt": "2026-01-01T00:00:00Z"
                }]
            }
        }).to_string(),
    ).unwrap();

    // Also create a user-level skill with the SAME bare name "brainstorming"
    let user_skill = claude_dir.join("skills/brainstorming");
    fs::create_dir_all(&user_skill).unwrap();
    fs::write(user_skill.join("SKILL.md"), "# Brainstorming\nFrom user.").unwrap();

    let registry = build_registry(claude_dir).await;

    // Plugin skill should exist under its qualified name
    let plugin = registry.lookup("superpowers:brainstorming");
    assert!(plugin.is_some());
    assert_eq!(plugin.unwrap().description, "From plugin.");

    // User skill should exist under its qualified name (different ID)
    let user = registry.lookup("user:brainstorming");
    assert!(user.is_some());
    assert_eq!(user.unwrap().description, "From user.");

    // Bare name lookup should return plugin (registered first)
    let bare = registry.lookup("brainstorming");
    assert!(bare.is_some());
    assert_eq!(bare.unwrap().id, "superpowers:brainstorming",
        "Plugin skill should win bare-name lookup (registered first)");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-core test_plugin_skills_take_precedence -- --nocapture`

Expected: FAIL — `user:brainstorming` doesn't exist yet.

**Step 3: Commit**

```bash
git add crates/core/src/registry.rs
git commit -m "test: add failing test for plugin-vs-user skill dedup precedence"
```

---

### Task 3: Write failing test for classify_tool_use with user skill

**Files:**
- Modify: `crates/core/src/invocation.rs` (test module, before the closing `}` of `mod tests` at line 707)

**Step 1: Write the failing test**

Add this test to verify the full classify pipeline works with user skills:

```rust
#[test]
fn test_skill_with_user_level_lookup() {
    // Build a registry that includes a user-level skill
    let registry = tokio_test::block_on(async {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path();

        // Create user-level skill
        let skill_dir = claude_dir.join("skills/prove-it");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# Prove It\nTest.").unwrap();

        crate::registry::build_registry(claude_dir).await
    });

    // Classify a Skill tool_use with bare name "prove-it"
    let input = Some(serde_json::json!({"skill": "prove-it"}));
    let result = classify_tool_use("Skill", &input, &registry);
    assert_eq!(
        result,
        ClassifyResult::Valid {
            invocable_id: "user:prove-it".into(),
            kind: InvocableKind::Skill,
        }
    );

    // Also works with qualified name
    let input2 = Some(serde_json::json!({"skill": "user:prove-it"}));
    let result2 = classify_tool_use("Skill", &input2, &registry);
    assert_eq!(
        result2,
        ClassifyResult::Valid {
            invocable_id: "user:prove-it".into(),
            kind: InvocableKind::Skill,
        }
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-core test_skill_with_user_level_lookup -- --nocapture`

Expected: FAIL — `lookup("prove-it")` returns `None`, so result is `Rejected`.

**Step 3: Commit**

```bash
git add crates/core/src/invocation.rs
git commit -m "test: add failing test for classify_tool_use with user-level skills"
```

---

### Task 4: Implement scan_user_skills and wire into build_registry

**Files:**
- Modify: `crates/core/src/registry.rs` (add helper function + call it in `build_registry`)

**Step 1: Add the `scan_user_skills` helper function**

Add this function in the "Internal helpers" section (after `read_first_line_description` around line 464, before `build_maps`):

```rust
/// Scan user-level custom skills at `{claude_dir}/skills/*/SKILL.md`.
/// These are skills created by the user directly, not installed via plugins.
fn scan_user_skills(claude_dir: &Path) -> Vec<InvocableInfo> {
    let skills_dir = claude_dir.join("skills");
    let entries = match std::fs::read_dir(&skills_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(), // dir doesn't exist, that's fine
    };

    let mut results = Vec::new();
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            let skill_md = entry_path.join("SKILL.md");
            if skill_md.exists() {
                let skill_name = entry_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let description = read_first_line_description(&skill_md);
                results.push(InvocableInfo {
                    id: format!("user:{skill_name}"),
                    plugin_name: None,
                    name: skill_name,
                    kind: InvocableKind::Skill,
                    description,
                });
            }
        }
    }

    results
}
```

**Step 2: Wire `scan_user_skills` into `build_registry()`**

In `build_registry()`, after line 263 (the `if let Some(installed)` closing brace, which is the outermost scope of the plugin scan block) and before the `// 3. Register built-in tools` comment at line 265, add:

```rust
    // 2a. Scan user-level custom skills: {claude_dir}/skills/*/SKILL.md
    let user_skills = scan_user_skills(claude_dir);
    for s in user_skills {
        if !global_seen_ids.insert(s.id.clone()) {
            debug!("Skipping duplicate user skill: {}", s.id);
            continue;
        }
        entries.push(s);
    }
```

**Step 3: Run all three failing tests**

Run: `cargo test -p claude-view-core test_user_level_skills_registered test_plugin_skills_take_precedence test_skill_with_user_level_lookup -- --nocapture`

Expected: All 3 PASS.

**Step 4: Run full core test suite to check for regressions**

Run: `cargo test -p claude-view-core`

Expected: All tests pass. No existing tests should break — `build_registry` signature is unchanged.

**Step 5: Commit**

```bash
git add crates/core/src/registry.rs
git commit -m "feat: scan user-level custom skills in registry builder

Skills at ~/.claude/skills/*/SKILL.md are now registered as
'user:{name}' invocables, enabling them to appear in Top Skills
analytics. Plugin skills take precedence in bare-name lookup."
```

---

### Task 5: Write failing test for missing skills dir (graceful handling)

**Files:**
- Modify: `crates/core/src/registry.rs` (test module)

**Step 1: Write the test**

This test should already pass (the `scan_user_skills` function returns empty vec when dir doesn't exist), but it's good to have an explicit regression test:

```rust
#[tokio::test]
async fn test_no_user_skills_dir_no_crash() {
    let tmp = TempDir::new().unwrap();
    let claude_dir = tmp.path();
    // Don't create {claude_dir}/skills/ at all

    let registry = build_registry(claude_dir).await;

    // Should still work, just no user skills
    assert_eq!(registry.len(), num_builtins());
    assert!(registry.lookup("user:anything").is_none());
}
```

**Step 2: Run test to verify it passes**

Run: `cargo test -p claude-view-core test_no_user_skills_dir_no_crash -- --nocapture`

Expected: PASS (already handled by the `Err(_) => return Vec::new()` branch).

**Step 3: Commit**

```bash
git add crates/core/src/registry.rs
git commit -m "test: add regression test for missing user skills directory"
```

---

### Task 6: Run full test suite across affected crates

**Files:** None (verification only)

**Step 1: Run core crate tests**

Run: `cargo test -p claude-view-core`

Expected: All pass.

**Step 2: Run db crate tests (uses build_registry in acceptance tests)**

Run: `cargo test -p claude-view-db`

Expected: All pass. `build_registry` signature unchanged, so no caller breakage.

**Step 3: Run server crate tests**

Run: `cargo test -p claude-view-server`

Expected: All pass.

**Step 4: Commit (no changes — verification only)**

No commit needed. If any test fails, debug and fix before proceeding.

---

### Task 7: Manual verification with real data

**Files:** None (verification only)

**Step 1: Build and run the server**

Run: `cargo run -p claude-view-server -- --reindex`

The `--reindex` flag forces a full re-index so existing sessions get classified against the updated registry.

**Step 2: Check server logs for user skill registration**

Look for log lines like:
```
DEBUG Registry built: N qualified entries, M bare names
```

The count should be higher than before (by the number of skills in `~/.claude/skills/`).

**Step 3: Check the dashboard**

Open `http://localhost:47892` and verify the Top Skills card now shows skills like `prove-it`, `auditing-plans`, `shippable`.

**Step 4: Final commit (if any log/debug tweaks needed)**

```bash
git add -A
git commit -m "chore: verification complete for user-level skill tracking"
```
