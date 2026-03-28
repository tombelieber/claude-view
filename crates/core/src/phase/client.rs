//! oMLX HTTP client for phase + scope classification.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use super::SessionPhase;

const TIMEOUT: Duration = Duration::from_secs(5);
const CIRCUIT_BREAKER_THRESHOLD: u32 = 10;
const CIRCUIT_BREAKER_COOLDOWN: Duration = Duration::from_secs(30);

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
- shipping: deploying, releasing, publishing, creating PRs"#;

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

pub struct OmlxClient {
    http: Client,
    base_url: String,
    model: String,
    consecutive_errors: AtomicU32,
    circuit_open_until: std::sync::Mutex<Option<std::time::Instant>>,
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
            circuit_open_until: std::sync::Mutex::new(None),
        }
    }

    pub async fn classify(
        &self,
        turns: &[ConversationTurn],
        temperature: f32,
    ) -> Option<(SessionPhase, Option<String>)> {
        if let Ok(guard) = self.circuit_open_until.lock() {
            if let Some(until) = *guard {
                if std::time::Instant::now() < until {
                    return None;
                }
            }
        }

        let conversation = format_conversation(turns);
        let req = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: SYSTEM_PROMPT.into(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: conversation,
                },
            ],
            temperature,
            max_tokens: 80,
            chat_template_kwargs: ChatTemplateKwargs {
                enable_thinking: false,
            },
        };

        let url = format!("{}/v1/chat/completions", self.base_url);
        let resp = self.http.post(&url).json(&req).send().await;

        match resp {
            Ok(r) if r.status().is_success() => {
                self.consecutive_errors.store(0, Ordering::Relaxed);
                let body: ChatResponse = r.json().await.ok()?;
                let content = body.choices.first()?.message.content.clone();
                parse_classify_response(&content)
            }
            _ => {
                let errors = self.consecutive_errors.fetch_add(1, Ordering::Relaxed) + 1;
                if errors >= CIRCUIT_BREAKER_THRESHOLD {
                    if let Ok(mut guard) = self.circuit_open_until.lock() {
                        *guard = Some(std::time::Instant::now() + CIRCUIT_BREAKER_COOLDOWN);
                    }
                    self.consecutive_errors.store(0, Ordering::Relaxed);
                }
                None
            }
        }
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
