# Unified Smart Search Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge Tantivy full-text search and ripgrep substring search into a single `/api/search` endpoint so the frontend sends one request and always gets results — no engine selection, no wiring gaps, no missing CJK matches.

**Architecture:** The existing `/api/search` handler becomes a two-phase search: Tantivy first (fast, ranked), grep fallback if Tantivy returns 0 results. The response type stays `SearchResponse` with a new optional `searchEngine` field. The frontend simplifies: remove `isRegex` gate, remove `useGrep` from main flow, one hook, one API.

**Tech Stack:** Rust (Axum, Tantivy, grep-regex/grep-searcher), TypeScript (React, TanStack Query, Vitest)

**Design Principle (from CLAUDE.md):** "The frontend NEVER orchestrates between multiple backend engines." One endpoint per capability. Backend decides strategy.

**Spec divergence:** The original smart-search spec (`docs/plans/2026-02-28-smart-search-design.md` §5) says grep orchestration is "handled in the frontend." This plan deliberately reverses that — backend orchestrates, frontend stays logic-clean. See CLAUDE.md "API Design Principle: One Endpoint Per Capability."

---

## File Structure

### Backend (Rust)

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/search/src/unified.rs` | **Create** | `unified_search()` — orchestrates Tantivy → grep fallback, normalizes grep results into `SearchResponse` |
| `crates/search/src/types.rs` | Modify | Add `search_engine: Option<String>` to `SearchResponse` |
| `crates/search/src/lib.rs` | Modify | Add `pub mod unified;`, re-export `unified_search` |
| `crates/server/src/routes/search.rs` | Modify | Call `unified_search()` instead of raw `search_index.search()` |
| `crates/server/src/routes/grep.rs` | Modify (extract) | Extract `collect_jsonl_files()` into shared fn, keep `/api/grep` for power users |

### Frontend (TypeScript)

| File | Action | Responsibility |
|------|--------|----------------|
| `apps/web/src/components/CommandPalette.tsx` | Modify | Remove `useGrep`, remove `isRegex` gate, use only `useSearch` |
| `apps/web/src/components/SearchResults.tsx` | Modify (minor) | Add `searchEngine` indicator when grep fallback was used |
| `apps/web/src/hooks/use-search.ts` | Modify (minor) | Remove `hasRegexMetacharacters` export |

### Tests

| File | Action | Tests |
|------|--------|-------|
| `crates/search/src/unified.rs` | Create (inline `#[cfg(test)]`) | 8 unit tests: Tantivy-only, grep-fallback, CJK, mixed, scoped, empty, regex, response shape |
| `crates/search/tests/unified_integration_test.rs` | Create | 3 integration tests: end-to-end with real JSONL files |
| `apps/web/src/components/CommandPalette.search.test.tsx` | Create | 3 regression guards: no isRegex, no useGrep, unified API only |

---

## Task Dependencies (HARD — do not parallelize across these boundaries)

```
Task 1 → Task 3 (unified.rs constructs SearchResponse, needs search_engine field)
Task 1 → Task 6 (TS uses searchEngine, needs codegen from Task 1 Step 4)
Task 2 → Task 4 (handler imports collect_jsonl_files from Task 2)
Task 3 → Task 4 (handler calls unified_search from Task 3)
Task 5 → Task 8 (regression tests verify Task 5 deletions)
```

Tasks within the same dependency level CAN run in parallel:
- **Level 1:** Task 1 + Task 2 (independent)
- **Level 2:** Task 3 (depends on Task 1)
- **Level 3:** Task 4 (depends on Tasks 2 + 3)
- **Level 4:** Task 5 + Task 6 + Task 7 (independent, all depend on Tasks 1-4)
- **Level 5:** Task 8 (depends on Tasks 5 + 6)

---

## Chunk 1: Backend — Unified Search Engine

### Task 1: Add `SearchEngine` enum and `search_engine` field to `SearchResponse`

**Files:**

- Modify: `crates/search/src/types.rs`

The `SearchResponse` struct gets a new optional `search_engine` field so the frontend knows which engine produced results. This is a `#[derive(TS)]` struct so the generated TypeScript type updates automatically.

- [ ] **Step 1: Add `search_engine` field to `SearchResponse`**

In `crates/search/src/types.rs`, add after `pub sessions: Vec<SessionHit>`:

```rust
/// Which search engine produced these results.
/// `None` = Tantivy (default), `"grep"` = grep fallback.
/// Allows the frontend to show a subtle indicator when grep fallback fired.
#[serde(skip_serializing_if = "Option::is_none")]
pub search_engine: Option<String>,
```

- [ ] **Step 2: Fix all `SearchResponse` construction sites**

Every place that constructs a `SearchResponse` now needs the field. Search for `SearchResponse {` in the search crate and add `search_engine: None` to each.

Run: `rg 'SearchResponse \{' crates/search/src/`

There are exactly **4 construction sites** in `crates/search/src/query.rs`:
- Line ~204: empty result for session ID lookup miss
- Line ~284: session ID found result
- Line ~467: empty result for no sub-queries
- Line ~731: main search result

Add `search_engine: None,` to each of these 4 sites.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p claude-view-search`
Expected: Clean compile.

- [ ] **Step 4: Regenerate TypeScript types**

The `SearchResponse` struct has `#[derive(TS)]` with `#[cfg_attr(feature = "codegen", ts(export))]`. After adding the new field, regenerate the TS types so `apps/web/src/types/generated/SearchResponse.ts` includes `searchEngine?: string`.

Run: `./scripts/generate-types.sh`

Verify: `grep searchEngine apps/web/src/types/generated/SearchResponse.ts`
Expected: `searchEngine?: string` appears in the generated type.

- [ ] **Step 5: Commit**

