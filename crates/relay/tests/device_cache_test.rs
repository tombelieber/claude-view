//! Unit tests for DeviceCache. Uses a mock SupabaseClient so no real
//! Supabase calls happen in CI.

use claude_view_relay::device_cache::DeviceCache;
use claude_view_relay::supabase::{DeviceRow, MockSupabaseClient, SupabaseClient};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn cache_hit_does_not_call_supabase_twice() {
    let mock = Arc::new(MockSupabaseClient::default());
    let device = DeviceRow {
        device_id: "mac-1111222233334444".to_string(),
        user_id: "00000000-0000-0000-0000-000000000001".to_string(),
        platform: "mac".to_string(),
        revoked_at: None,
    };
    mock.insert(device.clone());

    let cache = DeviceCache::new(
        mock.clone() as Arc<dyn SupabaseClient>,
        Duration::from_secs(60),
    );

    let first = cache
        .get(
            "00000000-0000-0000-0000-000000000001",
            "mac-1111222233334444",
        )
        .await
        .unwrap();
    assert!(first.is_some());

    let second = cache
        .get(
            "00000000-0000-0000-0000-000000000001",
            "mac-1111222233334444",
        )
        .await
        .unwrap();
    assert!(second.is_some());

    // The mock counts calls. Second fetch should hit the cache.
    assert_eq!(mock.call_count(), 1);
}

#[tokio::test]
async fn cache_expires_after_ttl() {
    let mock = Arc::new(MockSupabaseClient::default());
    mock.insert(DeviceRow {
        device_id: "mac-aaaabbbbccccdddd".to_string(),
        user_id: "00000000-0000-0000-0000-000000000002".to_string(),
        platform: "mac".to_string(),
        revoked_at: None,
    });

    let cache = DeviceCache::new(
        mock.clone() as Arc<dyn SupabaseClient>,
        Duration::from_millis(50),
    );

    let _ = cache
        .get(
            "00000000-0000-0000-0000-000000000002",
            "mac-aaaabbbbccccdddd",
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = cache
        .get(
            "00000000-0000-0000-0000-000000000002",
            "mac-aaaabbbbccccdddd",
        )
        .await
        .unwrap();

    assert_eq!(mock.call_count(), 2, "expected cache to expire and refetch");
}

#[tokio::test]
async fn cache_returns_none_for_revoked_device() {
    let mock = Arc::new(MockSupabaseClient::default());
    mock.insert(DeviceRow {
        device_id: "mac-eeeeffff00001111".to_string(),
        user_id: "00000000-0000-0000-0000-000000000003".to_string(),
        platform: "mac".to_string(),
        revoked_at: Some("2026-04-16T12:00:00Z".to_string()),
    });

    let cache = DeviceCache::new(mock as Arc<dyn SupabaseClient>, Duration::from_secs(60));

    let result = cache
        .get(
            "00000000-0000-0000-0000-000000000003",
            "mac-eeeeffff00001111",
        )
        .await
        .unwrap();
    assert!(result.is_none(), "revoked devices must be filtered out");
}

#[tokio::test]
async fn cache_rejects_device_belonging_to_different_user() {
    let mock = Arc::new(MockSupabaseClient::default());
    mock.insert(DeviceRow {
        device_id: "mac-22223333aaaabbbb".to_string(),
        user_id: "00000000-0000-0000-0000-000000000004".to_string(),
        platform: "mac".to_string(),
        revoked_at: None,
    });

    let cache = DeviceCache::new(mock as Arc<dyn SupabaseClient>, Duration::from_secs(60));

    // Ask for the device as a DIFFERENT user.
    let result = cache
        .get(
            "00000000-0000-0000-0000-000000000099",
            "mac-22223333aaaabbbb",
        )
        .await
        .unwrap();
    assert!(result.is_none(), "cache must cross-check user_id");
}
