// crates/providers/src/parsers/qwen.rs
//
// Qwen Code — JSONL chats at `<root>/<encoded-project>/chats/<id>.jsonl`
// in a Gemini-API-shaped schema:
//   { sessionId, cwd, timestamp, type: "user"|"assistant", model,
//     message: { role: "user"|"model", parts: [
//       {text, thought?} | {functionCall:{id,name,args}}
//       | {functionResponse:{id,name,response.output}} ] },
//     usageMetadata: { promptTokenCount (INCLUDES cached),
//                      candidatesTokenCount, cachedContentTokenCount } }
//
// THE format quirk: Qwen writes one type=assistant line PER model iteration
// of a multi-tool turn. Iterations must coalesce into ONE assistant block:
// consecutive assistant entries absorb into a pending buffer; tool-result-
// only user entries (functionResponse, no text) fold INTO the buffer; the
// buffer flushes on a closing assistant entry (text AND no functionCall),
// a real user text turn, or EOF.

use crate::discover::{stat_entry, DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta, UsageTotals};
use crate::util::{blocks, jsonl, preview, project_from_cwd, time};
use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
use serde_json::Value;
use std::path::Path;

pub struct QwenProvider;

impl Provider for QwenProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Qwen
    }

    fn discover(&self, root: &Path) -> Vec<DiscoveredSession> {
        let mut out = Vec::new();
        let Ok(projects) = std::fs::read_dir(root) else {
            return out;
        };
        for project in projects.flatten() {
            let chats = project.path().join("chats");
            let Ok(files) = std::fs::read_dir(&chats) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let Some((mtime, size_bytes)) = stat_entry(&path) else {
                    continue;
                };
                out.push(DiscoveredSession {
                    id: ProviderKind::Qwen.session_id(stem),
                    provider: ProviderKind::Qwen,
                    path,
                    project_hint: project.file_name().to_str().map(str::to_string),
                    mtime,
                    size_bytes,
                });
            }
        }
        out
    }

    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        let read = jsonl::read_values(path)?;
        let raw_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("no file stem"))?
            .to_string();
        let mut meta = ForeignSessionMeta::new(ProviderKind::Qwen, &raw_id, path.to_path_buf());
        meta.malformed_lines = read.malformed;
        // Project fallback: the encoded dir two levels up (…/<project>/chats/x.jsonl).
        if let Some(dir) = path
            .parent()
            .and_then(Path::parent)
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
        {
            meta.project = dir.to_string();
        }

        let mut out_blocks: Vec<ConversationBlock> = Vec::new();
        let mut pending = PendingTurn::default();
        let mut ordinal = 0usize;

        for line in &read.values {
            if meta.cwd.is_none() {
                if let Some(cwd) = line
                    .get("cwd")
                    .and_then(Value::as_str)
                    .filter(|c| !c.is_empty())
                {
                    meta.cwd = Some(cwd.to_string());
                    meta.project = project_from_cwd(cwd);
                }
            }
            let ts = line
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(|s| time::parse_timestamp(s, false));
            if let Some(ts) = ts {
                meta.observe_timestamp(ts);
            }
            let line_type = line.get("type").and_then(Value::as_str).unwrap_or("");
            let role = line
                .pointer("/message/role")
                .and_then(Value::as_str)
                .unwrap_or("");
            let parts = line
                .pointer("/message/parts")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();

            match (line_type, role) {
                ("user", "user") => {
                    let mut text_parts: Vec<&str> = Vec::new();
                    for part in &parts {
                        if let Some(resp) = part.get("functionResponse") {
                            attach_response(&mut pending, &mut out_blocks, resp);
                        } else if let Some(t) = part.get("text").and_then(Value::as_str) {
                            if !t.trim().is_empty() {
                                text_parts.push(t);
                            }
                        }
                    }
                    if !text_parts.is_empty() {
                        // Real user turn closes any pending assistant turn.
                        if pending.flush(&mut out_blocks, &raw_id, &mut ordinal) {
                            meta.message_count += 1;
                        }
                        let text = text_parts.join("\n");
                        if meta.first_message.is_empty() {
                            meta.first_message = preview(&text, 200);
                        }
                        meta.message_count += 1;
                        meta.user_message_count += 1;
                        out_blocks.push(blocks::user(
                            blocks::block_id(&raw_id, bump(&mut ordinal)),
                            text,
                            ts,
                        ));
                    }
                }
                ("assistant", "model") => {
                    let mut closing = false;
                    let mut has_call = false;
                    for part in &parts {
                        if let Some(call) = part.get("functionCall") {
                            has_call = true;
                            let name = call.get("name").and_then(Value::as_str).unwrap_or("tool");
                            let id = call.get("id").and_then(Value::as_str).unwrap_or_default();
                            if !id.is_empty() {
                                pending.segments.push(blocks::tool_segment(
                                    name.to_string(),
                                    call.get("args").cloned().unwrap_or(Value::Null),
                                    id.to_string(),
                                ));
                            }
                        } else if let Some(t) = part.get("text").and_then(Value::as_str) {
                            if t.trim().is_empty() {
                                continue;
                            }
                            if part.get("thought").and_then(Value::as_bool) == Some(true) {
                                pending.thinking.push(t.to_string());
                            } else {
                                pending.segments.push(blocks::text_segment(t.to_string()));
                                closing = true;
                            }
                        }
                    }
                    pending.ts = pending.ts.or(ts);
                    record_usage(&mut meta, line);
                    if closing && !has_call && pending.flush(&mut out_blocks, &raw_id, &mut ordinal)
                    {
                        meta.message_count += 1;
                    }
                }
                _ => {}
            }
        }
        if pending.flush(&mut out_blocks, &raw_id, &mut ordinal) {
            meta.message_count += 1;
        }

        if meta.message_count == 0 {
            return Ok(Vec::new());
        }
        Ok(vec![ForeignSession {
            meta,
            blocks: out_blocks,
        }])
    }
}

