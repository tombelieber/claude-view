//! Recursive JSON field inventory — collects every structural key with its
//! dotted path from a JSONL line. Used by evidence audit Phase 3 to detect
//! phantom fields and unextracted fields.

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::Deserializer as _;
use serde_json::Deserializer;
use std::collections::HashMap;
use std::fmt;

/// Result of inventorying a single JSONL line.
#[derive(Debug, Default)]
pub struct FieldInventory {
    /// Map of dotted path → occurrence count within this line.
    /// e.g., "message.content[].input.name" → 1
    pub paths: HashMap<String, usize>,
}

impl FieldInventory {
    pub fn merge(&mut self, other: &FieldInventory) {
        for (path, count) in &other.paths {
            *self.paths.entry(path.clone()).or_default() += count;
        }
    }
}

/// Extract all structural JSON key paths from a single JSONL line.
pub fn extract_field_inventory(line: &[u8]) -> FieldInventory {
    let mut inv = FieldInventory::default();
    let mut deserializer = Deserializer::from_slice(line);
    let visitor = ObjectVisitor {
        prefix: String::new(),
        inventory: &mut inv,
    };
    // Ignore errors — malformed lines are handled elsewhere
    let _ = deserializer.deserialize_any(visitor);
    inv
}

/// Recursive visitor that collects keys at any depth.
struct ObjectVisitor<'a> {
    prefix: String,
    inventory: &'a mut FieldInventory,
}

impl<'de, 'a> Visitor<'de> for ObjectVisitor<'a> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(key) = map.next_key::<String>()? {
            let path = if self.prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", self.prefix, key)
            };
            *self.inventory.paths.entry(path.clone()).or_default() += 1;

            // Recurse into the value — MUST reborrow, not move, because
            // self.inventory is used again on the next loop iteration.
            let child = ValueVisitor {
                prefix: path,
                inventory: &mut *self.inventory,
            };
            map.next_value_seed(child)?;
        }
        Ok(())
    }

    // Non-object roots: do nothing (shouldn't happen for JSONL)
    fn visit_str<E: de::Error>(self, _v: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_bool<E: de::Error>(self, _v: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _v: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _v: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, _v: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_none<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
        Ok(())
    }
}

/// DeserializeSeed that recurses into values (objects and arrays).
struct ValueVisitor<'a> {
    prefix: String,
    inventory: &'a mut FieldInventory,
}

impl<'de, 'a> de::DeserializeSeed<'de> for ValueVisitor<'a> {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(ValueDispatch {
            prefix: self.prefix,
            inventory: self.inventory,
        })
    }
}

/// Dispatches to ObjectVisitor for maps, array handler for seqs, ignores scalars.
struct ValueDispatch<'a> {
    prefix: String,
    inventory: &'a mut FieldInventory,
}

impl<'de, 'a> Visitor<'de> for ValueDispatch<'a> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<(), A::Error> {
        let obj = ObjectVisitor {
            prefix: self.prefix,
            inventory: self.inventory,
        };
        obj.visit_map(map)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        // Array: recurse into each element with "[]" suffix on the path.
        // MUST reborrow self.inventory — same reason as ObjectVisitor::visit_map.
        let array_prefix = format!("{}[]", self.prefix);
        while let Some(()) = {
            let child = ValueVisitor {
                prefix: array_prefix.clone(),
                inventory: &mut *self.inventory,
            };
            seq.next_element_seed(child)?
        } {}
        Ok(())
    }

    // Scalars: nothing to recurse into
    fn visit_str<E: de::Error>(self, _v: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_string<E: de::Error>(self, _v: String) -> Result<(), E> {
        Ok(())
    }
    fn visit_bool<E: de::Error>(self, _v: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _v: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _v: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, _v: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_none<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_top_level_keys() {
        let line = br#"{"type":"assistant","teamName":"demo","slug":"abc","message":{"role":"assistant"}}"#;
        let inv = extract_field_inventory(line);
        assert!(inv.paths.contains_key("type"));
        assert!(inv.paths.contains_key("teamName"));
        assert!(inv.paths.contains_key("slug"));
        assert!(inv.paths.contains_key("message"));
        assert_eq!(inv.paths["type"], 1);
    }

    #[test]
    fn test_nested_keys() {
        let line = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Agent","input":{"name":"sysinfo","subagent_type":"general-purpose"}}]}}"#;
        let inv = extract_field_inventory(line);
        assert!(inv.paths.contains_key("message.content[].type"));
        assert!(inv.paths.contains_key("message.content[].name"));
        assert!(inv.paths.contains_key("message.content[].input.name"));
        assert!(inv
            .paths
            .contains_key("message.content[].input.subagent_type"));
    }

    #[test]
    fn test_phantom_field_not_present() {
        let line = br#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Agent","input":{"name":"sysinfo","description":"System info","prompt":"..."}}]}}"#;
        let inv = extract_field_inventory(line);
        assert!(
            !inv.paths.contains_key("message.content[].input.team_name"),
            "team_name must not appear in Agent tool_use input"
        );
    }

    #[test]
    fn test_top_level_team_name_present() {
        let line = br#"{"type":"assistant","teamName":"demo-team","message":{"role":"assistant","content":[]}}"#;
        let inv = extract_field_inventory(line);
        assert!(inv.paths.contains_key("teamName"));
    }

    #[test]
    fn test_does_not_count_string_content_as_keys() {
        let line =
            br#"{"type":"user","message":{"role":"user","content":"I set team_name to demo"}}"#;
        let inv = extract_field_inventory(line);
        assert!(
            !inv.paths.contains_key("team_name"),
            "text content must not be inventoried as structural keys"
        );
    }
}
