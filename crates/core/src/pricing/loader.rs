use std::collections::HashMap;

use super::types::ModelPricing;

const PRICING_JSON: &str = include_str!("../../../../data/anthropic-pricing.json");
const PER_MTOK: f64 = 1_000_000.0;

/// JSON schema types (deserialization only).
#[derive(serde::Deserialize)]
struct PricingFile {
    models: HashMap<String, JsonModel>,
    #[serde(default)]
    aliases: HashMap<String, String>,
}

#[derive(serde::Deserialize)]
struct JsonModel {
    input: f64,
    output: f64,
    cache_write_5m: f64,
    cache_write_1hr: f64,
    cache_read: f64,
    long_context_pricing: Option<LongContextPricing>,
}

#[derive(serde::Deserialize)]
struct LongContextPricing {
    input: f64,
    output: f64,
    cache_write_5m: f64,
    cache_write_1hr: f64,
    cache_read: f64,
}

/// Load pricing from the embedded `data/anthropic-pricing.json`.
///
/// Converts per-million-token USD rates to per-token rates.
/// Aliases are flattened into the HashMap for direct key lookup.
///
/// Panics at startup if the embedded JSON is malformed (compile-time guarantee).
pub fn load_pricing() -> HashMap<String, ModelPricing> {
    let file: PricingFile = serde_json::from_str(PRICING_JSON)
        .expect("embedded anthropic-pricing.json is invalid — fix data/anthropic-pricing.json");

    let mut map = HashMap::with_capacity(file.models.len() + file.aliases.len());

    for (model_id, jm) in &file.models {
        map.insert(model_id.clone(), convert_model(jm));
    }

    for (alias, target) in &file.aliases {
        if let Some(mp) = map.get(target) {
            map.insert(alias.clone(), mp.clone());
        }
    }

    map
}

fn convert_model(jm: &JsonModel) -> ModelPricing {
    let (above_input, above_output, above_cache_create, above_cache_read, cache_1hr) =
        match &jm.long_context_pricing {
            Some(lcp) => (
                Some(lcp.input / PER_MTOK),
                Some(lcp.output / PER_MTOK),
                Some(lcp.cache_write_5m / PER_MTOK),
                Some(lcp.cache_read / PER_MTOK),
                Some(lcp.cache_write_1hr / PER_MTOK),
            ),
            None => (None, None, None, None, Some(jm.cache_write_1hr / PER_MTOK)),
        };

    ModelPricing {
        input_cost_per_token: jm.input / PER_MTOK,
        output_cost_per_token: jm.output / PER_MTOK,
        cache_creation_cost_per_token: jm.cache_write_5m / PER_MTOK,
        cache_read_cost_per_token: jm.cache_read / PER_MTOK,
        input_cost_per_token_above_200k: above_input,
        output_cost_per_token_above_200k: above_output,
        cache_creation_cost_per_token_above_200k: above_cache_create,
        cache_read_cost_per_token_above_200k: above_cache_read,
        cache_creation_cost_per_token_1hr: cache_1hr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_pricing_parses_all_models() {
        let pricing = load_pricing();
        // 15 models + 3 aliases = 18 entries
        assert_eq!(pricing.len(), 18);
    }

    #[test]
    fn test_opus_46_no_above_200k_fields() {
        let pricing = load_pricing();
        let p = pricing.get("claude-opus-4-6").unwrap();
        assert!(
            p.input_cost_per_token_above_200k.is_none(),
            "Opus 4.6 should have flat 1M pricing (no above-200k tiering)"
        );
        assert!(p.output_cost_per_token_above_200k.is_none());
        assert!(p.cache_creation_cost_per_token_above_200k.is_none());
        assert!(p.cache_read_cost_per_token_above_200k.is_none());
        // 1hr cache rate should still be set
        assert!(p.cache_creation_cost_per_token_1hr.is_some());
    }

    #[test]
    fn test_sonnet_46_no_above_200k_fields() {
        let pricing = load_pricing();
        let p = pricing.get("claude-sonnet-4-6").unwrap();
        assert!(
            p.input_cost_per_token_above_200k.is_none(),
            "Sonnet 4.6 should have flat 1M pricing (no above-200k tiering)"
        );
    }

    #[test]
    fn test_sonnet_45_has_above_200k_tiering() {
        let pricing = load_pricing();
        let p = pricing.get("claude-sonnet-4-5-20250929").unwrap();
        assert!(p.input_cost_per_token_above_200k.is_some());
        assert!(p.output_cost_per_token_above_200k.is_some());
        // Input above 200k: $6/MTok = 6e-6
        assert!((p.input_cost_per_token_above_200k.unwrap() - 6e-6).abs() < 1e-15);
        // Output above 200k: $22.50/MTok = 22.5e-6
        assert!((p.output_cost_per_token_above_200k.unwrap() - 22.5e-6).abs() < 1e-15);
    }

    #[test]
    fn test_aliases_in_map() {
        let pricing = load_pricing();
        assert!(pricing.get("opus").is_some());
        assert!(pricing.get("sonnet").is_some());
        assert!(pricing.get("haiku").is_some());
        // Alias should have same rates as target
        let opus_alias = pricing.get("opus").unwrap();
        let opus_direct = pricing.get("claude-opus-4-6").unwrap();
        assert_eq!(
            opus_alias.input_cost_per_token,
            opus_direct.input_cost_per_token
        );
    }

    #[test]
    fn test_all_base_rates_match_official() {
        let pricing = load_pricing();

        // Spot-check current-gen models against official pricing
        let opus46 = pricing.get("claude-opus-4-6").unwrap();
        assert!((opus46.input_cost_per_token - 5e-6).abs() < 1e-15);
        assert!((opus46.output_cost_per_token - 25e-6).abs() < 1e-15);
        assert!((opus46.cache_creation_cost_per_token - 6.25e-6).abs() < 1e-15);
        assert!((opus46.cache_read_cost_per_token - 0.5e-6).abs() < 1e-15);

        let haiku45 = pricing.get("claude-haiku-4-5-20251001").unwrap();
        assert!((haiku45.input_cost_per_token - 1e-6).abs() < 1e-15);
        assert!((haiku45.output_cost_per_token - 5e-6).abs() < 1e-15);

        let haiku3 = pricing.get("claude-3-haiku-20240307").unwrap();
        assert!((haiku3.input_cost_per_token - 0.25e-6).abs() < 1e-15);
        assert!((haiku3.output_cost_per_token - 1.25e-6).abs() < 1e-15);
    }
}
