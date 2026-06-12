// crates/core/src/pricing/foreign.rs
//
// Pricing resolution for foreign-agent sessions (Codex, Gemini CLI, …).
//
// Resolution: claude-prefixed ids route through the existing Anthropic
// table (exact → alias → prefix → family fallback); everything else needs
// an EXACT hit in the embedded foreign table (data/foreign-pricing.json).
// There is deliberately NO fuzzy matching and NO fallback for non-Claude
// ids — an unknown foreign model stays unpriced and the UI shows nothing
// (trust gate). The one normalization is dots→dashes retry, mirroring the
// industry-standard LiteLLM key shape ("claude-opus-4.7" → "claude-opus-4-7").

use std::collections::HashMap;
use std::sync::OnceLock;

use super::loader::parse_pricing_file;
use super::lookup::lookup_pricing;
use super::types::{ModelPricing, PricingTable};

const FOREIGN_PRICING_JSON: &str = include_str!("../../../../data/foreign-pricing.json");

static FOREIGN_CACHE: OnceLock<HashMap<String, ModelPricing>> = OnceLock::new();

/// The embedded foreign (non-Claude) pricing table.
pub fn load_foreign_pricing() -> &'static HashMap<String, ModelPricing> {
    FOREIGN_CACHE.get_or_init(|| {
        parse_pricing_file(FOREIGN_PRICING_JSON)
            .expect("embedded foreign-pricing.json is invalid — fix data/foreign-pricing.json")
    })
}

/// Resolve pricing for a model id reported by a foreign agent.
///
/// `anthropic` is the regular Claude table (`state.pricing`) so
/// claude-named models from foreign agents (Copilot, OpenCode, …) price
/// identically to native CC sessions, including the family fallback.
pub fn lookup_foreign_pricing(model_id: &str, anthropic: &PricingTable) -> Option<ModelPricing> {
    let id = model_id.trim();
    if id.is_empty() {
        return None;
    }
    if id.to_ascii_lowercase().starts_with("claude") {
        // Normalize dots→dashes BEFORE the Claude lookup: "claude-opus-4.7"
        // must resolve as "claude-opus-4-7" ($5). Looking up the dotted form
        // directly would prefix-match the legacy "claude-opus-4" entry and
        // charge 3x ($15) — caught by the routing test below.
        let canonical = id.replace('.', "-");
        return lookup_pricing(&canonical, anthropic).cloned();
    }
    // Non-Claude: exact key first (real OpenAI ids contain dots), then the
    // dashed spelling some agents emit.
    let foreign = load_foreign_pricing();
    if let Some(p) = foreign.get(id) {
        return Some(p.clone());
    }
    if id.contains('.') {
        return foreign.get(&id.replace('.', "-")).cloned();
    }
    None
}

/// Compute total USD for Anthropic-shape token totals at a given rate.
pub fn cost_for_totals(
    p: &ModelPricing,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_creation: u64,
) -> f64 {
    input as f64 * p.input_cost_per_token
        + output as f64 * p.output_cost_per_token
        + cache_read as f64 * p.cache_read_cost_per_token
        + cache_creation as f64 * p.cache_creation_cost_per_token
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::load_pricing;

    #[test]
    fn known_openai_models_priced_exactly() {
        let anthropic = load_pricing();
        let p = lookup_foreign_pricing("gpt-5.2-codex", &anthropic).expect("codex model priced");
        assert!((p.input_cost_per_token - 1.75e-6).abs() < 1e-15);
        assert!((p.output_cost_per_token - 14e-6).abs() < 1e-15);
        assert!((p.cache_read_cost_per_token - 0.175e-6).abs() < 1e-15);
    }

    #[test]
    fn claude_ids_from_foreign_agents_route_to_anthropic_table() {
        let anthropic = load_pricing();
        // Dotted form (OpenCode style) must normalize and hit the Claude table.
        let dotted = lookup_foreign_pricing("claude-opus-4.7", &anthropic).expect("dots→dashes");
        let direct = anthropic.get("claude-opus-4-7").unwrap();
        assert_eq!(dotted.input_cost_per_token, direct.input_cost_per_token);
        // Family fallback also applies (future point release).
        assert!(lookup_foreign_pricing("claude-opus-4-99", &anthropic).is_some());
    }

    #[test]
    fn unknown_models_stay_unpriced() {
        let anthropic = load_pricing();
        assert!(lookup_foreign_pricing("qwen3-coder-plus", &anthropic).is_none());
        assert!(lookup_foreign_pricing("auto-genius", &anthropic).is_none());
        assert!(lookup_foreign_pricing("", &anthropic).is_none());
    }

    #[test]
    fn cost_formula_matches_hand_computation() {
        let anthropic = load_pricing();
        let p = lookup_foreign_pricing("gpt-5.4", &anthropic).unwrap();
        // 1M input + 1M output + 1M cache-read at $2.50/$15/$0.25.
        let usd = cost_for_totals(&p, 1_000_000, 1_000_000, 1_000_000, 0);
        assert!((usd - 17.75).abs() < 1e-9, "got {usd}");
    }
}
