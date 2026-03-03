# Smart Search Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current fuzzy-OR-phrase search with a Google-level multi-signal query engine (phrase + BM25 + fuzzy simultaneously), add regex grep fallback, unified search input, and in-session Cmd+F.

**Architecture:** Multi-signal `BooleanQuery(Should)` in `query.rs` fires phrase, exact-term, and fuzzy queries simultaneously — documents matching more signals rank higher. Grep engine (ripgrep core) handles regex as a zero-result fallback. Frontend shares one `<SearchInput>` component across all contexts, differentiated by `scope` prop.

**Tech Stack:** Tantivy 0.22.1 (BoostQuery, PhraseQuery, FuzzyTermQuery, RangeQuery), grep-regex/grep-searcher/grep-matcher 0.1 (ripgrep core), React + TanStack Query, react-virtuoso.

**Design Doc:** `docs/plans/2026-02-28-smart-search-design.md`

---

## Task 1: Add grep crate dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root, line 28)
- Modify: `crates/search/Cargo.toml` (line 16)

**Step 1: Add grep crates to workspace dependencies**

In root `Cargo.toml`, add after `tantivy = "0.22"` (line 28):

```toml
# Grep (ripgrep core — regex search over raw JSONL)
grep-matcher = "0.1"
grep-regex = "0.1"
grep-searcher = "0.1"
```

**Step 2: Add grep crates to search crate**

In `crates/search/Cargo.toml`, add after `chrono = { workspace = true }` (line 16):

```toml
grep-matcher = { workspace = true }
grep-regex = { workspace = true }
grep-searcher = { workspace = true }
```

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-search`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add Cargo.toml crates/search/Cargo.toml
git commit -m "chore(search): add grep crate dependencies for regex search"
```

---

## Task 2: Multi-signal query engine

This is THE core change. Replace the mutually exclusive fuzzy/phrase branching in `query.rs` (lines 276-307) with a multi-signal `BooleanQuery` that fires phrase + exact + fuzzy simultaneously.

**Files:**
- Modify: `crates/search/src/query.rs` (lines 1-10 imports, lines 276-307 query construction)
- Test: existing tests in `crates/search/src/query.rs`

**Step 1: Add boost weight constants**

At the top of `query.rs`, after the existing imports (line 5), add `BoostQuery` and `PhraseQuery` to the tantivy import, and add the constants:

Change line 5 from:
```rust
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, Query, TermQuery};
```
to:
```rust
use tantivy::query::{BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, PhraseQuery, Query, TermQuery};
```

After the `truncate_utf8` function (after line 26), add:

```rust
/// Boost weights for multi-signal scoring.
/// Invariant: PHRASE > EXACT > FUZZY (verified by integration tests).
/// These are starting values — tune based on real session data.
const PHRASE_BOOST: f32 = 3.0;
const EXACT_BOOST: f32 = 1.5;
const FUZZY_BOOST: f32 = 0.5;
```

**Step 2: Write the failing test**

Add to the `#[cfg(test)] mod tests` block at the bottom of `query.rs`:

```rust
#[test]
fn test_multi_signal_ranks_phrase_above_fuzzy() {
    // Create an in-RAM index with two documents:
    // Doc A: contains exact phrase "deploy to production"
    // Doc B: contains all words but not as a phrase "production deploy to staging"
    let schema = crate::build_schema();
    let index = Index::create_in_ram(schema.clone());
    let content_field = schema.get_field("content").unwrap();
    let session_id_field = schema.get_field("session_id").unwrap();
    let project_field = schema.get_field("project").unwrap();
    let timestamp_field = schema.get_field("timestamp").unwrap();

    let mut writer = index.writer(10_000_000).unwrap();

    // Doc A: exact phrase
    writer.add_document(doc!(
        session_id_field => "session-a",
        project_field => "test",
        content_field => "we need to deploy to production tonight",
        timestamp_field => 1000i64,
    )).unwrap();

    // Doc B: words present but not adjacent
    writer.add_document(doc!(
        session_id_field => "session-b",
        project_field => "test",
        content_field => "production environment deploy scripts to run",
        timestamp_field => 2000i64,
    )).unwrap();

    writer.commit().unwrap();

    let reader = index.reader().unwrap();
    let si = crate::SearchIndex {
        index,
        reader,
        writer: std::sync::Mutex::new(writer),
        schema,
        needs_full_reindex: false,
        version_file_path: None,
        session_id_field,
        project_field,
        branch_field: schema.get_field("branch").unwrap(),
        model_field: schema.get_field("model").unwrap(),
        role_field: schema.get_field("role").unwrap(),
        content_field,
        turn_number_field: schema.get_field("turn_number").unwrap(),
        timestamp_field,
        skills_field: schema.get_field("skills").unwrap(),
    };

    let result = si.search("deploy to production", None, 10, 0).unwrap();
    assert!(result.total_sessions >= 2, "both sessions should match");

    // Session A (exact phrase) must rank above Session B (scattered terms)
    assert_eq!(result.sessions[0].session_id, "session-a",
        "exact phrase match should rank first");
    assert!(result.sessions[0].best_score > result.sessions[1].best_score,
        "phrase match score ({}) should exceed term match score ({})",
        result.sessions[0].best_score, result.sessions[1].best_score);
}

#[test]
fn test_fuzzy_catches_typos() {
    let schema = crate::build_schema();
    let index = Index::create_in_ram(schema.clone());
    let content_field = schema.get_field("content").unwrap();
    let session_id_field = schema.get_field("session_id").unwrap();
    let project_field = schema.get_field("project").unwrap();
    let timestamp_field = schema.get_field("timestamp").unwrap();

    let mut writer = index.writer(10_000_000).unwrap();
    writer.add_document(doc!(
        session_id_field => "session-typo",
        project_field => "test",
        content_field => "the deployment pipeline failed with timeout",
        timestamp_field => 1000i64,
    )).unwrap();
    writer.commit().unwrap();

    let reader = index.reader().unwrap();
    let si = crate::SearchIndex {
        index, reader,
        writer: std::sync::Mutex::new(writer),
        schema: schema.clone(),
        needs_full_reindex: false, version_file_path: None,
        session_id_field, project_field,
        branch_field: schema.get_field("branch").unwrap(),
        model_field: schema.get_field("model").unwrap(),
        role_field: schema.get_field("role").unwrap(),
        content_field,
        turn_number_field: schema.get_field("turn_number").unwrap(),
        timestamp_field,
        skills_field: schema.get_field("skills").unwrap(),
    };

    // "deploymnt" (typo) should still find "deployment" via fuzzy
    let result = si.search("deploymnt", None, 10, 0).unwrap();
    assert_eq!(result.total_sessions, 1, "fuzzy should catch single-char typo");
    assert_eq!(result.sessions[0].session_id, "session-typo");
}
```

**Step 3: Run tests to verify they fail**

