//! oMLX HTTP client for phase + scope classification.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use super::SessionPhase;

const TIMEOUT: Duration = Duration::from_secs(5);
/// After this many consecutive errors, signal omlx_ready=false
/// so the lifecycle re-probes with inference before resuming.
const ERROR_THRESHOLD: u32 = 3;

pub const SYSTEM_PROMPT: &str = r#"Classify this AI coding session. Output ONLY a JSON object, nothing else.

{"phase": "...", "scope": "..."}

phase — exactly one of: thinking, planning, building, testing, reviewing, shipping
scope — 3-8 word description of the task (e.g. "XGBoost phase classifier", "chat panel refactor")

Definitions:
- thinking: reading code, exploring, investigating
- planning: designing architecture, writing specs, brainstorming
- building: writing/editing code, implementing features
- testing: running tests, debugging, verifying
- reviewing: code review, auditing, quality checks
- shipping: deploying, releasing, publishing, creating PRs

Use the Signals section (files, commands, tools) alongside the conversation to determine the phase."#;

#[derive(Debug, Clone)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct ConversationTurn {
    pub role: Role,
    pub text: String,
    pub tools: Vec<String>,
}

/// Activity-aware context for a classify call. Enriches raw turns
/// with file references, commands, and tool distribution signals.
#[derive(Debug, Clone)]
pub struct ClassifyContext {
    /// Recent conversation turns (up to 12).
    pub turns: Vec<ConversationTurn>,
    /// First user message (session intent / scope signal).
    pub first_user_message: String,
    /// Files the user referenced via @mentions or IDE context (max 5).
    pub user_files: Vec<String>,
    /// Files edited by the assistant via Edit/Write (max 5).
    pub edited_files: Vec<String>,
    /// Recent bash commands (max 5).
    pub bash_commands: Vec<String>,
    /// Tool distribution summary, e.g. "Edit:12 Read:8 Bash:5 Write:2".
    pub tool_summary: String,
}

pub struct OmlxClient {
    http: Client,
    base_url: String,
    model: String,
    consecutive_errors: AtomicU32,
    /// Shared readiness flag — cleared on repeated errors so the lifecycle
    /// re-probes with inference. The drain loop gates on this same flag.
    omlx_ready: Option<Arc<AtomicBool>>,
    /// Optional debug channel — receives one JSON line per API call.
    debug_tx: Option<mpsc::Sender<String>>,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
    chat_template_kwargs: ChatTemplateKwargs,
}

