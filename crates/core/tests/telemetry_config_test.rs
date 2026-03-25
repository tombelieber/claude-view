// crates/core/tests/telemetry_config_test.rs
use serial_test::serial;
use tempfile::TempDir;

// === Path resolution tests ===

#[test]
fn telemetry_config_path_is_under_home_claude_view() {
    let path = claude_view_core::telemetry_config::telemetry_config_path();
    let home = dirs::home_dir().unwrap();
    assert_eq!(path, home.join(".claude-view").join("telemetry.json"));
}

#[test]
#[serial]
fn telemetry_config_path_respects_claude_view_data_dir() {
    std::env::set_var("CLAUDE_VIEW_DATA_DIR", "/tmp/custom-data");
    let path = claude_view_core::telemetry_config::telemetry_config_path();
    assert_eq!(
        path,
        std::path::PathBuf::from("/tmp/custom-data").join("telemetry.json")
    );
    std::env::remove_var("CLAUDE_VIEW_DATA_DIR");
}

// === Read/Write tests ===

use claude_view_core::telemetry_config::{
    read_telemetry_config, write_telemetry_config, TelemetryConfig,
};

#[test]
fn read_missing_file_returns_default_undecided() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config = read_telemetry_config(&path);
    assert!(config.enabled.is_none());
    assert!(!config.anonymous_id.is_empty());
    assert!(config.consent_given_at.is_none());
    assert!(config.last_milestone.is_none());
}

#[test]
fn write_then_read_roundtrips() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config = TelemetryConfig {
        enabled: Some(true),
        anonymous_id: "test-uuid-1234".to_string(),
        consent_given_at: Some("2026-03-19T14:00:00Z".to_string()),
        last_milestone: Some(100),
        ..TelemetryConfig::new_undecided()
    };
    write_telemetry_config(&path, &config).unwrap();
    let read_back = read_telemetry_config(&path);
    assert_eq!(read_back.enabled, Some(true));
    assert_eq!(read_back.anonymous_id, "test-uuid-1234");
    assert_eq!(read_back.last_milestone, Some(100));
}

#[test]
fn write_uses_atomic_tmp_rename() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config = TelemetryConfig {
        enabled: Some(false),
        anonymous_id: "test-uuid".to_string(),
        consent_given_at: None,
        last_milestone: None,
        ..TelemetryConfig::new_undecided()
    };
    write_telemetry_config(&path, &config).unwrap();
    assert!(!dir.path().join("telemetry.json.tmp").exists());
    assert!(path.exists());
}

#[test]
fn concurrent_create_does_not_panic() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config1 = TelemetryConfig::new_undecided();
    let config2 = TelemetryConfig::new_undecided();
    write_telemetry_config(&path, &config1).unwrap();
    write_telemetry_config(&path, &config2).unwrap();
    let read_back = read_telemetry_config(&path);
    assert_eq!(read_back.anonymous_id, config2.anonymous_id);
}

#[test]
fn corrupted_json_returns_default_undecided() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    std::fs::write(&path, "not valid json {{{").unwrap();
    let config = read_telemetry_config(&path);
    assert!(config.enabled.is_none());
    assert!(!config.anonymous_id.is_empty());
}

#[test]
fn consent_given_at_preserved_on_opt_out() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config = TelemetryConfig {
        enabled: Some(true),
        anonymous_id: "test-uuid".to_string(),
        consent_given_at: Some("2026-03-19T14:00:00Z".to_string()),
        last_milestone: None,
        ..TelemetryConfig::new_undecided()
    };
    write_telemetry_config(&path, &config).unwrap();
    let mut read_back = read_telemetry_config(&path);
    read_back.enabled = Some(false);
    write_telemetry_config(&path, &read_back).unwrap();
    let final_config = read_telemetry_config(&path);
    assert_eq!(final_config.enabled, Some(false));
    assert_eq!(
        final_config.consent_given_at,
        Some("2026-03-19T14:00:00Z".to_string()),
        "consent_given_at must be preserved when user opts out"
    );
}

// === Override hierarchy tests ===

use claude_view_core::telemetry_config::{resolve_telemetry_status, TelemetryStatus};

#[test]
fn no_api_key_means_disabled() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let status = resolve_telemetry_status(None, &path);
    assert_eq!(status, TelemetryStatus::Disabled);
}

#[test]
#[serial]
fn env_var_override_disables() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config = TelemetryConfig {
        enabled: Some(true),
        ..TelemetryConfig::new_undecided()
    };
    write_telemetry_config(&path, &config).unwrap();
    std::env::set_var("CLAUDE_VIEW_TELEMETRY", "0");
    let status = resolve_telemetry_status(Some("phc_test"), &path);
    assert_eq!(status, TelemetryStatus::Disabled);
    std::env::remove_var("CLAUDE_VIEW_TELEMETRY");
}

#[test]
#[serial]
fn env_var_value_1_does_not_disable() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config = TelemetryConfig {
        enabled: Some(true),
        ..TelemetryConfig::new_undecided()
    };
    write_telemetry_config(&path, &config).unwrap();
    std::env::set_var("CLAUDE_VIEW_TELEMETRY", "1");
    let status = resolve_telemetry_status(Some("phc_test"), &path);
    assert_eq!(
        status,
        TelemetryStatus::Enabled,
        "CLAUDE_VIEW_TELEMETRY=1 must NOT disable telemetry — only '0' disables"
    );
    std::env::remove_var("CLAUDE_VIEW_TELEMETRY");
}

