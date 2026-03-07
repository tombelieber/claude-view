//! Integration tests for Database system/storage query methods (indexer state).

use claude_view_db::{Database, IndexRunIntegrityCounters};

#[tokio::test]
async fn test_indexer_state_roundtrip() {
    let db = Database::new_in_memory().await.unwrap();

    let path = "/home/user/.claude/projects/test/session.jsonl";

    // Initially no state
    let state = db.get_indexer_state(path).await.unwrap();
    assert!(state.is_none(), "Should have no state initially");

    // Set state
    db.update_indexer_state(path, 4096, 1234567890)
        .await
        .unwrap();

    // Read back
    let state = db.get_indexer_state(path).await.unwrap();
    assert!(state.is_some(), "Should have state after update");
    let entry = state.unwrap();
    assert_eq!(entry.file_path, path);
    assert_eq!(entry.file_size, 4096);
    assert_eq!(entry.modified_at, 1234567890);
    assert!(entry.indexed_at > 0, "indexed_at should be set");

    // Update state (upsert)
    db.update_indexer_state(path, 8192, 1234567999)
        .await
        .unwrap();
    let entry = db.get_indexer_state(path).await.unwrap().unwrap();
    assert_eq!(entry.file_size, 8192);
    assert_eq!(entry.modified_at, 1234567999);
}

#[tokio::test]
async fn test_get_all_indexer_states() {
    let db = Database::new_in_memory().await.unwrap();

    // Initially empty
    let states = db.get_all_indexer_states().await.unwrap();
    assert!(states.is_empty(), "Should be empty initially");

    // Insert some indexer state entries
    let path_a = "/home/user/.claude/projects/test/a.jsonl";
    let path_b = "/home/user/.claude/projects/test/b.jsonl";
    let path_c = "/home/user/.claude/projects/test/c.jsonl";

    db.update_indexer_state(path_a, 1000, 100).await.unwrap();
    db.update_indexer_state(path_b, 2000, 200).await.unwrap();
    db.update_indexer_state(path_c, 3000, 300).await.unwrap();

    // Fetch all states
    let states = db.get_all_indexer_states().await.unwrap();
    assert_eq!(states.len(), 3, "Should have 3 entries");

    // Verify each entry is keyed correctly and has correct values
    let a = states.get(path_a).expect("Should contain path_a");
    assert_eq!(a.file_size, 1000);
    assert_eq!(a.modified_at, 100);

    let b = states.get(path_b).expect("Should contain path_b");
    assert_eq!(b.file_size, 2000);
    assert_eq!(b.modified_at, 200);

    let c = states.get(path_c).expect("Should contain path_c");
    assert_eq!(c.file_size, 3000);
    assert_eq!(c.modified_at, 300);

    // All entries should have indexed_at set
    assert!(a.indexed_at > 0);
    assert!(b.indexed_at > 0);
    assert!(c.indexed_at > 0);
}

#[tokio::test]
async fn test_get_latest_integrity_counters_defaults_and_latest_run() {
    let db = Database::new_in_memory().await.unwrap();

    // Empty DB should return all-zero counters.
    let empty = db.get_latest_integrity_counters().await.unwrap();
    assert_eq!(empty.unknown_top_level_type_count, 0);
    assert_eq!(empty.unknown_required_path_count, 0);
    assert_eq!(empty.imaginary_path_access_count, 0);
    assert_eq!(empty.legacy_fallback_path_count, 0);
    assert_eq!(empty.dropped_line_invalid_json_count, 0);
    assert_eq!(empty.schema_mismatch_count, 0);
    assert_eq!(empty.unknown_source_role_count, 0);
    assert_eq!(empty.derived_source_message_doc_count, 0);
    assert_eq!(empty.source_message_non_source_provenance_count, 0);

    // Insert two runs and ensure the latest one is returned.
    let run_1 = db.create_index_run("full", Some(0), None).await.unwrap();
    let counters_1 = IndexRunIntegrityCounters {
        unknown_top_level_type_count: 1,
        unknown_required_path_count: 1,
        imaginary_path_access_count: 1,
        legacy_fallback_path_count: 1,
        dropped_line_invalid_json_count: 1,
        schema_mismatch_count: 1,
        unknown_source_role_count: 1,
        derived_source_message_doc_count: 1,
        source_message_non_source_provenance_count: 1,
    };
    db.complete_index_run(run_1, Some(10), 1000, Some(2.5), Some(&counters_1))
        .await
        .unwrap();

    let run_2 = db
        .create_index_run("incremental", Some(10), None)
        .await
        .unwrap();
    let counters_2 = IndexRunIntegrityCounters {
        unknown_top_level_type_count: 2,
        unknown_required_path_count: 3,
        imaginary_path_access_count: 4,
        legacy_fallback_path_count: 5,
        dropped_line_invalid_json_count: 6,
        schema_mismatch_count: 7,
        unknown_source_role_count: 8,
        derived_source_message_doc_count: 9,
        source_message_non_source_provenance_count: 10,
    };
    db.complete_index_run(run_2, Some(12), 500, Some(3.1), Some(&counters_2))
        .await
        .unwrap();

    let latest = db.get_latest_integrity_counters().await.unwrap();
    assert_eq!(latest.unknown_top_level_type_count, 2);
    assert_eq!(latest.unknown_required_path_count, 3);
    assert_eq!(latest.imaginary_path_access_count, 4);
    assert_eq!(latest.legacy_fallback_path_count, 5);
    assert_eq!(latest.dropped_line_invalid_json_count, 6);
    assert_eq!(latest.schema_mismatch_count, 7);
    assert_eq!(latest.unknown_source_role_count, 8);
    assert_eq!(latest.derived_source_message_doc_count, 9);
    assert_eq!(latest.source_message_non_source_provenance_count, 10);
}
