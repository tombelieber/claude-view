//! Streaming serde visitors for extracting structural signals from JSONL lines.
//!
//! Uses `serde_json::Deserializer` + `IgnoredAny` to correctly distinguish
//! top-level "type" fields from nested ones (e.g. "type":"message" inside content).

use super::types::LineSignals;
use serde::de::{self, Deserializer as _, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fmt;

/// Extract structural signals from a single JSONL line using streaming visitors.
pub fn extract_line_signals(line: &[u8]) -> LineSignals {
    let mut deserializer = serde_json::Deserializer::from_slice(line);
    match deserializer.deserialize_map(TopLevelVisitor) {
        Ok(mut signals) => {
            // Only assistant messages have meaningful content block types
            if signals.top_level_type.as_deref() != Some("assistant") {
                signals.content_block_types.clear();
                signals.thinking_key_sets.clear();
            }
            signals
        }
        Err(_) => LineSignals::default(),
    }
}

// ─── TopLevelVisitor ────────────────────────────────────────────

struct TopLevelVisitor;

impl<'de> Visitor<'de> for TopLevelVisitor {
    type Value = LineSignals;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON object (JSONL line)")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<LineSignals, A::Error> {
        let mut signals = LineSignals::default();
        let mut message_signals: Option<MessageSignals> = None;
        let mut data_signals: Option<DataSignals> = None;

        while let Some(key) = map.next_key::<&str>()? {
            match key {
                "type" => {
                    signals.top_level_type = Some(map.next_value::<String>()?);
                }
                "subtype" => {
                    signals.subtype = Some(map.next_value::<String>()?);
                }
                "message" => {
                    message_signals = Some(map.next_value::<MessageSignals>()?);
                }
                "data" => {
                    data_signals = Some(map.next_value::<DataSignals>()?);
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        // Transfer message signals
        if let Some(ms) = message_signals {
            signals.content_block_types = ms.content_block_types;
            signals.thinking_key_sets = ms.thinking_key_sets;
        }

        // Transfer data signals
        if let Some(ds) = data_signals {
            signals.data_type = ds.data_type;
            signals.nesting_direct = ds.nesting_direct;
            signals.nesting_nested = ds.nesting_nested;
        }

        Ok(signals)
    }
}

// ─── MessageSignals ─────────────────────────────────────────────

#[derive(Debug, Default)]
struct MessageSignals {
    content_block_types: Vec<String>,
    thinking_key_sets: Vec<BTreeSet<String>>,
}

impl<'de> Deserialize<'de> for MessageSignals {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(MessageVisitor)
    }
}

struct MessageVisitor;

impl<'de> Visitor<'de> for MessageVisitor {
    type Value = MessageSignals;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a message object with optional content array")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<MessageSignals, A::Error> {
        let mut signals = MessageSignals::default();

        while let Some(key) = map.next_key::<&str>()? {
            match key {
                "content" => {
                    let content = map.next_value::<ContentArrayOrString>()?;
                    match content {
                        ContentArrayOrString::Blocks(blocks) => {
                            for block in blocks {
                                if let Some(t) = block.block_type {
                                    if t == "thinking" {
                                        signals.thinking_key_sets.push(block.all_keys);
                                    }
                                    signals.content_block_types.push(t);
                                }
                            }
                        }
                        ContentArrayOrString::Other => {}
                    }
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        Ok(signals)
    }
}

// ─── ContentArrayOrString ───────────────────────────────────────

enum ContentArrayOrString {
    Blocks(Vec<ContentBlock>),
    Other,
}

impl<'de> Deserialize<'de> for ContentArrayOrString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ContentArrayOrStringVisitor)
    }
}

struct ContentArrayOrStringVisitor;

impl<'de> Visitor<'de> for ContentArrayOrStringVisitor {
    type Value = ContentArrayOrString;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a content array, string, or null")
    }

    fn visit_str<E: de::Error>(self, _v: &str) -> Result<ContentArrayOrString, E> {
        Ok(ContentArrayOrString::Other)
    }

    fn visit_string<E: de::Error>(self, _v: String) -> Result<ContentArrayOrString, E> {
        Ok(ContentArrayOrString::Other)
    }

    fn visit_none<E: de::Error>(self) -> Result<ContentArrayOrString, E> {
        Ok(ContentArrayOrString::Other)
    }

    fn visit_unit<E: de::Error>(self) -> Result<ContentArrayOrString, E> {
        Ok(ContentArrayOrString::Other)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<ContentArrayOrString, A::Error> {
        let mut blocks = Vec::new();
        while let Some(block) = seq.next_element::<ContentBlock>()? {
            blocks.push(block);
        }
        Ok(ContentArrayOrString::Blocks(blocks))
    }
}

// ─── ContentBlock ───────────────────────────────────────────────

#[derive(Debug)]
struct ContentBlock {
    block_type: Option<String>,
    all_keys: BTreeSet<String>,
}

impl<'de> Deserialize<'de> for ContentBlock {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ContentBlockVisitor)
    }
}

struct ContentBlockVisitor;

