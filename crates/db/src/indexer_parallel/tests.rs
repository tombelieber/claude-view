// crates/db/src/indexer_parallel/tests.rs
// All test modules for indexer_parallel.

#[cfg(test)]
mod normalize_tests {
    use crate::indexer_parallel::cost::normalize_model_id;

    #[test]
    fn test_strips_date_suffix() {
        assert_eq!(
            normalize_model_id("claude-3-5-sonnet-20241022"),
            "claude-3.5-sonnet"
        );
        assert_eq!(
            normalize_model_id("claude-3-5-haiku-20241022"),
            "claude-3.5-haiku"
        );
        assert_eq!(
            normalize_model_id("claude-3-opus-20240229"),
            "claude-3-opus"
        );
    }

    #[test]
    fn test_preserves_canonical_names() {
        assert_eq!(normalize_model_id("claude-sonnet-4"), "claude-sonnet-4");
        assert_eq!(normalize_model_id("claude-haiku-3.5"), "claude-haiku-3.5");
        assert_eq!(normalize_model_id("gpt-4o"), "gpt-4o");
    }

    #[test]
    fn test_short_names() {
        assert_eq!(normalize_model_id("gpt-4"), "gpt-4");
        assert_eq!(normalize_model_id("o1"), "o1");
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::super::*;
    use crate::Database;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_parse_bytes_empty() {
        let result = parse_bytes(b"");
        assert_eq!(result.deep.turn_count, 0);
        assert!(result.deep.last_message.is_empty());
        assert!(result.deep.tool_counts.is_empty());
        assert!(result.raw_invocations.is_empty());
    }

    #[test]
    fn test_parse_bytes_counts_tools() {
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read"},{"type":"tool_use","name":"Edit"}]}}
{"type":"user","message":{"content":"thanks"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.turn_count, 2);
        assert_eq!(result.deep.tool_counts.read, 1);
        assert_eq!(result.deep.tool_counts.edit, 1);
        assert_eq!(result.deep.tool_counts.bash, 1);
        assert_eq!(result.deep.tool_counts.write, 0);
    }

    #[test]
    fn test_parse_bytes_last_message() {
        let data = br#"{"type":"user","message":{"content":"first question"}}
{"type":"assistant","message":{"content":"answer 1"}}
{"type":"user","message":{"content":"second question"}}
{"type":"assistant","message":{"content":"answer 2"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.last_message, "second question");
    }

    #[test]
    fn test_parse_bytes_extracts_raw_invocations() {
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","timestamp":1706200000,"message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"/src/lib.rs"}}]}}
{"type":"user","message":{"content":"run tests"}}
{"type":"assistant","timestamp":1706200100,"message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test"}},{"type":"text","text":"Done!"}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.raw_invocations.len(), 3);
        assert_eq!(result.raw_invocations[0].name, "Read");
        assert_eq!(
            result.raw_invocations[0]
                .input
                .as_ref()
                .and_then(|v| v.get("file_path"))
                .and_then(|v| v.as_str()),
            Some("/src/main.rs")
        );
        assert_eq!(result.raw_invocations[0].timestamp, 1706200000);
        assert_eq!(result.raw_invocations[1].name, "Edit");
        assert_eq!(result.raw_invocations[1].timestamp, 1706200000);
        assert_eq!(result.raw_invocations[2].name, "Bash");
        assert_eq!(
            result.raw_invocations[2]
                .input
                .as_ref()
                .and_then(|v| v.get("command"))
                .and_then(|v| v.as_str()),
            Some("cargo test")
        );
        assert_eq!(result.raw_invocations[2].timestamp, 1706200100);
        assert_eq!(
            result.raw_invocations[0].byte_offset, result.raw_invocations[1].byte_offset,
            "Read and Edit are on the same JSONL line"
        );
        assert_ne!(
            result.raw_invocations[0].byte_offset, result.raw_invocations[2].byte_offset,
            "Bash is on a different JSONL line"
        );
    }

    #[test]
    fn test_parse_bytes_no_invocations_without_tool_use() {
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","message":{"content":"Just text, no tools."}}
"#;
        let result = parse_bytes(data);
        assert!(result.raw_invocations.is_empty());
    }

    #[test]
    fn test_parse_bytes_timestamp_defaults_to_zero() {
        let data = br#"{"type":"user","message":{"content":"hello"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/foo"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.raw_invocations.len(), 1);
        assert_eq!(result.raw_invocations[0].timestamp, 0);
    }

    #[test]
    fn test_read_file_fast_nonexistent() {
        let result = read_file_fast(std::path::Path::new("/nonexistent/file.jsonl"));
        assert!(result.is_err());
    }

    #[test]
    fn test_read_file_fast_small_file() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut tmp, b"hello world").unwrap();
        let data = read_file_fast(tmp.path()).unwrap();
        assert_eq!(&*data, b"hello world");
    }

    #[test]
    fn test_truncate() {
        use crate::indexer_parallel::helpers::truncate;
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello...");
    }

    #[test]
    fn test_split_lines_simd() {
        use crate::indexer_parallel::helpers::split_lines_simd;
        let data = b"line1\nline2\nline3";
        let lines: Vec<&[u8]> = split_lines_simd(data).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], b"line1");
        assert_eq!(lines[1], b"line2");
        assert_eq!(lines[2], b"line3");
    }

    // ========================================================================
    // Phase 3: Atomic Unit Extraction Tests
    // ========================================================================

    #[test]
    fn test_user_prompt_count_basic() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":"a1"}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":"a2"}}
{"type":"user","message":{"content":"q3"}}
{"type":"assistant","message":{"content":"a3"}}
{"type":"user","message":{"content":"q4"}}
{"type":"assistant","message":{"content":"a4"}}
{"type":"user","message":{"content":"q5"}}
{"type":"assistant","message":{"content":"a5"}}
{"type":"assistant","message":{"content":"a6"}}
{"type":"assistant","message":{"content":"a7"}}
{"type":"assistant","message":{"content":"a8"}}
{"type":"assistant","message":{"content":"a9"}}
{"type":"assistant","message":{"content":"a10"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.user_prompt_count, 5);
    }

    #[test]
    fn test_user_prompt_count_zero() {
        let data = br#"{"type":"assistant","message":{"content":"a1"}}
{"type":"assistant","message":{"content":"a2"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.user_prompt_count, 0);
    }

    #[test]
    fn test_user_prompt_count_unicode() {
        let data = br#"{"type":"user","message":{"content":"Hello \u4e16\u754c"}}
{"type":"assistant","message":{"content":"Response"}}
{"type":"user","message":{"content":"\u2764\ufe0f emoji test"}}
{"type":"assistant","message":{"content":"Done"}}
{"type":"user","message":{"content":"Third with unicode: \u00e9\u00e8\u00ea"}}
{"type":"assistant","message":{"content":"Final"}}
{"type":"user","message":{"content":"Fourth"}}
{"type":"assistant","message":{"content":"Last"}}
{"type":"user","message":{"content":"Fifth"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.user_prompt_count, 5);
    }

    #[test]
    fn test_user_prompt_count_empty_file() {
        let result = parse_bytes(b"");
        assert_eq!(result.deep.user_prompt_count, 0);
    }

    #[test]
    fn test_api_call_count_basic() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":"a1"}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":"a2"}}
{"type":"user","message":{"content":"q3"}}
{"type":"assistant","message":{"content":"a3"}}
{"type":"user","message":{"content":"q4"}}
{"type":"assistant","message":{"content":"a4"}}
{"type":"user","message":{"content":"q5"}}
{"type":"assistant","message":{"content":"a5"}}
{"type":"assistant","message":{"content":"a6"}}
{"type":"assistant","message":{"content":"a7"}}
{"type":"assistant","message":{"content":"a8"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.api_call_count, 8);
    }

    #[test]
    fn test_api_call_count_zero() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"user","message":{"content":"q2"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.api_call_count, 0);
    }

    #[test]
    fn test_tool_call_count_multiple_per_message() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{}},{"type":"tool_use","name":"Edit","input":{}}]}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{}},{"type":"tool_use","name":"Write","input":{}}]}}
{"type":"user","message":{"content":"q3"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{}},{"type":"tool_use","name":"Read","input":{}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.tool_call_count, 6);
    }

    #[test]
    fn test_tool_call_count_zero() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":"Just text, no tools."}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":"More text."}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.tool_call_count, 0);
    }

    #[test]
    fn test_tool_call_count_parallel_tools() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{}},{"type":"tool_use","name":"Read","input":{}},{"type":"tool_use","name":"Read","input":{}},{"type":"tool_use","name":"Read","input":{}},{"type":"tool_use","name":"Read","input":{}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.tool_call_count, 5);
    }

    #[test]
    fn test_files_read_basic() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/a/foo.rs"}},{"type":"tool_use","name":"Read","input":{"file_path":"/a/bar.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_read_count, 2);
        assert!(result.deep.files_read.contains(&"/a/foo.rs".to_string()));
        assert!(result.deep.files_read.contains(&"/a/bar.rs".to_string()));
    }

    #[test]
    fn test_files_read_dedup() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/a/foo.rs"}}]}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/a/foo.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_read_count, 1);
        assert_eq!(result.deep.files_read.len(), 1);
        assert_eq!(result.deep.files_read[0], "/a/foo.rs");
    }

    #[test]
    fn test_files_read_empty() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/a/foo.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_read_count, 0);
        assert!(result.deep.files_read.is_empty());
    }

    #[test]
    fn test_files_read_missing_file_path() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"other":"value"}},{"type":"tool_use","name":"Read","input":{"file_path":"/valid/path.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_read_count, 1);
        assert_eq!(result.deep.files_read[0], "/valid/path.rs");
    }

    #[test]
    fn test_files_read_with_spaces() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/a/my file.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_read_count, 1);
        assert_eq!(result.deep.files_read[0], "/a/my file.rs");
    }

    #[test]
    fn test_files_edited_basic() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}},{"type":"tool_use","name":"Write","input":{"file_path":"bar.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_edited_count, 2);
        assert!(result.deep.files_edited.contains(&"foo.rs".to_string()));
        assert!(result.deep.files_edited.contains(&"bar.rs".to_string()));
    }

    #[test]
    fn test_files_edited_all_occurrences_stored() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}}]}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}}]}}
{"type":"user","message":{"content":"q3"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_edited.len(), 3);
        assert_eq!(result.deep.files_edited_count, 1);
    }

    #[test]
    fn test_files_edited_missing_file_path() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{}},{"type":"tool_use","name":"Edit","input":{"file_path":"valid.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_edited.len(), 1);
        assert_eq!(result.deep.files_edited_count, 1);
    }

    #[test]
    fn test_files_edited_write_tool() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Write","input":{"file_path":"new_file.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.files_edited.len(), 1);
        assert_eq!(result.deep.files_edited_count, 1);
        assert_eq!(result.deep.files_edited[0], "new_file.rs");
    }

    #[test]
    fn test_reedited_files_count_one_reedited() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}}]}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}}]}}
{"type":"user","message":{"content":"q3"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"foo.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"bar.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.reedited_files_count, 1);
    }

    #[test]
    fn test_reedited_files_count_all_unique() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"a.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"b.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"c.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.reedited_files_count, 0);
    }

    #[test]
    fn test_reedited_files_count_empty() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":"No edits"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.reedited_files_count, 0);
    }

    #[test]
    fn test_reedited_files_count_two_reedited() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"x.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"y.rs"}}]}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"x.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"y.rs"}}]}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.reedited_files_count, 2);
    }

    #[test]
    fn test_duration_seconds_basic() {
        let data =
            br#"{"type":"user","timestamp":"2026-01-27T10:00:00Z","message":{"content":"q1"}}
{"type":"assistant","timestamp":"2026-01-27T10:05:00Z","message":{"content":"a1"}}
{"type":"user","timestamp":"2026-01-27T10:10:00Z","message":{"content":"q2"}}
{"type":"assistant","timestamp":"2026-01-27T10:15:30Z","message":{"content":"a2"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.duration_seconds, 930);
    }

    #[test]
    fn test_duration_seconds_single_message() {
        let data =
            br#"{"type":"user","timestamp":"2026-01-27T10:00:00Z","message":{"content":"q1"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.duration_seconds, 0);
    }

    #[test]
    fn test_duration_seconds_empty_file() {
        let result = parse_bytes(b"");
        assert_eq!(result.deep.duration_seconds, 0);
        assert!(result.deep.first_timestamp.is_none());
        assert!(result.deep.last_timestamp.is_none());
    }

    #[test]
    fn test_duration_seconds_no_timestamps() {
        let data = br#"{"type":"user","message":{"content":"q1"}}
{"type":"assistant","message":{"content":"a1"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.duration_seconds, 0);
    }

    #[test]
    fn test_duration_seconds_unix_timestamp() {
        let data = br#"{"type":"user","timestamp":1706400000,"message":{"content":"q1"}}
{"type":"assistant","timestamp":1706400500,"message":{"content":"a1"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.duration_seconds, 500);
    }

    #[test]
    fn test_duration_seconds_mixed_messages() {
        let data =
            br#"{"type":"user","timestamp":"2026-01-27T10:00:00Z","message":{"content":"q1"}}
{"type":"assistant","message":{"content":"a1"}}
{"type":"user","message":{"content":"q2"}}
{"type":"assistant","timestamp":"2026-01-27T10:10:00Z","message":{"content":"a2"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.duration_seconds, 600);
    }

    #[test]
    fn test_count_reedited_files_helper() {
        use crate::indexer_parallel::helpers::count_reedited_files;
        assert_eq!(count_reedited_files(&[]), 0);
        let unique = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(count_reedited_files(&unique), 0);
        let one_reedit = vec!["a".to_string(), "a".to_string(), "b".to_string()];
        assert_eq!(count_reedited_files(&one_reedit), 1);
        let multi_reedit = vec![
            "a".to_string(),
            "a".to_string(),
            "b".to_string(),
            "b".to_string(),
            "b".to_string(),
            "c".to_string(),
        ];
        assert_eq!(count_reedited_files(&multi_reedit), 2);
    }

    #[test]
    fn test_all_metrics_together() {
        let data = br#"{"type":"user","timestamp":"2026-01-27T10:00:00Z","message":{"content":"Read and edit files"}}
{"type":"assistant","timestamp":"2026-01-27T10:05:00Z","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","name":"Read","input":{"file_path":"/src/lib.rs"}},{"type":"tool_use","name":"Edit","input":{"file_path":"/src/main.rs"}}]}}
{"type":"user","timestamp":"2026-01-27T10:10:00Z","message":{"content":"Edit again"}}
{"type":"assistant","timestamp":"2026-01-27T10:15:00Z","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/src/main.rs"}},{"type":"tool_use","name":"Write","input":{"file_path":"/src/new.rs"}}]}}
{"type":"user","timestamp":"2026-01-27T10:20:00Z","message":{"content":"Done"}}
{"type":"assistant","timestamp":"2026-01-27T10:25:00Z","message":{"content":"All done!"}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.user_prompt_count, 3);
        assert_eq!(result.deep.api_call_count, 3);
        assert_eq!(result.deep.tool_call_count, 5);
        assert_eq!(result.deep.files_read_count, 2);
        assert_eq!(result.deep.files_edited.len(), 3);
        assert_eq!(result.deep.files_edited_count, 2);
        assert_eq!(result.deep.reedited_files_count, 1);
        assert_eq!(result.deep.duration_seconds, 1500);
    }

    #[test]
    fn test_simd_prefilter_matches_full_parse() {
        let data = br#"{"type":"progress","uuid":"p1","data":{"type":"agent_progress"}}
{"type":"progress","uuid":"p2","data":{"type":"bash_progress"}}
{"type":"progress","uuid":"p3","data":{"type":"hook_progress"}}
{"type":"progress","uuid":"p4","data":{"type":"mcp_progress"}}
{"type":"progress","uuid":"p5","data":{"type":"waiting_for_task"}}
{"type":"queue-operation","uuid":"q1","operation":"enqueue"}
{"type":"queue-operation","uuid":"q2","operation":"dequeue"}
{"type":"file-history-snapshot","uuid":"f1","snapshot":{}}
{"type":"user","uuid":"u1","message":{"role":"user","content":"hello"}}
{"type":"assistant","uuid":"a1","message":{"role":"assistant","model":"claude-sonnet-4-20250514","content":[{"type":"text","text":"hi"}]}}
"#;
        let result = parse_bytes(data);
        let diag = &result.diagnostics;
        assert_eq!(result.deep.agent_spawn_count, 1);
        assert_eq!(result.deep.bash_progress_count, 1);
        assert_eq!(result.deep.hook_progress_count, 1);
        assert_eq!(result.deep.mcp_progress_count, 1);
        assert_eq!(diag.lines_progress, 5);
        assert_eq!(result.deep.queue_enqueue_count, 1);
        assert_eq!(result.deep.queue_dequeue_count, 1);
        assert_eq!(result.deep.file_snapshot_count, 1);
        assert_eq!(diag.lines_user, 1);
        assert_eq!(diag.lines_assistant, 1);
        assert_eq!(diag.json_parse_attempts, 1);
    }

    // ========================================================================
    // Pass 1 / Pass 2 / Integration Tests
    // ========================================================================

    fn setup_test_claude_dir() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().to_path_buf();
        let project_dir = claude_dir.join("projects").join("test-project");
        std::fs::create_dir_all(&project_dir).unwrap();
        let jsonl_path = project_dir.join("sess-001.jsonl");
        let jsonl_content = br#"{"type":"user","message":{"content":"hello world"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}}]}}
{"type":"user","message":{"content":"now edit it"}}
{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Edit","input":{"file_path":"/src/lib.rs"}}]}}
"#;
        std::fs::write(&jsonl_path, jsonl_content).unwrap();
        let index = format!(
            r#"[
            {{
                "sessionId": "sess-001",
                "fullPath": "{}",
                "firstPrompt": "hello world",
                "summary": "Test session about editing",
                "messageCount": 4,
                "modified": "2026-01-25T17:18:30.718Z",
                "gitBranch": "main",
                "isSidechain": false
            }}
        ]"#,
            jsonl_path.to_string_lossy().replace('\\', "\\\\")
        );
        std::fs::write(project_dir.join("sessions-index.json"), index).unwrap();
        (tmp, claude_dir)
    }

    #[tokio::test]
    async fn test_pass_1_reads_and_inserts() {
        let (_tmp, claude_dir) = setup_test_claude_dir();
        let db = Database::new_in_memory().await.unwrap();
        let (projects, sessions) = pass_1_read_indexes(&claude_dir, &db).await.unwrap();
        assert_eq!(projects, 1);
        assert_eq!(sessions, 1);
        let db_projects = db.list_projects().await.unwrap();
        assert_eq!(db_projects.len(), 1);
        assert_eq!(db_projects[0].sessions.len(), 1);
        assert_eq!(db_projects[0].sessions[0].id, "sess-001");
        assert_eq!(db_projects[0].sessions[0].preview, "hello world");
    }

    #[tokio::test]
    async fn test_pass_1_empty_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().to_path_buf();
        std::fs::create_dir_all(claude_dir.join("projects")).unwrap();
        let db = Database::new_in_memory().await.unwrap();
        let (projects, sessions) = pass_1_read_indexes(&claude_dir, &db).await.unwrap();
        assert_eq!(projects, 0);
        assert_eq!(sessions, 0);
    }

    #[tokio::test]
    async fn test_pass_2_fills_deep_fields() {
        let (_tmp, claude_dir) = setup_test_claude_dir();
        let db = Database::new_in_memory().await.unwrap();
        pass_1_read_indexes(&claude_dir, &db).await.unwrap();
        let progress = Arc::new(AtomicUsize::new(0));
        let progress_clone = progress.clone();
        let (indexed, _) = pass_2_deep_index(
            &db,
            None,
            None,
            |_| {},
            move |done, _total, _bytes| {
                progress_clone.store(done, Ordering::Relaxed);
            },
        )
        .await
        .unwrap();
        assert_eq!(indexed, 1);
        assert_eq!(progress.load(Ordering::Relaxed), 1);
        let projects = db.list_projects().await.unwrap();
        let session = &projects[0].sessions[0];
        assert!(session.deep_indexed);
        assert_eq!(session.turn_count, 2);
        assert_eq!(session.tool_counts.read, 1);
        assert_eq!(session.tool_counts.edit, 1);
        assert_eq!(session.last_message, "now edit it");
    }

    #[tokio::test]
    async fn test_pass_2_skips_already_indexed() {
        let (_tmp, claude_dir) = setup_test_claude_dir();
        let db = Database::new_in_memory().await.unwrap();
        pass_1_read_indexes(&claude_dir, &db).await.unwrap();
        let (first_run, _) = pass_2_deep_index(&db, None, None, |_| {}, |_, _, _| {})
            .await
            .unwrap();
        assert_eq!(first_run, 1);
        let (second_run, _) = pass_2_deep_index(&db, None, None, |_| {}, |_, _, _| {})
            .await
            .unwrap();
        assert_eq!(second_run, 0);
    }

    #[test]
    fn test_extract_commit_skill_invocations_commit() {
        let raw = vec![RawInvocation {
            name: "Skill".to_string(),
            input: Some(serde_json::json!({"skill": "commit"})),
            byte_offset: 0,
            timestamp: 1706400120,
        }];
        let result = extract_commit_skill_invocations(&raw);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].skill_name, "commit");
        assert_eq!(result[0].timestamp_unix, 1706400120);
    }

    #[test]
    fn test_extract_commit_skill_invocations_mixed() {
        let raw = vec![
            RawInvocation {
                name: "Skill".to_string(),
                input: Some(serde_json::json!({"skill": "brainstorm"})),
                byte_offset: 0,
                timestamp: 1706400000,
            },
            RawInvocation {
                name: "Skill".to_string(),
                input: Some(serde_json::json!({"skill": "commit"})),
                byte_offset: 100,
                timestamp: 1706400100,
            },
            RawInvocation {
                name: "Read".to_string(),
                input: Some(serde_json::json!({"file_path": "/foo"})),
                byte_offset: 200,
                timestamp: 1706400200,
            },
            RawInvocation {
                name: "Skill".to_string(),
                input: Some(serde_json::json!({"skill": "commit-commands:commit-push-pr"})),
                byte_offset: 300,
                timestamp: 1706400300,
            },
        ];
        let result = extract_commit_skill_invocations(&raw);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].skill_name, "commit");
        assert_eq!(result[1].skill_name, "commit-commands:commit-push-pr");
    }

    #[test]
    fn test_extract_commit_skill_invocations_empty_input() {
        let raw = vec![RawInvocation {
            name: "Skill".to_string(),
            input: None,
            byte_offset: 0,
            timestamp: 1706400000,
        }];
        let result = extract_commit_skill_invocations(&raw);
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_commit_skill_invocations_empty_list() {
        let raw: Vec<RawInvocation> = vec![];
        let result = extract_commit_skill_invocations(&raw);
        assert!(result.is_empty());
    }

    // ========================================================================
    // Golden Fixture Tests
    // ========================================================================

    #[test]
    fn test_golden_complete_session() {
        let data = include_bytes!("../../tests/golden_fixtures/complete_session.jsonl");
        let result = parse_bytes(data);
        let diag = &result.diagnostics;
        assert_eq!(diag.lines_total, 9);
        assert_eq!(diag.lines_user, 2);
        assert_eq!(diag.lines_assistant, 2);
        assert_eq!(diag.lines_system, 1);
        assert_eq!(diag.lines_progress, 1);
        assert_eq!(diag.lines_queue_op, 2);
        assert_eq!(diag.lines_file_snapshot, 1);
        assert_eq!(diag.lines_unknown_type, 0);
        assert_eq!(diag.json_parse_failures, 0);
        assert_eq!(result.deep.user_prompt_count, 2);
        assert_eq!(result.deep.api_call_count, 2);
        assert_eq!(result.deep.tool_call_count, 3);
        assert_eq!(result.deep.total_input_tokens, 3500);
        assert_eq!(result.deep.total_output_tokens, 350);
        assert_eq!(result.deep.total_task_time_seconds, 185);
        assert_eq!(result.deep.longest_task_seconds, Some(185));
    }

    #[test]
    fn test_golden_edge_cases() {
        let data = include_bytes!("../../tests/golden_fixtures/edge_cases.jsonl");
        let result = parse_bytes(data);
        let diag = &result.diagnostics;
        assert_eq!(diag.lines_unknown_type, 1);
        assert_eq!(diag.json_parse_failures, 1);
        assert_eq!(diag.content_not_array, 1);
        assert_eq!(diag.tool_use_missing_name, 1);
        assert_eq!(result.deep.api_error_count, 1);
        assert_eq!(result.deep.compaction_count, 1);
        assert_eq!(result.deep.agent_spawn_count, 1);
    }

    #[test]
    fn test_golden_spacing_variants() {
        let data = include_bytes!("../../tests/golden_fixtures/spacing_variants.jsonl");
        let result = parse_bytes(data);
        assert_eq!(result.diagnostics.lines_user, 3);
        assert_eq!(result.deep.user_prompt_count, 3);
    }

    #[test]
    fn test_golden_empty_session() {
        let data = include_bytes!("../../tests/golden_fixtures/empty_session.jsonl");
        let result = parse_bytes(data);
        assert_eq!(result.diagnostics.lines_total, 0);
        assert_eq!(result.deep.user_prompt_count, 0);
    }

    #[test]
    fn test_golden_text_only() {
        let data = include_bytes!("../../tests/golden_fixtures/text_only_session.jsonl");
        let result = parse_bytes(data);
        assert_eq!(result.deep.user_prompt_count, 2);
        assert_eq!(result.deep.api_call_count, 2);
        assert_eq!(result.deep.tool_call_count, 0);
        assert_eq!(result.deep.total_input_tokens, 300);
        assert_eq!(result.deep.total_output_tokens, 150);
    }

    #[test]
    fn test_multi_turn_task_time() {
        let data = include_bytes!("../../tests/golden_fixtures/multi_turn_task_time.jsonl");
        let result = parse_bytes(data);
        assert_eq!(result.deep.user_prompt_count, 6);
        assert_eq!(result.deep.total_task_time_seconds, 235);
        assert_eq!(result.deep.longest_task_seconds, Some(100));
    }

    #[test]
    fn test_parse_diagnostics_default_zeroes() {
        let diag = ParseDiagnostics::default();
        assert_eq!(diag.lines_total, 0);
        assert_eq!(diag.lines_user, 0);
        assert_eq!(diag.unknown_source_role_count, 0);
    }

    #[test]
    fn test_extended_metadata_new_fields_default() {
        let meta = ExtendedMetadata::default();
        assert_eq!(meta.total_input_tokens, 0);
        assert_eq!(meta.total_output_tokens, 0);
        assert_eq!(meta.api_error_count, 0);
        assert!(meta.summary_text.is_none());
        assert!(meta.turn_durations_ms.is_empty());
    }

    #[test]
    fn test_parse_bytes_deduplicates_content_blocks() {
        let data = br#"{"type":"user","uuid":"u1","message":{"content":"hello"}}
{"type":"assistant","uuid":"a1","parentUuid":"u1","requestId":"req_001","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_001","model":"claude-opus-4-6","content":[{"type":"thinking"}],"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":1000,"cache_creation_input_tokens":200}}}
{"type":"assistant","uuid":"a2","parentUuid":"a1","requestId":"req_001","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_001","model":"claude-opus-4-6","content":[{"type":"text","text":"hello"}],"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":1000,"cache_creation_input_tokens":200}}}
{"type":"assistant","uuid":"a3","parentUuid":"a2","requestId":"req_001","timestamp":"2026-01-01T00:00:00Z","message":{"id":"msg_001","model":"claude-opus-4-6","content":[{"type":"tool_use","name":"Read","input":{"file_path":"/foo.rs"}}],"usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":1000,"cache_creation_input_tokens":200}}}
"#;
        let result = parse_bytes(data);
        assert_eq!(result.deep.total_input_tokens, 100);
        assert_eq!(result.deep.total_output_tokens, 50);
        assert_eq!(result.deep.api_call_count, 1);
        assert_eq!(result.turns.len(), 3);
        assert_eq!(result.turns[0].input_tokens, Some(100));
        assert_eq!(result.turns[1].input_tokens, None);
        assert_eq!(result.turns[2].input_tokens, None);
    }

    #[test]
    fn test_golden_dedup_content_blocks() {
        let data = include_bytes!("../../tests/golden_fixtures/dedup_content_blocks.jsonl");
        let result = parse_bytes(data);
        assert_eq!(result.deep.api_call_count, 2);
        assert_eq!(result.deep.total_input_tokens, 3500);
        assert_eq!(result.deep.total_output_tokens, 350);
    }

    #[test]
    fn test_parse_bytes_ignores_unknown_top_level_fields() {
        let line1 = r#"{"type":"assistant","ignoredTopLevelCost":0.05,"message":{"role":"assistant","model":"claude-sonnet-4-6","content":[{"type":"text","text":"Hello"}],"usage":{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":200,"cache_creation_input_tokens":50}}}"#;
        let line2 = r#"{"type":"assistant","message":{"role":"assistant","model":"claude-sonnet-4-6","content":[{"type":"text","text":"World"}],"usage":{"input_tokens":800,"output_tokens":300,"cache_read_input_tokens":100,"cache_creation_input_tokens":30}}}"#;
        let input = format!("{}\n{}\n", line1, line2);
        let result = parse_bytes(input.as_bytes());
        assert_eq!(result.turns.len(), 2);
        assert_eq!(result.deep.total_input_tokens, 1800);
        assert_eq!(result.deep.total_output_tokens, 800);
    }

    // ========================================================================
    // Prune stale sessions tests
    // ========================================================================

    fn setup_two_session_claude_dir() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf)
    {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().to_path_buf();
        let project_a_dir = claude_dir.join("projects").join("project-a");
        std::fs::create_dir_all(&project_a_dir).unwrap();
        let jsonl_a = project_a_dir.join("sess-001.jsonl");
        std::fs::write(
            &jsonl_a,
            br#"{"type":"user","message":{"content":"hello from session 1"}}
{"type":"assistant","message":{"content":"hi back"}}
"#,
        )
        .unwrap();
        let index_a = format!(
            r#"[{{"sessionId":"sess-001","fullPath":"{}","firstPrompt":"hello from session 1","messageCount":2,"modified":"2026-01-25T10:00:00.000Z"}}]"#,
            jsonl_a.to_string_lossy().replace('\\', "\\\\")
        );
        std::fs::write(project_a_dir.join("sessions-index.json"), index_a).unwrap();
        let project_b_dir = claude_dir.join("projects").join("project-b");
        std::fs::create_dir_all(&project_b_dir).unwrap();
        let jsonl_b = project_b_dir.join("sess-002.jsonl");
        std::fs::write(
            &jsonl_b,
            br#"{"type":"user","message":{"content":"hello from session 2"}}
{"type":"assistant","message":{"content":"hi again"}}
"#,
        )
        .unwrap();
        let index_b = format!(
            r#"[{{"sessionId":"sess-002","fullPath":"{}","firstPrompt":"hello from session 2","messageCount":2,"modified":"2026-01-25T11:00:00.000Z"}}]"#,
            jsonl_b.to_string_lossy().replace('\\', "\\\\")
        );
        std::fs::write(project_b_dir.join("sessions-index.json"), index_b).unwrap();
        (tmp, claude_dir, jsonl_b)
    }

    #[tokio::test]
    async fn test_prune_stale_sessions_removes_deleted_files() {
        let (_tmp, claude_dir, jsonl_b_path) = setup_two_session_claude_dir();
        let db = Database::new_in_memory().await.unwrap();
        pass_1_read_indexes(&claude_dir, &db).await.unwrap();
        std::fs::remove_file(&jsonl_b_path).unwrap();
        let pruned = prune_stale_sessions(&db).await.unwrap();
        assert_eq!(pruned, 1);
    }

    #[tokio::test]
    async fn test_prune_stale_sessions_no_op_when_all_exist() {
        let (_tmp, claude_dir, _) = setup_two_session_claude_dir();
        let db = Database::new_in_memory().await.unwrap();
        pass_1_read_indexes(&claude_dir, &db).await.unwrap();
        let pruned = prune_stale_sessions(&db).await.unwrap();
        assert_eq!(pruned, 0);
    }

    #[tokio::test]
    async fn test_prune_stale_sessions_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let pruned = prune_stale_sessions(&db).await.unwrap();
        assert_eq!(pruned, 0);
    }

    #[test]
    fn test_first_user_prompt_extracted() {
        let data = br#"{"type":"user","message":{"content":"What is Rust?"}}
{"type":"assistant","message":{"content":"Rust is a systems programming language."}}
{"type":"user","message":{"content":"How do I use iterators?"}}
{"type":"assistant","message":{"content":"Iterators in Rust..."}}
"#;
        let result = parse_bytes(data);
        assert_eq!(
            result.deep.first_user_prompt,
            Some("What is Rust?".to_string())
        );
        assert_eq!(result.deep.last_message, "How do I use iterators?");
    }

    #[test]
    fn test_first_user_prompt_none_when_no_users() {
        let data = br#"{"type":"assistant","message":{"content":"Hello, I'm an assistant."}}
{"type":"assistant","message":{"content":"Here is more info."}}
"#;
        let result = parse_bytes(data);
        assert!(result.deep.first_user_prompt.is_none());
    }
}