fn bump(ordinal: &mut usize) -> usize {
    let v = *ordinal;
    *ordinal += 1;
    v
}

/// Accumulator for one assistant turn across Qwen's per-iteration lines.
#[derive(Default)]
struct PendingTurn {
    segments: Vec<AssistantSegment>,
    thinking: Vec<String>,
    ts: Option<f64>,
}

impl PendingTurn {
    /// Emit the buffered turn as one assistant block. Returns true when a
    /// block was emitted.
    fn flush(
        &mut self,
        out: &mut Vec<ConversationBlock>,
        raw_id: &str,
        ordinal: &mut usize,
    ) -> bool {
        if self.segments.is_empty() && self.thinking.is_empty() {
            return false;
        }
        let thinking = if self.thinking.is_empty() {
            None
        } else {
            Some(self.thinking.join("\n\n"))
        };
        out.push(blocks::assistant(
            blocks::block_id(raw_id, bump(ordinal)),
            std::mem::take(&mut self.segments),
            thinking,
            self.ts.take(),
        ));
        self.thinking.clear();
        true
    }
}

/// Attach a functionResponse to the pending turn's matching tool segment
/// (or, if the turn already flushed, to emitted blocks).
fn attach_response(pending: &mut PendingTurn, out: &mut [ConversationBlock], resp: &Value) {
    let id = resp.get("id").and_then(Value::as_str).unwrap_or_default();
    if id.is_empty() {
        return;
    }
    let output = resp
        .pointer("/response/output")
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default();
    for seg in pending.segments.iter_mut().rev() {
        if let AssistantSegment::Tool { execution } = seg {
            if execution.tool_use_id == id {
                execution.result = Some(claude_view_types::block_types::ToolResult {
                    output,
                    is_error: false,
                    is_replay: false,
                });
                return;
            }
        }
    }
    blocks::attach_tool_result(out, id, output, false);
}

