/// Integration tests for the search query pipeline (parsing + building + execution).

#[cfg(test)]
mod tests {
    use crate::SearchIndex;

    #[test]
    fn test_multi_signal_ranks_phrase_above_fuzzy() {
        let idx = SearchIndex::open_in_ram().unwrap();

        idx.index_session(
            "session-a",
            &[crate::indexer::SearchDocument {
                session_id: "session-a".to_string(),
                project: "test".to_string(),
                branch: String::new(),
                model: String::new(),
                role: "user".to_string(),
                content: "we need to deploy to production tonight".to_string(),
                turn_number: 1,
                timestamp: 1000,
                skills: vec![],
            }],
        )
        .unwrap();

        idx.index_session(
            "session-b",
            &[crate::indexer::SearchDocument {
                session_id: "session-b".to_string(),
                project: "test".to_string(),
                branch: String::new(),
                model: String::new(),
                role: "user".to_string(),
                content: "production environment deploy scripts to run".to_string(),
                turn_number: 1,
                timestamp: 2000,
                skills: vec![],
            }],
        )
        .unwrap();

        idx.commit().unwrap();
        idx.reader.reload().unwrap();

        let result = idx
            .search("deploy to production", None, 10, 0, false)
            .unwrap();
        assert!(result.total_sessions >= 2, "both sessions should match");

        // Session A (exact phrase) must rank above Session B (scattered terms)
        assert_eq!(
            result.sessions[0].session_id, "session-a",
            "exact phrase match should rank first"
        );
        assert!(
            result.sessions[0].best_score > result.sessions[1].best_score,
            "phrase match score ({}) should exceed term match score ({})",
            result.sessions[0].best_score,
            result.sessions[1].best_score
        );
    }

    #[test]
    fn test_fuzzy_catches_typos() {
        let idx = SearchIndex::open_in_ram().unwrap();

        idx.index_session(
            "session-typo",
            &[crate::indexer::SearchDocument {
                session_id: "session-typo".to_string(),
                project: "test".to_string(),
                branch: String::new(),
                model: String::new(),
                role: "user".to_string(),
                content: "the deployment pipeline failed with timeout".to_string(),
                turn_number: 1,
                timestamp: 1000,
                skills: vec![],
            }],
        )
        .unwrap();

        idx.commit().unwrap();
        idx.reader.reload().unwrap();

        // "deploymnt" (typo) should still find "deployment" via fuzzy
        let result = idx.search("deploymnt", None, 10, 0, false).unwrap();
        assert_eq!(
            result.total_sessions, 1,
            "fuzzy should catch single-char typo"
        );
        assert_eq!(result.sessions[0].session_id, "session-typo");
    }

    #[test]
    fn test_after_before_date_qualifiers() {
        let idx = SearchIndex::open_in_ram().unwrap();

        // Jan 15 2026 = 1768435200 unix
        idx.index_session(
            "old-session",
            &[crate::indexer::SearchDocument {
                session_id: "old-session".to_string(),
                project: "test".to_string(),
                branch: String::new(),
                model: String::new(),
                role: "user".to_string(),
                content: "deploy the app".to_string(),
                turn_number: 1,
                timestamp: 1768435200,
                skills: vec![],
            }],
        )
        .unwrap();

        // Feb 15 2026 = 1771113600 unix
        idx.index_session(
            "new-session",
            &[crate::indexer::SearchDocument {
                session_id: "new-session".to_string(),
                project: "test".to_string(),
                branch: String::new(),
                model: String::new(),
                role: "user".to_string(),
                content: "deploy the app".to_string(),
                turn_number: 1,
                timestamp: 1771113600,
                skills: vec![],
            }],
        )
        .unwrap();

        idx.commit().unwrap();
        idx.reader.reload().unwrap();

        // after:2026-02-01 should only return new-session
        let result = idx
            .search("deploy after:2026-02-01", None, 10, 0, false)
            .unwrap();
        assert_eq!(result.total_sessions, 1);
        assert_eq!(result.sessions[0].session_id, "new-session");

        // before:2026-02-01 should only return old-session
        let result = idx
            .search("deploy before:2026-02-01", None, 10, 0, false)
            .unwrap();
        assert_eq!(result.total_sessions, 1);
        assert_eq!(result.sessions[0].session_id, "old-session");
    }

