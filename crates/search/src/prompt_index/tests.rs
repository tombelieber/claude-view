//! Tests for the prompt search index.

use super::*;

#[test]
fn create_prompt_index_in_ram() {
    let index = PromptSearchIndex::open_in_ram().unwrap();
    assert_eq!(index.search("test", None, 10, 0).unwrap().total_matches, 0);
}

#[test]
fn index_and_search_prompt() {
    let index = PromptSearchIndex::open_in_ram().unwrap();
    let doc = PromptDocument {
        prompt_id: "p001".into(),
        display: "fix the authentication error".into(),
        paste_text: None,
        project: "claude-view".into(),
        session_id: Some("abc-123".into()),
        branch: "main".into(),
        model: "claude-opus-4-6".into(),
        git_root: "/Users/test/dev/claude-view".into(),
        intent: "fix".into(),
        complexity: "short".into(),
        timestamp: 1772924399,
        has_paste: false,
    };
    index.index_prompts(&[doc]).unwrap();
    index.commit().unwrap();
    index.reader.reload().unwrap();

    let results = index.search("authentication", None, 10, 0).unwrap();
    assert_eq!(results.total_matches, 1);
    assert_eq!(results.prompts[0].display, "fix the authentication error");
}

#[test]
fn search_paste_content() {
    let index = PromptSearchIndex::open_in_ram().unwrap();
    let doc = PromptDocument {
        prompt_id: "p002".into(),
        display: "[Pasted text #1 +18 lines]".into(),
        paste_text: Some("NullPointerException in UserService.java".into()),
        project: "proj".into(),
        session_id: None,
        branch: "".into(),
        model: "".into(),
        git_root: "".into(),
        intent: "other".into(),
        complexity: "short".into(),
        timestamp: 1772924399,
        has_paste: true,
    };
    index.index_prompts(&[doc]).unwrap();
    index.commit().unwrap();
    index.reader.reload().unwrap();

    let results = index.search("NullPointerException", None, 10, 0).unwrap();
    assert_eq!(results.total_matches, 1);
}

#[test]
fn search_with_intent_qualifier() {
    let index = PromptSearchIndex::open_in_ram().unwrap();
    let docs = vec![
        PromptDocument {
            prompt_id: "p003".into(),
            display: "fix the bug".into(),
            paste_text: None,
            project: "proj".into(),
            session_id: None,
            branch: "".into(),
            model: "".into(),
            git_root: "".into(),
            intent: "fix".into(),
            complexity: "micro".into(),
            timestamp: 100,
            has_paste: false,
        },
        PromptDocument {
            prompt_id: "p004".into(),
            display: "create a new module".into(),
            paste_text: None,
            project: "proj".into(),
            session_id: None,
            branch: "".into(),
            model: "".into(),
            git_root: "".into(),
            intent: "create".into(),
            complexity: "short".into(),
            timestamp: 200,
            has_paste: false,
        },
    ];
    index.index_prompts(&docs).unwrap();
    index.commit().unwrap();
    index.reader.reload().unwrap();

    let results = index.search("intent:fix", None, 10, 0).unwrap();
    assert_eq!(results.total_matches, 1);
    assert_eq!(results.prompts[0].intent, "fix");
}

// ── template_match tests ────────────────────────────────────────────────

fn make_doc(id: &str, display: &str, ts: i64) -> PromptDocument {
    PromptDocument {
        prompt_id: id.into(),
        display: display.into(),
        paste_text: None,
        project: "proj".into(),
        session_id: None,
        branch: "".into(),
        model: "".into(),
        git_root: "".into(),
        intent: "fix".into(),
        complexity: "short".into(),
        timestamp: ts,
        has_paste: false,
    }
}

#[test]
fn template_prompts_share_same_template_id() {
    // Three prompts that normalize to the same pattern get a non-empty, identical template_id.
    let index = PromptSearchIndex::open_in_ram().unwrap();
    let docs = vec![
        make_doc("t1", "review @docs/plan-a.md is this ready", 100),
        make_doc("t2", "review @docs/plan-b.md is this ready", 200),
        make_doc("t3", "review @docs/plan-c.md is this ready", 300),
    ];
    index.index_prompts(&docs).unwrap();
    index.commit().unwrap();
    index.reader.reload().unwrap();

    let results = index.search("review", None, 10, 0).unwrap();
    assert_eq!(results.total_matches, 3);

    let ids: Vec<&str> = results
        .prompts
        .iter()
        .map(|h| h.template_id.as_deref().unwrap_or(""))
        .collect();
    // All three must have the same non-empty template_id
    assert!(
        !ids[0].is_empty(),
        "template prompts must have a non-empty template_id"
    );
    assert_eq!(ids[0], ids[1], "same pattern => same template_id");
    assert_eq!(ids[1], ids[2], "same pattern => same template_id");
}

