use std::collections::HashMap;

use super::types::ModelPricing;

/// Resolve model aliases ("haiku", "sonnet", "opus") to current-gen full model IDs.
///
/// Full model IDs pass through unchanged. Returns `None` for "inherit" or unknown aliases.
/// Note: aliases are also flattened into the pricing HashMap by `load_pricing()`,
/// so this function serves as a secondary fallback.
pub fn resolve_model_alias(alias: &str) -> Option<&'static str> {
    match alias {
        "haiku" => Some("claude-haiku-4-5-20251001"),
        "sonnet" => Some("claude-sonnet-4-6"),
        "opus" => Some("claude-opus-4-6"),
        _ if alias.starts_with("claude-") => None,
        _ => None,
    }
}

/// Look up pricing for a model ID.
///
/// Resolution order: exact match → alias resolution → prefix match → reverse prefix.
pub fn lookup_pricing<'a>(
    model_id: &str,
    pricing: &'a HashMap<String, ModelPricing>,
) -> Option<&'a ModelPricing> {
    if let Some(p) = pricing.get(model_id) {
        return Some(p);
    }
    if let Some(resolved) = resolve_model_alias(model_id) {
        if let Some(p) = pricing.get(resolved) {
            return Some(p);
        }
    }
    // Prefix matching: key is prefix of model_id
    for (key, p) in pricing {
        if model_id.starts_with(key.as_str()) {
            return Some(p);
        }
    }
    // Reverse prefix: model_id is prefix of key
    for (key, p) in pricing {
        if key.starts_with(model_id) {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::load_pricing;

    #[test]
    fn test_resolve_model_alias() {
        assert_eq!(
            resolve_model_alias("haiku"),
            Some("claude-haiku-4-5-20251001")
        );
        assert_eq!(resolve_model_alias("sonnet"), Some("claude-sonnet-4-6"));
        assert_eq!(resolve_model_alias("opus"), Some("claude-opus-4-6"));
        assert_eq!(resolve_model_alias("claude-opus-4-6"), None);
        assert_eq!(resolve_model_alias("inherit"), None);
        assert_eq!(resolve_model_alias("unknown"), None);
    }

    #[test]
    fn test_lookup_pricing_resolves_alias() {
        let pricing = load_pricing();
        let haiku_pricing = lookup_pricing("haiku", &pricing);
        assert!(
            haiku_pricing.is_some(),
            "haiku alias should resolve to pricing"
        );
        let haiku_direct = pricing.get("claude-haiku-4-5-20251001").unwrap();
        assert_eq!(
            haiku_pricing.unwrap().input_cost_per_token,
            haiku_direct.input_cost_per_token,
        );
        assert!(lookup_pricing("sonnet", &pricing).is_some());
        assert!(lookup_pricing("opus", &pricing).is_some());
    }

    #[test]
    fn test_prefix_lookup_sonnet_46_dated() {
        let pricing = load_pricing();
        assert!(lookup_pricing("claude-sonnet-4-6-20260301", &pricing).is_some());
    }
}
