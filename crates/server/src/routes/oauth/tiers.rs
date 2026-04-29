//! Frontend-facing usage response + tier registry.
//!
//! Maps the upstream Anthropic API into the `OAuthUsageResponse` we send to
//! the browser. This is the **only** place that knows how to label/categorise
//! a tier — adding a new label is one entry in `label_for`.

use serde::Serialize;

use super::anthropic::{AnthropicUsageResponse, ExtraUsage, RateLimitWindow};

// ── Frontend-facing response types ──────────────────────────────────────

/// One pill row in the usage tooltip.
///
/// **Backward compat:** all new fields are optional or have a stable default.
/// `kind` was added 2026-04-30 alongside the new tier set; older clients will
/// read it as a string they don't recognise and just render the row generically.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UsageTier {
    /// Stable id (`five_hour`, `seven_day`, `seven_day_opus`, …, `extra`).
    pub id: String,
    /// Human-readable label for the row (e.g. "Session (5hr)").
    pub label: String,
    /// Grouping hint for the UI: see [`TierKind`].
    pub kind: TierKind,
    /// 0.0–100.0
    pub percentage: f64,
    /// ISO-8601 reset timestamp. Empty string when unknown — kept as `String`
    /// rather than `Option<String>` for backward compat with older frontends.
    pub reset_at: String,
    /// Dollar / amount description for budget tiers
    /// (e.g. `"$51.25 / $50.00 spent"`). `None` for non-credit tiers.
    pub spent: Option<String>,
    /// ISO 4217 code for credit tiers, when known. `None` for non-credit tiers.
    pub currency: Option<String>,
}

/// How the frontend should group a tier in the tooltip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TierKind {
    /// Short rolling window (currently the 5-hour session bucket).
    Session,
    /// 7-day rolling window we have a curated label for.
    Window,
    /// 7-day rolling window we don't have a curated label for —
    /// upstream codename, surfaced as-is.
    Other,
    /// Credit / pay-as-you-go bucket (`extra_usage`).
    Extra,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthUsageResponse {
    pub has_auth: bool,
    pub error: Option<String>,
    pub plan: Option<String>,
    pub tiers: Vec<UsageTier>,
}

// ── Constructors for early-exit cases ───────────────────────────────────

pub fn no_auth() -> OAuthUsageResponse {
    OAuthUsageResponse {
        has_auth: false,
        error: None,
        plan: None,
        tiers: vec![],
    }
}

pub fn auth_error(msg: impl Into<String>) -> OAuthUsageResponse {
    OAuthUsageResponse {
        has_auth: true,
        error: Some(msg.into()),
        plan: None,
        tiers: vec![],
    }
}

// ── Tier registry ───────────────────────────────────────────────────────

/// Curated label + grouping for known tier ids.
///
/// Returns `(kind, label)` where an empty label means "no curated label —
/// fall back to [`humanize_id`]". Trust > accuracy: never fabricate a
/// human-friendly label for an upstream codename we don't recognise.
fn label_for(id: &str) -> (TierKind, &'static str) {
    match id {
        "five_hour" => (TierKind::Session, "Session (5hr)"),
        "seven_day" => (TierKind::Window, "Weekly"),
        "seven_day_opus" => (TierKind::Window, "Weekly Opus"),
        "seven_day_sonnet" => (TierKind::Window, "Weekly Sonnet"),
        "seven_day_oauth_apps" => (TierKind::Window, "Weekly OAuth Apps"),
        _ => (TierKind::Other, ""),
    }
}

/// Best-effort humanisation for ids without a curated label.
///
/// `seven_day_omelette` → `"Weekly · omelette"`
/// `iguana_necktie`    → `"Iguana necktie"`
fn humanize_id(id: &str) -> String {
    if let Some(suffix) = id.strip_prefix("seven_day_") {
        return format!("Weekly · {}", suffix.replace('_', " "));
    }
    let mut chars = id.replace('_', " ");
    if let Some(first) = chars.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    chars
}

/// Format a credit amount honouring the upstream currency code.
///
/// USD (and unknown/null currency, which we assume USD per the historical
/// shape) renders with `$`. Other ISO codes render `12.34 EUR`-style — never
/// fabricate a symbol for a currency we don't know.
fn format_amount(cents: f64, currency: &str) -> String {
    let major = cents / 100.0;
    match currency {
        "" | "USD" => format!("${major:.2}"),
        code => format!("{major:.2} {code}"),
    }
}

// ── Trust gate ──────────────────────────────────────────────────────────

