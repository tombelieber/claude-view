//! Token usage extraction from JSONL `usage` sub-objects.

/// Token counts extracted from a `usage` sub-object.
#[derive(Debug, Default)]
pub(crate) struct UsageTokens {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_creation: Option<u64>,
    pub cache_creation_5m: Option<u64>,
    pub cache_creation_1hr: Option<u64>,
}

/// Extract token counts from a `usage` sub-object.
pub(crate) fn extract_usage(parsed: &serde_json::Value) -> UsageTokens {
    let usage = match parsed.get("usage") {
        Some(u) => u,
        None => return UsageTokens::default(),
    };

    let input = usage.get("input_tokens").and_then(|v| v.as_u64());
    let output = usage.get("output_tokens").and_then(|v| v.as_u64());
    let cache_read = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64());
    let cache_creation = usage
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_u64());

    // Extract ephemeral cache breakdown when present
    let (cache_creation_5m, cache_creation_1hr) = usage
        .get("cache_creation")
        .map(|cc| {
            let t5m = cc.get("ephemeral_5m_input_tokens").and_then(|v| v.as_u64());
            let t1h = cc.get("ephemeral_1h_input_tokens").and_then(|v| v.as_u64());
            (t5m, t1h)
        })
        .unwrap_or((None, None));

    UsageTokens {
        input,
        output,
        cache_read,
        cache_creation,
        cache_creation_5m,
        cache_creation_1hr,
    }
}
