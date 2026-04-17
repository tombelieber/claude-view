#![no_main]

use claude_view_session_parser::{parse_jsonl, PARSER_VERSION};
use libfuzzer_sys::fuzz_target;

// Invariant: parse_jsonl must never panic on arbitrary bytes. It may return
// Ok with an empty doc (all lines blank) or Err(MalformedJson { .. }), but
// it must NEVER unwind. This property protects every upstream consumer
// (Phase 2 shadow-mode writer, Phase 3 read-side cutover, Phase 4 rollup
// pipeline) from accepting untrusted-by-construction JSONL bytes.
//
// The proptest at crates/session-parser/tests/properties.rs exercises the
// same invariant with random Vec<u8>; libfuzzer adds coverage-guided
// mutation that explores parser internal branches proptest may miss.
fuzz_target!(|data: &[u8]| {
    let _ = parse_jsonl(data, PARSER_VERSION);
});