Run: `cargo test -p claude-view-search -- test_multi_signal_ranks_phrase_above_fuzzy test_fuzzy_catches_typos --nocapture`
Expected: `test_multi_signal_ranks_phrase_above_fuzzy` FAILS (phrase match doesn't rank higher because current code only fires fuzzy, not phrase+fuzzy combined). `test_fuzzy_catches_typos` may pass (existing fuzzy already handles this).

**Step 4: Implement multi-signal query construction**

Replace lines 276-307 in `query.rs` (the `if !text_query.trim().is_empty()` block) with:

```rust
    if !text_query.trim().is_empty() {
        let trimmed = text_query.trim();
        let tokens: Vec<String> = trimmed
            .split_whitespace()
            .map(|t| t.to_lowercase())
            .collect();

        // Build multi-signal query: phrase + exact + fuzzy, all as Should.
        // At least one signal must match (BooleanQuery with only Should = OR semantics).
        let mut text_signals: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        // Signal 1: Exact phrase (highest weight, only for 2+ terms)
        if tokens.len() >= 2 {
            let phrase_terms: Vec<Term> = tokens
                .iter()
                .map(|t| Term::from_field_text(self.content_field, t))
                .collect();
            let phrase_query = PhraseQuery::new(phrase_terms);
            text_signals.push((
                Occur::Should,
                Box::new(BoostQuery::new(Box::new(phrase_query), PHRASE_BOOST)),
            ));
        }

        // Signal 2: All exact terms present (BM25 scored)
        {
            let exact_term_queries: Vec<(Occur, Box<dyn Query>)> = tokens
                .iter()
                .map(|t| {
                    let term = Term::from_field_text(self.content_field, t);
                    (Occur::Must, Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs)) as Box<dyn Query>)
                })
                .collect();
            let exact_query = BooleanQuery::new(exact_term_queries);
            text_signals.push((
                Occur::Should,
                Box::new(BoostQuery::new(Box::new(exact_query), EXACT_BOOST)),
            ));
        }

        // Signal 3: Fuzzy terms (typo tolerance, lowest weight)
        {
            let fuzzy_term_queries: Vec<(Occur, Box<dyn Query>)> = tokens
                .iter()
                .map(|t| {
                    let term = Term::from_field_text(self.content_field, t);
                    (Occur::Must, Box::new(FuzzyTermQuery::new(term, 1, true)) as Box<dyn Query>)
                })
                .collect();
            let fuzzy_query = BooleanQuery::new(fuzzy_term_queries);
            text_signals.push((
                Occur::Should,
                Box::new(BoostQuery::new(Box::new(fuzzy_query), FUZZY_BOOST)),
            ));
        }

        let text_query_combined = BooleanQuery::new(text_signals);
        sub_queries.push((Occur::Must, Box::new(text_query_combined)));
    }
```

Also add the necessary import at the top of the file (after the tantivy imports):

```rust
use tantivy::schema::IndexRecordOption;
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p claude-view-search -- test_multi_signal_ranks_phrase_above_fuzzy test_fuzzy_catches_typos --nocapture`
Expected: BOTH pass. Phrase match ranks above scattered terms. Fuzzy catches typos.

**Step 6: Run all existing search tests to check for regressions**

Run: `cargo test -p claude-view-search`
Expected: All existing tests pass. The multi-signal approach is a superset of the old behavior.

**Step 7: Commit**

```bash
git add crates/search/src/query.rs
git commit -m "feat(search): multi-signal query engine — phrase + BM25 + fuzzy combined"
```

---

## Task 3: Add session:, after:, before: qualifiers

**Files:**
- Modify: `crates/search/src/query.rs` (lines 63, 309-344)

**Step 1: Write failing tests for date qualifiers**

Add to tests in `query.rs`:

```rust
#[test]
fn test_after_before_date_qualifiers() {
    let schema = crate::build_schema();
    let index = Index::create_in_ram(schema.clone());
    let content_field = schema.get_field("content").unwrap();
    let session_id_field = schema.get_field("session_id").unwrap();
    let project_field = schema.get_field("project").unwrap();
    let timestamp_field = schema.get_field("timestamp").unwrap();

    let mut writer = index.writer(10_000_000).unwrap();

    // Jan 15 2026 = 1768435200 unix
    writer.add_document(doc!(
        session_id_field => "old-session",
        project_field => "test",
        content_field => "deploy the app",
        timestamp_field => 1768435200i64,
    )).unwrap();

    // Feb 15 2026 = 1771113600 unix
    writer.add_document(doc!(
        session_id_field => "new-session",
        project_field => "test",
        content_field => "deploy the app",
        timestamp_field => 1771113600i64,
    )).unwrap();

    writer.commit().unwrap();

    let reader = index.reader().unwrap();
    let si = crate::SearchIndex {
        index, reader,
        writer: std::sync::Mutex::new(writer),
        schema: schema.clone(),
        needs_full_reindex: false, version_file_path: None,
        session_id_field, project_field,
        branch_field: schema.get_field("branch").unwrap(),
        model_field: schema.get_field("model").unwrap(),
        role_field: schema.get_field("role").unwrap(),
        content_field,
        turn_number_field: schema.get_field("turn_number").unwrap(),
        timestamp_field,
        skills_field: schema.get_field("skills").unwrap(),
    };

    // after:2026-02-01 should only return new-session
    let result = si.search("deploy after:2026-02-01", None, 10, 0).unwrap();
    assert_eq!(result.total_sessions, 1);
    assert_eq!(result.sessions[0].session_id, "new-session");

    // before:2026-02-01 should only return old-session
    let result = si.search("deploy before:2026-02-01", None, 10, 0).unwrap();
    assert_eq!(result.total_sessions, 1);
    assert_eq!(result.sessions[0].session_id, "old-session");
}

#[test]
fn test_session_qualifier() {
    let schema = crate::build_schema();
    let index = Index::create_in_ram(schema.clone());
    let content_field = schema.get_field("content").unwrap();
    let session_id_field = schema.get_field("session_id").unwrap();
    let project_field = schema.get_field("project").unwrap();
    let timestamp_field = schema.get_field("timestamp").unwrap();

    let mut writer = index.writer(10_000_000).unwrap();
    writer.add_document(doc!(
        session_id_field => "aaa-111",
        project_field => "test",
        content_field => "hello world",
        timestamp_field => 1000i64,
    )).unwrap();
    writer.add_document(doc!(
        session_id_field => "bbb-222",
        project_field => "test",
        content_field => "hello world",
        timestamp_field => 2000i64,
    )).unwrap();
    writer.commit().unwrap();

    let reader = index.reader().unwrap();
    let si = crate::SearchIndex {
        index, reader,
        writer: std::sync::Mutex::new(writer),
        schema: schema.clone(),
        needs_full_reindex: false, version_file_path: None,
        session_id_field, project_field,
        branch_field: schema.get_field("branch").unwrap(),
        model_field: schema.get_field("model").unwrap(),
        role_field: schema.get_field("role").unwrap(),
        content_field,
        turn_number_field: schema.get_field("turn_number").unwrap(),
        timestamp_field,
        skills_field: schema.get_field("skills").unwrap(),
    };

    // session:aaa-111 should only return that session
    let result = si.search("hello", Some("session:aaa-111"), 10, 0).unwrap();
    assert_eq!(result.total_sessions, 1);
    assert_eq!(result.sessions[0].session_id, "aaa-111");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-search -- test_after_before_date_qualifiers test_session_qualifier --nocapture`
Expected: FAIL — `after`, `before`, `session` not in `known_keys`.

**Step 3: Implement new qualifiers**

In `query.rs`, change line 63 from:
```rust
let known_keys = ["project", "branch", "model", "role", "skill"];
```
to:
```rust
let known_keys = ["project", "branch", "model", "role", "skill", "session", "after", "before"];
```

Add import at the top of the file:
```rust
use chrono::NaiveDate;
use tantivy::query::RangeQuery;
use std::ops::Bound;
```

In the qualifier handling block (lines 309-344), add new match arms inside the `match qual.key.as_str()` block. After the existing `"skill"` arm and before `_ => continue`:

```rust
                "session" => {
                    // Scope to a specific session by ID
                    let term = Term::from_field_text(self.session_id_field, &qual.value);
                    sub_queries.push((Occur::Must, Box::new(TermQuery::new(term, IndexRecordOption::Basic))));
                    continue;
                }
                "after" => {
                    // Date filter: only messages after YYYY-MM-DD
                    if let Ok(date) = NaiveDate::parse_from_str(&qual.value, "%Y-%m-%d") {
                        let ts = date.and_hms_opt(0, 0, 0)
                            .and_then(|dt| Some(dt.and_utc().timestamp()))
                            .unwrap_or(0);
                        let range = RangeQuery::new_i64_bounds(
                            "timestamp".to_string(),
                            Bound::Excluded(ts),
                            Bound::Unbounded,
                        );
                        sub_queries.push((Occur::Must, Box::new(range)));
                    }
                    continue;
                }
                "before" => {
                    if let Ok(date) = NaiveDate::parse_from_str(&qual.value, "%Y-%m-%d") {
                        let ts = date.and_hms_opt(0, 0, 0)
                            .and_then(|dt| Some(dt.and_utc().timestamp()))
                            .unwrap_or(0);
                        let range = RangeQuery::new_i64_bounds(
                            "timestamp".to_string(),
                            Bound::Unbounded,
                            Bound::Excluded(ts),
                        );
                        sub_queries.push((Occur::Must, Box::new(range)));
                    }
                    continue;
                }
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p claude-view-search -- test_after_before_date_qualifiers test_session_qualifier --nocapture`
Expected: PASS.

**Step 5: Run full test suite**

Run: `cargo test -p claude-view-search`
Expected: All pass.

**Step 6: Commit**

```bash
git add crates/search/src/query.rs
git commit -m "feat(search): add session:, after:, before: qualifiers"
```

---

## Task 4: Recency tiebreaker

**Files:**
- Modify: `crates/search/src/query.rs` (lines 398-406, session sorting)

**Step 1: Write failing test**

```rust
#[test]
fn test_recency_tiebreaks_equal_scores() {
    let schema = crate::build_schema();
    let index = Index::create_in_ram(schema.clone());
    let content_field = schema.get_field("content").unwrap();
    let session_id_field = schema.get_field("session_id").unwrap();
    let project_field = schema.get_field("project").unwrap();
    let timestamp_field = schema.get_field("timestamp").unwrap();

    let mut writer = index.writer(10_000_000).unwrap();

    // Two sessions with identical content (identical BM25 scores)
    // but different timestamps
    writer.add_document(doc!(
        session_id_field => "old",
        project_field => "test",
        content_field => "identical content for scoring",
        timestamp_field => 1000i64,
    )).unwrap();
    writer.add_document(doc!(
        session_id_field => "new",
        project_field => "test",
        content_field => "identical content for scoring",
        timestamp_field => 9999i64,
    )).unwrap();
    writer.commit().unwrap();

    let reader = index.reader().unwrap();
    let si = crate::SearchIndex {
        index, reader,
        writer: std::sync::Mutex::new(writer),
        schema: schema.clone(),
        needs_full_reindex: false, version_file_path: None,
        session_id_field, project_field,
        branch_field: schema.get_field("branch").unwrap(),
        model_field: schema.get_field("model").unwrap(),
        role_field: schema.get_field("role").unwrap(),
        content_field,
        turn_number_field: schema.get_field("turn_number").unwrap(),
        timestamp_field,
        skills_field: schema.get_field("skills").unwrap(),
    };

    let result = si.search("identical content scoring", None, 10, 0).unwrap();
    assert_eq!(result.total_sessions, 2);
    // With identical scores, newer session should rank first
    assert_eq!(result.sessions[0].session_id, "new",
        "recency should tiebreak equal scores — newer first");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-search -- test_recency_tiebreaks_equal_scores --nocapture`
Expected: FAIL — current sorting only uses score, not timestamp.

**Step 3: Implement recency tiebreaker**

Replace the session sorting block (lines 398-406) with:

```rust
    // Sort sessions: primary by best_score descending, tiebreak by recency.
    // When scores are within 10% of each other, newer session wins.
    session_entries.sort_by(|a, b| {
        let best_a = a.1.iter().map(|(s, _)| *s).fold(f32::NEG_INFINITY, f32::max);
        let best_b = b.1.iter().map(|(s, _)| *s).fold(f32::NEG_INFINITY, f32::max);

        let (hi, lo) = if best_a >= best_b { (best_a, best_b) } else { (best_b, best_a) };
        let scores_close = hi == 0.0 || (lo / hi) > 0.9;

        if scores_close {
            // Tiebreak by timestamp: need to find max timestamp per session
            let ts_a = a.1.iter().filter_map(|(_, addr)| {
                searcher.doc(*addr).ok().and_then(|d| {
                    d.get_first(self.timestamp_field).and_then(|v| v.as_i64())
                })
            }).max().unwrap_or(0);
            let ts_b = b.1.iter().filter_map(|(_, addr)| {
                searcher.doc(*addr).ok().and_then(|d| {
                    d.get_first(self.timestamp_field).and_then(|v| v.as_i64())
                })
            }).max().unwrap_or(0);
            ts_b.cmp(&ts_a) // newer first
        } else {
            best_b.partial_cmp(&best_a).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
```

**Step 4: Run tests**

Run: `cargo test -p claude-view-search -- test_recency_tiebreaks_equal_scores --nocapture`
Expected: PASS.

**Step 5: Run full suite**

Run: `cargo test -p claude-view-search`
Expected: All pass.

**Step 6: Commit**

```bash
git add crates/search/src/query.rs
git commit -m "feat(search): recency tiebreaker for equal-score sessions"
```

---

## Task 5: Fix snippet generation

Replace the `QueryParser` re-parse approach with explicit `PhraseQuery`/`TermQuery` construction for snippets. This makes the snippet query consistent with the search query terms (minus the fuzzy signal, which Tantivy's `SnippetGenerator` can't handle — `FuzzyTermQuery::query_terms()` is a no-op).

**Files:**
- Modify: `crates/search/src/query.rs` (lines 388-395)

**Step 1: Replace snippet generator construction**

Replace lines 388-395 (the `snippet_gen` block) with:

```rust
    // Build snippet query from original search terms (PhraseQuery + TermQuery).
    // IMPORTANT: Do NOT use FuzzyTermQuery for snippets — Tantivy's
    // SnippetGenerator calls query_terms() which is a no-op for FuzzyTermQuery
    // (inherits empty default from Query trait). Only exact terms highlight.
    let snippet_gen = if !text_query.trim().is_empty() {
        let tokens: Vec<String> = text_query.trim()
            .split_whitespace()
            .map(|t| t.to_lowercase())
            .collect();

        let snippet_query: Box<dyn Query> = if tokens.len() >= 2 {
            // PhraseQuery highlights exact phrase occurrences
            let phrase_terms: Vec<Term> = tokens
                .iter()
                .map(|t| Term::from_field_text(self.content_field, t))
                .collect();
            Box::new(PhraseQuery::new(phrase_terms))
        } else {
            // Single term
            let term = Term::from_field_text(self.content_field, &tokens[0]);
            Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs))
        };

        SnippetGenerator::create(&searcher, &*snippet_query, self.content_field).ok()
    } else {
        None
    };
```

**Step 2: Run full test suite**

Run: `cargo test -p claude-view-search`
Expected: All pass. Snippets now use explicit term construction instead of QueryParser re-parse.

**Step 3: Commit**

```bash
git add crates/search/src/query.rs
git commit -m "fix(search): explicit snippet query — avoid FuzzyTermQuery no-op in SnippetGenerator"
```

---

## Task 6: Create grep response types

**Files:**
- Create: `crates/search/src/grep_types.rs`
- Modify: `crates/search/src/lib.rs` (line 18, add module declaration)

**Step 1: Create grep types file**

```rust
use serde::Serialize;
use ts_rs::TS;

/// Response from the `/api/grep` endpoint (regex search over raw JSONL files).
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct GrepResponse {
    pub pattern: String,
    pub total_matches: usize,
    pub total_sessions: usize,
    pub elapsed_ms: f64,
    pub truncated: bool,
    pub results: Vec<GrepSessionHit>,
}

/// One session that matched the grep pattern.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct GrepSessionHit {
    pub session_id: String,
    pub project: String,
    pub project_path: String,
    pub modified_at: i64,
    pub matches: Vec<GrepLineMatch>,
}

/// One matching line within a session JSONL file.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../src/types/generated/")]
pub struct GrepLineMatch {
    pub line_number: usize,
    pub content: String,
    pub match_start: usize,
    pub match_end: usize,
}
```

**Step 2: Add module to lib.rs**

In `crates/search/src/lib.rs`, after line 18 (`pub mod types;`), add:

```rust
pub mod grep_types;
```

And add to the re-exports (after line 26):

```rust
pub use grep_types::{GrepResponse, GrepSessionHit, GrepLineMatch};
```

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-search`
Expected: Compiles.

**Step 4: Generate TypeScript types**

Run: `cargo test -p claude-view-search -- --ignored ts_export 2>/dev/null; true`

If auto-generation doesn't produce files, create them manually:

Create `src/types/generated/GrepResponse.ts`:
```typescript
// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { GrepSessionHit } from "./GrepSessionHit";

export type GrepResponse = {
  pattern: string
  totalMatches: number
  totalSessions: number
  elapsedMs: number
  truncated: boolean
  results: GrepSessionHit[]
}
```

Create `src/types/generated/GrepSessionHit.ts`:
```typescript
// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { GrepLineMatch } from "./GrepLineMatch";

export type GrepSessionHit = {
  sessionId: string
  project: string
  projectPath: string
  modifiedAt: number
  matches: GrepLineMatch[]
}
```

Create `src/types/generated/GrepLineMatch.ts`:
```typescript
// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
export type GrepLineMatch = {
  lineNumber: number
  content: string
  matchStart: number
  matchEnd: number
}
```

**Step 5: Add exports to index.ts**

In `src/types/generated/index.ts`, add at the bottom:

```typescript
// Grep search types
export type { GrepResponse } from './GrepResponse'
export type { GrepSessionHit } from './GrepSessionHit'
export type { GrepLineMatch } from './GrepLineMatch'
```

**Step 6: Commit**

```bash
git add crates/search/src/grep_types.rs crates/search/src/lib.rs \
  src/types/generated/GrepResponse.ts src/types/generated/GrepSessionHit.ts \
  src/types/generated/GrepLineMatch.ts src/types/generated/index.ts
git commit -m "feat(search): add grep response types with ts-rs exports"
```

---

## Task 7: Implement grep engine

**Files:**
- Create: `crates/search/src/grep.rs`
- Modify: `crates/search/src/lib.rs` (add module + re-export)

**Step 1: Write failing test**

At the bottom of the new `grep.rs` file, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_jsonl(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_grep_finds_pattern_in_jsonl() {
        let tmp = TempDir::new().unwrap();
        let file = create_test_jsonl(
            tmp.path(), "test.jsonl",
            "{\"type\":\"user\",\"message\":\"deploy to production\"}\n\
             {\"type\":\"assistant\",\"message\":\"running deploy script\"}\n\
             {\"type\":\"user\",\"message\":\"check the logs\"}\n"
        );

        let opts = GrepOptions {
            pattern: "deploy".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 100,
        };

        let files = vec![JsonlFile {
            path: file,
            session_id: "test-session".to_string(),
            project: "alpha".to_string(),
            project_path: tmp.path().to_string_lossy().to_string(),
            modified_at: 1000,
        }];

        let result = grep_files(&files, &opts).unwrap();
        assert_eq!(result.total_matches, 2);
        assert_eq!(result.results.len(), 1);
        assert_eq!(result.results[0].matches.len(), 2);
    }

    #[test]
    fn test_grep_case_sensitive() {
        let tmp = TempDir::new().unwrap();
        let file = create_test_jsonl(
            tmp.path(), "test.jsonl",
            "{\"message\":\"Deploy\"}\n{\"message\":\"deploy\"}\n"
        );

        let opts = GrepOptions {
            pattern: "Deploy".to_string(),
            case_sensitive: true,
            whole_word: false,
            limit: 100,
        };

        let files = vec![JsonlFile {
            path: file,
            session_id: "s1".to_string(),
            project: "alpha".to_string(),
            project_path: tmp.path().to_string_lossy().to_string(),
            modified_at: 1000,
        }];

        let result = grep_files(&files, &opts).unwrap();
        assert_eq!(result.total_matches, 1, "case sensitive should match only 'Deploy'");
    }
}
```

**Step 2: Implement grep engine**

Create `crates/search/src/grep.rs`:

```rust
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{Searcher, Sink, SinkMatch};

use crate::grep_types::{GrepLineMatch, GrepResponse, GrepSessionHit};

/// Options for a grep search.
pub struct GrepOptions {
    pub pattern: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub limit: usize,
}

/// Metadata for one JSONL file to search.
pub struct JsonlFile {
    pub path: PathBuf,
    pub session_id: String,
    pub project: String,
    pub project_path: String,
    pub modified_at: i64,
}

/// Search raw JSONL files for a regex pattern using ripgrep core crates.
pub fn grep_files(files: &[JsonlFile], opts: &GrepOptions) -> Result<GrepResponse, GrepError> {
    if files.is_empty() {
        return Ok(GrepResponse {
            pattern: opts.pattern.clone(),
            total_matches: 0,
            total_sessions: 0,
            elapsed_ms: 0.0,
            truncated: false,
            results: vec![],
        });
    }

    let start = std::time::Instant::now();

    // Validate regex upfront — fail fast on invalid patterns
    RegexMatcherBuilder::new()
        .case_insensitive(!opts.case_sensitive)
        .word(opts.whole_word)
        .build(&opts.pattern)
        .map_err(|e| GrepError::InvalidPattern(e.to_string()))?;

    let total_matches = AtomicUsize::new(0);
    let limit_reached = AtomicBool::new(false);
    let session_hits: std::sync::Mutex<Vec<GrepSessionHit>> = std::sync::Mutex::new(Vec::new());

    let parallelism = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    // Clone pattern + options for per-thread matcher construction.
    // Each thread builds its own RegexMatcher to avoid contention on internal
    // mutable state — even though RegexMatcher is Sync, per-thread construction
    // is cheaper than sharing and avoids lock overhead.
    let pattern = opts.pattern.clone();
    let case_sensitive = opts.case_sensitive;
    let whole_word = opts.whole_word;

    std::thread::scope(|scope| {
        let chunk_size = (files.len() + parallelism - 1) / parallelism;
        let chunks: Vec<&[JsonlFile]> = files.chunks(chunk_size.max(1)).collect();

        for chunk in chunks {
            let pattern = pattern.clone();
            let total_matches = &total_matches;
            let limit_reached = &limit_reached;
            let session_hits = &session_hits;
            let limit = opts.limit;

            scope.spawn(move || {
                // Each thread builds its own RegexMatcher — avoids contention
                let matcher = RegexMatcherBuilder::new()
                    .case_insensitive(!case_sensitive)
                    .word(whole_word)
                    .build(&pattern)
                    .expect("pattern already validated above");

                for file in chunk {
                    if limit_reached.load(Ordering::Relaxed) {
                        break;
                    }

                    let mut line_matches: Vec<GrepLineMatch> = Vec::new();

                    let _ = Searcher::new().search_path(
                        &matcher,
                        &file.path,
                        MatchCollector {
                            matches: &mut line_matches,
                            matcher: &matcher,
                            limit,
                            total_matches,
                            limit_reached,
                        },
                    );

                    if !line_matches.is_empty() {
                        let count = line_matches.len();
                        let hit = GrepSessionHit {
                            session_id: file.session_id.clone(),
                            project: file.project.clone(),
                            project_path: file.project_path.clone(),
                            modified_at: file.modified_at,
                            matches: line_matches,
                        };
                        session_hits.lock().unwrap().push(hit);
                        total_matches.fetch_add(count, Ordering::Relaxed);
                    }
                }
            });
        }
    });

    let total = total_matches.load(Ordering::Relaxed);
    let truncated = limit_reached.load(Ordering::Relaxed);
    let mut results = session_hits.into_inner().unwrap();
    results.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

    Ok(GrepResponse {
        pattern: opts.pattern.clone(),
        total_matches: total,
        total_sessions: results.len(),
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
        truncated,
        results,
    })
}

/// Sink implementation that collects grep matches.
struct MatchCollector<'a, M: Matcher> {
    matches: &'a mut Vec<GrepLineMatch>,
    matcher: &'a M,
    limit: usize,
    total_matches: &'a AtomicUsize,
    limit_reached: &'a AtomicBool,
}

impl<'a, M: Matcher> Sink for MatchCollector<'a, M> {
    type Error = std::io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        if self.limit_reached.load(Ordering::Relaxed) {
            return Ok(false);
        }

        let line_content = String::from_utf8_lossy(mat.bytes());
        // UTF-8 safe truncation
        let truncated = if line_content.len() > 500 {
            let end = line_content.char_indices().nth(500).map(|(i, _)| i).unwrap_or(line_content.len());
            format!("{}...", &line_content[..end])
        } else {
            line_content.to_string()
        };

        // Find match positions within the line
        let mut match_start = 0;
        let mut match_end = 0;
        if let Ok(Some(m)) = self.matcher.find(mat.bytes()) {
            match_start = m.start();
            match_end = m.end();
        }

        self.matches.push(GrepLineMatch {
            line_number: mat.line_number().unwrap_or(0) as usize,
            content: truncated,
            match_start,
            match_end,
        });

        if self.total_matches.load(Ordering::Relaxed) + self.matches.len() >= self.limit {
            self.limit_reached.store(true, Ordering::Relaxed);
            return Ok(false);
        }

        Ok(true)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GrepError {
    #[error("Invalid regex pattern: {0}")]
    InvalidPattern(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

**Step 3: Add module to lib.rs**

In `crates/search/src/lib.rs`, after the `grep_types` module declaration, add:

```rust
pub mod grep;
```

And add to re-exports:

```rust
pub use grep::{grep_files, GrepOptions, JsonlFile, GrepError};
```

**Step 4: Run tests**

Run: `cargo test -p claude-view-search -- grep --nocapture`
Expected: Both grep tests pass.

**Step 5: Commit**

```bash
git add crates/search/src/grep.rs crates/search/src/lib.rs
git commit -m "feat(search): grep engine using ripgrep core crates"
```

---

## Task 8: Grep API route

**Files:**
- Create: `crates/server/src/routes/grep.rs`
- Modify: `crates/server/src/routes/mod.rs` (add route)

**Step 1: Create grep route handler**

Create `crates/server/src/routes/grep.rs`:

```rust
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use tokio::task::spawn_blocking;

use claude_view_core::discovery::{claude_projects_dir, resolve_project_path_with_cwd};
use claude_view_search::grep_types::GrepResponse;
use claude_view_search::{grep_files, GrepOptions, JsonlFile};

use crate::error::{ApiError, ApiResult};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct GrepQuery {
    pub pattern: Option<String>,
    pub project: Option<String>,
    pub limit: Option<usize>,
    #[serde(rename = "caseSensitive")]
    pub case_sensitive: Option<bool>,
    #[serde(rename = "wholeWord")]
    pub whole_word: Option<bool>,
}

pub async fn grep_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GrepQuery>,
) -> ApiResult<Json<GrepResponse>> {
    let pattern = params.pattern
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| ApiError::BadRequest("Missing 'pattern' parameter".into()))?;

    let limit = params.limit.unwrap_or(200).min(1000);
    let case_sensitive = params.case_sensitive.unwrap_or(false);
    let whole_word = params.whole_word.unwrap_or(false);

    let projects_dir = claude_projects_dir()?;

    // Collect JSONL files to search
    let mut files: Vec<JsonlFile> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let project_dir = entry.path();
            if !project_dir.is_dir() {
                continue;
            }

            let dir_name = entry.file_name().to_string_lossy().to_string();
            let resolved = resolve_project_path_with_cwd(&dir_name, None);

            // Filter by project if specified
            if let Some(ref proj) = params.project {
                if resolved.display_name != *proj {
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
                        let modified_at = path.metadata()
                            .and_then(|m| m.modified())
                            .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64)
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

    let opts = GrepOptions {
        pattern,
        case_sensitive,
        whole_word,
        limit,
    };

    let result = spawn_blocking(move || grep_files(&files, &opts))
        .await
        .map_err(|e| ApiError::Internal(format!("Grep task failed: {e}")))?
        .map_err(|e| ApiError::BadRequest(format!("{e}")))?;

    Ok(Json(result))
}
```

**Step 2: Register the route**

In `crates/server/src/routes/mod.rs`, add module declaration (after line 22):

```rust
pub mod grep;
```

Add the grep route to the router. Find the `.nest("/api", search::router())` line (line 118) and change it to:

```rust
.nest("/api", search::router()
    .route("/grep", axum::routing::get(grep::grep_handler))
)
```

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-server`
Expected: Compiles.

