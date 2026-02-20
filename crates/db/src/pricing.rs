//! Pricing data management: litellm fetch + merge with defaults.

use std::collections::HashMap;
use claude_view_core::pricing::ModelPricing;

const LITELLM_URL: &str =
    "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json";

/// Fetch Claude model pricing from litellm's community-maintained JSON.
///
/// Filters to `claude-`, `claude/`, and `anthropic/` entries.
/// Returns a map keyed by model ID without provider prefix.
/// On any network or parse error, returns `Err`.
pub async fn fetch_litellm_pricing() -> Result<HashMap<String, ModelPricing>, String> {
    let response = reqwest::get(LITELLM_URL)
        .await
        .map_err(|e| format!("HTTP fetch failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("HTTP status error: {e}"))?;

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("JSON parse failed: {e}"))?;

    let obj = body.as_object().ok_or("Expected top-level JSON object")?;
    let mut result = HashMap::new();

    for (key, value) in obj {
        let model_id = if let Some(stripped) = key.strip_prefix("anthropic/") {
            stripped.to_string()
        } else if let Some(stripped) = key.strip_prefix("claude/") {
            stripped.to_string()
        } else if key.starts_with("claude-") {
            key.clone()
        } else {
            continue;
        };

        let input = value.get("input_cost_per_token").and_then(|v| v.as_f64());
        let output = value.get("output_cost_per_token").and_then(|v| v.as_f64());

        let (input_cost_per_token, output_cost_per_token) = match (input, output) {
            (Some(input), Some(output)) if input > 0.0 && output > 0.0 => (input, output),
            _ => continue,
        };

        let cache_read_cost_per_token = value
            .get("cache_read_input_token_cost")
            .and_then(|v| v.as_f64())
            .unwrap_or(input_cost_per_token * 0.1);

        let cache_creation_cost_per_token = value
            .get("cache_creation_input_token_cost")
            .and_then(|v| v.as_f64())
            .unwrap_or(input_cost_per_token * 1.25);

        let input_cost_per_token_above_200k = value
            .get("input_cost_per_token_above_200k_tokens")
            .and_then(|v| v.as_f64());

        let output_cost_per_token_above_200k = value
            .get("output_cost_per_token_above_200k_tokens")
            .and_then(|v| v.as_f64());

        let cache_creation_cost_per_token_above_200k = value
            .get("cache_creation_input_token_cost_above_200k_tokens")
            .and_then(|v| v.as_f64());

        let cache_read_cost_per_token_above_200k = value
            .get("cache_read_input_token_cost_above_200k_tokens")
            .and_then(|v| v.as_f64());

        let cache_creation_cost_per_token_1hr = value
            .get("cache_creation_input_token_cost_above_1hr")
            .and_then(|v| v.as_f64());

        result.insert(
            model_id,
            ModelPricing {
                input_cost_per_token,
                output_cost_per_token,
                cache_creation_cost_per_token,
                cache_read_cost_per_token,
                input_cost_per_token_above_200k,
                output_cost_per_token_above_200k,
                cache_creation_cost_per_token_above_200k,
                cache_read_cost_per_token_above_200k,
                cache_creation_cost_per_token_1hr,
            },
        );
    }

    if result.is_empty() {
        return Err("No Claude models found in litellm data".into());
    }

    Ok(result)
}

/// Merge litellm pricing into hardcoded defaults.
///
/// - litellm entries override base rates for matching model IDs.
/// - default-only models remain available.
/// - 200k tiering from defaults is preserved when a key already exists.
pub fn merge_pricing(
    defaults: &HashMap<String, ModelPricing>,
    litellm: &HashMap<String, ModelPricing>,
) -> HashMap<String, ModelPricing> {
    let mut merged = defaults.clone();

    for (key, litellm_pricing) in litellm {
        if let Some(existing) = merged.get(key) {
            merged.insert(
                key.clone(),
                ModelPricing {
                    input_cost_per_token: litellm_pricing.input_cost_per_token,
                    output_cost_per_token: litellm_pricing.output_cost_per_token,
                    cache_creation_cost_per_token: litellm_pricing.cache_creation_cost_per_token,
                    cache_read_cost_per_token: litellm_pricing.cache_read_cost_per_token,
                    input_cost_per_token_above_200k: litellm_pricing.input_cost_per_token_above_200k.or(existing.input_cost_per_token_above_200k),
                    output_cost_per_token_above_200k: litellm_pricing.output_cost_per_token_above_200k.or(existing.output_cost_per_token_above_200k),
                    cache_creation_cost_per_token_above_200k: litellm_pricing.cache_creation_cost_per_token_above_200k.or(existing.cache_creation_cost_per_token_above_200k),
                    cache_read_cost_per_token_above_200k: litellm_pricing.cache_read_cost_per_token_above_200k.or(existing.cache_read_cost_per_token_above_200k),
                    cache_creation_cost_per_token_1hr: litellm_pricing.cache_creation_cost_per_token_1hr.or(existing.cache_creation_cost_per_token_1hr),
                },
            );
        } else {
            merged.insert(key.clone(), litellm_pricing.clone());
        }
    }

    merged
}

/// Persist the full pricing map to SQLite for cross-restart durability.
///
/// Overwrites any previous cache. Called after successful litellm fetch + merge.
pub async fn save_pricing_cache(
    db: &crate::Database,
    pricing: &HashMap<String, ModelPricing>,
) -> Result<(), String> {
    let json = serde_json::to_string(pricing).map_err(|e| format!("serialize: {e}"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    sqlx::query(
        "INSERT OR REPLACE INTO pricing_cache (id, data, fetched_at) VALUES (1, ?, ?)",
    )
    .bind(&json)
    .bind(now)
    .execute(db.pool())
    .await
    .map_err(|e| format!("save pricing cache: {e}"))?;

    Ok(())
}

/// Load the cached pricing map from SQLite.
///
/// Returns `None` if no cache exists (first run). Returns the full map otherwise.
pub async fn load_pricing_cache(
    db: &crate::Database,
) -> Result<Option<HashMap<String, ModelPricing>>, String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT data FROM pricing_cache WHERE id = 1")
            .fetch_optional(db.pool())
            .await
            .map_err(|e| format!("load pricing cache: {e}"))?;

    match row {
        Some((json,)) => {
            let map: HashMap<String, ModelPricing> =
                serde_json::from_str(&json).map_err(|e| format!("deserialize: {e}"))?;
            Ok(Some(map))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mp(input: f64, output: f64, cache_read: f64, cache_write: f64) -> ModelPricing {
        ModelPricing {
            input_cost_per_token: input,
            output_cost_per_token: output,
            cache_creation_cost_per_token: cache_write,
            cache_read_cost_per_token: cache_read,
            input_cost_per_token_above_200k: Some(input * 2.0),
            output_cost_per_token_above_200k: Some(output * 1.5),
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        }
    }

    #[test]
    fn test_merge_preserves_tiering_on_existing_model() {
        let mut defaults = HashMap::new();
        defaults.insert(
            "claude-opus-4-6".to_string(),
            mp(5e-6, 25e-6, 0.5e-6, 6.25e-6),
        );

        let mut litellm = HashMap::new();
        litellm.insert(
            "claude-opus-4-6".to_string(),
            ModelPricing {
                input_cost_per_token: 4e-6,
                output_cost_per_token: 20e-6,
                cache_creation_cost_per_token: 5e-6,
                cache_read_cost_per_token: 0.4e-6,
                input_cost_per_token_above_200k: None,
                output_cost_per_token_above_200k: None,
                cache_creation_cost_per_token_above_200k: None,
                cache_read_cost_per_token_above_200k: None,
                cache_creation_cost_per_token_1hr: None,
            },
        );

        let merged = merge_pricing(&defaults, &litellm);
        let merged_model = merged.get("claude-opus-4-6").unwrap();

        assert_eq!(merged_model.input_cost_per_token, 4e-6);
        assert_eq!(merged_model.output_cost_per_token, 20e-6);
        assert_eq!(merged_model.input_cost_per_token_above_200k, Some(10e-6));
        assert_eq!(merged_model.output_cost_per_token_above_200k, Some(25e-6 * 1.5));
        assert_eq!(merged_model.cache_creation_cost_per_token_above_200k, None);
        assert_eq!(merged_model.cache_read_cost_per_token_above_200k, None);
        assert_eq!(merged_model.cache_creation_cost_per_token_1hr, None);
    }

    #[test]
    fn test_merge_adds_new_litellm_models() {
        let defaults = HashMap::new();
        let mut litellm = HashMap::new();
        litellm.insert(
            "claude-new-model".to_string(),
            ModelPricing {
                input_cost_per_token: 2e-6,
                output_cost_per_token: 10e-6,
                cache_creation_cost_per_token: 2.5e-6,
                cache_read_cost_per_token: 0.2e-6,
                input_cost_per_token_above_200k: None,
                output_cost_per_token_above_200k: None,
                cache_creation_cost_per_token_above_200k: None,
                cache_read_cost_per_token_above_200k: None,
                cache_creation_cost_per_token_1hr: None,
            },
        );

        let merged = merge_pricing(&defaults, &litellm);
        assert!(merged.contains_key("claude-new-model"));
    }

    #[test]
    fn test_merge_uses_litellm_tiering_over_defaults() {
        let mut defaults = HashMap::new();
        defaults.insert(
            "claude-opus-4-6".to_string(),
            ModelPricing {
                input_cost_per_token: 5e-6,
                output_cost_per_token: 25e-6,
                cache_creation_cost_per_token: 6.25e-6,
                cache_read_cost_per_token: 0.5e-6,
                input_cost_per_token_above_200k: Some(10e-6),
                output_cost_per_token_above_200k: Some(37.5e-6),
                cache_creation_cost_per_token_above_200k: Some(12.5e-6),
                cache_read_cost_per_token_above_200k: Some(1e-6),
                cache_creation_cost_per_token_1hr: Some(10e-6),
            },
        );

        let mut litellm = HashMap::new();
        litellm.insert(
            "claude-opus-4-6".to_string(),
            ModelPricing {
                input_cost_per_token: 5e-6,
                output_cost_per_token: 25e-6,
                cache_creation_cost_per_token: 6.25e-6,
                cache_read_cost_per_token: 0.5e-6,
                input_cost_per_token_above_200k: Some(11e-6),
                output_cost_per_token_above_200k: Some(38e-6),
                cache_creation_cost_per_token_above_200k: Some(13e-6),
                cache_read_cost_per_token_above_200k: Some(1.1e-6),
                cache_creation_cost_per_token_1hr: Some(11e-6),
            },
        );

        let merged = merge_pricing(&defaults, &litellm);
        let m = merged.get("claude-opus-4-6").unwrap();
        assert_eq!(m.input_cost_per_token_above_200k, Some(11e-6));
        assert_eq!(m.cache_creation_cost_per_token_above_200k, Some(13e-6));
        assert_eq!(m.cache_creation_cost_per_token_1hr, Some(11e-6));
    }

    #[tokio::test]
    async fn test_save_and_load_pricing_cache() {
        let db = crate::Database::new_in_memory().await.unwrap();
        let mut map = HashMap::new();
        map.insert(
            "claude-opus-4-6".to_string(),
            ModelPricing {
                input_cost_per_token: 5e-6,
                output_cost_per_token: 25e-6,
                cache_creation_cost_per_token: 6.25e-6,
                cache_read_cost_per_token: 0.5e-6,
                input_cost_per_token_above_200k: Some(10e-6),
                output_cost_per_token_above_200k: Some(37.5e-6),
                cache_creation_cost_per_token_above_200k: Some(12.5e-6),
                cache_read_cost_per_token_above_200k: Some(1e-6),
                cache_creation_cost_per_token_1hr: Some(10e-6),
            },
        );

        save_pricing_cache(&db, &map).await.unwrap();
        let loaded = load_pricing_cache(&db).await.unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.len(), 1);
        let m = loaded.get("claude-opus-4-6").unwrap();
        assert_eq!(m.input_cost_per_token, 5e-6);
        assert_eq!(m.cache_creation_cost_per_token_1hr, Some(10e-6));
    }

    #[tokio::test]
    async fn test_load_empty_cache_returns_none() {
        let db = crate::Database::new_in_memory().await.unwrap();
        let loaded = load_pricing_cache(&db).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_save_overwrites_previous_cache() {
        let db = crate::Database::new_in_memory().await.unwrap();

        let mut map1 = HashMap::new();
        map1.insert("model-a".to_string(), ModelPricing {
            input_cost_per_token: 1e-6,
            output_cost_per_token: 5e-6,
            cache_creation_cost_per_token: 1.25e-6,
            cache_read_cost_per_token: 0.1e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        });
        save_pricing_cache(&db, &map1).await.unwrap();

        let mut map2 = HashMap::new();
        map2.insert("model-b".to_string(), ModelPricing {
            input_cost_per_token: 2e-6,
            output_cost_per_token: 10e-6,
            cache_creation_cost_per_token: 2.5e-6,
            cache_read_cost_per_token: 0.2e-6,
            input_cost_per_token_above_200k: None,
            output_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_above_200k: None,
            cache_read_cost_per_token_above_200k: None,
            cache_creation_cost_per_token_1hr: None,
        });
        save_pricing_cache(&db, &map2).await.unwrap();

        let loaded = load_pricing_cache(&db).await.unwrap().unwrap();
        assert!(!loaded.contains_key("model-a"));
        assert!(loaded.contains_key("model-b"));
    }

    #[tokio::test]
    async fn test_three_tier_fallback_litellm_to_cache() {
        use claude_view_core::pricing::fill_tiering_gaps;

        let db = crate::Database::new_in_memory().await.unwrap();

        // Simulate: litellm succeeded previously, saved to cache
        let mut original = HashMap::new();
        original.insert(
            "claude-opus-4-6".to_string(),
            ModelPricing {
                input_cost_per_token: 5e-6,
                output_cost_per_token: 25e-6,
                cache_creation_cost_per_token: 6.25e-6,
                cache_read_cost_per_token: 0.5e-6,
                input_cost_per_token_above_200k: Some(10e-6),
                output_cost_per_token_above_200k: Some(37.5e-6),
                cache_creation_cost_per_token_above_200k: Some(12.5e-6),
                cache_read_cost_per_token_above_200k: Some(1e-6),
                cache_creation_cost_per_token_1hr: Some(10e-6),
            },
        );
        save_pricing_cache(&db, &original).await.unwrap();

        // Simulate: litellm fails, load from cache
        let mut cached = load_pricing_cache(&db).await.unwrap().unwrap();
        fill_tiering_gaps(&mut cached);

        let m = cached.get("claude-opus-4-6").unwrap();
        assert_eq!(m.input_cost_per_token, 5e-6);
        assert_eq!(m.input_cost_per_token_above_200k, Some(10e-6));
        assert_eq!(m.cache_creation_cost_per_token_1hr, Some(10e-6));
    }
}
