use claude_view_search::{SearchDocument, SearchIndex};

fn make_doc(session_id: &str, role: &str, content: &str, turn_number: u64) -> SearchDocument {
    SearchDocument {
        session_id: session_id.to_string(),
        project: "project-a".to_string(),
        branch: String::new(),
        model: String::new(),
        role: role.to_string(),
        content: content.to_string(),
        turn_number,
        timestamp: 1,
        skills: Vec::new(),
    }
}

#[test]
fn role_filtering_skips_unknown_source_roles() {
    let idx = SearchIndex::open_in_ram().expect("create index");
    let session_id = "sess-role-filter";
    let docs = vec![
        make_doc(session_id, "user", "rolechecktoken", 1),
        make_doc(session_id, "assistant", "rolechecktoken", 2),
        make_doc(session_id, "system", "rolechecktoken", 3),
    ];

    idx.index_session(session_id, &docs).expect("index session");
    idx.commit().expect("commit");
    idx.reader.reload().expect("reload");

    let all = idx
        .search("rolechecktoken", None, 10, 0, false)
        .expect("search");
    assert_eq!(all.total_sessions, 1);
    assert_eq!(all.total_matches, 2, "invalid role doc should be skipped");
    assert!(all.sessions[0]
        .matches
        .iter()
        .all(|m| m.role == "user" || m.role == "assistant"));

    let invalid_role = idx
        .search("role:system rolechecktoken", None, 10, 0, false)
        .expect("search by invalid role");
    assert_eq!(invalid_role.total_sessions, 0);
}

#[test]
fn summary_role_docs_are_excluded_from_source_message_index() {
    let idx = SearchIndex::open_in_ram().expect("create index");
    let session_id = "sess-summary-filter";
    let docs = vec![
        make_doc(session_id, "summary", "summaryuniquetoken", 1),
        make_doc(session_id, "tool", "tooltoken", 2),
    ];

    idx.index_session(session_id, &docs).expect("index session");
    idx.commit().expect("commit");
    idx.reader.reload().expect("reload");

    let from_summary = idx
        .search("summaryuniquetoken", None, 10, 0, false)
        .expect("search summary term");
    assert_eq!(
        from_summary.total_sessions, 0,
        "summary docs must not be indexed"
    );

    let from_tool = idx
        .search("tooltoken", None, 10, 0, false)
        .expect("search tool");
    assert_eq!(from_tool.total_sessions, 1);
    assert_eq!(from_tool.sessions[0].top_match.role, "tool");

    let role_summary = idx
        .search("role:summary", None, 10, 0, false)
        .expect("search role");
    assert_eq!(role_summary.total_sessions, 0);
}