**Step 4: Commit**

```bash
git add crates/server/src/routes/grep.rs crates/server/src/routes/mod.rs
git commit -m "feat(search): add /api/grep endpoint for regex search"
```

---

## Task 9: Frontend useGrep hook + regex fallback

**Files:**
- Create: `src/hooks/use-grep.ts`
- Modify: `src/hooks/use-search.ts` (add regex fallback)

**Step 1: Create useGrep hook**

Create `src/hooks/use-grep.ts`:

```typescript
import { useQuery } from '@tanstack/react-query'
import { useState, useEffect } from 'react'
import type { GrepResponse } from '../types/generated'

interface UseGrepOptions {
  caseSensitive?: boolean
  wholeWord?: boolean
  project?: string
  limit?: number
  enabled?: boolean
}

export function useGrep(pattern: string, options: UseGrepOptions = {}) {
  const {
    caseSensitive = false,
    wholeWord = false,
    project,
    limit = 200,
    enabled = true,
  } = options

  // 300ms debounce
  const [debouncedPattern, setDebouncedPattern] = useState(pattern)
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedPattern(pattern), 300)
    return () => clearTimeout(timer)
  }, [pattern])

  const queryResult = useQuery<GrepResponse>({
    queryKey: ['grep', debouncedPattern, caseSensitive, wholeWord, project, limit],
    queryFn: async () => {
      const params = new URLSearchParams()
      params.set('pattern', debouncedPattern)
      params.set('limit', String(limit))
      if (caseSensitive) params.set('caseSensitive', 'true')
      if (wholeWord) params.set('wholeWord', 'true')
      if (project) params.set('project', project)
      const res = await fetch(`/api/grep?${params}`)
      if (!res.ok) throw new Error(await res.text())
      return res.json()
    },
    enabled: enabled && debouncedPattern.trim().length > 0,
    staleTime: 30_000,
    gcTime: 5 * 60_000,
  })

  return {
    ...queryResult,
    debouncedPattern,
    isDebouncing: pattern !== debouncedPattern,
  }
}
```

