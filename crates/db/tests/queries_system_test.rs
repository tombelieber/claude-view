//! Integration tests for Database system/storage query methods (indexer state).

use vibe_recall_db::Database;

#[tokio::test]
async fn test_indexer_state_roundtrip() {
    let db = Database::new_in_memory().await.unwrap();

    let path = "/home/user/.claude/projects/test/session.jsonl";

    // Initially no state
    let state = db.get_indexer_state(path).await.unwrap();
    assert!(state.is_none(), "Should have no state initially");

    // Set state
    db.update_indexer_state(path, 4096, 1234567890).await.unwrap();

    // Read back
    let state = db.get_indexer_state(path).await.unwrap();
    assert!(state.is_some(), "Should have state after update");
    let entry = state.unwrap();
    assert_eq!(entry.file_path, path);
    assert_eq!(entry.file_size, 4096);
    assert_eq!(entry.modified_at, 1234567890);
    assert!(entry.indexed_at > 0, "indexed_at should be set");

    // Update state (upsert)
    db.update_indexer_state(path, 8192, 1234567999).await.unwrap();
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
