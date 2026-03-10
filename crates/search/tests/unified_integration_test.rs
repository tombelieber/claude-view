// crates/search/tests/unified_integration_test.rs
//! Integration tests for unified search (Tantivy + grep fallback).
//!
//! These tests use real temporary JSONL files and an in-RAM Tantivy index
//! to verify the complete two-phase search pipeline.

use claude_view_search::indexer::SearchDocument;
use claude_view_search::unified::{unified_search, SearchEngine, UnifiedSearchOptions};
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
    index_doc(&idx, "s1", "SessionStart hook 嘅 payload 本身冇 git_branch");
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
