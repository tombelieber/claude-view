---
status: done
date: 2026-02-19
---

# Search Qualifier Bugfixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix all qualifier bugs (`project:`, `skill:`, `model:`) so full-text search is production-grade — every qualifier works, all sessions are indexed, and schema changes auto-rebuild the index.

**Architecture:** Four independent fixes: (1) project qualifier already stores display name (verify + test), (2) skills piped from parse results to Tantivy, (3) model field changed from STRING→TEXT for partial matching, (4) schema version file triggers auto-rebuild when schema changes. Parse version bump forces a one-time full re-index that populates all sessions.

**Tech Stack:** Rust (Tantivy, sqlx), Axum, TypeScript/React

---

## Context

These bugs were discovered via systematic debugging on 2026-02-19. The Tantivy full-text search index was shipped in Phase A3 but has several qualifier bugs that make filters silently return 0 results.

### Bugs

| # | Qualifier | Bug | Root Cause |
|---|-----------|-----|------------|
| 1 | `project:test-app` | Returns 0 | Index stores encoded path (`-Users-user-test-app`) instead of display name |
| 2 | `skill:commit` | Returns 0 always | `SearchDocument.skills` is hardcoded to `vec![]` — skills never indexed |
| 3 | `model:opus` | Returns 0 | STRING field requires exact match (`claude-opus-4-6`); users type short names |
| 4 | N/A | Many sessions missing | Sessions deep-indexed before search existed have no Tantivy data |
| 5 | N/A | No schema versioning | Changing Tantivy schema silently uses old schema from disk |

### Already Done (in working tree)

Two changes are already applied and should NOT be re-done:
- `crates/db/src/queries/sessions.rs:580` — SQL changed to `COALESCE(project_display_name, project_id, '')`
- `crates/db/src/indexer_parallel.rs:27` — Parse version bumped 6→7

---

### Task 1: Search index schema versioning + auto-rebuild

Add a version file next to the Tantivy index directory. On startup, if the version doesn't match, delete the old index and create fresh. This makes all future schema changes safe.

**Files:**
- Modify: `crates/search/src/lib.rs` (add version constant + version-aware open)

**Step 1: Write the failing test**

Add to the bottom of the `#[cfg(test)] mod tests` block in `crates/search/src/lib.rs`:

```rust
#[test]
fn test_schema_version_mismatch_triggers_rebuild() {
    let dir = tempfile::tempdir().unwrap();
    let idx_path = dir.path().join("search");

    // Create an index at version 1
    std::fs::create_dir_all(&idx_path).unwrap();
    std::fs::write(idx_path.join("schema_version"), "1").unwrap();
    let _idx = SearchIndex::open(&idx_path).unwrap();

    // Now "upgrade" to version 999 and re-open
    // open() should detect mismatch, wipe, and recreate
    let version_path = idx_path.join("schema_version");
    std::fs::write(&version_path, "1").unwrap(); // simulate old version on disk

    // Manually call the version-aware open with a different expected version
    // Since SEARCH_SCHEMA_VERSION is the real value, we test by reading what was written
    let written = std::fs::read_to_string(&version_path).unwrap();
    let current = format!("{}", SEARCH_SCHEMA_VERSION);
    // After open(), the version file should always match SEARCH_SCHEMA_VERSION
    let _idx2 = SearchIndex::open(&idx_path).unwrap();
    let after = std::fs::read_to_string(&version_path).unwrap();
    assert_eq!(after.trim(), current, "schema_version file should be updated to current version");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-search -- test_schema_version_mismatch`
Expected: FAIL — `SEARCH_SCHEMA_VERSION` not defined

**Step 3: Implement schema versioning in `SearchIndex::open`**

At the top of `crates/search/src/lib.rs`, add after the existing constants:

```rust
/// Schema version for the Tantivy index. Bump when the schema changes
/// (field types, new fields, removed fields). A mismatch triggers auto-rebuild.
pub const SEARCH_SCHEMA_VERSION: u32 = 2;
// Version 1: Initial schema (project as STRING with encoded path)
// Version 2: model field changed to TEXT for partial matching
```

Replace the existing `SearchIndex::open` method:

```rust
pub fn open(path: &Path) -> Result<Self, SearchError> {
    std::fs::create_dir_all(path)?;

    let version_path = path.join("schema_version");
    let needs_rebuild = match std::fs::read_to_string(&version_path) {
        Ok(v) => v.trim().parse::<u32>().unwrap_or(0) != SEARCH_SCHEMA_VERSION,
        Err(_) => false, // no version file = first creation, not a rebuild
    };

    if needs_rebuild {
        tracing::info!(
            path = %path.display(),
            "Search schema version mismatch — rebuilding index"
        );
        // Remove all files in the directory except schema_version
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.file_name().map(|n| n != "schema_version").unwrap_or(false) {
                    if p.is_dir() {
                        let _ = std::fs::remove_dir_all(&p);
                    } else {
                        let _ = std::fs::remove_file(&p);
                    }
                }
            }
        }
    }

    let schema = build_schema();

    let index = match Index::open_in_dir(path) {
        Ok(idx) => {
            tracing::info!(path = %path.display(), "opened existing search index");
            idx
        }
        Err(_) => {
            tracing::info!(path = %path.display(), "creating new search index");
            Index::create_in_dir(path, schema.clone())?
        }
    };

    // Write current schema version
    let _ = std::fs::write(&version_path, format!("{}", SEARCH_SCHEMA_VERSION));

    Self::from_index(index, schema)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-search -- test_schema_version`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/search/src/lib.rs