impl<'de> Visitor<'de> for ContentBlockVisitor {
    type Value = ContentBlock;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a content block object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<ContentBlock, A::Error> {
        let mut block_type = None;
        let mut all_keys = BTreeSet::new();

        while let Some(key) = map.next_key::<String>()? {
            all_keys.insert(key.clone());
            if key == "type" {
                block_type = Some(map.next_value::<String>()?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }

        Ok(ContentBlock {
            block_type,
            all_keys,
        })
    }
}

// ─── DataSignals ────────────────────────────────────────────────

#[derive(Debug, Default)]
struct DataSignals {
    data_type: Option<String>,
    nesting_direct: bool,
    nesting_nested: bool,
}

impl<'de> Deserialize<'de> for DataSignals {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(DataVisitor)
    }
}

struct DataVisitor;

impl<'de> Visitor<'de> for DataVisitor {
    type Value = DataSignals;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a data object with optional type and message")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<DataSignals, A::Error> {
        let mut signals = DataSignals::default();

        while let Some(key) = map.next_key::<&str>()? {
            match key {
                "type" => {
                    let t = map.next_value::<String>()?;
                    if t == "agent_progress" {
                        signals.nesting_direct = true;
                    }
                    signals.data_type = Some(t);
                }
                "message" => {
                    // Check for nested agent_progress: data.message.message.content
                    let nesting = map.next_value::<NestingCheck>()?;
                    if nesting.has_nested_content {
                        signals.nesting_nested = true;
                    }
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        Ok(signals)
    }
}

// ─── NestingCheck ───────────────────────────────────────────────

#[derive(Debug, Default)]
struct NestingCheck {
    has_nested_content: bool,
}

impl<'de> Deserialize<'de> for NestingCheck {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NestingVisitor)
    }
}

struct NestingVisitor;

impl<'de> Visitor<'de> for NestingVisitor {
    type Value = NestingCheck;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a message envelope that may contain nested message.content")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<NestingCheck, A::Error> {
        let mut result = NestingCheck::default();

        while let Some(key) = map.next_key::<&str>()? {
            match key {
                "message" => {
                    // This is data.message.message — check if it has a content array
                    let inner = map.next_value::<InnerMessageHasContentArray>()?;
                    result.has_nested_content = inner.has_content_array;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        Ok(result)
    }

    // data.message might not be an object
    fn visit_str<E: de::Error>(self, _v: &str) -> Result<NestingCheck, E> {
        Ok(NestingCheck::default())
    }

    fn visit_string<E: de::Error>(self, _v: String) -> Result<NestingCheck, E> {
        Ok(NestingCheck::default())
    }

    fn visit_none<E: de::Error>(self) -> Result<NestingCheck, E> {
        Ok(NestingCheck::default())
    }

    fn visit_unit<E: de::Error>(self) -> Result<NestingCheck, E> {
        Ok(NestingCheck::default())
    }
}

// ─── InnerMessageHasContentArray ────────────────────────────────

#[derive(Debug, Default)]
struct InnerMessageHasContentArray {
    has_content_array: bool,
}

impl<'de> Deserialize<'de> for InnerMessageHasContentArray {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(InnerMessageVisitor)
    }
}

struct InnerMessageVisitor;

impl<'de> Visitor<'de> for InnerMessageVisitor {
    type Value = InnerMessageHasContentArray;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an inner message object")
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> Result<InnerMessageHasContentArray, A::Error> {
        let mut result = InnerMessageHasContentArray::default();

        while let Some(key) = map.next_key::<&str>()? {
            match key {
                "content" => {
                    let is_arr = map.next_value::<IsArray>()?;
                    result.has_content_array = is_arr.0;
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        Ok(result)
    }

    // Inner message might not be an object
    fn visit_str<E: de::Error>(self, _v: &str) -> Result<InnerMessageHasContentArray, E> {
        Ok(InnerMessageHasContentArray::default())
    }

    fn visit_string<E: de::Error>(self, _v: String) -> Result<InnerMessageHasContentArray, E> {
        Ok(InnerMessageHasContentArray::default())
    }

    fn visit_none<E: de::Error>(self) -> Result<InnerMessageHasContentArray, E> {
        Ok(InnerMessageHasContentArray::default())
    }

    fn visit_unit<E: de::Error>(self) -> Result<InnerMessageHasContentArray, E> {
        Ok(InnerMessageHasContentArray::default())
    }
}

// ─── IsArray ────────────────────────────────────────────────────

/// Zero-allocation check: is this JSON value an array?
struct IsArray(bool);

impl<'de> Deserialize<'de> for IsArray {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(IsArrayVisitor)
    }
}

struct IsArrayVisitor;

impl<'de> Visitor<'de> for IsArrayVisitor {
    type Value = IsArray;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<IsArray, A::Error> {
        // Consume the sequence without allocating
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(IsArray(true))
    }

    fn visit_str<E: de::Error>(self, _v: &str) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_string<E: de::Error>(self, _v: String) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<IsArray, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(IsArray(false))
    }

    fn visit_bool<E: de::Error>(self, _v: bool) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_i64<E: de::Error>(self, _v: i64) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_u64<E: de::Error>(self, _v: u64) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_f64<E: de::Error>(self, _v: f64) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_none<E: de::Error>(self) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }

    fn visit_unit<E: de::Error>(self) -> Result<IsArray, E> {
        Ok(IsArray(false))
    }
}