**Step 2: Add regex fallback to useSearch**

In `src/hooks/use-search.ts`, add a helper function and modify the return:

```typescript
// Add at bottom of file, before the final export or after the hook:

/** Detect regex metacharacters for grep fallback. */
export function hasRegexMetacharacters(input: string): boolean {
  const patterns = ['.*', '\\b', '\\d', '\\w', '\\s', '[a-', '(?:', '^$']
  return patterns.some(p => input.includes(p))
}
```

The regex fallback logic will live in the components that use `useSearch` — when `useSearch` returns 0 results and `hasRegexMetacharacters(query)` is true, the component calls `useGrep`. This keeps the hooks independent and testable.

**Step 3: Commit**

```bash
git add src/hooks/use-grep.ts src/hooks/use-search.ts
git commit -m "feat(search): add useGrep hook and regex fallback detection"
```

---

## Task 10: GrepResults component

**Files:**
- Create: `src/components/GrepResults.tsx`

**Step 1: Create component**

```tsx
import { useState, useMemo } from 'react'
import { ChevronDown, ChevronRight, FileText } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import type { GrepResponse, GrepSessionHit, GrepLineMatch } from '../types/generated'

interface GrepResultsProps {
  data: GrepResponse
}

export function GrepResults({ data }: GrepResultsProps) {
  const navigate = useNavigate()

  if (data.results.length === 0) {
    return (
      <div className="p-4 text-center text-slate-500 dark:text-slate-400">
        No regex matches found
      </div>
    )
  }

  return (
    <div className="py-2">
      <p className="px-4 py-1 text-xs font-medium text-slate-400 dark:text-slate-500 uppercase tracking-wider">
        {data.totalSessions} {data.totalSessions === 1 ? 'session' : 'sessions'}, {data.totalMatches} {data.totalMatches === 1 ? 'match' : 'matches'}
        <span className="ml-2 normal-case tracking-normal">({data.elapsedMs.toFixed(1)}ms)</span>
        {data.truncated && <span className="ml-2 text-amber-500">(truncated)</span>}
      </p>
      <p className="px-4 py-1 text-xs text-slate-400 dark:text-slate-500">
        Showing regex matches
      </p>
      <div className="px-3 py-1 space-y-1">
        {data.results.map((hit) => (
          <GrepSessionCard
            key={hit.sessionId}
            hit={hit}
            onNavigate={(sessionId) => navigate(`/sessions/${sessionId}`)}
          />
        ))}
      </div>
    </div>
  )
}

function GrepSessionCard({
  hit,
  onNavigate,
}: {
  hit: GrepSessionHit
  onNavigate: (sessionId: string) => void
}) {
  const [expanded, setExpanded] = useState(false)
  const previewMatches = useMemo(() => hit.matches.slice(0, 3), [hit.matches])
  const hasMore = hit.matches.length > 3

  return (
    <div className="rounded-lg border border-slate-200/80 dark:border-white/[0.06] bg-white dark:bg-white/[0.02] overflow-hidden">
      <button
        onClick={() => onNavigate(hit.sessionId)}
        className="w-full px-3 py-2 flex items-center gap-2 text-left hover:bg-slate-50 dark:hover:bg-white/[0.03] transition-colors"
      >
        <FileText className="w-4 h-4 text-slate-400 flex-shrink-0" />
        <span className="text-sm font-medium text-slate-700 dark:text-slate-200 truncate">
          {hit.project}
        </span>
        <span className="text-xs text-slate-400 dark:text-slate-500 ml-auto flex-shrink-0">
          {hit.matches.length} {hit.matches.length === 1 ? 'match' : 'matches'}
        </span>
      </button>

      <div className="border-t border-slate-100 dark:border-white/[0.04]">
        {(expanded ? hit.matches : previewMatches).map((match, i) => (
          <div
            key={`${match.lineNumber}-${i}`}
            className="px-3 py-1 text-xs font-mono text-slate-600 dark:text-slate-300 border-b border-slate-50 dark:border-white/[0.02] last:border-b-0 hover:bg-slate-50/50 dark:hover:bg-white/[0.02]"
          >
            <span className="text-slate-400 dark:text-slate-500 mr-2 select-none">
              {match.lineNumber}:
            </span>
            <HighlightedLine content={match.content} start={match.matchStart} end={match.matchEnd} />
          </div>
        ))}

        {hasMore && (
          <button
            onClick={() => setExpanded(!expanded)}
            className="w-full px-3 py-1.5 text-xs text-emerald-600 dark:text-emerald-400 hover:bg-slate-50 dark:hover:bg-white/[0.03] flex items-center gap-1 transition-colors"
          >
            {expanded ? (
              <><ChevronDown className="w-3 h-3" /> Show fewer</>
            ) : (
              <><ChevronRight className="w-3 h-3" /> Show {hit.matches.length - 3} more</>
            )}
          </button>
        )}
      </div>
    </div>
  )
}

function HighlightedLine({ content, start, end }: { content: string; start: number; end: number }) {
  if (start === end || start >= content.length) {
    return <span>{content}</span>
  }
  const safeEnd = Math.min(end, content.length)
  return (
    <span>
      {content.slice(0, start)}
      <mark className="bg-amber-200 dark:bg-amber-900/50 text-amber-900 dark:text-amber-200 rounded-sm px-0.5">
        {content.slice(start, safeEnd)}
      </mark>
      {content.slice(safeEnd)}
    </span>
  )
}
```

