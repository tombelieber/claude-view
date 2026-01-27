---
status: pending
date: 2026-01-27
---

# Rust Backend Parity Fix Plan

## Overview

This document audits all gaps between the old Node.js/Express/TypeScript backend and the current Rust/Axum backend, and provides a TDD-based fix plan with clear success criteria.

## Audit Summary

| Issue | Severity | Category | Status |
|-------|----------|----------|--------|
| 1. `modifiedAt` date format | 🔴 Critical | Data Format | Frontend broken |
| 2. `activeCount` calculation | 🟠 High | Business Logic | Wrong value |
| 3. Path resolution algorithm | 🟠 High | Business Logic | Missing features |
| 4. Skills extraction | 🔴 Critical | Business Logic | Completely broken |
| 5. Skills format (missing `/`) | 🟡 Medium | Data Format | Inconsistent |
| 6. `filesTouched` truncation | 🟡 Medium | Business Logic | Missing limit |

---

## Issue 1: `modifiedAt` Date Format

### Problem
- **Node**: Returns ISO string `"2026-01-27T02:50:32.000Z"` (via `Date.toJSON()`)
- **Rust**: Returns Unix seconds `1769482232`
- **Frontend**: Uses `new Date(modifiedAt)` which expects milliseconds for numbers

### Impact
- "Since when" date shows 1970
- Activity heatmap broken
- Last activity timestamp wrong

### Test Cases (TDD)

```rust
// crates/core/src/types.rs

#[test]
fn test_session_info_modified_at_serializes_as_iso_string() {
    let session = SessionInfo {
        modified_at: 1769482232, // internal storage as Unix seconds
        // ... other fields
    };
    let json = serde_json::to_string(&session).unwrap();

    // Should serialize as ISO string, not number
    assert!(json.contains("\"modifiedAt\":\"2026-"));
    assert!(!json.contains("\"modifiedAt\":1769"));
}

#[test]
fn test_session_info_iso_format_with_timezone() {
    let session = SessionInfo { modified_at: 1769482232, /* ... */ };
    let json = serde_json::to_string(&session).unwrap();

    // Should end with Z for UTC
    assert!(json.contains("T") && json.contains("Z"));
}
```

### Success Criteria
- [ ] `modifiedAt` serializes as ISO 8601 string with `Z` suffix
- [ ] Frontend `new Date(modifiedAt)` works correctly
- [ ] "Since when" shows correct year (2026, not 1970)
- [ ] Activity heatmap displays correct dates

### Implementation Notes
- Use custom serde serializer for `modified_at` field
- Use `chrono` crate for ISO 8601 formatting
- Internal storage remains `i64` Unix seconds for efficient comparisons

---

## Issue 2: `activeCount` Calculation

### Problem
- **Node**: Count of sessions modified in last 5 minutes
  ```typescript
  const fiveMinutesAgo = Date.now() - 5 * 60 * 1000
  const activeCount = sessions.filter(s => s.modifiedAt.getTime() > fiveMinutesAgo).length
  ```
- **Rust**: Returns `sessions.len()` (total count, not active)

### Impact
- Green "active" indicator always shows total sessions
- UI misleading about what's currently active

### Test Cases (TDD)

```rust
// crates/core/src/discovery.rs

#[tokio::test]
async fn test_active_count_only_includes_recent_sessions() {
    // Setup: Create temp dir with 3 sessions
    // - 1 modified 1 minute ago (active)
    // - 1 modified 10 minutes ago (not active)
    // - 1 modified 1 hour ago (not active)

    let project = create_test_project_with_sessions(vec![
        SessionAge::MinutesAgo(1),
        SessionAge::MinutesAgo(10),
        SessionAge::MinutesAgo(60),
    ]);

    assert_eq!(project.active_count, 1);
    assert_eq!(project.sessions.len(), 3);
}

#[tokio::test]
async fn test_active_count_zero_when_no_recent_sessions() {
    let project = create_test_project_with_sessions(vec![
        SessionAge::MinutesAgo(10),
        SessionAge::MinutesAgo(30),
    ]);

    assert_eq!(project.active_count, 0);
}

#[tokio::test]
async fn test_active_count_uses_5_minute_window() {
    let project = create_test_project_with_sessions(vec![
        SessionAge::MinutesAgo(4),  // Just under 5 min (active)
        SessionAge::MinutesAgo(6),  // Just over 5 min (not active)
    ]);

    assert_eq!(project.active_count, 1);
}
```

### Success Criteria
- [ ] `activeCount` only counts sessions modified within 5 minutes
- [ ] Sessions older than 5 minutes are excluded
- [ ] Edge case: exactly 5 minutes is excluded (exclusive)

---

## Issue 3: Path Resolution Algorithm

### Problem
- **Node**: Full recursive filesystem verification with:
  - `--` → `/@` conversion (scoped packages like `@vicky-ai`)
  - `.` separator support (domains like `famatch.io`)
  - Recursive segment joining with filesystem checks
