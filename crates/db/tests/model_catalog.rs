//! Integration tests for model catalog COALESCE merge behavior.
//! TDD: Written before implementation — these tests define the contract.

use claude_view_db::{Database, LiteLlmModelContext};

async fn test_db() -> Database {
    Database::new_in_memory().await.unwrap()
}

// === Unit-level: COALESCE merge correctness ===

#[tokio::test]
async fn litellm_upsert_does_not_overwrite_sdk_fields() {
    let db = test_db().await;

    // First: SDK upsert sets display_name + description
    db.upsert_sdk_models(&[(
        "claude-opus-4-6".into(),
        "anthropic".into(),
        "opus".into(),
        Some("Claude Opus 4.6".into()),
        Some("Most capable".into()),
    )])
    .await
    .unwrap();

    // Then: LiteLLM upsert sets max_input_tokens — should NOT overwrite display_name
    db.upsert_litellm_context(&[LiteLlmModelContext {
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
    assert_eq!(opus.display_name.as_deref(), Some("Claude Opus 4.6"));
    assert_eq!(opus.description.as_deref(), Some("Most capable"));
    assert_eq!(opus.max_input_tokens, Some(1_000_000));
    assert_eq!(opus.max_output_tokens, Some(64_000));
}

#[tokio::test]
async fn sdk_upsert_does_not_overwrite_litellm_fields() {
    let db = test_db().await;

    // First: LiteLLM upsert sets context window
    db.upsert_litellm_context(&[LiteLlmModelContext {
        model_id: "claude-sonnet-4-6".into(),
        provider: "anthropic".into(),
        family: "sonnet".into(),
        max_input_tokens: Some(200_000),
        max_output_tokens: Some(64_000),
    }])
    .await
    .unwrap();

    // Then: SDK upsert sets display_name — should NOT overwrite max_input_tokens
    db.upsert_sdk_models(&[(
        "claude-sonnet-4-6".into(),
        "anthropic".into(),
        "sonnet".into(),
        Some("Claude Sonnet 4.6".into()),
        Some("Best for everyday tasks".into()),
    )])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    let sonnet = models.iter().find(|m| m.id == "claude-sonnet-4-6").unwrap();
    assert_eq!(sonnet.max_input_tokens, Some(200_000));
    assert_eq!(sonnet.max_output_tokens, Some(64_000));
    assert_eq!(sonnet.display_name.as_deref(), Some("Claude Sonnet 4.6"));
    assert_eq!(
        sonnet.description.as_deref(),
        Some("Best for everyday tasks")
    );
}

// === Unit-level: NULL handling for legacy/unknown models ===

#[tokio::test]
async fn legacy_model_appears_with_null_metadata() {
    let db = test_db().await;

    // Simulate indexer inserting a model from user history (no LiteLLM or SDK data)
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
    // new_in_memory() calls run_migrations() then seed_models_if_empty(),
    // so the table is already seeded by the time we get here.

    let models = db.get_all_models().await.unwrap();
    let initial_count = models.len();
    assert!(
        initial_count > 0,
        "constructor should have seeded models from default_pricing()"
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

// === Integration: Full merge lifecycle ===

#[tokio::test]
async fn full_merge_lifecycle_seed_then_litellm_then_sdk() {
    let db = test_db().await;

    // Step 1: Seed populates baseline model IDs
    db.seed_models_if_empty().await.unwrap();
    let models = db.get_all_models().await.unwrap();
    let opus = models.iter().find(|m| m.id == "claude-opus-4-6");
    assert!(
        opus.is_some(),
        "seed should include claude-opus-4-6 from default_pricing()"
    );
    let opus = opus.unwrap();
    assert!(
        opus.display_name.is_none(),
        "seed should not set display_name"
    );
    assert!(
        opus.max_input_tokens.is_none(),
        "seed should not set max_input_tokens"
    );

    // Step 2: LiteLLM enriches with context window
    db.upsert_litellm_context(&[LiteLlmModelContext {
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
    assert!(
        opus.display_name.is_none(),
        "LiteLLM should not set display_name"
    );

    // Step 3: SDK enriches with display name
    db.upsert_sdk_models(&[(
        "claude-opus-4-6".into(),
        "anthropic".into(),
        "opus".into(),
        Some("Claude Opus 4.6".into()),
        Some("Most capable for complex work".into()),
    )])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    let opus = models.iter().find(|m| m.id == "claude-opus-4-6").unwrap();
    assert_eq!(
        opus.max_input_tokens,
        Some(1_000_000),
        "SDK upsert must preserve LiteLLM data"
    );
    assert_eq!(opus.display_name.as_deref(), Some("Claude Opus 4.6"));
    assert_eq!(
        opus.description.as_deref(),
        Some("Most capable for complex work")
    );
}

// === Regression: COALESCE does not clobber on repeated upserts ===

#[tokio::test]
async fn repeated_litellm_upsert_updates_values() {
    let db = test_db().await;

    // First upsert: Opus has 200K context (hypothetical old value)
    db.upsert_litellm_context(&[LiteLlmModelContext {
        model_id: "claude-opus-4-6".into(),
        provider: "anthropic".into(),
        family: "opus".into(),
        max_input_tokens: Some(200_000),
        max_output_tokens: Some(32_000),
    }])
    .await
    .unwrap();

    // Second upsert: Opus upgraded to 1M context
    db.upsert_litellm_context(&[LiteLlmModelContext {
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
    assert_eq!(
        opus.max_input_tokens,
        Some(1_000_000),
        "should update to new value, not keep old"
    );
    assert_eq!(opus.max_output_tokens, Some(64_000));
}

// === Regression: NULL source values do not overwrite existing non-NULL values ===

#[tokio::test]
async fn null_litellm_values_do_not_overwrite_existing() {
    let db = test_db().await;

    // First: LiteLLM sets context window
    db.upsert_litellm_context(&[LiteLlmModelContext {
        model_id: "claude-opus-4-6".into(),
        provider: "anthropic".into(),
        family: "opus".into(),
        max_input_tokens: Some(1_000_000),
        max_output_tokens: Some(64_000),
    }])
    .await
    .unwrap();

    // Second: LiteLLM upserts same model but with NULL max_output_tokens
    db.upsert_litellm_context(&[LiteLlmModelContext {
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

// === Regression: New unknown model from LiteLLM gets inserted ===

#[tokio::test]
async fn new_model_from_litellm_gets_inserted() {
    let db = test_db().await;

    db.upsert_litellm_context(&[LiteLlmModelContext {
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
    assert!(
        new_model.is_some(),
        "new model from LiteLLM should be inserted"
    );
    let new_model = new_model.unwrap();
    assert_eq!(new_model.max_input_tokens, Some(2_000_000));
    assert_eq!(new_model.provider.as_deref(), Some("anthropic"));
}

// === sdk_supported flag: SDK is the source of truth ===

#[tokio::test]
async fn sdk_upsert_sets_sdk_supported_flag() {
    let db = test_db().await;

    // Seed creates models with sdk_supported = 0
    let models = db.get_all_models().await.unwrap();
    let opus = models.iter().find(|m| m.id == "claude-opus-4-6").unwrap();
    assert!(
        !opus.sdk_supported,
        "seeded models start as NOT sdk_supported"
    );

    // SDK upsert marks them as supported
    db.upsert_sdk_models(&[(
        "claude-opus-4-6".into(),
        "anthropic".into(),
        "opus".into(),
        Some("Claude Opus 4.6".into()),
        None,
    )])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    let opus = models.iter().find(|m| m.id == "claude-opus-4-6").unwrap();
    assert!(
        opus.sdk_supported,
        "SDK upsert must set sdk_supported = true"
    );
}

#[tokio::test]
async fn sdk_upsert_clears_stale_sdk_supported_flags() {
    let db = test_db().await;

    // First SDK upsert: opus + sonnet are supported
    db.upsert_sdk_models(&[
        (
            "claude-opus-4-6".into(),
            "anthropic".into(),
            "opus".into(),
            Some("Claude Opus 4.6".into()),
            None,
        ),
        (
            "claude-sonnet-4-6".into(),
            "anthropic".into(),
            "sonnet".into(),
            Some("Claude Sonnet 4.6".into()),
            None,
        ),
    ])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    assert!(
        models
            .iter()
            .find(|m| m.id == "claude-opus-4-6")
            .unwrap()
            .sdk_supported
    );
    assert!(
        models
            .iter()
            .find(|m| m.id == "claude-sonnet-4-6")
            .unwrap()
            .sdk_supported
    );

    // Second SDK upsert: only sonnet is supported (opus removed from SDK)
    db.upsert_sdk_models(&[(
        "claude-sonnet-4-6".into(),
        "anthropic".into(),
        "sonnet".into(),
        Some("Claude Sonnet 4.6".into()),
        None,
    )])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    assert!(
        !models
            .iter()
            .find(|m| m.id == "claude-opus-4-6")
            .unwrap()
            .sdk_supported,
        "opus must be cleared when SDK no longer reports it"
    );
    assert!(
        models
            .iter()
            .find(|m| m.id == "claude-sonnet-4-6")
            .unwrap()
            .sdk_supported,
        "sonnet must remain supported"
    );
}

#[tokio::test]
async fn litellm_upsert_does_not_affect_sdk_supported() {
    let db = test_db().await;

    // SDK marks opus as supported
    db.upsert_sdk_models(&[(
        "claude-opus-4-6".into(),
        "anthropic".into(),
        "opus".into(),
        Some("Claude Opus 4.6".into()),
        None,
    )])
    .await
    .unwrap();

    // LiteLLM upsert updates context window — must NOT change sdk_supported
    db.upsert_litellm_context(&[LiteLlmModelContext {
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
    assert!(
        opus.sdk_supported,
        "LiteLLM upsert must NOT clear sdk_supported flag"
    );
    assert_eq!(opus.max_input_tokens, Some(1_000_000));
}

#[tokio::test]
async fn models_not_in_sdk_have_sdk_supported_false() {
    let db = test_db().await;

    // Only upsert via LiteLLM (NOT SDK)
    db.upsert_litellm_context(&[LiteLlmModelContext {
        model_id: "claude-3-opus-20240229".into(),
        provider: "anthropic".into(),
        family: "opus".into(),
        max_input_tokens: Some(200_000),
        max_output_tokens: Some(4_096),
    }])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    let legacy = models
        .iter()
        .find(|m| m.id == "claude-3-opus-20240229")
        .unwrap();
    assert!(
        !legacy.sdk_supported,
        "models only from LiteLLM/indexer must NOT be sdk_supported"
    );
}

// === Regression: SDK upsert with NULL display_name clears stale alias names ===

#[tokio::test]
async fn sdk_upsert_null_display_name_clears_stale_alias() {
    let db = test_db().await;

    // Simulate old behavior: SDK previously wrote alias names like "Default (recommended)"
    db.upsert_sdk_models(&[(
        "claude-opus-4-6".into(),
        "anthropic".into(),
        "opus".into(),
        Some("Default (recommended)".into()),
        Some("Opus 4.6 with 1M context [NEW] · Most capable".into()),
    )])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    let opus = models.iter().find(|m| m.id == "claude-opus-4-6").unwrap();
    assert_eq!(opus.display_name.as_deref(), Some("Default (recommended)"));

    // New behavior: SDK upsert with NULL display_name must CLEAR the stale alias.
    // Frontend falls back to formatModelName("claude-opus-4-6") → "Claude Opus 4.6".
    db.upsert_sdk_models(&[(
        "claude-opus-4-6".into(),
        "anthropic".into(),
        "opus".into(),
        None, // intentionally NULL — don't use SDK alias names
        Some("Most capable for complex work".into()),
    )])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    let opus = models.iter().find(|m| m.id == "claude-opus-4-6").unwrap();
    assert!(
        opus.display_name.is_none(),
        "SDK upsert with None must CLEAR old display_name, not preserve it via COALESCE. Got: {:?}",
        opus.display_name
    );
    // Description should be updated to the new value
    assert_eq!(
        opus.description.as_deref(),
        Some("Most capable for complex work")
    );
}

#[tokio::test]
async fn sdk_upsert_null_description_clears_stale_description() {
    let db = test_db().await;

    // Old SDK wrote a verbose description
    db.upsert_sdk_models(&[(
        "claude-opus-4-6".into(),
        "anthropic".into(),
        "opus".into(),
        Some("Old Name".into()),
        Some("Old verbose description with SDK noise".into()),
    )])
    .await
    .unwrap();

    // New SDK passes None for both — must clear both
    db.upsert_sdk_models(&[(
        "claude-opus-4-6".into(),
        "anthropic".into(),
        "opus".into(),
        None,
        None,
    )])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    let opus = models.iter().find(|m| m.id == "claude-opus-4-6").unwrap();
    assert!(opus.display_name.is_none(), "display_name must be cleared");
    assert!(opus.description.is_none(), "description must be cleared");
    // sdk_supported should still be true
    assert!(opus.sdk_supported);
}

#[tokio::test]
async fn litellm_upsert_does_not_set_display_name() {
    let db = test_db().await;

    // LiteLLM only sets context window — never display_name
    db.upsert_litellm_context(&[LiteLlmModelContext {
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
    assert!(
        opus.display_name.is_none(),
        "LiteLLM must never set display_name — that's SDK's domain"
    );
}

// === Integration: get_all_models returns all fields correctly ===

#[tokio::test]
async fn get_all_models_returns_new_catalog_fields() {
    let db = test_db().await;

    // Insert via both sources
    db.upsert_sdk_models(&[(
        "claude-haiku-4-5-20251001".into(),
        "anthropic".into(),
        "haiku".into(),
        Some("Claude Haiku 4.5".into()),
        Some("Fastest for quick answers".into()),
    )])
    .await
    .unwrap();

    db.upsert_litellm_context(&[LiteLlmModelContext {
        model_id: "claude-haiku-4-5-20251001".into(),
        provider: "anthropic".into(),
        family: "haiku".into(),
        max_input_tokens: Some(200_000),
        max_output_tokens: Some(8_192),
    }])
    .await
    .unwrap();

    let models = db.get_all_models().await.unwrap();
    let haiku = models
        .iter()
        .find(|m| m.id == "claude-haiku-4-5-20251001")
        .unwrap();

    // Verify ALL new fields are present in the query result
    assert_eq!(haiku.display_name.as_deref(), Some("Claude Haiku 4.5"));
    assert_eq!(
        haiku.description.as_deref(),
        Some("Fastest for quick answers")
    );
    assert_eq!(haiku.max_input_tokens, Some(200_000));
    assert_eq!(haiku.max_output_tokens, Some(8_192));
    assert_eq!(haiku.provider.as_deref(), Some("anthropic"));
    assert_eq!(haiku.family.as_deref(), Some("haiku"));
    // Existing fields should still work
    assert_eq!(haiku.total_turns, 0);
    assert_eq!(haiku.total_sessions, 0);
}
