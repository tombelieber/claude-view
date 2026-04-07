//! JSONL file classification using SIMD-accelerated pre-filtering.

use memchr::memmem;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::error::SessionIndexError;

use super::types::{SessionClassification, SessionKind, StartType};

/// Classify a JSONL file by scanning its content.
/// Uses memmem SIMD pre-filter -- only JSON-parses lines that match.
pub fn classify_jsonl_file(path: &Path) -> Result<SessionClassification, SessionIndexError> {
    let file = File::open(path).map_err(|e| SessionIndexError::io(path, e))?;
    let reader = BufReader::new(file);

    let user_finder = memmem::Finder::new(br#""type":"user""#);
    let user_finder_spaced = memmem::Finder::new(br#""type": "user""#);
    let assistant_finder = memmem::Finder::new(br#""type":"assistant""#);
    let assistant_finder_spaced = memmem::Finder::new(br#""type": "assistant""#);

    let mut start_type = StartType::Unknown;
    let mut has_user = false;
    let mut has_assistant = false;
    let mut cwd: Option<String> = None;
    let mut parent_id: Option<String> = None;
    let mut first_line = true;

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };
        let bytes = line.as_bytes();

        // Determine start_type from first non-empty line
        if first_line && !line.trim().is_empty() {
            first_line = false;
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
                start_type = match obj.get("type").and_then(|v| v.as_str()) {
                    Some("user") => StartType::User,
                    Some("file-history-snapshot") => StartType::FileHistorySnapshot,
                    Some("queue-operation") => StartType::QueueOperation,
                    Some("progress") => StartType::Progress,
                    Some("summary") => StartType::Summary,
                    Some("assistant") => StartType::Assistant,
                    _ => StartType::Unknown,
                };
            }
        }

        // SIMD pre-filter: check for user/assistant type markers
        let is_user = user_finder.find(bytes).is_some() || user_finder_spaced.find(bytes).is_some();
        let is_assistant =
            assistant_finder.find(bytes).is_some() || assistant_finder_spaced.find(bytes).is_some();

        // Parse user lines to extract cwd and parentUuid
        if is_user && !has_user {
            has_user = true;
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
                if cwd.is_none() {
                    cwd = obj
                        .get("cwd")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
                parent_id = obj
                    .get("parentUuid")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }

        if is_assistant {
            has_assistant = true;
        }

        // Extract cwd from any line if we haven't found it yet
        if cwd.is_none() && line.contains("\"cwd\"") {
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
                cwd = obj
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }

        // Early exit: both conditions resolved
        if has_user && has_assistant && cwd.is_some() {
            break;
        }
    }

    let kind = if has_user && has_assistant {
        SessionKind::Conversation
    } else {
        SessionKind::MetadataOnly
    };

    Ok(SessionClassification {
        kind,
        start_type,
        cwd,
        parent_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_normal_session() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r#"{{"type":"user","uuid":"u1","message":{{"content":"hello"}},"cwd":"/proj"}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","uuid":"a1","message":{{"content":"hi"}}}}"#
        )
        .unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::Conversation);
        assert_eq!(c.start_type, StartType::User);
        assert_eq!(c.cwd.as_deref(), Some("/proj"));
        assert!(c.parent_id.is_none());
    }

    #[test]
    fn test_classify_forked_session() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, r#"{{"type":"user","uuid":"u1","parentUuid":"p1","message":{{"content":"hello"}},"cwd":"/proj"}}"#).unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","uuid":"a1","message":{{"content":"hi"}}}}"#
        )
        .unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::Conversation);
        assert!(c.parent_id.is_some());
    }

    #[test]
    fn test_classify_file_history_snapshot() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r#"{{"type":"file-history-snapshot","messageId":"m1","snapshot":{{}}}}"#
        )
        .unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::MetadataOnly);
        assert_eq!(c.start_type, StartType::FileHistorySnapshot);
    }

    #[test]
    fn test_classify_resumed_session_with_preamble() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r#"{{"type":"file-history-snapshot","messageId":"m1","snapshot":{{}}}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"file-history-snapshot","messageId":"m2","snapshot":{{}}}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"user","uuid":"u1","message":{{"content":"continue"}},"cwd":"/proj"}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"assistant","uuid":"a1","message":{{"content":"sure"}}}}"#
        )
        .unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::Conversation);
        assert_eq!(c.start_type, StartType::FileHistorySnapshot);
        assert_eq!(c.cwd.as_deref(), Some("/proj"));
    }

    #[test]
    fn test_classify_metadata_only_no_conversation() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r#"{{"type":"progress","data":{{"type":"bash_progress"}}}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"type":"progress","data":{{"type":"bash_progress"}}}}"#
        )
        .unwrap();
        let c = classify_jsonl_file(f.path()).unwrap();
        assert_eq!(c.kind, SessionKind::MetadataOnly);
        assert!(c.cwd.is_none());
    }
}