**Step 2: Commit**

```bash
git add src/components/GrepResults.tsx
git commit -m "feat(search): add GrepResults component for regex match display"
```

---

## Task 11: Shared SearchInput component

Extract the search input into a reusable component used by CommandPalette, Header, and ConversationView.

**Files:**
- Create: `src/components/SearchInput.tsx`

**Step 1: Create shared component**

```tsx
import { forwardRef } from 'react'
import { Search, X } from 'lucide-react'

interface SearchInputProps {
  value: string
  onChange: (value: string) => void
  placeholder?: string
  autoFocus?: boolean
  shortcutHint?: string
  matchInfo?: { current: number; total: number }
  onPrev?: () => void
  onNext?: () => void
  onClose?: () => void
  onKeyDown?: (e: React.KeyboardEvent) => void
  className?: string
}

export const SearchInput = forwardRef<HTMLInputElement, SearchInputProps>(
  function SearchInput(
    {
      value,
      onChange,
      placeholder = 'Search conversations...',
      autoFocus = false,
      shortcutHint,
      matchInfo,
      onPrev,
      onNext,
      onClose,
      onKeyDown,
      className = '',
    },
    ref,
  ) {
    return (
      <div className={`flex items-center gap-2 ${className}`}>
        <Search className="w-4 h-4 text-slate-400 dark:text-slate-500 flex-shrink-0" />
        <input
          ref={ref}
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          autoFocus={autoFocus}
          onKeyDown={onKeyDown}
          className="flex-1 bg-transparent text-sm text-slate-900 dark:text-slate-100 placeholder:text-slate-400 dark:placeholder:text-slate-500 outline-none"
        />
        {matchInfo && matchInfo.total > 0 && (
          <div className="flex items-center gap-1 text-xs text-slate-500 dark:text-slate-400 flex-shrink-0">
            <span>{matchInfo.current} of {matchInfo.total}</span>
            {onPrev && (
              <button onClick={onPrev} className="p-0.5 hover:text-slate-700 dark:hover:text-slate-200" title="Previous match (Shift+Enter)">
                ▲
              </button>
            )}
            {onNext && (
              <button onClick={onNext} className="p-0.5 hover:text-slate-700 dark:hover:text-slate-200" title="Next match (Enter)">
                ▼
              </button>
            )}
          </div>
        )}
        {shortcutHint && !value && (
          <kbd className="text-xs text-slate-400 dark:text-slate-500 bg-slate-100 dark:bg-white/[0.06] px-1.5 py-0.5 rounded flex-shrink-0">
            {shortcutHint}
          </kbd>
        )}
        {value && onClose && (
          <button
            onClick={() => { onChange(''); onClose?.() }}
            className="p-0.5 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 flex-shrink-0"
          >
            <X className="w-4 h-4" />
          </button>
        )}
      </div>
    )
  },
)
```