#[test]
fn unique_prompt_has_empty_template_id() {
    let index = PromptSearchIndex::open_in_ram().unwrap();
    // Each prompt is completely different — no pattern match
    let docs = vec![
        make_doc("u1", "what is the meaning of life", 100),
        make_doc("u2", "refactor the database connection pool", 200),
        make_doc("u3", "explain how oauth2 pkce works", 300),
    ];
    index.index_prompts(&docs).unwrap();
    index.commit().unwrap();
    index.reader.reload().unwrap();

    let results = index.search("", None, 10, 0).unwrap();
    assert_eq!(results.total_matches, 3);

    for hit in &results.prompts {
        assert!(
            hit.template_id.as_deref().unwrap_or("").is_empty(),
            "unique prompt '{}' should have empty template_id",
            hit.display
        );
    }
}

#[test]
fn template_match_filter_returns_only_template_prompts() {
    let index = PromptSearchIndex::open_in_ram().unwrap();
    let docs = vec![
        make_doc("t1", "review @docs/plan-a.md is this ready", 100),
        make_doc("t2", "review @docs/plan-b.md is this ready", 200),
        make_doc("t3", "review @docs/plan-c.md is this ready", 300),
        make_doc("u1", "what is the meaning of life", 400),
    ];
    index.index_prompts(&docs).unwrap();
    index.commit().unwrap();
    index.reader.reload().unwrap();

    let results = index
        .search_with(PromptSearchParams {
            query: "",
            scope: None,
            template_match: Some("template"),
            limit: 10,
            offset: 0,
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        results.total_matches, 3,
        "only template prompts should be returned"
    );
    for hit in &results.prompts {
        assert!(
            !hit.template_id.as_deref().unwrap_or("").is_empty(),
            "all hits should have a template_id"
        );
    }
}

#[test]
fn template_match_filter_returns_only_unique_prompts() {
    let index = PromptSearchIndex::open_in_ram().unwrap();
    let docs = vec![
        make_doc("t1", "review @docs/plan-a.md is this ready", 100),
        make_doc("t2", "review @docs/plan-b.md is this ready", 200),
        make_doc("u1", "what is the meaning of life", 300),
        make_doc("u2", "explain oauth2 pkce flow", 400),
    ];
    index.index_prompts(&docs).unwrap();
    index.commit().unwrap();
    index.reader.reload().unwrap();

    let results = index
        .search_with(PromptSearchParams {
            query: "",
            scope: None,
            template_match: Some("unique"),
            limit: 10,
            offset: 0,
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        results.total_matches, 2,
        "only unique prompts should be returned"
    );
    for hit in &results.prompts {
        assert!(
            hit.template_id.as_deref().unwrap_or("").is_empty(),
            "all hits should have empty template_id"
        );
    }
}

#[test]
fn template_match_any_returns_all_prompts() {
    let index = PromptSearchIndex::open_in_ram().unwrap();
    let docs = vec![
        make_doc("t1", "review @docs/plan-a.md is this ready", 100),
        make_doc("t2", "review @docs/plan-b.md is this ready", 200),
        make_doc("u1", "what is the meaning of life", 300),
    ];
    index.index_prompts(&docs).unwrap();
    index.commit().unwrap();
    index.reader.reload().unwrap();

    // None = "any" — no template filter applied
    let results = index
        .search_with(PromptSearchParams {
            query: "",
            scope: None,
            template_match: None,
            limit: 10,
            offset: 0,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.total_matches, 3);
}

#[test]
fn search_pagination() {
    let index = PromptSearchIndex::open_in_ram().unwrap();
    let docs: Vec<PromptDocument> = (0..5)
        .map(|i| PromptDocument {
            prompt_id: format!("p{i:03}"),
            display: format!("test prompt number {i}"),
            paste_text: None,
            project: "proj".into(),
            session_id: None,
            branch: "".into(),
            model: "".into(),
            git_root: "".into(),
            intent: "other".into(),
            complexity: "short".into(),
            timestamp: i as i64,
            has_paste: false,
        })
        .collect();
    index.index_prompts(&docs).unwrap();
    index.commit().unwrap();
    index.reader.reload().unwrap();

    let page1 = index.search("test prompt", None, 2, 0).unwrap();
    assert_eq!(page1.prompts.len(), 2);
    assert_eq!(page1.total_matches, 5);

    let page2 = index.search("test prompt", None, 2, 2).unwrap();
    assert_eq!(page2.prompts.len(), 2);
}
