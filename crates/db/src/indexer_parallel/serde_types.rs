// crates/db/src/indexer_parallel/serde_types.rs
// Typed structs for fast dispatched parsing (avoid full serde_json::Value).
// Private to this module -- only used inside parse_bytes().

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

/// Handles both integer and ISO8601 string timestamps.
#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum TimestampValue {
    Integer(i64),
    Iso(String),
}

impl TimestampValue {
    pub fn to_unix(&self) -> Option<i64> {
        match self {
            TimestampValue::Integer(v) => Some(*v),
            TimestampValue::Iso(s) => chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.timestamp()),
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct AssistantLine {
    pub timestamp: Option<TimestampValue>,
    pub uuid: Option<String>,
    #[serde(rename = "parentUuid")]
    pub parent_uuid: Option<String>,
    #[serde(rename = "requestId")]
    pub request_id: Option<String>,
    pub message: Option<AssistantMessage>,
}

#[derive(Deserialize)]
pub(crate) struct AssistantMessage {
    pub id: Option<String>,
    pub model: Option<String>,
    pub usage: Option<UsageBlock>,
    #[serde(default, deserialize_with = "deserialize_content")]
    pub content: ContentResult,
}

#[derive(Deserialize)]
pub(crate) struct UsageBlock {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_creation: Option<CacheCreationUsage>,
    pub service_tier: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct CacheCreationUsage {
    pub ephemeral_5m_input_tokens: Option<u64>,
    pub ephemeral_1h_input_tokens: Option<u64>,
}

/// Custom visitor result -- avoids #[serde(untagged)] buffering overhead.
#[derive(Default)]
pub(crate) enum ContentResult {
    Blocks(Vec<FlatContentBlock>),
    NotArray,
    #[default]
    Missing,
}

/// Custom deserializer for message content that avoids `#[serde(untagged)]` buffering.
/// Directly deserializes array elements as `FlatContentBlock` -- no intermediate Value tree.
fn deserialize_content<'de, D: Deserializer<'de>>(d: D) -> Result<ContentResult, D::Error> {
    struct ContentVisitor;

    impl<'de> Visitor<'de> for ContentVisitor {
        type Value = ContentResult;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an array of content blocks, a string, or null")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<ContentResult, A::Error> {
            let mut blocks = Vec::new();
            while let Some(block) = seq.next_element::<FlatContentBlock>()? {
                blocks.push(block);
            }
            Ok(ContentResult::Blocks(blocks))
        }

        fn visit_str<E: de::Error>(self, _: &str) -> Result<ContentResult, E> {
            Ok(ContentResult::NotArray)
        }

        fn visit_string<E: de::Error>(self, _: String) -> Result<ContentResult, E> {
            Ok(ContentResult::NotArray)
        }

        fn visit_none<E: de::Error>(self) -> Result<ContentResult, E> {
            Ok(ContentResult::Missing)
        }

        fn visit_unit<E: de::Error>(self) -> Result<ContentResult, E> {
            Ok(ContentResult::Missing)
        }
    }

    d.deserialize_any(ContentVisitor)
}

/// Flat content block -- only declares fields we actually read.
/// Serde skips undeclared fields (text content, thinking content) without allocating them.
#[derive(Deserialize)]
pub(crate) struct FlatContentBlock {
    #[serde(rename = "type")]
    pub block_type: Option<String>,
    pub name: Option<String>,
    pub input: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub(crate) struct SystemLine {
    pub timestamp: Option<TimestampValue>,
    pub subtype: Option<String>,
    #[serde(rename = "durationMs")]
    pub duration_ms: Option<u64>,
    #[serde(rename = "retryAttempt")]
    pub retry_attempt: Option<u64>,
    #[serde(rename = "preventedContinuation")]
    pub prevented_continuation: Option<bool>,
}