/// Drop windows that are unreliable or signal a decommissioned bucket.
///
/// Two cases bring nothing to the user:
/// 1. `utilization` is missing → upstream sent a window object with no number.
/// 2. `utilization == 0.0` AND `resets_at` is null/empty — Anthropic's signal
///    that the bucket exists in the schema but isn't being tracked for this
///    account (e.g. `seven_day_sonnet` after the model lineup change).
///
/// Returns `Some(pct)` when the window is worth surfacing.
fn pass_trust_gate(w: &RateLimitWindow) -> Option<f64> {
    let pct = w.utilization?;
    let has_reset = w.resets_at.as_deref().is_some_and(|s| !s.is_empty());
    if pct == 0.0 && !has_reset {
        return None;
    }
    Some(pct)
}

// ── Main builder ────────────────────────────────────────────────────────

/// Project the upstream Anthropic response onto the frontend `Vec<UsageTier>`.
pub fn build_tiers(resp: &AnthropicUsageResponse) -> Vec<UsageTier> {
    let mut tiers = Vec::new();

    push_known(&mut tiers, "five_hour", resp.five_hour.as_ref());
    push_known(&mut tiers, "seven_day", resp.seven_day.as_ref());
    push_known(&mut tiers, "seven_day_opus", resp.seven_day_opus.as_ref());
    push_known(
        &mut tiers,
        "seven_day_sonnet",
        resp.seven_day_sonnet.as_ref(),
    );
    push_known(
        &mut tiers,
        "seven_day_oauth_apps",
        resp.seven_day_oauth_apps.as_ref(),
    );

    push_unknown_siblings(&mut tiers, &resp.extra_windows);

    if let Some(extra) = resp.extra_usage.as_ref() {
        if let Some(tier) = build_extra_tier(extra) {
            tiers.push(tier);
        }
    }

    tiers
}

fn push_known(tiers: &mut Vec<UsageTier>, id: &str, window: Option<&RateLimitWindow>) {
    let Some(w) = window else { return };
    let Some(pct) = pass_trust_gate(w) else {
        return;
    };
    let (kind, label) = label_for(id);
    tiers.push(UsageTier {
        id: id.to_string(),
        label: label.to_string(),
        kind,
        percentage: pct,
        reset_at: w.resets_at.clone().unwrap_or_default(),
        spent: None,
        currency: None,
    });
}

fn push_unknown_siblings(
    tiers: &mut Vec<UsageTier>,
    extras: &std::collections::HashMap<String, serde_json::Value>,
) {
    let mut keys: Vec<&String> = extras.keys().collect();
    keys.sort(); // deterministic order
    for key in keys {
        let val = &extras[key];
        if val.is_null() {
            continue;
        }
        let Ok(window) = serde_json::from_value::<RateLimitWindow>(val.clone()) else {
            continue;
        };
        let Some(pct) = pass_trust_gate(&window) else {
            continue;
        };
        let (kind, label_static) = label_for(key);
        let label = if label_static.is_empty() {
            humanize_id(key)
        } else {
            label_static.to_string()
        };
        tiers.push(UsageTier {
            id: key.clone(),
            label,
            kind,
            percentage: pct,
            reset_at: window.resets_at.unwrap_or_default(),
            spent: None,
            currency: None,
        });
    }
}

