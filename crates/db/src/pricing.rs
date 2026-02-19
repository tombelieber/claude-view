//! Pricing data management: litellm fetch + merge with defaults.

use std::collections::HashMap;
use vibe_recall_core::pricing::ModelPricing;

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

        result.insert(
            model_id,
            ModelPricing {
                input_cost_per_token,
                output_cost_per_token,
                cache_creation_cost_per_token,
                cache_read_cost_per_token,
                // litellm does not reliably encode Anthropic's 200k tiering.
                input_cost_per_token_above_200k: None,
                output_cost_per_token_above_200k: None,
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
                    input_cost_per_token_above_200k: existing.input_cost_per_token_above_200k,
                    output_cost_per_token_above_200k: existing.output_cost_per_token_above_200k,
                },
            );
        } else {
            merged.insert(key.clone(), litellm_pricing.clone());
        }
    }

    merged
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
            },
        );

        let merged = merge_pricing(&defaults, &litellm);
        let merged_model = merged.get("claude-opus-4-6").unwrap();

        assert_eq!(merged_model.input_cost_per_token, 4e-6);
        assert_eq!(merged_model.output_cost_per_token, 20e-6);
        assert_eq!(merged_model.input_cost_per_token_above_200k, Some(10e-6));
        assert_eq!(merged_model.output_cost_per_token_above_200k, Some(37.5e-6));
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
            },
        );

        let merged = merge_pricing(&defaults, &litellm);
        assert!(merged.contains_key("claude-new-model"));
    }
}