```bash
git add crates/search/src/types.rs crates/search/src/query.rs apps/web/src/types/generated/SearchResponse.ts
git commit -m "feat(search): add search_engine field to SearchResponse

Optional field indicates which engine produced results ('grep' for
fallback, None for Tantivy). Enables frontend indicator for grep results.
Regenerated TS types include searchEngine?: string."
```

---

### Task 2: Extract `collect_jsonl_files()` from grep handler

**Files:**

- Modify: `crates/server/src/routes/grep.rs`

The grep handler at `grep.rs:60-111` has a directory scanning loop that collects JSONL files. The unified search handler needs the same function. Extract it as a shared `pub fn collect_jsonl_files(project_filter: Option<&str>) -> Result<Vec<JsonlFile>, ApiError>`.

- [ ] **Step 1: Extract the function**

Add at the bottom of `grep.rs` (before any test module):

```rust
/// Scan ~/.claude/projects/ for all JSONL session files.
/// Optionally filter by project display name or full path.
///
/// Used by both `/api/grep` and `/api/search` (unified search grep fallback).
///
/// NOTE: project filter checks BOTH display_name AND full_path to match
/// the polymorphic project filter pattern (CLAUDE.md Hard Rule).
pub fn collect_jsonl_files(project_filter: Option<&str>) -> Result<Vec<JsonlFile>, ApiError> {
    let projects_dir =
        claude_projects_dir().map_err(|e| ApiError::Internal(format!("Projects dir: {e}")))?;

    let mut files: Vec<JsonlFile> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let project_dir = entry.path();
            if !project_dir.is_dir() {
                continue;
            }

            let dir_name = entry.file_name().to_string_lossy().to_string();
            let resolved = resolve_project_path_with_cwd(&dir_name, None);

            if let Some(proj) = project_filter {
                if resolved.display_name != proj && resolved.full_path != proj {
                    continue;
                }
            }

            if let Ok(sessions) = std::fs::read_dir(&project_dir) {
                for session in sessions.flatten() {
                    let path = session.path();
                    if path.extension().is_some_and(|e| e == "jsonl") {
                        let session_id = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let modified_at = path
                            .metadata()
                            .and_then(|m| m.modified())
                            .map(|t| {
                                t.duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs() as i64
                            })
                            .unwrap_or(0);

                        files.push(JsonlFile {
                            path,
                            session_id,
                            project: resolved.display_name.clone(),
                            project_path: resolved.full_path.clone(),
                            modified_at,
                        });
                    }
                }
            }
        }
    }

    Ok(files)
}
```

- [ ] **Step 2: Refactor grep_handler to use it**

In `grep_handler`'s `spawn_blocking` closure, **DELETE the entire inline directory scan block** (from the `claude_projects_dir()` call through the nested `for` loops that build `files: Vec<JsonlFile>`) and replace with a single call:

```rust
let files = collect_jsonl_files(project_filter.as_deref())?;
```

The `project_filter` variable comes from the existing `params.project` field — keep that extraction as-is. Only the scan loop body is replaced. Everything after (the `grep_files()` call and response construction) stays unchanged.

- [ ] **Step 3: Verify grep still works**

Run: `cargo test -p claude-view-server`
Expected: All existing tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/server/src/routes/grep.rs
git commit -m "refactor(grep): extract collect_jsonl_files for reuse

Shared function scans ~/.claude/projects/ for JSONL files with optional
project filter (checks both display_name and full_path per CLAUDE.md
polymorphic project filter rule). Used by both /api/grep and unified search.

Also fixes pre-existing bug: /api/grep only checked display_name for
project filter — now checks full_path too (polymorphic pattern)."
```

---

### Task 3: Create `unified_search` with TDD — tests first, then implement

**Files:**

- Create: `crates/search/src/unified.rs`
- Modify: `crates/search/src/lib.rs`

This is the core feature. The function orchestrates Tantivy → grep fallback.

**Critical design decisions (from reviewer):**

1. **`search_index: None` still returns 503.** The handler decides — if index is `None` (still building), the handler returns 503 as before. `unified_search` receives `Option<&SearchIndex>` so it CAN skip Tantivy when called from contexts that don't have it (e.g. tests), but the route handler is NOT one of those contexts.
2. **Grep does NOT support session-level offset.** When grep fallback fires, `offset` is ignored (grep returns all matching sessions up to `limit`, frontend paginates client-side). This is documented in the response via `search_engine: Some("grep")` so the frontend knows.
3. **`scope` filter is passed to grep.** The `scope` query param (e.g. `project:claude-view`) is parsed and passed as `project_filter` to `collect_jsonl_files`.

- [ ] **Step 0: Verify Task 1 is complete (hard dependency)**

Run: `cargo check -p claude-view-search`
Expected: Clean compile — confirms `SearchResponse` has the `search_engine` field from Task 1.
If this fails, STOP — Task 1 must be completed first.

- [ ] **Step 1: Create `unified.rs` with types, implementation, and 8 tests**

```rust
// crates/search/src/unified.rs
//! Unified search: Tantivy first, grep fallback if 0 results.
//!
//! Implements the "One Endpoint Per Capability" design principle:
//! the frontend sends one request, the backend tries all strategies
//! internally and returns a unified `SearchResponse`.

use crate::grep::{grep_files, GrepOptions, JsonlFile};
use crate::types::{MatchHit, SearchResponse, SessionHit};
use crate::SearchIndex;

/// Which engine produced the search results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchEngine {
    /// Results came from Tantivy full-text index.
    Tantivy,
    /// Results came from grep fallback (Tantivy returned 0).
    Grep,
}

/// Options for unified search.
pub struct UnifiedSearchOptions {
    /// The raw query string.
    pub query: String,
    /// Optional scope filter (e.g. `"project:claude-view"`).
    pub scope: Option<String>,
    /// Maximum session groups to return.
    pub limit: usize,
    /// Session groups to skip (pagination). NOTE: only applies to Tantivy.
    /// Grep fallback ignores offset (no session-level pagination in grep).
    pub offset: usize,
}