#[test]
#[serial]
fn ci_env_var_disables() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config = TelemetryConfig {
        enabled: Some(true),
        ..TelemetryConfig::new_undecided()
    };
    write_telemetry_config(&path, &config).unwrap();
    std::env::set_var("CI", "true");
    let status = resolve_telemetry_status(Some("phc_test"), &path);
    assert_eq!(status, TelemetryStatus::Disabled);
    std::env::remove_var("CI");
}

#[test]
fn file_enabled_true_means_enabled() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config = TelemetryConfig {
        enabled: Some(true),
        ..TelemetryConfig::new_undecided()
    };
    write_telemetry_config(&path, &config).unwrap();
    let status = resolve_telemetry_status(Some("phc_test"), &path);
    assert_eq!(status, TelemetryStatus::Enabled);
}

#[test]
fn file_enabled_false_means_disabled() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config = TelemetryConfig {
        enabled: Some(false),
        ..TelemetryConfig::new_undecided()
    };
    write_telemetry_config(&path, &config).unwrap();
    let status = resolve_telemetry_status(Some("phc_test"), &path);
    assert_eq!(status, TelemetryStatus::Disabled);
}

#[test]
fn file_missing_means_undecided() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let status = resolve_telemetry_status(Some("phc_test"), &path);
    assert_eq!(status, TelemetryStatus::Undecided);
}

#[test]
fn file_enabled_null_means_undecided() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config = TelemetryConfig::new_undecided();
    write_telemetry_config(&path, &config).unwrap();
    let status = resolve_telemetry_status(Some("phc_test"), &path);
    assert_eq!(status, TelemetryStatus::Undecided);
}

// === Milestone dedup tests ===

use claude_view_core::telemetry_config::check_milestone;

#[test]
fn milestone_10_fires_at_10() {
    assert_eq!(check_milestone(10, 0), Some(10));
}

#[test]
fn milestone_skips_already_fired() {
    assert_eq!(check_milestone(10, 10), None);
}

#[test]
fn milestone_catches_highest() {
    assert_eq!(check_milestone(150, 10), Some(100));
}

#[test]
fn milestone_none_below_10() {
    assert_eq!(check_milestone(5, 0), None);
}

#[test]
fn milestone_jumps_multiple() {
    assert_eq!(check_milestone(500, 0), Some(500));
}

// === Init flow tests (Task 6 prep) ===

#[test]
fn init_flow_creates_config_then_resolves_undecided() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    claude_view_core::telemetry_config::create_telemetry_config_if_missing(&path).unwrap();
    assert!(path.exists());
    let config = read_telemetry_config(&path);
    assert!(config.enabled.is_none());
    assert!(!config.anonymous_id.is_empty());
    let status = resolve_telemetry_status(Some("phc_test"), &path);
    assert_eq!(status, TelemetryStatus::Undecided);
}

#[test]
fn create_if_missing_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    claude_view_core::telemetry_config::create_telemetry_config_if_missing(&path).unwrap();
    let first = read_telemetry_config(&path);
    claude_view_core::telemetry_config::create_telemetry_config_if_missing(&path).unwrap();
    let second = read_telemetry_config(&path);
    assert_eq!(
        first.anonymous_id, second.anonymous_id,
        "create_if_missing must not overwrite existing config"
    );
}

#[test]
fn post_index_milestone_flow() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config = TelemetryConfig {
        enabled: Some(true),
        anonymous_id: "test-uuid".to_string(),
        consent_given_at: Some("2026-03-19T14:00:00Z".to_string()),
        last_milestone: None,
        ..TelemetryConfig::new_undecided()
    };
    write_telemetry_config(&path, &config).unwrap();
    let mut config = read_telemetry_config(&path);
    let session_count = 150u64;
    if let Some(milestone) = check_milestone(session_count, config.last_milestone.unwrap_or(0)) {
        config.last_milestone = Some(milestone);
        write_telemetry_config(&path, &config).unwrap();
    }
    let final_config = read_telemetry_config(&path);
    assert_eq!(final_config.last_milestone, Some(100));
    assert_eq!(
        check_milestone(150, final_config.last_milestone.unwrap_or(0)),
        None
    );
}

#[test]
fn first_index_completed_fires_only_once() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let config = TelemetryConfig::new_undecided();
    write_telemetry_config(&path, &config).unwrap();
    let c = read_telemetry_config(&path);
    assert!(
        !c.first_index_completed,
        "fresh install: first_index_completed = false → fire event"
    );
    let mut c2 = c.clone();
    c2.first_index_completed = true;
    write_telemetry_config(&path, &c2).unwrap();
    let c3 = read_telemetry_config(&path);
    assert!(
        c3.first_index_completed,
        "after first index: flag set → skip event"
    );
}

#[test]
fn first_index_completed_dedup_works_below_milestone_threshold() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("telemetry.json");
    let mut config = TelemetryConfig::new_undecided();
    config.first_index_completed = true;
    config.last_milestone = None;
    write_telemetry_config(&path, &config).unwrap();
    let read_back = read_telemetry_config(&path);
    assert!(
        read_back.first_index_completed,
        "flag persists even without reaching a milestone"
    );
    assert!(
        read_back.last_milestone.is_none(),
        "no milestone for < 10 sessions"
    );
}
