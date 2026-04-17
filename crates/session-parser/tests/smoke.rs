//! Smoke tests for PR 1.2 — Stage A parse_jsonl + Stage B extract_stats
//! + blake3 staleness helpers.
//!
//! These are written TDD-style: each test was RED (compile-error or panic)
//! before its corresponding source module existed.

use std::io::Write;

use claude_view_session_parser::{
    blake3_head_tail, blake3_mid, extract_stats, parse_jsonl, PARSER_VERSION, STATS_VERSION,
};

#[test]
fn parses_empty_input() {
    let doc = parse_jsonl(b"", PARSER_VERSION).unwrap();
    assert_eq!(doc.lines.len(), 0);
}

#[test]
fn parses_single_user_line() {
    let jsonl = br#"{"type":"user","timestamp":"2026-04-18T00:00:00Z","message":{"role":"user","content":[{"type":"text","text":"hi"}]}}
"#;
    let doc = parse_jsonl(jsonl, PARSER_VERSION).unwrap();
    assert_eq!(doc.lines.len(), 1);
}

#[test]
fn extract_counts_user_prompt() {
    let jsonl = br#"{"type":"user","timestamp":"2026-04-18T00:00:00Z","message":{"role":"user","content":[{"type":"text","text":"hi"}]}}
"#;
    let doc = parse_jsonl(jsonl, PARSER_VERSION).unwrap();
    let stats = extract_stats(&doc, STATS_VERSION);
    assert_eq!(stats.user_prompt_count, 1);
}

#[test]
fn blake3_head_tail_is_deterministic() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.write_all(b"hello world").unwrap();
    tmp.flush().unwrap();
    let h1 = blake3_head_tail(tmp.path()).unwrap();
    let h2 = blake3_head_tail(tmp.path()).unwrap();
    assert_eq!(h1, h2);
}

#[test]
fn blake3_mid_differs_from_head_tail_on_large_file() {
    // Shape: 64 KB 'a' + 32 KB 'M' + 64 KB 'z' + 40 KB 'a' (200 KB total).
    // head_tail sees only the two 'a'/'z' windows; mid sees the 'M' block.
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    let chunk_a = vec![b'a'; 64 * 1024];
    let chunk_mid = vec![b'M'; 32 * 1024];
    let chunk_z = vec![b'z'; 64 * 1024];
    let chunk_trail = vec![b'a'; 40 * 1024];
    tmp.write_all(&chunk_a).unwrap();
    tmp.write_all(&chunk_mid).unwrap();
    tmp.write_all(&chunk_z).unwrap();
    tmp.write_all(&chunk_trail).unwrap();
    tmp.flush().unwrap();

    let head_tail = blake3_head_tail(tmp.path()).unwrap();
    let mid = blake3_mid(tmp.path()).unwrap();
    assert_ne!(head_tail, mid, "mid and head_tail windows must differ");
}