fn build_extra_tier(extra: &ExtraUsage) -> Option<UsageTier> {
    if !extra.is_enabled {
        return None;
    }
    let used_credits = extra.used_credits.unwrap_or(0.0);
    let monthly_limit = extra.monthly_limit.unwrap_or(0.0);
    let pct = extra.utilization.unwrap_or_else(|| {
        if monthly_limit > 0.0 {
            ((used_credits / monthly_limit) * 100.0).min(100.0)
        } else {
            0.0
        }
    });
    let currency = extra.currency.clone().unwrap_or_default();
    let used_str = format_amount(used_credits, &currency);
    let limit_str = format_amount(monthly_limit, &currency);
    let spent = if monthly_limit > 0.0 {
        format!("{used_str} / {limit_str} spent")
    } else {
        format!("{used_str} spent (no limit)")
    };
    Some(UsageTier {
        id: "extra".to_string(),
        label: "Extra usage".to_string(),
        kind: TierKind::Extra,
        percentage: pct,
        reset_at: String::new(),
        spent: Some(spent),
        currency: if currency.is_empty() {
            None
        } else {
            Some(currency)
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse_anthropic(value: serde_json::Value) -> AnthropicUsageResponse {
        serde_json::from_value(value).expect("test fixture parses")
    }

    #[test]
    fn old_shape_still_renders() {
        // Pre-2026-04 shape: only the original three windows + extra_usage,
        // no codenames, no currency.
        let resp = parse_anthropic(json!({
            "five_hour":        { "utilization": 12.0, "resets_at": "2026-04-29T22:30:00Z" },
            "seven_day":        { "utilization":  5.0, "resets_at": "2026-05-01T05:00:00Z" },
            "seven_day_sonnet": { "utilization": 30.0, "resets_at": "2026-05-02T05:00:00Z" },
            "extra_usage":      { "is_enabled": true, "used_credits": 5125.0, "monthly_limit": 5000.0, "utilization": 102.5 },
        }));
        let tiers = build_tiers(&resp);
        let ids: Vec<&str> = tiers.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["five_hour", "seven_day", "seven_day_sonnet", "extra"]
        );
        // Extra tier should render USD when currency missing.
        let extra = tiers.iter().find(|t| t.id == "extra").unwrap();
        assert_eq!(extra.spent.as_deref(), Some("$51.25 / $50.00 spent"));
        assert!(extra.currency.is_none());
    }

    #[test]
    fn live_2026_04_shape_drops_decommissioned_and_keeps_unknown_codenames() {
        // The exact shape we observed live on 2026-04-30.
        let resp = parse_anthropic(json!({
            "five_hour":            { "utilization": 12.0, "resets_at": "2026-04-29T22:30:00Z" },
            "seven_day":            { "utilization":  5.0, "resets_at": "2026-05-01T05:00:00Z" },
            "seven_day_oauth_apps": null,
            "seven_day_opus":       null,
            "seven_day_sonnet":     { "utilization": 0.0, "resets_at": null },
            "seven_day_cowork":     null,
            "seven_day_omelette":   { "utilization": 8.0, "resets_at": "2026-05-03T05:00:00Z" },
            "iguana_necktie":       null,
            "omelette_promotional": null,
            "extra_usage":          { "is_enabled": false, "monthly_limit": null, "used_credits": null, "utilization": null, "currency": null },
        }));
        let tiers = build_tiers(&resp);
        let ids: Vec<&str> = tiers.iter().map(|t| t.id.as_str()).collect();
        // Trust gate drops `seven_day_sonnet` (0.0 + null reset).
        // `seven_day_omelette` is unknown but has real data → kept.
        // `extra_usage.is_enabled = false` → no extra tier.
        assert_eq!(ids, vec!["five_hour", "seven_day", "seven_day_omelette"]);
        let omelette = tiers.iter().find(|t| t.id == "seven_day_omelette").unwrap();
        assert_eq!(omelette.label, "Weekly · omelette");
        assert_eq!(omelette.kind, TierKind::Other);
    }

    #[test]
    fn null_utilization_does_not_fail_parse() {
        // The latent failure mode: a window object with utilization explicitly
        // set to null. Old code would fail-parse the entire response. Now the
        // window is dropped by the trust gate.
        let resp = parse_anthropic(json!({
            "five_hour": { "utilization": null, "resets_at": "2026-05-01T05:00:00Z" },
            "seven_day": { "utilization": 5.0,  "resets_at": "2026-05-01T05:00:00Z" },
        }));
        let tiers = build_tiers(&resp);
        assert_eq!(tiers.len(), 1);
        assert_eq!(tiers[0].id, "seven_day");
    }

    #[test]
    fn currency_eur_renders_iso_code_not_dollar_sign() {
        let resp = parse_anthropic(json!({
            "extra_usage": {
                "is_enabled": true,
                "used_credits": 1234.0,
                "monthly_limit": 5000.0,
                "utilization": 24.68,
                "currency": "EUR"
            }
        }));
        let tiers = build_tiers(&resp);
        let extra = tiers.iter().find(|t| t.id == "extra").unwrap();
        assert_eq!(extra.spent.as_deref(), Some("12.34 EUR / 50.00 EUR spent"));
        assert_eq!(extra.currency.as_deref(), Some("EUR"));
    }

    #[test]
    fn extra_disabled_omits_tier() {
        let resp = parse_anthropic(json!({
            "extra_usage": { "is_enabled": false, "used_credits": null, "monthly_limit": null, "utilization": null, "currency": null }
        }));
        let tiers = build_tiers(&resp);
        assert!(tiers.iter().all(|t| t.id != "extra"));
    }

    #[test]
    fn humanize_id_handles_seven_day_prefix() {
        assert_eq!(humanize_id("seven_day_omelette"), "Weekly · omelette");
        assert_eq!(humanize_id("seven_day_oauth_apps"), "Weekly · oauth apps");
    }

    #[test]
    fn humanize_id_titlecases_other_codenames() {
        assert_eq!(humanize_id("iguana_necktie"), "Iguana necktie");
        assert_eq!(humanize_id("omelette"), "Omelette");
    }
}