**Step 2: Commit**

```bash
git add src/components/SearchInput.tsx
git commit -m "feat(search): shared SearchInput component"
```

---

## Task 12: ConversationView Cmd+F

**Files:**
- Modify: `src/components/ConversationView.tsx`

**Step 1: Add imports**

At the top of `ConversationView.tsx`, add:

```tsx
import { SearchInput } from './SearchInput'
import { useSearch } from '../hooks/use-search'
import type { VirtuosoHandle } from 'react-virtuoso'
```

**Step 2: Add state and hook — AFTER filteredMessages (line 263), BEFORE early returns (line 350)**

After `const hiddenCount = allMessages.length - filteredMessages.length` (line 263), add:

```tsx
  // In-session search (Cmd+F) — uses same smart search engine scoped to this session
  const [inSessionQuery, setInSessionQuery] = useState('')
  const [inSessionOpen, setInSessionOpen] = useState(false)
  const [activeMatchIndex, setActiveMatchIndex] = useState(0)

  const sessionScope = session?.session_id ? `session:${session.session_id}` : undefined
  const inSessionResults = useSearch(inSessionQuery, {
    scope: sessionScope,
    limit: 100,
    enabled: inSessionOpen && !!sessionScope,
  })

  const matchCount = inSessionResults.data?.totalMatches ?? 0
  const matchedTurns = useMemo(() => {
    if (!inSessionResults.data) return []
    return inSessionResults.data.sessions
      .flatMap(s => s.matches)
      .map(m => Number(m.turnNumber))
      .sort((a, b) => a - b)
  }, [inSessionResults.data])

  const virtuosoRef = useRef<VirtuosoHandle>(null)
```

