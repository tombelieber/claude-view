// crates/core/src/classification/mod.rs
//! Classification taxonomy types, prompt templates, and response parsing.
//!
//! This module defines the 30-category taxonomy for session classification,
//! builds batch classification prompts, and parses LLM responses.

mod parsing;
mod prompt;
mod taxonomy;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public items to preserve the module's public API.
pub use parsing::{parse_batch_response, parse_category_string, BATCH_SIZE};
pub use prompt::{build_batch_prompt, truncate_preview, SYSTEM_PROMPT};
pub use taxonomy::{CategoryL1, CategoryL2, CategoryL3};
pub use types::{
    BatchClassificationResponse, ClassificationInput, ClassificationResult, ValidatedClassification,
};
