//! Model context data for catalog upserts.
//!
//! Previously contained LiteLLM fetch, merge, and SQLite cache logic.
//! All pricing data now comes from `data/anthropic-pricing.json` via
//! `claude_view_core::pricing::load_pricing()`.

/// Context window data for upserting into the models table.
pub struct ModelContext {
    pub model_id: String,
    pub provider: String,
    pub family: String,
    pub max_input_tokens: Option<i64>,
    pub max_output_tokens: Option<i64>,
}

/// Type alias for backward compatibility during migration.
pub type LiteLlmModelContext = ModelContext;