**Step 3: Add Cmd+F keyboard handler**

In the existing `handleKeyDown` useEffect (lines 200-219), add inside the `handleKeyDown` function body:

```tsx
    // Cmd+F / Ctrl+F: open in-session search
    if (modifierKey && e.key.toLowerCase() === 'f') {
      e.preventDefault()
      setInSessionOpen(true)
    }
    // Escape: close in-session search
    if (e.key === 'Escape' && inSessionOpen) {
      e.preventDefault()
      setInSessionOpen(false)
      setInSessionQuery('')
      setActiveMatchIndex(0)
    }
```

Add `inSessionOpen` to the useEffect dependency array.

**Step 4: Add scroll-to-match effect**

After the search state block (after `virtuosoRef`), add:

```tsx
  // Scroll to active match
  useEffect(() => {
    if (matchedTurns.length === 0 || !virtuosoRef.current) return
    const turnNumber = matchedTurns[activeMatchIndex]
    if (turnNumber === undefined) return
    // Find the index of this turn in filteredMessages
    const msgIndex = filteredMessages.findIndex(
      (m) => Number(m.index ?? 0) === turnNumber
    )
    if (msgIndex >= 0) {
      // scrollToIndex needs the VIRTUAL index because ConversationView uses firstItemIndex
      virtuosoRef.current.scrollToIndex({
        index: firstItemIndex + msgIndex,
        behavior: 'smooth',
        align: 'center',
      })
    }
  }, [activeMatchIndex, matchedTurns, filteredMessages, firstItemIndex])
```

**Step 5: Add search bar UI**

Find the Virtuoso component (`<Virtuoso` at approximately line 618). Add the search bar just before it, inside the same parent `div`:

```tsx
  {inSessionOpen && (
    <div className="sticky top-0 z-10 bg-white/95 dark:bg-slate-900/95 backdrop-blur border-b border-slate-200 dark:border-white/[0.06] px-4 py-2">
      <SearchInput
        value={inSessionQuery}
        onChange={setInSessionQuery}
        autoFocus
        matchInfo={matchCount > 0 ? { current: activeMatchIndex + 1, total: matchCount } : undefined}
        onPrev={() => setActiveMatchIndex((i) => (i - 1 + matchedTurns.length) % matchedTurns.length)}
        onNext={() => setActiveMatchIndex((i) => (i + 1) % matchedTurns.length)}
        onClose={() => {
          setInSessionOpen(false)
          setInSessionQuery('')
          setActiveMatchIndex(0)
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault()
            setActiveMatchIndex((i) => (i + 1) % matchedTurns.length)
          }
          if (e.key === 'Enter' && e.shiftKey) {
            e.preventDefault()
            setActiveMatchIndex((i) => (i - 1 + matchedTurns.length) % matchedTurns.length)
          }
        }}
      />
    </div>
  )}
```

**Step 6: Add ref to Virtuoso**

On the `<Virtuoso` component (line 618), add the `ref` prop:

```tsx
<Virtuoso
  ref={virtuosoRef}
  data={filteredMessages}
  ...
```

**Step 7: Handle URL param auto-open**

At the top of the component, after the existing `useSearchParams` call, add:

```tsx
const qParam = searchParams.get('q')
useEffect(() => {
  if (qParam) {
    setInSessionOpen(true)
    setInSessionQuery(qParam)
  }
}, [qParam])
```

**Step 8: Commit**

```bash
git add src/components/ConversationView.tsx
git commit -m "feat(search): in-session Cmd+F using smart search with session: qualifier"
```

---

## Task 13: CommandPalette regex fallback + scrollable results

**Files:**
- Modify: `src/components/CommandPalette.tsx`

**Step 1: Add imports**

```tsx
import { useGrep } from '../hooks/use-grep'
import { hasRegexMetacharacters } from '../hooks/use-search'
import { GrepResults } from './GrepResults'
```

**Step 2: Add useGrep hook call**

After the `useSearch` hook call (line 70), add:

```tsx
  // Regex fallback: fires only when smart search returns 0 results AND input looks like regex
  const shouldTryGrep = hasRegexMetacharacters(query) &&
    !isSearching && !isDebouncing &&
    searchResults?.totalSessions === 0 &&
    query.trim().length > 0
  const grepResult = useGrep(query, { enabled: shouldTryGrep })
```

**Step 3: Update results rendering**

Replace the `{hasLiveResults && (...)}` block (lines 441-469) with:

```tsx
{grepResult.data && grepResult.data.totalMatches > 0 ? (
  <GrepResults data={grepResult.data} />
) : hasLiveResults ? (
  <div className="py-2 border-b border-slate-200/80 dark:border-white/[0.06]">
    <p className="px-4 py-1 text-xs font-medium text-slate-400 dark:text-slate-500 uppercase tracking-wider">
      {searchResults.totalSessions} {searchResults.totalSessions === 1 ? 'session' : 'sessions'}, {searchResults.totalMatches} {searchResults.totalMatches === 1 ? 'match' : 'matches'}
      <span className="ml-2 normal-case tracking-normal">({searchResults.elapsedMs}ms)</span>
    </p>
    <div className="px-3 py-1 space-y-1 max-h-[60vh] overflow-y-auto">
      {searchResults.sessions.map((hit, i) => {
        const itemIndex = searchResultsStartIndex + i
        return (
          <SearchResultCard
            key={hit.sessionId}
            hit={hit}
            isSelected={selectedIndex === itemIndex}
            onSelect={() => handleSelectSearchResult(hit.sessionId)}
          />
        )
      })}
    </div>
    {searchResults.totalSessions > searchResults.sessions.length && (
      <button
        onClick={() => handleSelect(query.trim())}
        className="w-full px-4 py-2 text-xs text-emerald-600 dark:text-emerald-400 hover:text-emerald-500 dark:hover:text-emerald-300 transition-colors text-center"
      >
        View all {searchResults.totalSessions} results
      </button>
    )}
  </div>
) : null}
```

Key change: added `max-h-[60vh] overflow-y-auto` to the results container — makes it scrollable instead of hard-capped.

**Step 4: Commit**

```bash
git add src/components/CommandPalette.tsx
git commit -m "feat(search): CommandPalette regex fallback + scrollable results"
```

---

## Task 14: End-to-end verification

**Step 1: Run full backend test suite**

Run: `cargo test -p claude-view-search`
Expected: All tests pass including the new multi-signal, date qualifier, session qualifier, recency, and grep tests.

**Step 2: Run full frontend build**

Run: `bun run build`
Expected: Compiles with no TypeScript errors.

**Step 3: Manual E2E test**

Start the dev server: `bun run dev:server` and `bun run dev`

Test these scenarios:
1. **Smart search:** Type "deploy" in CommandPalette — results should include fuzzy matches
2. **Phrase ranking:** Type "deploy to production" — sessions with exact phrase should rank above scattered words
3. **Typo tolerance:** Type "deploymnt" — should still find "deployment"
4. **Date filter:** Type "deploy after:2026-02-01" — only recent sessions
5. **Regex fallback:** Type "error.*timeout" and get 0 smart results → should auto-fallback to grep
6. **Cmd+F:** Open a session, hit Cmd+F, type a search term → should highlight and scroll to matches
7. **Scrollable results:** Search for a common term → results should be scrollable, not cut off

**Step 4: Commit final state**

```bash
git add -A
git commit -m "feat(search): smart search — Google-level search for Claude sessions"
```

---

## Quick Reference

| Task | Time | Key Files |
|------|------|-----------|
| 1 | Deps | 2 min | `Cargo.toml`, `crates/search/Cargo.toml` |
| 2 | Multi-signal query | 20 min | `crates/search/src/query.rs` |
| 3 | New qualifiers | 15 min | `crates/search/src/query.rs` |
| 4 | Recency tiebreaker | 10 min | `crates/search/src/query.rs` |
| 5 | Snippet fix | 5 min | `crates/search/src/query.rs` |
| 6 | Grep types | 5 min | `crates/search/src/grep_types.rs`, `lib.rs` |
| 7 | Grep engine | 20 min | `crates/search/src/grep.rs` |
| 8 | Grep API route | 10 min | `crates/server/src/routes/grep.rs`, `mod.rs` |
| 9 | useGrep hook | 10 min | `src/hooks/use-grep.ts`, `src/hooks/use-search.ts` |
| 10 | GrepResults | 10 min | `src/components/GrepResults.tsx` |
| 11 | SearchInput | 10 min | `src/components/SearchInput.tsx` |
| 12 | Cmd+F | 20 min | `src/components/ConversationView.tsx` |
| 13 | CommandPalette | 10 min | `src/components/CommandPalette.tsx` |
| 14 | E2E verify | 10 min | Manual testing |

**Total: ~2.5 hours of implementation time.**