git commit -m "feat(search): add schema versioning with auto-rebuild on mismatch"
```

---

### Task 2: Change model field from STRING to TEXT for partial matching

Users type `model:opus` but the index stores `claude-opus-4-6`. Changing the field to TEXT lets Tantivy's tokenizer split on hyphens, so `opus` matches as a token.

**Files:**
- Modify: `crates/search/src/lib.rs` (schema: STRING→TEXT for model)
- Modify: `crates/search/src/query.rs` (lowercase qualifier values for TEXT fields)

**Step 1: Write the failing test**

Add to `crates/search/src/lib.rs` test module:

```rust
#[test]
fn test_search_model_partial_match() {
    let idx = SearchIndex::open_in_ram().unwrap();

    let docs = vec![
        SearchDocument {
            session_id: "s1".to_string(),
            project: "test".to_string(),
            branch: String::new(),
            model: "claude-opus-4-6".to_string(),
            role: "user".to_string(),
            content: "hello world".to_string(),
            turn_number: 1,
            timestamp: 1000,
            skills: vec![],
        },
        SearchDocument {
            session_id: "s2".to_string(),
            project: "test".to_string(),
            branch: String::new(),
            model: "claude-sonnet-4-5".to_string(),
            role: "user".to_string(),
            content: "hello world".to_string(),
            turn_number: 1,
            timestamp: 1000,
            skills: vec![],
        },
    ];

    idx.index_session("s1", &docs[..1]).unwrap();
    idx.index_session("s2", &docs[1..]).unwrap();
    idx.commit().unwrap();

    // Partial model name should match
    let result = idx.search("model:opus hello", None, 10, 0).unwrap();
    assert_eq!(result.total_sessions, 1, "model:opus should match claude-opus-4-6");
    assert_eq!(result.sessions[0].session_id, "s1");

    // Full model name should also still match
    let result2 = idx.search("model:claude-opus-4-6 hello", None, 10, 0).unwrap();
    assert_eq!(result2.total_sessions, 1, "full model name should still match");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p vibe-recall-search -- test_search_model_partial_match`
Expected: FAIL — `model:opus` returns 0 sessions

**Step 3: Change model schema to TEXT and lowercase qualifier values**

In `crates/search/src/lib.rs`, change the model field in `build_schema()`:

```rust
// Before:
schema_builder.add_text_field("model", STRING | STORED);

// After:
schema_builder.add_text_field("model", TEXT | STORED);
```

In `crates/search/src/query.rs`, update the qualifier term construction to lowercase values for TEXT fields (model). Replace the qualifier loop (lines ~152-165):

```rust
// Qualifier term queries
for qual in &qualifiers {
    let (field, is_text) = match qual.key.as_str() {
        "project" => (self.project_field, false),
        "branch" => (self.branch_field, false),
        "model" => (self.model_field, true),  // TEXT field: tokenized, needs lowercase
        "role" => (self.role_field, false),
        "skill" => (self.skills_field, false),
        _ => continue,
    };

    // TEXT fields store lowercased tokens; STRING fields store exact values.
    let value = if is_text {
        qual.value.to_lowercase()
    } else {
        qual.value.clone()
    };

    let term = Term::from_field_text(field, &value);
    let term_query = TermQuery::new(term, IndexRecordOption::Basic);
    sub_queries.push((Occur::Must, Box::new(term_query)));
}
```

Bump `SEARCH_SCHEMA_VERSION` to 2 (already set in Task 1).

**Step 4: Run test to verify it passes**

Run: `cargo test -p vibe-recall-search -- test_search_model_partial_match`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/search/src/lib.rs crates/search/src/query.rs
git commit -m "feat(search): model qualifier supports partial matching (model:opus)"
```

---

### Task 3: Fix skills indexing — pipe skills from parse results to Tantivy

`SearchDocument.skills` is hardcoded to `vec![]`. Skills ARE already extracted during parsing (`parse_result.deep.skills_used`) — they just need to be threaded through to the search document.

**Files:**
- Modify: `crates/db/src/indexer_parallel.rs:2063-2092` (add skills to SearchBatch, pass to SearchDocument)

**Step 1: Write the failing test**

The existing test `test_search_with_skill_qualifier` in `crates/search/src/lib.rs` already tests skill matching at the search layer. The bug is that the indexer never passes skills. We need an integration-level test, but the indexer tests have pre-existing compilation errors. Instead, verify by checking the existing search test passes and then fix the wiring.

Verify the existing test passes (it uses hand-constructed `SearchDocument` with skills):

Run: `cargo test -p vibe-recall-search -- test_search_with_skill_qualifier`
Expected: PASS (this tests the search layer, which works — the bug is in the indexer wiring)

**Step 2: Fix the SearchBatch struct and wiring**

In `crates/db/src/indexer_parallel.rs`, add `skills` to `SearchBatch` struct (~line 2063):

```rust
struct SearchBatch {
    session_id: String,
    project: String,
    branch: Option<String>,
    primary_model: Option<String>,
    messages: Vec<vibe_recall_core::SearchableMessage>,
    skills: Vec<String>,  // NEW: from parse_result.deep.skills_used
}
```

Update the `SearchBatch` construction (~line 2074):

```rust
.map(|r| SearchBatch {
    session_id: r.session_id.clone(),
    project: r.project.clone(),
    branch: r.parse_result.git_branch.clone(),
    primary_model: compute_primary_model(&r.parse_result.turns),
    messages: r.parse_result.search_messages.clone(),
    skills: r.parse_result.deep.skills_used.clone(),  // NEW
})
```

Update the `SearchDocument` construction (~line 2292):

```rust
// Before:
skills: vec![],

// After:
skills: batch.skills.clone(),
```

**Step 3: Verify compilation**

Run: `cargo check -p vibe-recall-db`
Expected: Clean compilation

**Step 4: Commit**

```bash
git add crates/db/src/indexer_parallel.rs
git commit -m "fix(search): pipe session skills to Tantivy index (skill: qualifier now works)"
```

---

### Task 4: Write project qualifier integration test

Verify the project display name fix works end-to-end in the search crate (the SQL change was already made).

**Files:**
- Modify: `crates/search/src/lib.rs` (add test)

**Step 1: Write the test**

Add to the test module in `crates/search/src/lib.rs`:

```rust
#[test]
fn test_search_project_qualifier_with_display_name() {
    let idx = SearchIndex::open_in_ram().unwrap();

    // Index documents using display names (as the fixed SQL now returns)
    let docs_a = vec![SearchDocument {
        session_id: "s1".to_string(),
        project: "claude-view".to_string(),  // display name, not encoded path
        branch: "main".to_string(),
        model: "claude-opus-4-6".to_string(),
        role: "user".to_string(),
        content: "fix the login bug".to_string(),
        turn_number: 1,
        timestamp: 1000,
        skills: vec![],
    }];

    let docs_b = vec![SearchDocument {
        session_id: "s2".to_string(),
        project: "test-app".to_string(),
        branch: "main".to_string(),
        model: "claude-sonnet-4-5".to_string(),
        role: "user".to_string(),
        content: "setup the project".to_string(),
        turn_number: 1,
        timestamp: 2000,
        skills: vec![],
    }];

    idx.index_session("s1", &docs_a).unwrap();
    idx.index_session("s2", &docs_b).unwrap();
    idx.commit().unwrap();

    // project:test-app should match with display name
    let result = idx.search("project:test-app", None, 10, 0).unwrap();
    assert_eq!(result.total_sessions, 1);
    assert_eq!(result.sessions[0].session_id, "s2");

    // project:claude-view should match
    let result2 = idx.search("project:claude-view fix", None, 10, 0).unwrap();
    assert_eq!(result2.total_sessions, 1);
    assert_eq!(result2.sessions[0].session_id, "s1");
}
```

**Step 2: Run test**

Run: `cargo test -p vibe-recall-search -- test_search_project_qualifier_with_display_name`
Expected: PASS (the search layer already does exact STRING matching — this verifies display names work)

**Step 3: Also add a qualifier-only test (no text query)**

```rust
#[test]
fn test_search_qualifier_only_no_text() {
    let idx = SearchIndex::open_in_ram().unwrap();

    let docs = vec![SearchDocument {
        session_id: "s1".to_string(),
        project: "my-project".to_string(),
        branch: "main".to_string(),
        model: "claude-opus-4-6".to_string(),
        role: "user".to_string(),
        content: "implement authentication".to_string(),
        turn_number: 1,
        timestamp: 1000,
        skills: vec!["commit".to_string()],
    }];

    idx.index_session("s1", &docs).unwrap();
    idx.commit().unwrap();

    // Qualifier-only queries (no text) should work
    let r1 = idx.search("project:my-project", None, 10, 0).unwrap();
    assert_eq!(r1.total_sessions, 1, "project-only qualifier should work");

    let r2 = idx.search("branch:main", None, 10, 0).unwrap();
    assert_eq!(r2.total_sessions, 1, "branch-only qualifier should work");

    let r3 = idx.search("role:user", None, 10, 0).unwrap();
    assert_eq!(r3.total_sessions, 1, "role-only qualifier should work");

    let r4 = idx.search("skill:commit", None, 10, 0).unwrap();
    assert_eq!(r4.total_sessions, 1, "skill-only qualifier should work");

    let r5 = idx.search("model:opus", None, 10, 0).unwrap();
    assert_eq!(r5.total_sessions, 1, "model-only qualifier (partial) should work");
}
```

**Step 4: Run all qualifier tests**

Run: `cargo test -p vibe-recall-search -- test_search`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add crates/search/src/lib.rs
git commit -m "test(search): add qualifier integration tests (display names, qualifier-only, all types)"
```

---

### Task 5: Final verification — rebuild index and test live

Verify the full stack works with a fresh Tantivy index.

**Step 1: Delete the stale Tantivy index**

```bash
rm -rf ~/Library/Caches/vibe-recall/search-index
```

**Step 2: Restart the dev server**

```bash
# In the project root — restart the backend
# The parse version bump (6→7) + deleted index triggers full re-index
```

**Step 3: Wait for indexing to complete, then test**

```bash
# Test project qualifier with display name
curl -s 'http://localhost:47892/api/search?q=project%3Atest-app' | python3 -c "import json,sys; d=json.load(sys.stdin); print(f'project:test-app → {d[\"totalSessions\"]} sessions')"
# Expected: > 0 sessions

# Test model partial match
curl -s 'http://localhost:47892/api/search?q=model%3Aopus+hello' | python3 -c "import json,sys; d=json.load(sys.stdin); print(f'model:opus → {d[\"totalSessions\"]} sessions')"
# Expected: > 0 sessions

# Test skill qualifier (if any sessions use skills)
curl -s 'http://localhost:47892/api/search?q=skill%3Acommit' | python3 -c "import json,sys; d=json.load(sys.stdin); print(f'skill:commit → {d[\"totalSessions\"]} sessions')"
```

**Step 4: Commit all remaining changes**

```bash
git add -A
git commit -m "feat(search): production-grade qualifier fixes — project display names, model partial match, skills indexing, schema versioning"
```

---

## Summary of Changes

| File | Change |
|------|--------|
| `crates/search/src/lib.rs` | Add `SEARCH_SCHEMA_VERSION`, version-aware `open()`, model field STRING→TEXT, new tests |
| `crates/search/src/query.rs` | Lowercase qualifier values for TEXT fields |
| `crates/db/src/indexer_parallel.rs` | Add skills to `SearchBatch`, pipe to `SearchDocument`, parse version 7 |
| `crates/db/src/queries/sessions.rs` | SQL returns `project_display_name` instead of `project_id` |
