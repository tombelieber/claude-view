//! Integration tests for bandwidth SSE events during deep indexing.
//!
//! Verifies that the `/api/indexing/progress` SSE endpoint correctly reports
//! `bytes_processed` and `bytes_total` fields as the indexing state machine
//! transitions through its phases.
//!
//! These tests simulate the indexing lifecycle by driving `IndexingState`
//! from a background task, then reading the SSE body to verify event contents.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;
use vibe_recall_db::Database;
use vibe_recall_server::indexing_state::{IndexingState, IndexingStatus};
use vibe_recall_server::create_app_with_indexing;

/// Helper: create an in-memory database for tests.
async fn test_db() -> Database {
    Database::new_in_memory().await.expect("in-memory DB for tests")
}

/// Simulate deep indexing progress by updating the shared `IndexingState`
/// from a background task. This mimics what `run_background_index` does in
/// production: it transitions through ReadingIndexes -> DeepIndexing -> Done
/// while incrementing `bytes_processed` in batches.
///
/// Returns the file sizes used so callers can verify totals.
fn spawn_indexing_simulation(
    state: Arc<IndexingState>,
    file_sizes: Vec<u64>,
) {
    let total_bytes: u64 = file_sizes.iter().sum();
    let num_files = file_sizes.len();

    tokio::spawn(async move {
        // Small delay to let the SSE handler connect and start polling.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Phase 1: ReadingIndexes
        state.set_status(IndexingStatus::ReadingIndexes);
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Transition to DeepIndexing with discovery counts
        state.set_projects_found(1);
        state.set_sessions_found(num_files);
        state.set_total(num_files);
        state.set_bytes_total(total_bytes);
        state.set_status(IndexingStatus::DeepIndexing);

        // Phase 2: Process files one at a time
        for (i, size) in file_sizes.iter().enumerate() {
            tokio::time::sleep(Duration::from_millis(30)).await;
            state.add_bytes_processed(*size);
            state.set_indexed(i + 1);
        }

        // Small pause so SSE handler picks up the last deep-progress event
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Phase 3: Done
        state.set_status(IndexingStatus::Done);
    });
}

/// Parse SSE event lines from the body string into a vec of (event_name, json_data) pairs.
fn parse_sse_events(body: &str) -> Vec<(String, serde_json::Value)> {
    let mut events = Vec::new();
    let mut current_event = String::new();
    let mut current_data = String::new();

    for line in body.lines() {
        if let Some(event_name) = line.strip_prefix("event: ") {
            current_event = event_name.trim().to_string();
        } else if let Some(data) = line.strip_prefix("data: ") {
            current_data = data.trim().to_string();
        } else if line.is_empty() && !current_event.is_empty() {
            // End of event block
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&current_data) {
                events.push((current_event.clone(), json));
            }
            current_event.clear();
            current_data.clear();
        }
    }

    // Handle final event (SSE body may not have trailing blank line)
    if !current_event.is_empty() && !current_data.is_empty() {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&current_data) {
            events.push((current_event, json));
        }
    }

    events
}

// =============================================================================
// Tests
// =============================================================================