#[derive(Serialize)]
struct ChatTemplateKwargs {
    enable_thinking: bool,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

impl OmlxClient {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            http: Client::builder()
                .timeout(TIMEOUT)
                .build()
                .expect("reqwest client"),
            base_url,
            model,
            consecutive_errors: AtomicU32::new(0),
            omlx_ready: None,
            debug_tx: None,
        }
    }

    /// Attach the shared readiness flag. On repeated errors, the client
    /// clears this flag so the lifecycle re-probes before the drain loop
    /// sends more requests.
    pub fn with_ready_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.omlx_ready = Some(flag);
        self
    }

    /// Attach a debug log channel. Each API call emits one JSON line.
    pub fn with_debug_tx(mut self, tx: mpsc::Sender<String>) -> Self {
        self.debug_tx = Some(tx);
        self
    }

    pub async fn classify(
        &self,
        context: &ClassifyContext,
        temperature: f32,
        session_id: &str,
        generation: u64,
    ) -> Option<(SessionPhase, Option<String>)> {
        let conversation = format_context(context);
        let req = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: SYSTEM_PROMPT.into(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: conversation.clone(),
                },
            ],
            temperature,
            max_tokens: 80,
            chat_template_kwargs: ChatTemplateKwargs {
                enable_thinking: false,
            },
        };

        let url = format!("{}/v1/chat/completions", self.base_url);
        let t0 = Instant::now();
        let resp = self.http.post(&url).json(&req).send().await;
        let latency_ms = t0.elapsed().as_millis() as u64;

        match resp {
            Ok(r) if r.status().is_success() => {
                self.consecutive_errors.store(0, Ordering::Relaxed);
                let body: ChatResponse = r.json().await.ok()?;
                let content = body.choices.first()?.message.content.clone();
                let parsed = parse_classify_response(&content);
                self.debug_log_call(
                    session_id, generation,
                    context.turns.len(), temperature, &conversation, latency_ms,
                    Some(&content), parsed.as_ref(), None,
                );
                parsed
            }
            Ok(r) => {
                let status = r.status().as_u16();
                let body = r.text().await.unwrap_or_default();
                self.signal_error();
                self.debug_log_call(
                    session_id, generation,
                    context.turns.len(), temperature, &conversation, latency_ms,
                    None, None, Some(&format!("http_{status}: {body}")),
                );
                None
            }
            Err(e) => {
                self.signal_error();
                self.debug_log_call(
                    session_id, generation,
                    context.turns.len(), temperature, &conversation, latency_ms,
                    None, None, Some(&e.to_string()),
                );
                None
            }
        }
    }

    /// Track consecutive errors. After ERROR_THRESHOLD, clear omlx_ready
    /// so the lifecycle re-probes with inference before the drain loop resumes.
    fn signal_error(&self) {
        let errors = self.consecutive_errors.fetch_add(1, Ordering::Relaxed) + 1;
        if errors >= ERROR_THRESHOLD {
            if let Some(ref flag) = self.omlx_ready {
                flag.store(false, Ordering::Release);
            }
            self.consecutive_errors.store(0, Ordering::Relaxed);
        }
    }

    /// Fire-and-forget one JSONL debug line per API call.
    #[allow(clippy::too_many_arguments)]
    fn debug_log_call(
        &self,
        session_id: &str,
        generation: u64,
        turn_count: usize,
        temperature: f32,
        conversation: &str,
        latency_ms: u64,
        raw_response: Option<&str>,
        parsed: Option<&(SessionPhase, Option<String>)>,
        error: Option<&str>,
    ) {
        let Some(tx) = &self.debug_tx else { return };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let (phase, scope) = match parsed {
            Some((p, s)) => (Some(format!("{p:?}").to_lowercase()), s.clone()),
            None => (None, None),
        };
        let line = serde_json::json!({
            "ts": now,
            "session_id": session_id,
            "generation": generation,
            "model": &self.model,
            "turns": turn_count,
            "temperature": temperature,
            "latency_ms": latency_ms,
            "conversation": conversation,
            "raw_response": raw_response,
            "phase": phase,
            "scope": scope,
            "error": error,
        });
        let _ = tx.try_send(line.to_string());
    }
}

pub fn parse_classify_response(content: &str) -> Option<(SessionPhase, Option<String>)> {
    let json_str = if let Some(start) = content.find('{') {
        if let Some(end) = content.rfind('}') {
            &content[start..=end]
        } else {
            return None;
        }
    } else {
        return None;
    };

    let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let phase_str = parsed.get("phase")?.as_str()?;
    let phase = match phase_str {
        "thinking" => SessionPhase::Thinking,
        "planning" => SessionPhase::Planning,
        "building" => SessionPhase::Building,
        "testing" => SessionPhase::Testing,
        "reviewing" => SessionPhase::Reviewing,
        "shipping" => SessionPhase::Shipping,
        _ => return None,
    };
    let scope = parsed
        .get("scope")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    Some((phase, scope))
}

pub fn format_conversation(turns: &[ConversationTurn]) -> String {
    let mut out = String::with_capacity(4096);

    if let Some(first_user) = turns.iter().find(|t| matches!(t.role, Role::User)) {
        out.push_str("User: ");
        out.push_str(&first_user.text);
        out.push_str("\n---\n");
    }

    let recent: Vec<&ConversationTurn> = turns
        .iter()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    for turn in recent {
        match turn.role {
            Role::User => out.push_str("User: "),
            Role::Assistant => out.push_str("Assistant: "),
        }
        out.push_str(&turn.text);
        if !turn.tools.is_empty() {
            let tools: String = turn
                .tools
                .iter()
                .take(10)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(" [tools: {}]", tools));
        }
        out.push('\n');
    }

    let char_count = out.chars().count();
    if char_count > 8000 {
        let start_idx = out
            .char_indices()
            .nth(char_count - 8000)
            .map(|(i, _)| i)
            .unwrap_or(0);
        out = out[start_idx..].to_string();
    }
    out
}