    #[test]
    fn test_session_qualifier() {
        let idx = SearchIndex::open_in_ram().unwrap();

        idx.index_session(
            "aaa-111",
            &[crate::indexer::SearchDocument {
                session_id: "aaa-111".to_string(),
                project: "test".to_string(),
                branch: String::new(),
                model: String::new(),
                role: "user".to_string(),
                content: "hello world".to_string(),
                turn_number: 1,
                timestamp: 1000,
                skills: vec![],
            }],
        )
        .unwrap();

        idx.index_session(
            "bbb-222",
            &[crate::indexer::SearchDocument {
                session_id: "bbb-222".to_string(),
                project: "test".to_string(),
                branch: String::new(),
                model: String::new(),
                role: "user".to_string(),
                content: "hello world".to_string(),
                turn_number: 1,
                timestamp: 2000,
                skills: vec![],
            }],
        )
        .unwrap();

        idx.commit().unwrap();
        idx.reader.reload().unwrap();

        // session:aaa-111 should only return that session
        let result = idx
            .search("hello", Some("session:aaa-111"), 10, 0, false)
            .unwrap();
        assert_eq!(result.total_sessions, 1);
        assert_eq!(result.sessions[0].session_id, "aaa-111");
    }

    #[test]
    fn test_recency_tiebreaks_equal_scores() {
        let idx = SearchIndex::open_in_ram().unwrap();

        // Two sessions with identical content (identical BM25 scores)
        // but different timestamps
        idx.index_session(
            "old",
            &[crate::indexer::SearchDocument {
                session_id: "old".to_string(),
                project: "test".to_string(),
                branch: String::new(),
                model: String::new(),
                role: "user".to_string(),
                content: "identical content for scoring".to_string(),
                turn_number: 1,
                timestamp: 1000,
                skills: vec![],
            }],
        )
        .unwrap();

        idx.index_session(
            "new",
            &[crate::indexer::SearchDocument {
                session_id: "new".to_string(),
                project: "test".to_string(),
                branch: String::new(),
                model: String::new(),
                role: "user".to_string(),
                content: "identical content for scoring".to_string(),
                turn_number: 1,
                timestamp: 9999,
                skills: vec![],
            }],
        )
        .unwrap();

        idx.commit().unwrap();
        idx.reader.reload().unwrap();

        let result = idx
            .search("identical content scoring", None, 10, 0, false)
            .unwrap();
        assert_eq!(result.total_sessions, 2);
        // With identical scores, newer session should rank first
        assert_eq!(
            result.sessions[0].session_id, "new",
            "recency should tiebreak equal scores — newer first"
        );
    }

    #[test]
    fn test_total_sessions_is_true_count_not_paginated() {
        let idx = SearchIndex::open_in_ram().expect("create index");
        // 200 sessions * 5 docs = 1000 docs total.
        // With limit=1, fetch_limit = (1+0)*50 = 50, which only covers ~10 sessions.
        // True total must still be 200.
        for i in 0..200 {
            let session_id = format!("session-{i:03}");
            let docs: Vec<_> = (0..5)
                .map(|j| crate::indexer::SearchDocument {
                    session_id: session_id.clone(),
                    project: "test".to_string(),
                    branch: "main".to_string(),
                    model: "opus".to_string(),
                    role: "user".to_string(),
                    content: format!("deploy message {j}"),
                    turn_number: j as u64,
                    timestamp: 1710000000 + (i * 100 + j) as i64,
                    skills: vec![],
                })
                .collect();
            idx.index_session(&session_id, &docs).expect("index");
        }
        idx.commit().expect("commit");
        idx.reader.reload().expect("reload");

        let resp = idx.search("deploy", None, 1, 0, false).unwrap();
        assert_eq!(resp.sessions.len(), 1, "paginated: 1 session returned");
        assert_eq!(
            resp.total_sessions, 200,
            "TRUE total must be 200, not subset"
        );
    }

    #[test]
    fn test_hyphenated_text_query_matches() {
        let idx = SearchIndex::open_in_ram().unwrap();

        idx.index_session(
            "session-hyphen",
            &[crate::indexer::SearchDocument {
                session_id: "session-hyphen".to_string(),
                project: "test".to_string(),
                branch: String::new(),
                model: String::new(),
                role: "user".to_string(),
                content: "pm status dashboard is red".to_string(),
                turn_number: 1,
                timestamp: 1000,
                skills: vec![],
            }],
        )
        .unwrap();

        idx.commit().unwrap();
        idx.reader.reload().unwrap();

        let result = idx.search("pm-status", None, 10, 0, false).unwrap();
        assert_eq!(
            result.total_sessions, 1,
            "hyphenated query should match tokenized content terms"
        );
        assert_eq!(result.sessions[0].session_id, "session-hyphen");
    }
}