#[cfg(test)]
mod index_hints_tests {
    use std::path::Path;

    #[test]
    fn build_index_hints_returns_empty_for_missing_dir() {
        let hints = super::super::build_index_hints(Path::new("/nonexistent"));
        assert!(hints.is_empty());
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod scan_and_index_tests {
    use super::super::*;
    use crate::Database;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[tokio::test]
    async fn scan_and_index_skips_unchanged_files() {
        let db = Database::new_in_memory().await.unwrap();
        let tmp = tempdir().unwrap();
        let project_dir = tmp.path().join("projects").join("-test-project");
        std::fs::create_dir_all(&project_dir).unwrap();
        let session_file = project_dir.join("sess-001.jsonl");
        std::fs::write(&session_file, concat!(
            r#"{"parentUuid":null,"isFinal":false,"type":"user","uuid":"u1","message":{"role":"user","content":[{"type":"text","text":"Hello world"}]}}"#, "\n",
            r#"{"parentUuid":"u1","isFinal":false,"type":"assistant","uuid":"a1","timestamp":1706200000,"message":{"model":"claude-sonnet-4-5-20250929","role":"assistant","content":[{"type":"text","text":"Hi there!"}],"usage":{"input_tokens":100,"output_tokens":50}}}"#, "\n",
        )).unwrap();
        let (indexed, skipped) = scan_and_index_all(
            tmp.path(),
            &db,
            &HashMap::new(),
            None,
            None,
            |_| {},
            |_| {},
            || {},
        )
        .await
        .unwrap();
        assert_eq!(indexed, 1);
        assert_eq!(skipped, 0);
        let (indexed2, skipped2) = scan_and_index_all(
            tmp.path(),
            &db,
            &HashMap::new(),
            None,
            None,
            |_| {},
            |_| {},
            || {},
        )
        .await
        .unwrap();
        assert_eq!(indexed2, 0);
        assert_eq!(skipped2, 1);
    }
}