/// Record one line's usageMetadata. promptTokenCount INCLUDES the cached
/// portion — subtract before filling input_tokens. Summing per line equals
/// agentsview's per-turn accumulation for session totals.
fn record_usage(meta: &mut ForeignSessionMeta, line: &Value) {
    let Some(usage) = line.get("usageMetadata").and_then(Value::as_object) else {
        return;
    };
    if usage.is_empty() {
        return;
    }
    let get = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
    let prompt = get("promptTokenCount");
    let cached = get("cachedContentTokenCount");
    let output = get("candidatesTokenCount");
    let model = line.get("model").and_then(Value::as_str).unwrap_or("");
    meta.record_model(model);
    meta.usage.record(
        model,
        UsageTotals {
            input_tokens: prompt.saturating_sub(cached),
            output_tokens: output,
            cache_read_input_tokens: cached,
            cache_creation_input_tokens: 0,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const MULTI_ITERATION: &str = concat!(
        r#"{"sessionId":"s1","cwd":"/Users/x/dev/myapp","timestamp":"2026-01-02T03:04:05Z","type":"user","model":"","message":{"role":"user","parts":[{"text":"list files then summarize"}]}}"#,
        "\n",
        r#"{"sessionId":"s1","timestamp":"2026-01-02T03:04:06Z","type":"assistant","model":"qwen3-coder-plus","message":{"role":"model","parts":[{"text":"planning","thought":true},{"functionCall":{"id":"fc1","name":"list_directory","args":{"path":"."}}}]},"usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":10,"cachedContentTokenCount":40}}"#,
        "\n",
        r#"{"sessionId":"s1","timestamp":"2026-01-02T03:04:07Z","type":"user","message":{"role":"user","parts":[{"functionResponse":{"id":"fc1","name":"list_directory","response":{"output":"a.txt b.txt"}}}]}}"#,
        "\n",
        r#"{"sessionId":"s1","timestamp":"2026-01-02T03:04:08Z","type":"assistant","model":"qwen3-coder-plus","message":{"role":"model","parts":[{"text":"Two files: a.txt and b.txt."}]},"usageMetadata":{"promptTokenCount":150,"candidatesTokenCount":20,"cachedContentTokenCount":100}}"#,
        "\n",
    );

    fn parse_str(content: &str, name: &str) -> Vec<ForeignSession> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        QwenProvider.parse(&path).unwrap()
    }

    #[test]
    fn coalesces_iterations_into_one_assistant_block() {
        let sessions = parse_str(MULTI_ITERATION, "sess-1.jsonl");
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        // 1 user + 1 coalesced assistant — NOT 2 assistant blocks.
        assert_eq!(s.blocks.len(), 2);
        assert_eq!(s.meta.message_count, 2);
        assert_eq!(s.meta.user_message_count, 1);
        let ConversationBlock::Assistant(a) = &s.blocks[1] else {
            panic!("expected assistant block")
        };
        assert_eq!(a.thinking.as_deref(), Some("planning"));
        // tool segment + closing text segment, in order.
        assert_eq!(a.segments.len(), 2);
        let AssistantSegment::Tool { execution } = &a.segments[0] else {
            panic!("expected tool segment first")
        };
        assert_eq!(execution.tool_name, "list_directory");
        assert_eq!(execution.result.as_ref().unwrap().output, "a.txt b.txt");
    }

    #[test]
    fn usage_sums_across_iterations_with_cache_subtraction() {
        let sessions = parse_str(MULTI_ITERATION, "sess-1.jsonl");
        let u = &sessions[0].meta.usage;
        assert!(u.has_usage);
        // (100-40)+(150-100)=110 uncached input; 40+100 cached; 10+20 output.
        assert_eq!(u.totals.input_tokens, 110);
        assert_eq!(u.totals.cache_read_input_tokens, 140);
        assert_eq!(u.totals.output_tokens, 30);
        assert_eq!(u.per_model["qwen3-coder-plus"].output_tokens, 30);
        assert_eq!(sessions[0].meta.models, vec!["qwen3-coder-plus"]);
    }

    #[test]
    fn project_and_cwd_derivation() {
        let sessions = parse_str(MULTI_ITERATION, "sess-1.jsonl");
        assert_eq!(sessions[0].meta.cwd.as_deref(), Some("/Users/x/dev/myapp"));
        assert_eq!(sessions[0].meta.project, "myapp");
        assert_eq!(sessions[0].meta.first_message, "list files then summarize");
    }

    #[test]
    fn empty_sessions_are_skipped() {
        assert!(parse_str("", "empty.jsonl").is_empty());
        // Entry with no contentful messages.
        assert!(parse_str(
            r#"{"sessionId":"s2","type":"user","message":{"role":"user","parts":[]}}"#,
            "s2.jsonl"
        )
        .is_empty());
    }

    #[test]
    fn discovery_walks_project_chats_layout() {
        let dir = tempfile::tempdir().unwrap();
        let chats = dir.path().join("-Users-x-dev-myapp").join("chats");
        std::fs::create_dir_all(&chats).unwrap();
        std::fs::write(chats.join("abc.jsonl"), MULTI_ITERATION).unwrap();
        std::fs::write(chats.join("notes.txt"), "x").unwrap();
        let found = QwenProvider.discover(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "qwen:abc");
        assert_eq!(found[0].project_hint.as_deref(), Some("-Users-x-dev-myapp"));
    }
}