- **Rust**: Simplified heuristics only:
  - Missing `--` → `/@` conversion
  - Missing `.` separator support
  - Limited variant generation (only 3-4 patterns)

### Impact
- Scoped packages like `@vicky-ai/claude-view` show wrong project name
- Domain-style directories like `famatch.io` show wrong project name

### Test Cases (TDD)

```rust
// crates/core/src/discovery.rs

#[test]
fn test_resolve_scoped_package_path() {
    // Create test directory: /tmp/test/@vicky-ai/claude-view
    let temp = create_temp_scoped_package();

    let resolved = resolve_project_path("-tmp-test--vicky-ai-claude-view");

    assert_eq!(resolved.full_path, "/tmp/test/@vicky-ai/claude-view");
    assert_eq!(resolved.display_name, "claude-view");
}

#[test]
fn test_resolve_double_dash_to_at_symbol() {
    // Verify -- decodes to @
    let variants = get_join_variants("-Users-TBGor-dev--vicky-ai-project");

    assert!(variants.iter().any(|v| v.contains("/@vicky-ai/")));
}

#[test]
fn test_resolve_domain_style_path() {
    // Create test directory: /tmp/test/famatch.io
    let temp = create_temp_domain_dir("famatch.io");

    let resolved = resolve_project_path("-tmp-test-famatch-io");

    assert_eq!(resolved.full_path, "/tmp/test/famatch.io");
    assert_eq!(resolved.display_name, "famatch.io");
}

#[test]
fn test_resolve_dot_separator_variants() {
    // Verify dot separator is tried
    let variants = get_join_variants("-Users-test-myapp-io");

    assert!(variants.iter().any(|v| v.ends_with("myapp.io")));
}

#[test]
fn test_resolve_recursive_verification() {
    // Create nested structure: /tmp/foo-bar/baz-qux/project
    let temp = create_nested_hyphenated_dirs();

    let resolved = resolve_project_path("-tmp-foo-bar-baz-qux-project");

    // Should find the correct interpretation through filesystem checks
    assert_eq!(resolved.full_path, "/tmp/foo-bar/baz-qux/project");
}
```

### Success Criteria
- [ ] `--` converts to `/@` for scoped packages
- [ ] `.` separator tried for domain-style names
- [ ] Recursive filesystem verification finds correct path
- [ ] Prefers longer existing paths (greedy matching)

---

## Issue 4: Skills Extraction

### Problem
- **Node**: Regex `/\/[\w:-]+/g` captures `/commit`, `/review`, etc.
- **Rust**: Regex `/([a-zA-Z][a-zA-Z0-9_-]*)` captures random text

### Current Behavior
```json
// Rust returns garbage:
["A0MP8lwqHwFlT8ZfpLhb", "A0NUfmKISgyGGA5DVH5iOAyHB9", ...]

// Node returned actual skills:
["/commit", "/review-pr", "/help"]
```

### Impact
- Skills panel completely broken
- Search by skill broken
- Analytics wrong

### Test Cases (TDD)

```rust
// crates/core/src/discovery.rs

#[tokio::test]
async fn test_skills_extraction_captures_slash_commands() {
    let temp_file = create_session_with_content(r#"
        {"type":"user","message":{"content":"Please /commit my changes"}}
    "#);

    let metadata = extract_session_metadata(&temp_file).await;

    assert!(metadata.skills_used.contains(&"/commit".to_string()));
}

#[tokio::test]
async fn test_skills_extraction_with_colon_separator() {
    let temp_file = create_session_with_content(r#"
        {"type":"user","message":{"content":"Run /superpowers:brainstorm please"}}
    "#);

    let metadata = extract_session_metadata(&temp_file).await;

    assert!(metadata.skills_used.contains(&"/superpowers:brainstorm".to_string()));
}

#[tokio::test]
async fn test_skills_extraction_ignores_non_skills() {
    let temp_file = create_session_with_content(r#"
        {"type":"user","message":{"content":"Check file at /Users/test/path"}}
    "#);

    let metadata = extract_session_metadata(&temp_file).await;

    // /Users is a path, not a skill
    assert!(!metadata.skills_used.iter().any(|s| s.contains("Users")));
}

#[tokio::test]
async fn test_skills_extraction_multiple_skills() {
    let temp_file = create_session_with_content(r#"
        {"type":"user","message":{"content":"/commit then /push please"}}
    "#);

    let metadata = extract_session_metadata(&temp_file).await;

    assert!(metadata.skills_used.contains(&"/commit".to_string()));
    assert!(metadata.skills_used.contains(&"/push".to_string()));
}

#[test]
fn test_skill_regex_pattern() {
    let re = Regex::new(r"(?:^|[^/\w])(/[a-zA-Z][\w:-]*)").unwrap();

    // Should match
    assert!(re.is_match("/commit"));
    assert!(re.is_match("Run /review-pr please"));
    assert!(re.is_match("/superpowers:brainstorm"));

    // Should NOT match file paths
    let caps: Vec<_> = re.captures_iter("/Users/test/file").collect();
    assert!(caps.is_empty() || !caps.iter().any(|c| c.get(1).unwrap().as_str().contains("Users")));
}
```

