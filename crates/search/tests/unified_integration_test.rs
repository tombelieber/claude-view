use claude_view_search::indexer::SearchDocument;
use claude_view_search::unified::{unified_search, UnifiedSearchOptions};
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

fn make_jsonl_file(
    dir: &std::path::Path,
    session_id: &str,
    content: &str,
    modified_at: i64,
) -> JsonlFile {
    let path = dir.join(format!("{session_id}.jsonl"));
    fs::write(&path, content).unwrap();
    JsonlFile {
        path,
        session_id: session_id.to_string(),
        project: "integration-test".to_string(),
        project_path: dir.to_string_lossy().to_string(),
        modified_at,
    }
}

/// Both engines find the same session — grep primary, Tantivy supplements.
#[test]
fn test_co_primary_both_find_session() {
    let idx = SearchIndex::open_in_ram().unwrap();
    index_doc(&idx, "s1", "deploy to production");
    idx.commit().unwrap();
    idx.reader.reload().unwrap();

    let tmp = TempDir::new().unwrap();
    let files = vec![make_jsonl_file(
        tmp.path(),
        "s1",
        "{\"content\":\"deploy to production\"}\n",
        1710000000,
    )];

    let opts = UnifiedSearchOptions {
        query: "deploy".to_string(),
        scope: None,
        limit: 10,
        offset: 0,
    };

    let result = unified_search(Some(&idx), &files, &opts).unwrap();
    assert_eq!(result.response.total_sessions, 1);
    let engines = &result.response.sessions[0].engines;
    assert!(engines.contains(&"grep".to_string()));
    assert!(engines.contains(&"tantivy".to_string()));
}

/// CJK — grep finds it, Tantivy may or may not.
#[test]
fn test_co_primary_cjk_grep_only() {
    let idx = SearchIndex::open_in_ram().unwrap();
    index_doc(&idx, "s1", "自動部署到生產環境完成");
    idx.commit().unwrap();
    idx.reader.reload().unwrap();

    let tmp = TempDir::new().unwrap();
    let files = vec![make_jsonl_file(
        tmp.path(),
        "s1",
        "{\"content\":\"自動部署到生產環境完成\"}\n",
        1710000000,
    )];

    let opts = UnifiedSearchOptions {
        query: "部署".to_string(),
        scope: None,
        limit: 10,
        offset: 0,
    };

    let result = unified_search(Some(&idx), &files, &opts).unwrap();
    assert_eq!(result.response.total_sessions, 1);
    assert!(result.response.sessions[0]
        .engines
        .contains(&"grep".to_string()));
}

/// Results sorted by recency (modified_at DESC).
#[test]
fn test_co_primary_sorted_by_recency() {
    let idx = SearchIndex::open_in_ram().unwrap();
    idx.commit().unwrap();
    idx.reader.reload().unwrap();

    let tmp = TempDir::new().unwrap();
    let files = vec![
        make_jsonl_file(
            tmp.path(),
            "s-old",
            "{\"content\":\"deploy\"}\n",
            1710000000,
        ),
        make_jsonl_file(
            tmp.path(),
            "s-new",
            "{\"content\":\"deploy\"}\n",
            1710099999,
        ),
    ];

    let opts = UnifiedSearchOptions {
        query: "deploy".to_string(),
        scope: None,
        limit: 10,
        offset: 0,
    };

    let result = unified_search(Some(&idx), &files, &opts).unwrap();
    assert_eq!(result.response.sessions.len(), 2);
    assert_eq!(result.response.sessions[0].session_id, "s-new");
    assert_eq!(result.response.sessions[1].session_id, "s-old");
}
