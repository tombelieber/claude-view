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

/// Claude model family — the pricing-relevant grouping.
///
/// Pricing has historically been stable *within* a family across point releases
/// (Opus 4.5/4.6/4.7/4.8 are all $5/$25), which is what makes family-nearest
/// fallback safe for a brand-new release whose exact rate isn't in the table yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Opus,
    Sonnet,
    Haiku,
}

impl Family {
    fn from_token(token: &str) -> Option<Self> {
        match token {
            "opus" => Some(Family::Opus),
            "sonnet" => Some(Family::Sonnet),
            "haiku" => Some(Family::Haiku),
            _ => None,
        }
    }
}

/// How a model id was matched to a pricing entry. Lets the drift audit distinguish
/// "exactly priced" from "served by the family fallback" (i.e. add a precise entry).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    /// Direct key in the pricing table.
    Exact,
    /// Resolved through a short alias ("opus"/"sonnet"/"haiku").
    Alias,
    /// Prefix relationship with a table key (e.g. a dated variant of a base id).
    Prefix,
    /// Inherited from the nearest-version sibling in the same model family.
    FamilyFallback,
}

/// A resolved pricing entry plus how it was matched.
pub struct PricingMatch<'a> {
    pub pricing: &'a ModelPricing,
    pub kind: MatchKind,
}

/// Parse a Claude model id into `(family, (major, minor))` for cross-release fallback.
///
/// Handles both id orderings:
/// - new: `claude-opus-4-8`        → (Opus, (4, 8))
/// - new dated: `claude-opus-4-5-20251101` → (Opus, (4, 5))  (date suffix ignored)
/// - legacy: `claude-3-5-sonnet-20241022`  → (Sonnet, (3, 5))
/// - legacy: `claude-3-opus-20240229`      → (Opus, (3, 0))
///
/// Returns `None` for non-Claude or family-less ids (e.g. `gpt-4o`, `unknown-model`),
/// which preserves the project's "never fabricate a rate for a foreign model" rule.
fn parse_claude_model(model_id: &str) -> Option<(Family, (u32, u32))> {
    let lower = model_id.to_ascii_lowercase();
    if !lower.starts_with("claude") {
        return None;
    }
    let mut family: Option<Family> = None;
    let mut versions: Vec<u32> = Vec::new();
    for token in lower.split('-') {
        if let Some(f) = Family::from_token(token) {
            family = Some(f);
        } else if let Ok(n) = token.parse::<u32>() {
            // Version components are small; 8-digit date suffixes (e.g. 20251101)
            // and other large numbers are not version numbers.
            if n < 1000 {
                versions.push(n);
            }
        }
    }
    let family = family?;
    let major = versions.first().copied().unwrap_or(0);
    let minor = versions.get(1).copied().unwrap_or(0);
    Some((family, (major, minor)))
}

/// Find the nearest-version pricing entry in the same family as `model_id`.
///
/// Prefers the highest known version `<=` the requested version (the most recent
/// *preceding* release — a point release inherits the current rate). If the
/// requested model predates everything known, falls back to the lowest known
/// version in the family. Returns `None` when the family can't be identified or
/// has no priced members.
fn family_nearest_pricing<'a>(
    model_id: &str,
    pricing: &'a HashMap<String, ModelPricing>,
) -> Option<&'a ModelPricing> {
    let (want_family, want_version) = parse_claude_model(model_id)?;

    // Same-family table entries with a real version. Bare aliases ("opus") don't
    // start with "claude" so they parse to `None` and are naturally excluded.
    let mut candidates: Vec<((u32, u32), &ModelPricing)> = pricing
        .iter()
        .filter_map(|(key, p)| {
            let (family, version) = parse_claude_model(key)?;
            (family == want_family && version != (0, 0)).then_some((version, p))
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0));

    // Highest version <= requested (nearest preceding), else lowest known.
    candidates
        .iter()
        .rev()
        .find(|(version, _)| *version <= want_version)
        .or_else(|| candidates.first())
        .map(|(_, p)| *p)
}

/// Resolve pricing for a model id, reporting how it was matched.
///
/// Resolution order: exact → alias → prefix (either direction) → family-nearest
/// fallback. The family fallback is what keeps a brand-new point release (e.g.
/// `claude-opus-4-8` before its exact row is added) from showing "Unavailable":
/// it inherits the newest same-family rate. Only genuinely foreign ids (no Claude
/// family) return `None`.
pub fn resolve_pricing<'a>(
    model_id: &str,
    pricing: &'a HashMap<String, ModelPricing>,
) -> Option<PricingMatch<'a>> {
    if let Some(p) = pricing.get(model_id) {
        return Some(PricingMatch {
            pricing: p,
            kind: MatchKind::Exact,
        });
    }
    if let Some(resolved) = resolve_model_alias(model_id) {
        if let Some(p) = pricing.get(resolved) {
            return Some(PricingMatch {
                pricing: p,
                kind: MatchKind::Alias,
            });
        }
    }
    // Prefix matching: a table key is a prefix of model_id (dated variant of a base id).
    for (key, p) in pricing {
        if model_id.starts_with(key.as_str()) {
            return Some(PricingMatch {
                pricing: p,
                kind: MatchKind::Prefix,
            });
        }
    }
    // Reverse prefix: model_id is a prefix of a table key.
    for (key, p) in pricing {
        if key.starts_with(model_id) {
            return Some(PricingMatch {
                pricing: p,
                kind: MatchKind::Prefix,
            });
        }
    }
    // Family-nearest-version fallback: a brand-new point release inherits the
    // newest known same-family rate instead of being left unpriced.
    family_nearest_pricing(model_id, pricing).map(|p| PricingMatch {
        pricing: p,
        kind: MatchKind::FamilyFallback,
    })
}

