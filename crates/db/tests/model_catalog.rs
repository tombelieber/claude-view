//! Integration tests for model catalog — context upsert and seed behavior.

use claude_view_db::{Database, ModelContext};

async fn test_db() -> Database {
    Database::new_in_memory().await.unwrap()
}

// === Unit-level: NULL handling for legacy/unknown models ===

#[tokio::test]
async fn legacy_model_appears_with_null_metadata() {
    let db = test_db().await;

    sqlx::query("INSERT OR REPLACE INTO models (id, provider, family, first_seen, last_seen) VALUES (?, ?, ?, ?, ?)")
        .bind("claude-3-opus-20240229")
        .bind("anthropic")
        .bind("opus")
        .bind(1700000000_i64)
        .bind(1700000000_i64)
        .execute(db.pool())
        .await
        .unwrap();

    let models = db.get_all_models().await.unwrap();
    let legacy = models
        .iter()
        .find(|m| m.id == "claude-3-opus-20240229")
        .unwrap();
    assert!(legacy.display_name.is_none());
    assert!(legacy.description.is_none());
    assert!(legacy.max_input_tokens.is_none());
    assert!(legacy.max_output_tokens.is_none());
    assert_eq!(legacy.provider.as_deref(), Some("anthropic"));
}

// === Unit-level: Seed idempotency ===

#[tokio::test]
async fn seed_runs_during_construction_and_is_idempotent() {
    let db = test_db().await;

    let models = db.get_all_models().await.unwrap();
    let initial_count = models.len();
    assert!(
        initial_count > 0,
        "constructor should have seeded models from pricing JSON"
    );

    // Calling seed again should be a no-op (table is not empty)
    db.seed_models_if_empty().await.unwrap();
    let models2 = db.get_all_models().await.unwrap();
    assert_eq!(
        models2.len(),
        initial_count,
        "seed should not duplicate rows"
    );
}

// === Model context upsert behavior ===

#[tokio::test]
async fn repeated_context_upsert_updates_values() {
    let db = test_db().await;

    db.upsert_model_context(&[ModelContext {
        model_id: "claude-opus-4-6".into(),
        provider: "anthropic".into(),
        family: "opus".into(),
        max_input_tokens: Some(200_000),
        max_output_tokens: Some(32_000),
    }])
    .await
    .unwrap();

    db.upsert_model_context(&[ModelContext {
        model_id: "claude-opus-4-6".into(),
        provider: "anthropic".into(),
        family: "opus".into(),
        max_input_tokens: Some(1_000_000),
        max_output_tokens: Some(64_000),
    }])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    let opus = models.iter().find(|m| m.id == "claude-opus-4-6").unwrap();
    assert_eq!(opus.max_input_tokens, Some(1_000_000));
    assert_eq!(opus.max_output_tokens, Some(64_000));
}

#[tokio::test]
async fn null_context_values_do_not_overwrite_existing() {
    let db = test_db().await;

    db.upsert_model_context(&[ModelContext {
        model_id: "claude-opus-4-6".into(),
        provider: "anthropic".into(),
        family: "opus".into(),
        max_input_tokens: Some(1_000_000),
        max_output_tokens: Some(64_000),
    }])
    .await
    .unwrap();

    db.upsert_model_context(&[ModelContext {
        model_id: "claude-opus-4-6".into(),
        provider: "anthropic".into(),
        family: "opus".into(),
        max_input_tokens: Some(1_000_000),
        max_output_tokens: None,
    }])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    let opus = models.iter().find(|m| m.id == "claude-opus-4-6").unwrap();
    assert_eq!(
        opus.max_output_tokens,
        Some(64_000),
        "NULL should not overwrite existing value via COALESCE"
    );
}

#[tokio::test]
async fn new_model_from_context_upsert_gets_inserted() {
    let db = test_db().await;

    db.upsert_model_context(&[ModelContext {
        model_id: "claude-5-opus-20260601".into(),
        provider: "anthropic".into(),
        family: "opus".into(),
        max_input_tokens: Some(2_000_000),
        max_output_tokens: Some(128_000),
    }])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    let new_model = models.iter().find(|m| m.id == "claude-5-opus-20260601");
    assert!(new_model.is_some());
    assert_eq!(new_model.unwrap().max_input_tokens, Some(2_000_000));
}

#[tokio::test]
async fn context_upsert_does_not_set_display_name() {
    let db = test_db().await;

    db.upsert_model_context(&[ModelContext {
        model_id: "claude-opus-4-6".into(),
        provider: "anthropic".into(),
        family: "opus".into(),
        max_input_tokens: Some(1_000_000),
        max_output_tokens: Some(64_000),
    }])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    let opus = models.iter().find(|m| m.id == "claude-opus-4-6").unwrap();
    assert!(opus.display_name.is_none());
}

// === Full merge lifecycle (seed → context upsert) ===

#[tokio::test]
async fn full_merge_lifecycle_seed_then_context_upsert() {
    let db = test_db().await;

    db.seed_models_if_empty().await.unwrap();
    let models = db.get_all_models().await.unwrap();
    let opus = models.iter().find(|m| m.id == "claude-opus-4-6");
    assert!(opus.is_some());
    assert!(opus.unwrap().max_input_tokens.is_none());

    db.upsert_model_context(&[ModelContext {
        model_id: "claude-opus-4-6".into(),
        provider: "anthropic".into(),
        family: "opus".into(),
        max_input_tokens: Some(1_000_000),
        max_output_tokens: Some(32_000),
    }])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    let opus = models.iter().find(|m| m.id == "claude-opus-4-6").unwrap();
    assert_eq!(opus.max_input_tokens, Some(1_000_000));
}
