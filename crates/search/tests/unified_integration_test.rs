use claude_view_search::unified::{unified_search, SearchEngine, UnifiedSearchOptions};
use claude_view_search::JsonlFile;
use std::fs;
use tempfile::TempDir;

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

#[test]
fn test_session_search_uses_grep() {
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
        skip_snippets: false,
    };

    let result = unified_search(&files, &opts).unwrap();
    assert_eq!(result.response.total_sessions, 1);
    assert_eq!(result.engine, SearchEngine::Grep);
    assert_eq!(result.response.sessions[0].engines, vec!["grep"]);
}

/// CJK works via grep over the raw JSONL line.
#[test]
fn test_cjk_grep_search() {
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
        skip_snippets: false,
    };

    let result = unified_search(&files, &opts).unwrap();
    assert_eq!(result.response.total_sessions, 1);
    assert_eq!(result.engine, SearchEngine::Grep);
    assert!(result.response.sessions[0]
        .engines
        .contains(&"grep".to_string()));
}

/// Results sorted by recency (modified_at DESC).
#[test]
fn test_grep_sorted_by_recency() {
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
        skip_snippets: false,
    };

    let result = unified_search(&files, &opts).unwrap();
    assert_eq!(result.engine, SearchEngine::Grep);
    assert_eq!(result.response.sessions.len(), 2);
    assert_eq!(result.response.sessions[0].session_id, "s-new");
    assert_eq!(result.response.sessions[1].session_id, "s-old");
}