/// Verify that `bytes_total` in the done event matches the sum of all file sizes.
#[tokio::test]
async fn test_bytes_total_matches_sum_of_file_sizes() {
    let db = test_db().await;
    let state = Arc::new(IndexingState::new());
    let file_sizes: Vec<u64> = vec![1_000, 2_500, 3_750, 500];
    let expected_total: u64 = file_sizes.iter().sum(); // 7750

    spawn_indexing_simulation(state.clone(), file_sizes);

    let app = create_app_with_indexing(db, state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/indexing/progress")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    let events = parse_sse_events(&body_str);

    // Find the done event
    let done_event = events
        .iter()
        .find(|(name, _)| name == "done")
        .expect("Expected a 'done' event in the SSE stream");

    let bytes_total = done_event.1["bytes_total"]
        .as_u64()
        .expect("bytes_total should be a u64");

    assert_eq!(
        bytes_total, expected_total,
        "bytes_total in done event ({}) should match sum of file sizes ({})",
        bytes_total, expected_total
    );
}

/// Verify that `bytes_processed` increases monotonically across deep-progress events.
#[tokio::test]
async fn test_bytes_processed_increases_monotonically() {
    let db = test_db().await;
    let state = Arc::new(IndexingState::new());
    let file_sizes: Vec<u64> = vec![1_000, 2_000, 3_000, 4_000, 5_000];

    spawn_indexing_simulation(state.clone(), file_sizes);

    let app = create_app_with_indexing(db, state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/indexing/progress")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    let events = parse_sse_events(&body_str);

    // Collect bytes_processed from all deep-progress events
    let progress_bytes: Vec<u64> = events
        .iter()
        .filter(|(name, _)| name == "deep-progress")
        .filter_map(|(_, data)| data["bytes_processed"].as_u64())
        .collect();

    // Must have at least one deep-progress event
    assert!(
        !progress_bytes.is_empty(),
        "Expected at least one deep-progress event with bytes_processed, got events: {:?}",
        events.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
    );

    // Verify monotonic increase
    for window in progress_bytes.windows(2) {
        assert!(
            window[1] >= window[0],
            "bytes_processed should increase monotonically: {} -> {}",
            window[0],
            window[1]
        );
    }
}

/// Verify that the final done event has `bytes_processed == bytes_total`.
#[tokio::test]
async fn test_done_event_bytes_processed_equals_bytes_total() {
    let db = test_db().await;
    let state = Arc::new(IndexingState::new());
    let file_sizes: Vec<u64> = vec![10_000, 20_000, 30_000];
    let expected_total: u64 = file_sizes.iter().sum(); // 60000

    spawn_indexing_simulation(state.clone(), file_sizes);

    let app = create_app_with_indexing(db, state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/indexing/progress")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    let events = parse_sse_events(&body_str);

    let done_event = events
        .iter()
        .find(|(name, _)| name == "done")
        .expect("Expected a 'done' event in the SSE stream");

    let bytes_processed = done_event.1["bytes_processed"]
        .as_u64()
        .expect("done event should have bytes_processed");
    let bytes_total = done_event.1["bytes_total"]
        .as_u64()
        .expect("done event should have bytes_total");

    assert_eq!(
        bytes_processed, bytes_total,
        "In the done event, bytes_processed ({}) should equal bytes_total ({})",
        bytes_processed, bytes_total
    );
    assert_eq!(
        bytes_total, expected_total,
        "bytes_total ({}) should match the expected sum ({})",
        bytes_total, expected_total
    );
}

/// Verify the full event sequence: status -> ready -> deep-progress(s) -> done.
#[tokio::test]
async fn test_full_event_sequence() {
    let db = test_db().await;
    let state = Arc::new(IndexingState::new());
    let file_sizes: Vec<u64> = vec![5_000, 10_000];

    spawn_indexing_simulation(state.clone(), file_sizes);

    let app = create_app_with_indexing(db, state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/indexing/progress")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    let events = parse_sse_events(&body_str);

    let event_names: Vec<&str> = events.iter().map(|(n, _)| n.as_str()).collect();

    // Should start with "status" (reading-indexes)
    assert_eq!(
        event_names.first().copied(),
        Some("status"),
        "First event should be 'status', got: {:?}",
        event_names
    );

    // Should contain "ready" event
    assert!(
        event_names.contains(&"ready"),
        "Should contain 'ready' event, got: {:?}",
        event_names
    );

    // Should end with "done"
    assert_eq!(
        event_names.last().copied(),
        Some("done"),
        "Last event should be 'done', got: {:?}",
        event_names
    );
}

/// Verify that deep-progress events contain both bytes_processed and bytes_total fields.
#[tokio::test]
async fn test_deep_progress_events_contain_bandwidth_fields() {
    let db = test_db().await;
    let state = Arc::new(IndexingState::new());
    let file_sizes: Vec<u64> = vec![100, 200, 300];

    spawn_indexing_simulation(state.clone(), file_sizes);

    let app = create_app_with_indexing(db, state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/indexing/progress")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    let events = parse_sse_events(&body_str);

    let deep_events: Vec<&serde_json::Value> = events
        .iter()
        .filter(|(name, _)| name == "deep-progress")
        .map(|(_, data)| data)
        .collect();

    assert!(
        !deep_events.is_empty(),
        "Expected at least one deep-progress event"
    );

    for (i, event) in deep_events.iter().enumerate() {
        assert!(
            event.get("bytes_processed").is_some(),
            "deep-progress event {} missing bytes_processed: {}",
            i,
            event
        );
        assert!(
            event.get("bytes_total").is_some(),
            "deep-progress event {} missing bytes_total: {}",
            i,
            event
        );
        assert!(
            event.get("indexed").is_some(),
            "deep-progress event {} missing indexed: {}",
            i,
            event
        );
        assert!(
            event.get("total").is_some(),
            "deep-progress event {} missing total: {}",
            i,
            event
        );
    }
}

/// Verify the polling endpoint also returns bandwidth fields.
#[tokio::test]
async fn test_polling_status_includes_bandwidth_fields() {
    let db = test_db().await;
    let state = Arc::new(IndexingState::new());

    // Set up mid-indexing state
    state.set_status(IndexingStatus::DeepIndexing);
    state.set_total(10);
    state.set_indexed(5);
    state.set_bytes_total(100_000);
    state.add_bytes_processed(50_000);

    let app = create_app_with_indexing(db, state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/indexing/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["phase"], "deep-indexing");
    assert_eq!(json["bytesProcessed"], 50_000);
    assert_eq!(json["bytesTotal"], 100_000);
    assert_eq!(json["indexed"], 5);
    assert_eq!(json["total"], 10);
}

/// Verify bandwidth works with large byte counts (>4GB) to ensure no u32 overflow.
#[tokio::test]
async fn test_large_byte_counts_no_overflow() {
    let db = test_db().await;
    let state = Arc::new(IndexingState::new());

    // Simulate 52.1 GB total (realistic for a large Claude history)
    let large_total: u64 = 52_100_000_000;
    state.set_status(IndexingStatus::Done);
    state.set_sessions_found(500);
    state.set_projects_found(10);
    state.set_total(500);
    state.set_indexed(500);
    state.set_bytes_total(large_total);
    state.add_bytes_processed(large_total);

    let app = create_app_with_indexing(db, state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/indexing/progress")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    let events = parse_sse_events(&body_str);

    let done_event = events
        .iter()
        .find(|(name, _)| name == "done")
        .expect("Expected a 'done' event");

    let bytes_total = done_event.1["bytes_total"]
        .as_u64()
        .expect("bytes_total should be a u64");
    let bytes_processed = done_event.1["bytes_processed"]
        .as_u64()
        .expect("bytes_processed should be a u64");

    assert_eq!(
        bytes_total, large_total,
        "bytes_total should handle values >4GB without overflow"
    );
    assert_eq!(
        bytes_processed, large_total,
        "bytes_processed should handle values >4GB without overflow"
    );
}