/// Extended search response with engine metadata.
pub struct UnifiedSearchResult {
    pub response: SearchResponse,
    pub engine: SearchEngine,
}

/// Run unified search: Tantivy first, grep fallback if 0 results.
///
/// - `search_index`: If `None`, skips Tantivy and goes straight to grep.
///   (The route handler should return 503 when index is building — this
///    `None` path is for tests and future direct-grep contexts only.)
/// - `jsonl_files`: Pre-collected files to grep if fallback is needed.
/// - `opts`: Query, scope, pagination.
///
/// **Qualifier limitation:** Qualifier-based project filters (e.g. `project:foo`
/// embedded in `opts.query`) are Tantivy-only. Grep fallback scoping requires
/// the explicit `scope` field in `opts`, which the route handler populates from
/// the `scope` query parameter. This is an inherent grep limitation.
pub fn unified_search(
    search_index: Option<&SearchIndex>,
    jsonl_files: &[JsonlFile],
    opts: &UnifiedSearchOptions,
) -> Result<UnifiedSearchResult, UnifiedSearchError> {
    // Phase 1: Tantivy
    if let Some(idx) = search_index {
        let tantivy_result =
            idx.search(&opts.query, opts.scope.as_deref(), opts.limit, opts.offset)?;

        if tantivy_result.total_sessions > 0 {
            return Ok(UnifiedSearchResult {
                response: tantivy_result,
                engine: SearchEngine::Tantivy,
            });
        }
    }

    // Phase 2: Grep fallback
    if jsonl_files.is_empty() {
        // No files to grep — return empty results with no engine indicator.
        // Don't set search_engine: "grep" here because grep didn't actually run.
        return Ok(UnifiedSearchResult {
            response: SearchResponse {
                query: opts.query.clone(),
                total_sessions: 0,
                total_matches: 0,
                elapsed_ms: 0.0,
                sessions: vec![],
                search_engine: None,
            },
            engine: SearchEngine::Grep,
        });
    }

    let grep_opts = GrepOptions {
        pattern: regex_escape_for_literal(&opts.query),
        case_sensitive: false,
        whole_word: false,
        limit: opts.limit * 10, // over-fetch lines, we group by session
    };

    let grep_result = grep_files(jsonl_files, &grep_opts)?;

    // Normalize grep results into SearchResponse shape
    let sessions: Vec<SessionHit> = grep_result
        .results
        .into_iter()
        .take(opts.limit)
        .map(|hit| {
            let match_count = hit.matches.len();
            let top_match = hit
                .matches
                .first()
                .map(|m| MatchHit {
                    role: "unknown".to_string(),
                    turn_number: 0,
                    snippet: truncate_and_highlight(&m.content, m.match_start, m.match_end),
                    timestamp: hit.modified_at,
                })
                .unwrap_or_else(|| MatchHit {
                    role: "unknown".to_string(),
                    turn_number: 0,
                    snippet: String::new(),
                    timestamp: 0,
                });

            let matches: Vec<MatchHit> = hit
                .matches
                .iter()
                .map(|m| MatchHit {
                    role: "unknown".to_string(),
                    turn_number: 0,
                    snippet: truncate_and_highlight(&m.content, m.match_start, m.match_end),
                    timestamp: hit.modified_at,
                })
                .collect();

            SessionHit {
                session_id: hit.session_id,
                project: hit.project,
                branch: None,
                modified_at: hit.modified_at,
                match_count,
                best_score: 1.0, // grep has no scoring — uniform
                top_match,
                matches,
            }
        })
        .collect();

    let total_sessions = sessions.len();
    let total_matches: usize = sessions.iter().map(|s| s.match_count).sum();

    Ok(UnifiedSearchResult {
        response: SearchResponse {
            query: opts.query.clone(),
            total_sessions,
            total_matches,
            elapsed_ms: grep_result.elapsed_ms,
            sessions,
            search_engine: Some("grep".to_string()),
        },
        engine: SearchEngine::Grep,
    })
}

