use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(pub String);

impl TraceId {
    pub fn new() -> Self {
        let bytes = ulid::Ulid::new().to_bytes();
        Self(hex::encode(bytes))
    }

    pub fn from_traceparent(header: &str) -> Option<Self> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() < 4 {
            return None;
        }
        let tid = parts[1];
        if tid.len() == 32 && tid.chars().all(|c| c.is_ascii_hexdigit()) {
            Some(Self(tid.to_string()))
        } else {
            None
        }
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub String);

impl RequestId {
    pub fn new() -> Self {
        Self(ulid::Ulid::new().to_string())
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CliSessionId(pub String);

impl CliSessionId {
    pub fn new() -> Self {
        Self(format!("cli_{}", ulid::Ulid::new()))
    }
}

impl Default for CliSessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CliSessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_id_is_32_hex() {
        let id = TraceId::new();
        assert_eq!(
            id.0.len(),
            32,
            "trace_id should be 32 hex chars, got {}",
            id.0.len()
        );
        assert!(id.0.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn trace_id_from_valid_traceparent() {
        let id =
            TraceId::from_traceparent("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01");
        assert_eq!(id.unwrap().0, "0af7651916cd43dd8448eb211c80319c");
    }

    #[test]
    fn trace_id_rejects_malformed() {
        assert!(TraceId::from_traceparent("garbage").is_none());
        assert!(TraceId::from_traceparent("00-short-xx-01").is_none());
    }

    #[test]
    fn request_id_is_26_char_ulid() {
        let id = RequestId::new();
        assert_eq!(id.0.len(), 26);
    }

    #[test]
    fn cli_session_id_has_prefix() {
        let id = CliSessionId::new();
        assert!(id.0.starts_with("cli_"));
    }
}