/// Format a `ClassifyContext` into the structured prompt sent to oMLX.
/// Includes session goal, activity signals, and recent conversation.
pub fn format_context(ctx: &ClassifyContext) -> String {
    let mut out = String::with_capacity(6000);

    // Section 1: Session goal (first user message)
    if !ctx.first_user_message.is_empty() {
        out.push_str("## Session Goal\n");
        let truncated = truncate_str(&ctx.first_user_message, 200);
        out.push_str(&truncated);
        out.push_str("\n\n");
    }

    // Section 2: Activity signals (only if any exist)
    let has_signals = !ctx.user_files.is_empty()
        || !ctx.edited_files.is_empty()
        || !ctx.bash_commands.is_empty()
        || !ctx.tool_summary.is_empty();
    if has_signals {
        out.push_str("## Signals\n");
        if !ctx.user_files.is_empty() {
            out.push_str("Files referenced: ");
            out.push_str(&ctx.user_files.join(", "));
            out.push('\n');
        }
        if !ctx.edited_files.is_empty() {
            out.push_str("Files edited: ");
            out.push_str(&ctx.edited_files.join(", "));
            out.push('\n');
        }
        if !ctx.bash_commands.is_empty() {
            out.push_str("Commands: ");
            out.push_str(&ctx.bash_commands.join("; "));
            out.push('\n');
        }
        if !ctx.tool_summary.is_empty() {
            out.push_str("Tools: ");
            out.push_str(&ctx.tool_summary);
            out.push('\n');
        }
        out.push('\n');
    }

    // Section 3: Recent conversation (reuse format_conversation logic)
    out.push_str("## Recent Activity\n");
    let recent: Vec<&ConversationTurn> = ctx
        .turns
        .iter()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    for turn in recent {
        match turn.role {
            Role::User => out.push_str("User: "),
            Role::Assistant => out.push_str("Assistant: "),
        }
        out.push_str(&turn.text);
        if !turn.tools.is_empty() {
            let tools: String = turn.tools.iter().take(10).cloned().collect::<Vec<_>>().join(", ");
            out.push_str(&format!(" [tools: {}]", tools));
        }
        out.push('\n');
    }

    // Budget cap: ~1500 tokens ≈ 6000 chars
    let char_count = out.chars().count();
    if char_count > 6000 {
        let start_idx = out
            .char_indices()
            .nth(char_count - 6000)
            .map(|(i, _)| i)
            .unwrap_or(0);
        out = out[start_idx..].to_string();
    }
    out
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_json() {
        let json = r#"{"phase": "building", "scope": "auth system"}"#;
        let result = parse_classify_response(json);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.0, SessionPhase::Building);
        assert_eq!(r.1.as_deref(), Some("auth system"));
    }

    #[test]
    fn parse_invalid_phase() {
        let json = r#"{"phase": "debugging", "scope": "test"}"#;
        assert!(parse_classify_response(json).is_none());
    }

    #[test]
    fn parse_invalid_json() {
        assert!(parse_classify_response("not json").is_none());
    }

    #[test]
    fn parse_missing_scope() {
        let json = r#"{"phase": "testing"}"#;
        let result = parse_classify_response(json);
        assert!(result.is_some());
        assert_eq!(result.unwrap().1, None);
    }

    #[test]
    fn parse_json_with_preamble() {
        let text = r#"Here is the classification: {"phase": "building", "scope": "test"}"#;
        let result = parse_classify_response(text);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, SessionPhase::Building);
    }

    #[test]
    fn format_preserves_first_user_turn() {
        let turns = vec![
            ConversationTurn {
                role: Role::User,
                text: "refactor auth system".into(),
                tools: vec![],
            },
            ConversationTurn {
                role: Role::Assistant,
                text: "I'll help with that".into(),
                tools: vec!["Read".into()],
            },
        ];
        let formatted = format_conversation(&turns);
        assert!(formatted.contains("refactor auth system"));
    }

    #[test]
    fn format_utf8_safe() {
        let long_cjk = "寫個廣東話嘅".repeat(200);
        let turns = vec![ConversationTurn {
            role: Role::User,
            text: long_cjk,
            tools: vec![],
        }];
        let formatted = format_conversation(&turns);
        assert!(!formatted.is_empty());
    }
}