/// Escape regex metacharacters for literal grep search.
/// When the user types plain text, we want grep to find it literally.
fn regex_escape_for_literal(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len() + 8);
    for ch in input.chars() {
        if "\\.*+?()[]{}|^$".contains(ch) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

/// Truncate raw JSONL line content and wrap match region with <mark> tags.
fn truncate_and_highlight(content: &str, match_start: usize, match_end: usize) -> String {
    let chars: Vec<char> = content.chars().collect();
    let total = chars.len();

    let context_before = 80;
    let context_after = 150;
    let start = match_start.saturating_sub(context_before);
    let end = (match_end + context_after).min(total);

    let prefix = if start > 0 { "..." } else { "" };
    let suffix = if end < total { "..." } else { "" };

    let before: String = chars[start..match_start.min(total)].iter().collect();
    let matched: String = chars[match_start.min(total)..match_end.min(total)]
        .iter()
        .collect();
    let after: String = chars[match_end.min(total)..end].iter().collect();

    format!("{prefix}{before}<mark>{matched}</mark>{after}{suffix}")
}

#[derive(Debug, thiserror::Error)]
pub enum UnifiedSearchError {
    #[error("Search error: {0}")]
    Search(#[from] crate::SearchError),
    #[error("Grep error: {0}")]
    Grep(#[from] crate::grep::GrepError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::SearchDocument;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create an in-RAM Tantivy index with some documents.
    fn create_test_index(docs: &[(&str, &str, &str)]) -> SearchIndex {
        let idx = SearchIndex::open_in_ram().expect("create index");
        for (session_id, role, content) in docs {
            let doc = SearchDocument {
                session_id: session_id.to_string(),
                project: "test-project".to_string(),
                branch: "main".to_string(),
                model: "opus".to_string(),
                role: role.to_string(),
                content: content.to_string(),
                turn_number: 1,
                timestamp: 1710000000,
                skills: vec![],
            };
            idx.index_session(session_id, &[doc]).expect("index");
        }
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");
        idx
    }

    /// Helper: create temp JSONL files for grep testing.
    fn create_test_jsonl_files(
        dir: &std::path::Path,
        entries: &[(&str, &str)],
    ) -> Vec<JsonlFile> {
        entries
            .iter()
            .map(|(session_id, content)| {
                let path = dir.join(format!("{session_id}.jsonl"));
                fs::write(&path, content).unwrap();
                JsonlFile {
                    path,
                    session_id: session_id.to_string(),
                    project: "test-project".to_string(),
                    project_path: dir.to_string_lossy().to_string(),
                    modified_at: 1710000000,
                }
            })
            .collect()
    }

    #[test]
    fn test_tantivy_hit_returns_tantivy_engine() {
        let idx = create_test_index(&[("s1", "user", "deploy to production tonight")]);
        let opts = UnifiedSearchOptions {
            query: "deploy".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(Some(&idx), &[], &opts).unwrap();
        assert_eq!(result.engine, SearchEngine::Tantivy);
        assert_eq!(result.response.total_sessions, 1);
        // Tantivy path should NOT set search_engine
        assert!(result.response.search_engine.is_none());
    }

    #[test]
    fn test_tantivy_miss_falls_back_to_grep() {
        let tmp = TempDir::new().unwrap();
        let idx = create_test_index(&[("s1", "user", "deploy to production")]);
        let files = create_test_jsonl_files(
            tmp.path(),
            &[("s2", "{\"role\":\"user\",\"content\":\"hook 嘅 payload 本身冇問題\"}\n")],
        );

        let opts = UnifiedSearchOptions {
            query: "嘅 payload".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(Some(&idx), &files, &opts).unwrap();
        assert_eq!(result.engine, SearchEngine::Grep);
        assert!(result.response.total_sessions > 0);
        assert_eq!(result.response.search_engine.as_deref(), Some("grep"));
    }

    #[test]
    fn test_cjk_text_found_via_grep_fallback() {
        let tmp = TempDir::new().unwrap();
        let idx = create_test_index(&[]); // empty index
        let files = create_test_jsonl_files(
            tmp.path(),
            &[("s1", "{\"content\":\"自動部署到生產環境完成\"}\n")],
        );

        let opts = UnifiedSearchOptions {
            query: "部署".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(Some(&idx), &files, &opts).unwrap();
        assert_eq!(result.engine, SearchEngine::Grep);
        assert_eq!(result.response.total_sessions, 1);
        let snippet = &result.response.sessions[0].top_match.snippet;
        assert!(
            snippet.contains("<mark>"),
            "snippet should have highlight: {snippet}"
        );
    }

    #[test]
    fn test_no_index_goes_straight_to_grep() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_jsonl_files(
            tmp.path(),
            &[("s1", "{\"msg\":\"hello world\"}\n")],
        );

        let opts = UnifiedSearchOptions {
            query: "hello".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(None, &files, &opts).unwrap();
        assert_eq!(result.engine, SearchEngine::Grep);
        assert_eq!(result.response.total_sessions, 1);
    }

    #[test]
    fn test_both_empty_returns_zero_results() {
        let idx = create_test_index(&[]);
        let opts = UnifiedSearchOptions {
            query: "nonexistent".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(Some(&idx), &[], &opts).unwrap();
        assert_eq!(result.response.total_sessions, 0);
    }

    #[test]
    fn test_regex_metacharacters_escaped_for_grep() {
        let tmp = TempDir::new().unwrap();
        let idx = create_test_index(&[]);
        let files = create_test_jsonl_files(
            tmp.path(),
            &[("s1", "{\"content\":\"auth.*middleware pattern\"}\n")],
        );

        // User types literal "auth.*middleware" — should find it literally,
        // NOT interpret .* as regex "any characters"
        let opts = UnifiedSearchOptions {
            query: "auth.*middleware".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(Some(&idx), &files, &opts).unwrap();
        assert_eq!(result.engine, SearchEngine::Grep);
        assert_eq!(result.response.total_sessions, 1);
    }

    #[test]
    fn test_grep_results_normalized_to_session_hit_shape() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_jsonl_files(
            tmp.path(),
            &[(
                "s1",
                "{\"content\":\"line one match\"}\n{\"content\":\"line two match\"}\n",
            )],
        );

        let opts = UnifiedSearchOptions {
            query: "match".to_string(),
            scope: None,
            limit: 10,
            offset: 0,
        };
        let result = unified_search(None, &files, &opts).unwrap();

        assert_eq!(result.response.sessions.len(), 1);
        let session = &result.response.sessions[0];
        assert_eq!(session.session_id, "s1");
        assert_eq!(session.match_count, 2);
        assert_eq!(session.matches.len(), 2);
        assert_eq!(session.best_score, 1.0);
        assert!(session.branch.is_none());
        assert!(session.top_match.snippet.contains("<mark>match</mark>"));
    }

    #[test]
    fn test_regex_escape_for_literal() {
        assert_eq!(regex_escape_for_literal("hello"), "hello");
        assert_eq!(regex_escape_for_literal("a.*b"), "a\\.\\*b");
        assert_eq!(regex_escape_for_literal("fn()"), "fn\\(\\)");
        assert_eq!(regex_escape_for_literal("[test]"), "\\[test\\]");
        assert_eq!(regex_escape_for_literal("a|b"), "a\\|b");
        // CJK passes through unchanged
        assert_eq!(regex_escape_for_literal("部署"), "部署");
    }
}
```

- [ ] **Step 2: Register the module in `lib.rs`**

Add to `crates/search/src/lib.rs`, near the other `pub mod` declarations:

```rust
pub mod unified;
```

And in the re-exports section:

```rust
pub use unified::{
    unified_search, SearchEngine, UnifiedSearchOptions, UnifiedSearchResult, UnifiedSearchError,
};
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p claude-view-search unified -- --nocapture`
Expected: All 8 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/search/src/unified.rs crates/search/src/lib.rs
git commit -m "feat(search): add unified_search — Tantivy first, grep fallback

TDD: 8 unit tests covering Tantivy-hit, grep-fallback, CJK text,
no-index mode, empty results, regex escaping, and response normalization.
Grep results include search_engine='grep' for frontend indicator."
```

---

### Task 4: Wire unified search into the `/api/search` route handler

**Files:**

- Modify: `crates/server/src/routes/search.rs`

**Critical: 503 behavior preserved.** When `search_index` is `None` (still building), the handler returns 503 as before. The grep fallback only fires when Tantivy is available but returns 0 results — it does NOT replace the 503 path.

- [ ] **Step 1: Update the handler to use `unified_search`**

Replace the handler body in `crates/server/src/routes/search.rs`:

```rust
use claude_view_search::{
    unified_search, UnifiedSearchOptions,
};

use super::grep::collect_jsonl_files;

/// GET /api/search — Unified smart search.
///
/// Tries Tantivy full-text index first. If 0 results, falls back to
/// grep over raw JSONL files. Returns a single `SearchResponse` shape
/// regardless of which engine produced the results.
///
/// Returns 503 if the search index is still building (grep is NOT
/// a substitute for the missing index — it's a fallback for Tantivy
/// misses, not Tantivy absence).
async fn search_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<SearchResponse>> {
    let q = query.q.as_deref().unwrap_or("").trim();
    if q.is_empty() {
        return Err(crate::error::ApiError::BadRequest(
            "query parameter 'q' is required".to_string(),
        ));
    }

    // Read-lock the holder, clone the Option<Arc<SearchIndex>>, drop the lock immediately.
    let search_index = state
        .search_index
        .read()
        .map_err(|_| crate::error::ApiError::Internal("search index lock poisoned".into()))?
        .clone();

    // 503 if index not ready — grep is NOT a substitute for missing index
    let search_index = search_index.ok_or_else(|| {
        crate::error::ApiError::ServiceUnavailable(
            "Search index is not available. It may still be building.".to_string(),
        )
    })?;

    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    let scope = query.scope.clone();

    let q_owned = q.to_string();

    let response = tokio::task::spawn_blocking(move || {
        // Parse scope for project filter (e.g. "project:claude-view" -> "claude-view")
        let project_filter = scope.as_deref().and_then(|s| {
            s.strip_prefix("project:").map(|p| p.to_string())
        });

        // Collect JSONL files for grep fallback, scoped by project if specified.
        // Log errors but don't fail the request — grep is a fallback, not primary.
        let jsonl_files = match collect_jsonl_files(project_filter.as_deref()) {
            Ok(files) => files,
            Err(e) => {
                tracing::warn!("Failed to collect JSONL files for grep fallback: {e}");
                vec![]
            }
        };

        let opts = UnifiedSearchOptions {
            query: q_owned,
            scope,
            limit,
            offset,
        };

        // search_index is Arc<SearchIndex> — .as_ref() dereferences to &SearchIndex.
        // Rust does NOT auto-deref Arc<T> to &T in function argument position.
        unified_search(Some(search_index.as_ref()), &jsonl_files, &opts)
    })
    .await
    .map_err(|e| {
        let msg = if e.is_panic() {
            let panic_payload = e.into_panic();
            if let Some(s) = panic_payload.downcast_ref::<String>() {
                format!("Search panicked: {}", s)
            } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                format!("Search panicked: {}", s)
            } else {
                "Search panicked (unknown payload)".to_string()
            }
        } else {
            format!("Search task failed: {}", e)
        };
        tracing::error!("{}", msg);
        crate::error::ApiError::Internal(msg)
    })?
    .map_err(|e| crate::error::ApiError::Internal(format!("Search failed: {}", e)))?;

    Ok(Json(response.response))
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p claude-view-server`
Expected: Clean compile, no errors.

- [ ] **Step 3: Run all search tests**

Run: `cargo test -p claude-view-search && cargo test -p claude-view-server`
Expected: All existing tests still pass.

- [ ] **Step 4: Commit**

```bash
git add crates/server/src/routes/search.rs
git commit -m "feat(search): wire unified_search into /api/search handler

/api/search now tries Tantivy first, falls back to grep if 0 results.
503 preserved when index is building. Scope/project filter passed to
grep. Single response shape — frontend needs no changes to benefit."
```

---

## Chunk 2: Frontend Simplification

### Task 5: Remove `isRegex` gate from CommandPalette

**Files:**

- Modify: `apps/web/src/components/CommandPalette.tsx`

The CommandPalette currently has a binary `isRegex` switch that routes to either `useSearch` OR `useGrep`. Since `/api/search` now handles grep internally, we remove the split.

- [ ] **Step 1: Remove grep imports and `isRegex` logic**

In `CommandPalette.tsx`, make these changes:

1. Remove import: `import { useGrep } from '../hooks/use-grep'`
2. Change import: `import { hasRegexMetacharacters, useSearch } from '../hooks/use-search'`
   → `import { useSearch } from '../hooks/use-search'`
3. Remove import: `import { GrepResults } from './GrepResults'`
4. Remove: `const isRegex = useMemo(() => hasRegexMetacharacters(query), [query])`
5. Change useSearch `enabled` from `isOpen && !isRegex` to just `isOpen`:
   ```tsx
   const {
     data: searchResults,
     isLoading: isSearching,
     isDebouncing,
   } = useSearch(query, {
     enabled: isOpen,
     limit: 5,
   })
   ```
6. Remove entire `useGrep` block
7. Remove `hasGrepResults` variable
8. Remove regex badge in input
9. Simplify `showLoading`:
   ```tsx
   const showLoading = query.trim().length > 0 && (isSearching || isDebouncing)
   ```
10. Remove grep results rendering block
11. Simplify no-results check — remove `isRegex` branching

- [ ] **Step 2: Delete dead files and obsolete tests**

After removing `useGrep` and `GrepResults` imports from CommandPalette, these files have zero consumers:

1. Delete `apps/web/src/hooks/use-grep.ts` — no remaining imports (verified: only CommandPalette imported it)
2. Delete `apps/web/src/components/GrepResults.tsx` — no remaining imports (verified: only CommandPalette imported it)
3. Delete `apps/web/src/components/CommandPalette.regex.test.tsx` — imports `hasRegexMetacharacters` which is being removed from `use-search.ts`. The Task 8 regression guards supersede this test.

The `/api/grep` endpoint remains available for power users — only the frontend wiring is removed.

- [ ] **Step 3: Remove `hasRegexMetacharacters` export from `use-search.ts`**

In `apps/web/src/hooks/use-search.ts`, remove the `hasRegexMetacharacters` function export. No other consumers remain after Step 2 deletes the regex test.

- [ ] **Step 4: Verify it compiles**

Run: `cd apps/web && bunx tsc --noEmit`
Expected: Clean compile — no dangling imports of `useGrep`, `GrepResults`, or `hasRegexMetacharacters`.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/components/CommandPalette.tsx
git rm apps/web/src/hooks/use-grep.ts
git rm apps/web/src/components/GrepResults.tsx
git rm apps/web/src/components/CommandPalette.regex.test.tsx
git add apps/web/src/hooks/use-search.ts
git commit -m "refactor(search): remove isRegex gate from CommandPalette

Backend now handles grep fallback internally. CommandPalette uses
only useSearch — one hook, one API, one response shape. Removed
useGrep, hasRegexMetacharacters, GrepResults, and isRegex branching.

Deleted dead files: use-grep.ts, GrepResults.tsx, regex.test.tsx.
The /api/grep endpoint is kept for power users — only frontend wiring removed."
```

---

### Task 6: Add grep indicator to SearchResults page

**Files:**

- Modify: `apps/web/src/components/SearchResults.tsx`

When grep fallback fires, show a subtle amber badge so users understand why results look different (no BM25 ranking, raw substring matches).

- [ ] **Step 1: Add indicator after stats line**

In `SearchResults.tsx`, after the closing `</p>` of the stats paragraph (currently around line 77), add:

```tsx
{searchResults?.searchEngine === 'grep' && (
  <span className="ml-2 text-xs text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-500/10 px-1.5 py-0.5 rounded">
    Substring matches
  </span>
)}
```

- [ ] **Step 2: Verify build**

**Requires:** Task 1 Step 4 (`./scripts/generate-types.sh`) must have completed — otherwise `searchEngine` does not exist in the generated `SearchResponse.ts` and `tsc` will fail.

Run: `cd apps/web && bunx tsc --noEmit && bun run build`
Expected: Clean compile and build.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/SearchResults.tsx
git commit -m "feat(search): show 'Substring matches' indicator on grep fallback

Subtle amber badge when results came from grep instead of Tantivy.
Helps users understand why ranking differs from full-text results."
```

---

## Chunk 3: Regression Guard Tests

### Task 7: Integration tests — end-to-end with real JSONL files

**Files:**

- Create: `crates/search/tests/unified_integration_test.rs`

These tests verify the full pipeline: Tantivy index + JSONL files + unified_search.

- [ ] **Step 1: Write integration tests**

```rust
// crates/search/tests/unified_integration_test.rs
//! Integration tests for unified search (Tantivy + grep fallback).
//!
//! These tests use real temporary JSONL files and an in-RAM Tantivy index
//! to verify the complete two-phase search pipeline.

use claude_view_search::indexer::SearchDocument;
use claude_view_search::unified::{
    unified_search, SearchEngine, UnifiedSearchOptions,
};
use claude_view_search::{JsonlFile, SearchIndex};
use std::fs;
use tempfile::TempDir;

fn index_doc(idx: &SearchIndex, session_id: &str, content: &str) {
    let doc = SearchDocument {
        session_id: session_id.to_string(),
        project: "integration-test".to_string(),
        branch: "main".to_string(),
        model: "opus".to_string(),
        role: "user".to_string(),
        content: content.to_string(),
        turn_number: 1,
        timestamp: 1710000000,
        skills: vec![],
    };
    idx.index_session(session_id, &[doc]).unwrap();
}

fn make_jsonl_file(dir: &std::path::Path, session_id: &str, content: &str) -> JsonlFile {
    let path = dir.join(format!("{session_id}.jsonl"));
    fs::write(&path, content).unwrap();
    JsonlFile {
        path,
        session_id: session_id.to_string(),
        project: "integration-test".to_string(),
        project_path: dir.to_string_lossy().to_string(),
        modified_at: 1710000000,
    }
}

/// Tantivy finds the result — grep is never called.
#[test]
fn test_tantivy_sufficient_no_grep() {
    let idx = SearchIndex::open_in_ram().unwrap();
    index_doc(&idx, "s1", "deploy to production");
    idx.commit().unwrap();
    idx.reader.reload().unwrap();

    let tmp = TempDir::new().unwrap();
    let files = vec![make_jsonl_file(
        tmp.path(),
        "s2",
        "{\"content\":\"deploy unrelated\"}\n",
    )];

    let opts = UnifiedSearchOptions {
        query: "deploy to production".to_string(),
        scope: None,
        limit: 10,
        offset: 0,
    };

    let result = unified_search(Some(&idx), &files, &opts).unwrap();
    assert_eq!(result.engine, SearchEngine::Tantivy);
    assert_eq!(result.response.sessions.len(), 1);
    assert_eq!(result.response.sessions[0].session_id, "s1");
}

/// CJK without spaces — Tantivy misses, grep catches.
/// This is the exact bug reported on 2026-03-11.
#[test]
fn test_cjk_without_spaces_grep_fallback() {
    let idx = SearchIndex::open_in_ram().unwrap();
    // Index the same CJK content — Tantivy tokenizes as one giant token
    index_doc(&idx, "s1", "自動部署到生產環境完成");
    idx.commit().unwrap();
    idx.reader.reload().unwrap();

    let tmp = TempDir::new().unwrap();
    let files = vec![make_jsonl_file(
        tmp.path(),
        "s1",
        "{\"content\":\"自動部署到生產環境完成\"}\n",
    )];

    // Search for a substring — Tantivy can't find "部署" inside the mega-token
    let opts = UnifiedSearchOptions {
        query: "部署".to_string(),
        scope: None,
        limit: 10,
        offset: 0,
    };

    let result = unified_search(Some(&idx), &files, &opts).unwrap();
    assert_eq!(result.engine, SearchEngine::Grep);
    assert_eq!(result.response.total_sessions, 1);
    assert!(
        result.response.sessions[0]
            .top_match
            .snippet
            .contains("部署"),
        "snippet should contain the CJK search term"
    );
}

/// Mixed English/Cantonese — the original bug report: "hook 嘅 payload"
#[test]
fn test_mixed_cantonese_english_search() {
    let idx = SearchIndex::open_in_ram().unwrap();
    index_doc(
        &idx,
        "s1",
        "SessionStart hook 嘅 payload 本身冇 git_branch",
    );
    idx.commit().unwrap();
    idx.reader.reload().unwrap();

    let tmp = TempDir::new().unwrap();
    let files = vec![make_jsonl_file(
        tmp.path(),
        "s1",
        "{\"content\":\"SessionStart hook 嘅 payload 本身冇 git_branch\"}\n",
    )];

    let opts = UnifiedSearchOptions {
        query: "hook 嘅 payload".to_string(),
        scope: None,
        limit: 10,
        offset: 0,
    };

    let result = unified_search(Some(&idx), &files, &opts).unwrap();
    // This specific case may work in Tantivy (spaces delimit tokens)
    // but if it doesn't, grep catches it. Either way: results > 0.
    assert!(
        result.response.total_sessions > 0,
        "Must find the session regardless of which engine"
    );
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test -p claude-view-search --test unified_integration_test -- --nocapture`
Expected: All 3 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/search/tests/unified_integration_test.rs
git commit -m "test(search): integration tests for unified search

Regression guards for:
- Tantivy-sufficient (no unnecessary grep)
- CJK without spaces (the 2026-03-11 bug)
- Mixed Cantonese/English (original bug report)"
```

---

### Task 8: Frontend regression guard tests

**Files:**

- Create: `apps/web/src/components/CommandPalette.search.test.tsx`

Static analysis tests that verify the "One Endpoint Per Capability" principle is maintained — no isRegex routing, no useGrep in main flow.

- [ ] **Step 1: Write CommandPalette search tests**

```tsx
// apps/web/src/components/CommandPalette.search.test.tsx
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

// ESM-compatible __dirname — required because the project uses "type": "module"
// and CJS globals (__dirname, __filename) are not available in ESM modules.
const __dirname = dirname(fileURLToPath(import.meta.url))

/**
 * Regression tests for unified search wiring in CommandPalette.
 *
 * These tests verify that:
 * 1. There is no isRegex branching — all queries go through useSearch
 * 2. useGrep is NOT imported or used in the main search flow
 * 3. SearchResults page uses unified API and searchEngine indicator
 *
 * Design principle: "One Endpoint Per Capability" (CLAUDE.md)
 */

describe('CommandPalette search wiring', () => {
  it('does not import useGrep', () => {
    const source = readFileSync(
      resolve(__dirname, 'CommandPalette.tsx'),
      'utf-8',
    )
    expect(source).not.toContain("from '../hooks/use-grep'")
    expect(source).not.toContain('useGrep')
  })

  it('does not use isRegex or hasRegexMetacharacters for routing', () => {
    const source = readFileSync(
      resolve(__dirname, 'CommandPalette.tsx'),
      'utf-8',
    )
    expect(source).not.toContain('isRegex')
    expect(source).not.toContain('hasRegexMetacharacters')
  })

  it('SearchResults page uses searchEngine indicator from unified API', () => {
    const source = readFileSync(
      resolve(__dirname, 'SearchResults.tsx'),
      'utf-8',
    )
    // Positive assertion: SearchResults DOES reference the searchEngine field
    // from the unified API response (grep fallback indicator)
    expect(source).toContain('searchEngine')
    // Negative assertion: no direct grep hook usage
    expect(source).not.toContain("from '../hooks/use-grep'")
  })
})
```

- [ ] **Step 2: Run the tests**

Run: `cd apps/web && bunx vitest run src/components/CommandPalette.search.test.tsx`
Expected: PASS (after Task 5 is completed).

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/CommandPalette.search.test.tsx
git commit -m "test(search): regression guard — no isRegex routing in CommandPalette

Static analysis tests verify that CommandPalette and SearchResults
use only useSearch (unified API), never useGrep or isRegex branching.
Guards the 'One Endpoint Per Capability' design principle."
```

---

## Summary

| Task | What | Type | Tests |
|------|------|------|-------|
| 1 | `search_engine` field on `SearchResponse` + TS codegen | Backend (modify) | Compile check + TS regen |
| 2 | Extract `collect_jsonl_files()` from grep handler | Backend (refactor) | Existing tests |
| 3 | `unified_search` engine with 8 unit tests | Backend (new file) | 8 unit + 1 helper test |
| 4 | Wire into `/api/search` handler (503 preserved) | Backend (modify) | Existing route tests |
| 5 | Remove `isRegex` gate + delete dead files | Frontend (simplify) | Task 8 guards |
| 6 | `searchEngine` grep indicator on SearchResults | Frontend (polish) | Build verification |
| 7 | Integration tests (CJK, mixed lang) | Backend tests | 3 integration tests |
| 8 | Frontend regression guard tests | Frontend tests | 3 static analysis tests |

**Total new tests: 14** (8 unit + 3 integration + 3 frontend regression guards)

**Files deleted by this plan:** `use-grep.ts`, `GrepResults.tsx`, `CommandPalette.regex.test.tsx` (all become dead code after Task 5). The `/api/grep` endpoint is preserved for power users.

**Net code change:** Backend gains ~250 lines (`unified.rs` + type changes), frontend loses ~80 lines (removed `isRegex`/`useGrep` wiring + 3 dead files). Net positive and the codebase gets simpler.

**Critical reviewer fixes incorporated:**

1. **503 preserved** — `search_index: None` returns 503, NOT grep fallback (Task 4)
2. **Offset/pagination** — grep ignores `offset` (no session-level pagination), documented via `search_engine: "grep"` (Task 3 docstring)
3. **Scope/project filter** — parsed from `scope` param and passed to `collect_jsonl_files` with polymorphic matching (Tasks 2 + 4)

**Post-implementation verification:**

1. `cargo test -p claude-view-search` — all search tests pass
2. `cargo test -p claude-view-server` — all server tests pass
3. `cd apps/web && bunx vitest run` — all frontend tests pass
4. `bun run build` — production build succeeds
5. Manual: open Cmd+K, type `hook 嘅 payload` → results appear
6. Manual: open Cmd+K, type `部署` → CJK results appear via grep fallback
7. Manual: navigate to `/search?q=deploy` → results render (same API)
8. Manual: restart server, immediately search → 503 (index building), NOT grep results

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | No ts-rs type regeneration step between Task 1 (Rust struct) and Task 6 (TS usage) — `searchEngine` would fail `tsc --noEmit` | Blocker | Added Step 4 to Task 1: `./scripts/generate-types.sh` + verify `searchEngine` in generated `.ts` |
| 2 | `CommandPalette.regex.test.tsx` imports `hasRegexMetacharacters` which Task 5 removes — breaks `vitest run` | Blocker | Added Step 2 to Task 5: explicitly delete `CommandPalette.regex.test.tsx` (Task 8 supersedes it) |
| 3 | Plan didn't enumerate all 4 `SearchResponse` construction sites in `query.rs` | Blocker | Added exact line numbers (~204, ~284, ~467, ~731) to Task 1 Step 2 |
| 4 | `use-grep.ts` becomes dead code after Task 5 — zero consumers remain | Warning | Added `git rm use-grep.ts` to Task 5 Step 5 with verification note |
| 5 | `GrepResults.tsx` becomes dead code after Task 5 — zero consumers remain | Warning | Added `git rm GrepResults.tsx` to Task 5 Step 5 with verification note |
| 6 | Task 2 silently fixes pre-existing polymorphic filter bug (`/api/grep` only checked `display_name`) | Warning | Updated Task 2 commit message to document the behavioral fix |
| 7 | `import.meta.url` path resolution untested in this Vitest suite — fragile | Warning | Replaced with `resolve(__dirname, 'CommandPalette.tsx')` pattern in Task 8 tests |
| 8 | Qualifier `project:foo` in `q` param is Tantivy-only, grep can't parse it | Warning | Added `Qualifier limitation` note to `unified_search` docstring in Task 3 |
| 9 | Task 6 line reference "around line 76" was off by one | Minor | Updated to "after closing `</p>` of stats paragraph (currently around line 77)" |
| 10 | Task 8 SearchResults `useGrep` test already passes before changes — guards nothing new | Minor | Changed to positive assertion: `expect(source).toContain('searchEngine')` |
| 11 | `__dirname` is CJS-only, doesn't exist in ESM — Task 8 tests would throw `ReferenceError` | Blocker | Added ESM-compatible `const __dirname = dirname(fileURLToPath(import.meta.url))` |
| 12 | `unwrap_or_default()` silently swallows `claude_projects_dir()` failure — user sees "no results" instead of error | Warning | Changed to `match` with `tracing::warn!` on error path |
| 13 | Task dependency ordering not explicit — parallel executor could compile-fail | Warning | Added "Task Dependencies" section with dependency graph and parallelization levels |
| 14 | Task 2 intro text said `-> Vec<JsonlFile>` but code returns `Result<Vec<JsonlFile>, ApiError>` | Minor | Fixed intro text to match actual function signature |
| 15 | Task 2 Step 2 ambiguous about what to delete in grep_handler | Warning | Made explicit: "DELETE the entire inline directory scan block" with scope boundaries |
| 16 | Task 4: `unified_search(Some(&search_index))` — `&Arc<SearchIndex>` ≠ `&SearchIndex`, compile error | Blocker | Changed to `Some(search_index.as_ref())` with explanatory comment |
| 17 | Task 4: unused `SearchEngine` import — CI clippy `-D warnings` fails on unused imports | Blocker | Removed `SearchEngine` from the import statement |
| 18 | Grep empty-files early return sets `search_engine: "grep"` even though grep didn't run — misleading UI | Warning | Changed to `search_engine: None` in the empty-files early return path |
| 19 | Task 6 Step 2 `tsc` will fail if type regen not done — cryptic error for executor | Minor | Added explicit "Requires: Task 1 Step 4" note before the `tsc` command |