### Success Criteria
- [ ] Skills extracted with leading `/` (e.g., `/commit` not `commit`)
- [ ] Supports `:` separator (e.g., `/superpowers:brainstorm`)
- [ ] Supports `-` separator (e.g., `/review-pr`)
- [ ] Ignores file paths like `/Users/...`
- [ ] No garbage/random text in skills list

---

## Issue 5: Skills Format (Missing `/`)

### Problem
Even when extraction works, Node returned `/commit` but Rust returns `commit` (missing leading slash).

### Impact
- UI shows `commit` instead of `/commit`
- Search queries broken (user searches `/commit`)

### Test Cases (TDD)

```rust
#[test]
fn test_skills_include_leading_slash() {
    // Skills should start with /
    let skills = vec!["/commit", "/review", "/superpowers:test"];

    for skill in skills {
        assert!(skill.starts_with('/'), "Skill '{}' missing leading slash", skill);
    }
}
```

### Success Criteria
- [ ] All skill names start with `/`

---

## Issue 6: `filesTouched` Truncation

### Problem
- **Node**: Limits to 5 files, shows only filename
  ```typescript
  result.filesTouched = Array.from(filesSet).slice(0, 5).map(f => {
    const parts = f.split('/')
    return parts[parts.length - 1]
  })
  ```
- **Rust**: Returns full paths, no limit

### Impact
- Potentially large response payloads
- Inconsistent display (full paths vs filenames)

### Test Cases (TDD)

```rust
#[tokio::test]
async fn test_files_touched_limited_to_5() {
    let temp_file = create_session_with_many_files(10);

    let metadata = extract_session_metadata(&temp_file).await;

    assert!(metadata.files_touched.len() <= 5);
}

#[tokio::test]
async fn test_files_touched_shows_filename_only() {
    let temp_file = create_session_with_file("/Users/test/project/src/main.rs");

    let metadata = extract_session_metadata(&temp_file).await;

    assert!(metadata.files_touched.contains(&"main.rs".to_string()));
    assert!(!metadata.files_touched.iter().any(|f| f.contains('/')));
}
```

### Success Criteria
- [ ] `filesTouched` limited to maximum 5 entries
- [ ] Only filename shown (not full path)

---

## Implementation Order

### Phase 1: Critical Fixes (Blocking)
1. **Issue 1: `modifiedAt` format** - Frontend completely broken
2. **Issue 4: Skills extraction** - Panel shows garbage

### Phase 2: High Priority
3. **Issue 2: `activeCount`** - Wrong indicator value
4. **Issue 3: Path resolution** - Wrong project names

### Phase 3: Medium Priority
5. **Issue 5: Skills format** - Missing `/` prefix
6. **Issue 6: `filesTouched`** - Truncation and format

---

## TDD Implementation Checklist

For each issue:

1. [ ] Write failing test cases FIRST
2. [ ] Run tests to verify they fail
3. [ ] Implement fix
4. [ ] Run tests to verify they pass
5. [ ] Run full test suite (`cargo test --workspace`)
6. [ ] Manual verification in browser

---

## Verification Plan

### After All Fixes

1. Start Rust server: `cargo run -p vibe-recall-server`
2. Open browser: `http://localhost:47892`
3. Verify:
   - [ ] "Since when" shows 2026 (not 1970)
   - [ ] Activity heatmap shows correct dates
   - [ ] Active count shows reasonable number (not total sessions)
   - [ ] Project names correct (including scoped packages)
   - [ ] Skills panel shows `/commit`, `/review`, etc.
   - [ ] Files touched shows filenames, not paths

### API Response Comparison

```bash
# Capture Node response (from git history)
git stash
git checkout 944ea94^
bun run dev &
curl http://localhost:3000/api/projects > /tmp/node-response.json
pkill -f node

# Capture Rust response
git checkout -
cargo run -p vibe-recall-server &
curl http://localhost:47892/api/projects > /tmp/rust-response.json
pkill -f vibe-recall-server

# Compare key fields
jq '.[0].sessions[0] | {modifiedAt, activeCount, skillsUsed, filesTouched}' /tmp/node-response.json
jq '.[0].sessions[0] | {modifiedAt, activeCount, skillsUsed, filesTouched}' /tmp/rust-response.json
```

---

## Dependencies

- `chrono` - For ISO 8601 date formatting (Issue 1)
- No new dependencies for other issues

---

## Estimated Scope

| Issue | Files Changed | Test Count |
|-------|---------------|------------|
| 1. modifiedAt | types.rs | 3 tests |
| 2. activeCount | discovery.rs | 3 tests |
| 3. Path resolution | discovery.rs | 5 tests |
| 4. Skills extraction | discovery.rs | 5 tests |
| 5. Skills format | discovery.rs | 1 test |
| 6. filesTouched | discovery.rs | 2 tests |
| **Total** | 2 files | 19 tests |