/// Look up pricing for a model ID.
///
/// Thin wrapper over [`resolve_pricing`] that discards the match kind. Resolution
/// order: exact → alias → prefix → family-nearest fallback.
pub fn lookup_pricing<'a>(
    model_id: &str,
    pricing: &'a HashMap<String, ModelPricing>,
) -> Option<&'a ModelPricing> {
    resolve_pricing(model_id, pricing).map(|m| m.pricing)
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

    // ---- RED: a brand-new point release must never show "Unavailable" again ----

    #[test]
    fn test_future_point_release_resolves_via_family_fallback() {
        let pricing = load_pricing();
        // A model id newer than anything in the table must still resolve, inheriting
        // the nearest known same-family rate. This is the regression guard for the
        // recurring "cost Unavailable" bug (claude-opus-4-8 etc.).
        let opus_future = lookup_pricing("claude-opus-4-99", &pricing)
            .expect("future opus point release must resolve via family fallback");
        let opus_latest = pricing.get("claude-opus-4-7").unwrap();
        assert_eq!(
            opus_future.input_cost_per_token, opus_latest.input_cost_per_token,
            "future opus should inherit the newest known opus input rate"
        );
        assert_eq!(
            opus_future.output_cost_per_token,
            opus_latest.output_cost_per_token
        );
        assert!(lookup_pricing("claude-sonnet-9-9", &pricing).is_some());
        assert!(lookup_pricing("claude-haiku-9-9", &pricing).is_some());
    }

    #[test]
    fn test_parse_claude_model_both_orderings() {
        // New ordering: family then version.
        assert_eq!(
            parse_claude_model("claude-opus-4-8"),
            Some((Family::Opus, (4, 8)))
        );
        assert_eq!(
            parse_claude_model("claude-sonnet-4-6"),
            Some((Family::Sonnet, (4, 6)))
        );
        // New dated: trailing 8-digit date is ignored.
        assert_eq!(
            parse_claude_model("claude-opus-4-5-20251101"),
            Some((Family::Opus, (4, 5)))
        );
        // Legacy ordering: version before family.
        assert_eq!(
            parse_claude_model("claude-3-5-sonnet-20241022"),
            Some((Family::Sonnet, (3, 5)))
        );
        assert_eq!(
            parse_claude_model("claude-3-opus-20240229"),
            Some((Family::Opus, (3, 0)))
        );
        // Foreign / family-less ids must NOT parse (preserves "no fabricated rate").
        assert_eq!(parse_claude_model("gpt-4o"), None);
        assert_eq!(parse_claude_model("unknown-model"), None);
        assert_eq!(parse_claude_model("claude-instant"), None);
    }

    #[test]
    fn test_resolve_pricing_classifies_match_kind() {
        let pricing = load_pricing();
        // Exact table hit.
        assert_eq!(
            resolve_pricing("claude-opus-4-7", &pricing).unwrap().kind,
            MatchKind::Exact
        );
        // `load_pricing()` flattens aliases into the map, so "opus" is itself an
        // exact key — the dedicated alias branch is only reached for a table that
        // lacks the flattened entry (covered below).
        assert_eq!(
            resolve_pricing("opus", &pricing).unwrap().kind,
            MatchKind::Exact
        );
        // Family fallback for an unknown future version.
        assert_eq!(
            resolve_pricing("claude-opus-4-99", &pricing).unwrap().kind,
            MatchKind::FamilyFallback
        );
        // Foreign model: no fabricated rate.
        assert!(resolve_pricing("gpt-4o", &pricing).is_none());
        assert!(resolve_pricing("unknown-model", &pricing).is_none());
    }

    #[test]
    fn test_resolve_pricing_alias_branch_on_unflattened_table() {
        // A table WITHOUT flattened aliases exercises the resolve_model_alias branch.
        let full = load_pricing();
        let mut table: HashMap<String, ModelPricing> = HashMap::new();
        table.insert(
            "claude-opus-4-6".to_string(),
            full.get("claude-opus-4-6").unwrap().clone(),
        );
        let m = resolve_pricing("opus", &table).expect("alias should resolve to target");
        assert_eq!(m.kind, MatchKind::Alias);
    }

    #[test]
    fn test_family_fallback_picks_nearest_preceding_not_oldest() {
        let pricing = load_pricing();
        // opus-4-99 must inherit the NEWEST opus rate ($5/$25), never the old
        // $15/$25 Opus 4.1/4.0 rate — otherwise cost would be 3x wrong.
        let p = lookup_pricing("claude-opus-4-99", &pricing).unwrap();
        assert!(
            (p.input_cost_per_token - 5e-6).abs() < 1e-15,
            "expected newest opus input rate $5/MTok, got {}",
            p.input_cost_per_token * 1e6
        );
        // Sonnet future inherits $3/$15.
        let s = lookup_pricing("claude-sonnet-9-9", &pricing).unwrap();
        assert!((s.input_cost_per_token - 3e-6).abs() < 1e-15);
    }
}
