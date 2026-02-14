---
status: draft
date: 2026-02-15
---

# Intelligent Session State Classification for Mission Control

## Problem

Mission Control's Kanban board has two UX failures:

1. **Board order is Claude-centric, not operator-centric.** The 4 columns (Working → Waiting → Idle → Done) are organized by "what is Claude doing?" instead of "what do I need to know?"

2. **Cards bounce between columns on arbitrary timers.** The 60-second Idle threshold means a session moves from "Waiting" to "Idle" just because the user took a minute to read. The 5-second process detector poll causes constant re-evaluation. Nothing actually changed — a timer ticked.

### Root Cause

The state machine is **rule-based on timers**, not **content-aware**. It treats `seconds_since_modified > 60` as a state change, when the real signal is in the JSONL content: what did Claude last say? Did it ask a question? Did it finish the task?

## Design

### 3-Column Board: Working → Paused → Done

Replace the 4-column layout with 3 operator-centric columns:

| Column | Meaning | Transition trigger |
|--------|---------|-------------------|
| **Working** | Agent is actively streaming or using tools | JSONL shows `streaming` or `tool_use` activity |
| **Paused** | Agent stopped — needs input, finished task, or interrupted | JSONL shows `end_turn` or no new writes for 30s |
| **Done** | Session is over | Process exited + no new writes for 60s, OR user manually marked done |

**"Idle" is eliminated as a column.** It was never a real state — it was "Paused, but a timer ticked." In the new model, paused sessions stay in Paused with a timestamp ("5m ago") so the user can visually triage. No bouncing.

> **Breaking Behavior Change**: The new logic marks sessions as Done after **60 seconds** of inactivity without a process (previously 300s). This is intentional — with process detection as the primary signal, 60s is sufficient as a safety net for when the process exits cleanly. Sessions with a running process never transition to Done regardless of inactivity.

### Intelligent Pause Classification

When a session transitions from Working → Paused, classify **why** it paused using a two-tier system:

#### Tier 1: Structural Signals (free, instant, high confidence)

Parse JSONL content for known patterns. These are structural facts, not heuristics:

| Signal | Classification | Confidence |
|--------|---------------|------------|
| Last tool = `AskUserQuestion` | **Needs your answer** | 0.99 |
| Last tool = `ExitPlanMode` | **Plan ready for review** | 0.99 |
| Tool denied / permission prompt | **Needs permission** | 0.95 |
| Process exited + stale file | **Session ended** | 0.95 |
| Recent tools included `git commit` + `end_turn` | **Committed changes** | 0.85 |
| Single-turn Q&A (turn_count ≤ 2) + `end_turn` | **Answered your question** | 0.80 |

Covers ~70% of cases. When confidence ≥ 0.9, use directly without AI.

#### Tier 2: AI Classification (on-transition, cached, provider-agnostic)

When Tier 1 is ambiguous (confidence < 0.9 or no match), send the last 3-5 messages to the configured LLM:

**Prompt:**
```
Classify this Claude Code session's pause state.

Recent conversation:
{last_3_to_5_messages_with_roles_and_tools}

Respond in JSON:
{
  "state": "needs_input" | "task_complete" | "mid_work" | "error",
  "label": "<human-readable summary, max 50 chars>",
  "confidence": 0.0-1.0
}

Guidelines:
- "needs_input": Claude asked a question or needs user decision
- "task_complete": Claude finished the requested work successfully
- "mid_work": Work was interrupted, paused, or partially done
- "error": Something failed (build error, test failure, blocked)
```

**Cost:** ~$0.001 per classification (Haiku). With ~10-20 transitions/hour/session, this is <$0.02/hour.

**Caching:** Result stored on the session accumulator. Cleared when session transitions back to Working. Re-classified on next Working → Paused transition. No repeated calls for the same state.

### Transition Rules (Anti-Bounce)

```
Working → Paused:
  Trigger: JSONL shows stop_reason "end_turn" (Claude finished turn)
  OR: No new JSONL bytes for 30 seconds (safety net)
  Action: Run Tier 1 classification immediately, Tier 2 async if needed

Paused → Working:
  Trigger: New streaming or tool_use content detected in JSONL
  Action: Clear cached classification, move card immediately

Paused → Done:
  Trigger: Process exited AND no new writes for 60 seconds
  OR: User clicks "Mark Done" on the card
  Action: Move card, keep for 10 minutes, then remove

Done → Working:
  Trigger: New JSONL activity detected (user resumed session)
  Action: Move card back to Working
```

**Why this eliminates bouncing:**
- Current: `Working → (60s timer) → Idle → (user types) → Working → (60s) → Idle ...`
- New: `Working → (end_turn) → Paused → (new activity) → Working → (end_turn) → Paused`

Transitions are content-driven, not timer-driven. A card only moves when something *actually happened*.

### LLM Provider Abstraction Layer

**EXTEND** the existing `LlmProvider` trait in `crates/core/src/llm/` — do NOT replace it. The current trait has a `classify()` method used by the AI Fluency Score feature. We add `complete()` alongside it.

#### Extended Provider Trait

```rust
// crates/core/src/llm/provider.rs

/// Provider-agnostic LLM interface.
/// Implementations exist for Claude CLI, Anthropic API, OpenAI, Ollama, custom endpoints.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Classify a session's first prompt into the category taxonomy.
    /// Used by: AI Fluency Score (Theme 4). DO NOT REMOVE.
    async fn classify(&self, request: ClassificationRequest) -> Result<ClassificationResponse, LlmError>;

    /// Run a general-purpose completion with system + user prompt.
    /// Used by: Intelligent Session States (pause classification), future features.
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError>;

    /// Health check — can we reach this provider?
    async fn health_check(&self) -> Result<(), LlmError>;

    /// Provider display name (e.g., "Anthropic API", "Ollama")
    fn name(&self) -> &str;

    /// Model identifier (e.g., "claude-haiku-4-5-20251001", "gpt-4o-mini")
    fn model(&self) -> &str;
}

pub struct CompletionRequest {
    pub system_prompt: Option<String>,
    pub user_prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub response_format: ResponseFormat,
}

pub enum ResponseFormat {
    Text,
    Json,
}

pub struct CompletionResponse {
    pub content: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub latency_ms: u64,
}
```

The existing `ClaudeCliProvider` must be updated to implement the new `complete()` method in addition to its existing `classify()` method.

#### Provider Implementations

| Provider | Module | How it works | Phase |
|----------|--------|-------------|-------|
| `ClaudeCliProvider` | `claude_cli.rs` | Spawns `claude -p --output-format json` (already exists, add `complete()`) | **MVP** |
| `AnthropicApiProvider` | `anthropic_api.rs` | Direct HTTPS to `api.anthropic.com/v1/messages` | Phase 2 |
| `OpenAiProvider` | `openai_api.rs` | HTTPS to `api.openai.com/v1/chat/completions` | Phase 2 |
| `OllamaProvider` | `ollama.rs` | HTTP to `localhost:11434/api/generate` | Phase 2 |
| `CustomEndpointProvider` | `custom.rs` | User-configured URL, OpenAI-compatible format | Phase 2 |

Each provider implements the same `LlmProvider` trait. The session state classifier doesn't know or care which provider is active. Phase 2 providers are not required for MVP — the feature works with structural classification alone (Tier 1), falling back to `ClaudeCliProvider` for Tier 2.

#### Provider Factory

```rust
// crates/core/src/llm/factory.rs

pub fn create_provider(config: &LlmConfig) -> Result<Arc<dyn LlmProvider>, LlmError> {
    match config.provider {
        ProviderType::ClaudeCli => Ok(Arc::new(ClaudeCliProvider::new(&config.model))),
        ProviderType::AnthropicApi => Ok(Arc::new(AnthropicApiProvider::new(
            config.api_key.as_deref().ok_or(LlmError::NotAvailable("API key required"))?,
            &config.model,
        ))),
        ProviderType::OpenAi => Ok(Arc::new(OpenAiProvider::new(
            config.api_key.as_deref().ok_or(LlmError::NotAvailable("API key required"))?,
            &config.model,
            config.endpoint.as_deref(),
        ))),
        ProviderType::Ollama => Ok(Arc::new(OllamaProvider::new(
            &config.model,
            config.endpoint.as_deref().unwrap_or("http://localhost:11434"),
        ))),
        ProviderType::Custom => Ok(Arc::new(CustomEndpointProvider::new(
            config.endpoint.as_deref().ok_or(LlmError::NotAvailable("Endpoint required"))?,
            config.api_key.as_deref(),
            &config.model,
        ))),
    }
}
```

#### Configuration

```rust
// crates/core/src/llm/config.rs

pub struct LlmConfig {
    pub provider: ProviderType,
    pub model: String,
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
    pub enabled: bool,
    pub timeout_secs: u64,
}

pub enum ProviderType {
    ClaudeCli,
    AnthropicApi,
    OpenAi,
    Ollama,
    Custom,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: ProviderType::ClaudeCli,
            model: "claude-haiku-4-5-20251001".into(),
            api_key: None,
            endpoint: None,
            enabled: true,
            timeout_secs: 10,
        }
    }
}
```

Stored in SQLite settings table. Configurable via future Settings UI (`/settings`). Falls back to structural-only classification when `enabled: false` or provider unavailable.

### Session State Classifier Service

```rust
// crates/server/src/live/classifier.rs

use std::sync::Arc;
use vibe_recall_core::llm::{CompletionRequest, LlmError, LlmProvider, ResponseFormat};

#[derive(Clone)]
pub struct SessionStateClassifier {
    provider: Option<Arc<dyn LlmProvider>>,
}

pub struct SessionStateContext {
    /// Last 3-5 messages with role, content_preview, tool_names
    pub recent_messages: Vec<MessageSummary>,
    /// Stop reason of last assistant message
    pub last_stop_reason: Option<String>,
    /// Last tool name used
    pub last_tool: Option<String>,
    /// Whether Claude process is running
    pub has_running_process: bool,
    /// Seconds since last file modification
    pub seconds_since_modified: u64,
    /// Number of user turns
    pub turn_count: u32,
}

#[derive(Debug, Clone)]
pub struct MessageSummary {
    pub role: String,
    pub content_preview: String,
    pub tool_names: Vec<String>,
}

// NOTE: Using camelCase to match LiveSession's #[serde(rename_all = "camelCase")]
// so the entire API is consistent. Frontend expects camelCase everywhere.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseClassification {
    pub reason: PauseReason,
    pub label: String,          // "Asked which auth approach to use"
    pub confidence: f32,
    pub source: ClassificationSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum PauseReason {
    #[serde(rename = "needsInput")]
    NeedsInput,
    #[serde(rename = "taskComplete")]
    TaskComplete,
    #[serde(rename = "midWork")]
    MidWork,
    #[serde(rename = "error")]
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub enum ClassificationSource {
    #[serde(rename = "structural")]
    Structural,
    #[serde(rename = "ai")]
    Ai,
    #[serde(rename = "fallback")]
    Fallback,
}
```

#### Classification Logic

```rust
impl SessionStateClassifier {
    pub fn new(provider: Option<Arc<dyn LlmProvider>>) -> Self {
        Self { provider }
    }

    pub async fn classify(&self, ctx: &SessionStateContext) -> PauseClassification {
        // Tier 1: Structural signals
        if let Some(c) = self.structural_classify(ctx) {
            if c.confidence >= 0.9 {
                return c;
            }
        }

        // Tier 2: AI classification
        if let Some(provider) = &self.provider {
            match self.ai_classify(provider.as_ref(), ctx).await {
                Ok(c) => return c,
                Err(e) => {
                    tracing::warn!("AI classification failed: {e}, falling back");
                }
            }
        }

        // Fallback: basic rule
        self.fallback_classify(ctx)
    }

    pub fn structural_classify(&self, ctx: &SessionStateContext) -> Option<PauseClassification> {
        // Check last tool for explicit signals
        if let Some(tool) = &ctx.last_tool {
            match tool.as_str() {
                "AskUserQuestion" => return Some(PauseClassification {
                    reason: PauseReason::NeedsInput,
                    label: "Asked you a question".into(),
                    confidence: 0.99,
                    source: ClassificationSource::Structural,
                }),
                "ExitPlanMode" => return Some(PauseClassification {
                    reason: PauseReason::NeedsInput,
                    label: "Plan ready for review".into(),
                    confidence: 0.99,
                    source: ClassificationSource::Structural,
                }),
                _ => {}
            }
        }

        // Process exited = session ended
        if !ctx.has_running_process && ctx.seconds_since_modified > 30 {
            return Some(PauseClassification {
                reason: PauseReason::TaskComplete,
                label: "Session ended".into(),
                confidence: 0.95,
                source: ClassificationSource::Structural,
            });
        }

        // Check content patterns in last message
        if let Some(last_msg) = ctx.recent_messages.last() {
            if last_msg.role == "assistant" {
                let content = last_msg.content_preview.to_lowercase();

                // Commit/push pattern
                if last_msg.tool_names.iter().any(|t| t == "Bash")
                    && (content.contains("commit") || content.contains("pushed"))
                    && ctx.last_stop_reason.as_deref() == Some("end_turn")
                {
                    return Some(PauseClassification {
                        reason: PauseReason::TaskComplete,
                        label: "Committed changes".into(),
                        confidence: 0.85,
                        source: ClassificationSource::Structural,
                    });
                }

                // Single-turn Q&A
                if ctx.turn_count <= 2
                    && ctx.last_stop_reason.as_deref() == Some("end_turn")
                {
                    return Some(PauseClassification {
                        reason: PauseReason::TaskComplete,
                        label: "Answered your question".into(),
                        confidence: 0.80,
                        source: ClassificationSource::Structural,
                    });
                }
            }
        }

        None // Ambiguous — fall through to AI
    }

    async fn ai_classify(
        &self,
        provider: &dyn LlmProvider,
        ctx: &SessionStateContext,
    ) -> Result<PauseClassification, LlmError> {
        let messages_text = ctx.recent_messages.iter()
            .map(|m| {
                let tools = if m.tool_names.is_empty() {
                    String::new()
                } else {
                    format!(" [tools: {}]", m.tool_names.join(", "))
                };
                format!("[{}]{}: {}", m.role, tools, m.content_preview)
            })
            .collect::<Vec<_>>()
            .join("\n");

        let response = provider.complete(CompletionRequest {
            system_prompt: Some(
                "Classify this Claude Code session's pause state. \
                 Respond ONLY in JSON: \
                 {\"state\": \"needs_input\"|\"task_complete\"|\"mid_work\"|\"error\", \
                  \"label\": \"<max 50 chars>\", \
                  \"confidence\": 0.0-1.0}".into()
            ),
            user_prompt: messages_text,
            max_tokens: 100,
            temperature: 0.0,
            response_format: ResponseFormat::Json,
        }).await?;

        // Parse JSON response
        let parsed: serde_json::Value = serde_json::from_str(&response.content)
            .map_err(|e| LlmError::ParseFailed(e.to_string()))?;

        let state = parsed["state"].as_str().unwrap_or("mid_work");
        let reason = match state {
            "needs_input" => PauseReason::NeedsInput,
            "task_complete" => PauseReason::TaskComplete,
            "error" => PauseReason::Error,
            _ => PauseReason::MidWork,
        };

        Ok(PauseClassification {
            reason,
            label: parsed["label"].as_str().unwrap_or("Paused").to_string(),
            confidence: parsed["confidence"].as_f64().unwrap_or(0.7) as f32,
            source: ClassificationSource::Ai,
        })
    }

    fn fallback_classify(&self, ctx: &SessionStateContext) -> PauseClassification {
        if ctx.last_stop_reason.as_deref() == Some("end_turn") {
            PauseClassification {
                reason: PauseReason::MidWork,
                label: "Waiting for your next message".into(),
                confidence: 0.5,
                source: ClassificationSource::Fallback,
            }
        } else {
            PauseClassification {
                reason: PauseReason::MidWork,
                label: "Session paused".into(),
                confidence: 0.3,
                source: ClassificationSource::Fallback,
            }
        }
    }
}
```

### Backend Changes

#### Modified SessionStatus Enum

```rust
// crates/server/src/live/state.rs

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Agent is actively streaming or using tools.
    Working,
    /// Agent paused — reason available in pause_classification.
    Paused,
    /// Session is over (process exited).
    Done,
}
```

#### Modified LiveSession Struct

```rust
// crates/server/src/live/state.rs
// Add pause_classification field to the existing LiveSession struct.
// LiveSession uses #[serde(rename_all = "camelCase")], so this serializes as "pauseClassification".
pub struct LiveSession {
    // ... all existing fields unchanged ...
    pub status: SessionStatus,
    /// Why the session is paused (only set when status == Paused).
    /// null for Working/Done sessions, or briefly null while AI classification is in-flight.
    pub pause_classification: Option<PauseClassification>,
}
```

#### Modified SessionEvent::Summary

```rust
// crates/server/src/live/state.rs
// Update the Summary variant to match the new 3-state model.
SessionEvent::Summary {
    working_count: usize,       // was: active_count
    paused_count: usize,        // was: waiting_count + idle_count
    done_count: usize,          // NEW — sessions in Done state (kept in memory for 10min)
    total_cost_today_usd: f64,
    total_tokens_today: u64,
},
```

#### Modified SessionAccumulator

```rust
// crates/server/src/live/manager.rs
use std::collections::VecDeque;
use super::classifier::{MessageSummary, PauseClassification, SessionStateClassifier};

struct SessionAccumulator {
    // ... all existing fields unchanged ...
    /// Cached pause classification (cleared on Working transition)
    pause_classification: Option<PauseClassification>,
    /// Recent messages for classification context (ring buffer, last 5)
    recent_messages: VecDeque<MessageSummary>,
    /// Previous status for transition detection
    last_status: Option<SessionStatus>,
}

impl SessionAccumulator {
    fn new() -> Self {
        Self {
            offset: 0,
            tokens: TokenUsage::default(),
            context_window_tokens: 0,
            model: None,
            user_turn_count: 0,
            first_user_message: String::new(),
            last_user_message: String::new(),
            git_branch: None,
            started_at: None,
            last_line: None,
            completed_at: None,
            pause_classification: None,     // NEW
            recent_messages: VecDeque::new(), // NEW
            last_status: None,               // NEW
        }
    }
}
```

#### Message History Accumulation

In `process_jsonl_update()`, after extracting content from parsed lines (around line 463), add message tracking:

```rust
// Track recent messages for pause classification
for line in &new_lines {
    if line.line_type == LineType::User || line.line_type == LineType::Assistant {
        acc.recent_messages.push_back(MessageSummary {
            role: match line.line_type {
                LineType::User => "user".to_string(),
                LineType::Assistant => "assistant".to_string(),
                _ => continue,
            },
            content_preview: line.content_preview.clone(),
            tool_names: line.tool_names.clone(),
        });

        // Keep only last 5 messages
        const MAX_RECENT_MESSAGES: usize = 5;
        while acc.recent_messages.len() > MAX_RECENT_MESSAGES {
            acc.recent_messages.pop_front();
        }
    }
}
```

#### Modified derive_status

```rust
pub fn derive_status(
    last_line: Option<&LiveLine>,
    seconds_since_modified: u64,
    has_running_process: bool,
) -> SessionStatus {
    let last_line = match last_line {
        Some(ll) => ll,
        None => return SessionStatus::Paused, // No data = paused, not "idle"
    };

    // Done: process exited AND file stale >60s
    if !has_running_process && seconds_since_modified > 60 {
        return SessionStatus::Done;
    }

    // Working: active streaming or tool use (within last 30s)
    if seconds_since_modified <= 30 {
        match last_line.line_type {
            LineType::Assistant => {
                if !last_line.tool_names.is_empty() {
                    return SessionStatus::Working; // Tool use
                }
                if last_line.stop_reason.as_deref() != Some("end_turn") {
                    return SessionStatus::Working; // Still streaming
                }
                // end_turn = Claude finished → Paused
                SessionStatus::Paused
            }
            LineType::Progress => SessionStatus::Working,
            // User message just sent → momentarily Paused (will be Working once Claude responds)
            _ => SessionStatus::Paused,
        }
    } else {
        // >30s since last write — Paused (not "Idle")
        SessionStatus::Paused
    }
}
```

#### Shared Transition Detection + Classification

Both `process_jsonl_update()` and the process detector poll need to detect Working→Paused transitions and trigger classification. Extract this into a shared method on `LiveSessionManager`:

```rust
// crates/server/src/live/manager.rs

impl LiveSessionManager {
    /// Handle a status change for a session. Detects transitions, triggers
    /// classification on Working→Paused, clears classification on →Working.
    /// Called from both process_jsonl_update() and the process detector loop.
    async fn handle_status_change(
        &self,
        session_id: &str,
        new_status: SessionStatus,
        acc: &mut SessionAccumulator,
        has_running_process: bool,
        seconds_since_modified: u64,
    ) {
        let old_status = acc.last_status.clone();

        // Working → Paused: trigger classification
        if new_status == SessionStatus::Paused && old_status == Some(SessionStatus::Working) {
            let ctx = SessionStateContext {
                recent_messages: acc.recent_messages.iter().cloned().collect(),
                last_stop_reason: acc.last_line.as_ref().and_then(|l| l.stop_reason.clone()),
                last_tool: acc.last_line.as_ref().and_then(|l| l.tool_names.last().cloned()),
                has_running_process,
                seconds_since_modified,
                turn_count: acc.user_turn_count,
            };

            // Tier 1: structural classification (instant)
            let classification = self.classifier.structural_classify(&ctx);

            if let Some(c) = classification {
                if c.confidence >= 0.9 {
                    acc.pause_classification = Some(c);
                } else {
                    // Use Tier 1 result temporarily
                    acc.pause_classification = Some(c);
                    // Spawn Tier 2 async
                    self.spawn_ai_classification(session_id.to_string(), ctx);
                }
            } else {
                // No structural match — show placeholder, spawn AI
                acc.pause_classification = Some(PauseClassification {
                    reason: PauseReason::MidWork,
                    label: "Classifying...".into(),
                    confidence: 0.0,
                    source: ClassificationSource::Fallback,
                });
                self.spawn_ai_classification(session_id.to_string(), ctx);
            }
        }

        // → Working: clear classification
        if new_status == SessionStatus::Working {
            acc.pause_classification = None;
        }

        // Track Done timestamp for cleanup
        if new_status == SessionStatus::Done && acc.completed_at.is_none() {
            acc.completed_at = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            );
        }

        // Update last_status for next transition check
        acc.last_status = Some(new_status);
    }

    /// Spawn an async Tier 2 AI classification task.
    /// Re-checks session status before broadcasting to avoid stale updates.
    fn spawn_ai_classification(&self, session_id: String, ctx: SessionStateContext) {
        let classifier = self.classifier.clone();
        let sessions = Arc::clone(&self.sessions);
        let tx = self.tx.clone();

        tokio::spawn(async move {
            match classifier.classify(&ctx).await {
                classification => {
                    // Re-acquire lock and check current status before broadcasting.
                    // The session may have transitioned back to Working while AI was running.
                    let mut sessions = sessions.write().await;
                    if let Some(session) = sessions.get_mut(&session_id) {
                        if session.status == SessionStatus::Paused {
                            session.pause_classification = Some(classification);
                            let _ = tx.send(SessionEvent::SessionUpdated {
                                session: session.clone(),
                            });
                        }
                        // If status is no longer Paused, discard the classification
                    }
                }
            }
        });
    }
}
```

#### Classifier Storage in LiveSessionManager

```rust
// crates/server/src/live/manager.rs

pub struct LiveSessionManager {
    sessions: LiveSessionMap,
    tx: broadcast::Sender<SessionEvent>,
    finders: Arc<TailFinders>,
    accumulators: Arc<RwLock<HashMap<String, SessionAccumulator>>>,
    processes: Arc<RwLock<HashMap<String, u32>>>,
    pricing: Arc<HashMap<String, cost::ModelPricing>>,
    classifier: Arc<SessionStateClassifier>,  // NEW
}

// In server startup (crates/server/src/lib.rs or main.rs):
let llm_config = LlmConfig::default();
let provider = match llm::factory::create_provider(&llm_config) {
    Ok(p) => Some(p),
    Err(e) => {
        tracing::warn!("LLM provider unavailable: {e}, using structural classification only");
        None
    }
};
let classifier = Arc::new(SessionStateClassifier::new(provider));

let (manager, live_sessions, live_tx) = LiveSessionManager::start(pricing, classifier);
```

#### Updated build_summary() in routes/live.rs

```rust
// crates/server/src/routes/live.rs
fn build_summary(map: &HashMap<String, LiveSession>) -> serde_json::Value {
    let mut working_count = 0usize;
    let mut paused_count = 0usize;
    let mut done_count = 0usize;
    let mut total_cost = 0.0f64;
    let mut total_tokens = 0u64;

    for session in map.values() {
        match session.status {
            SessionStatus::Working => working_count += 1,
            SessionStatus::Paused => paused_count += 1,
            SessionStatus::Done => done_count += 1,
        }
        total_cost += session.cost.total_usd;
        total_tokens += session.tokens.total_tokens;
    }

    serde_json::json!({
        "workingCount": working_count,
        "pausedCount": paused_count,
        "doneCount": done_count,
        "totalCostTodayUsd": total_cost,
        "totalTokensToday": total_tokens,
    })
}
```

#### Updated Cleanup Task

```rust
// crates/server/src/live/manager.rs — cleanup task
// Change SessionStatus::Complete to SessionStatus::Done
for (session_id, session) in sessions.iter() {
    if session.status == SessionStatus::Done {  // was: SessionStatus::Complete
        if let Some(acc) = accumulators.get(session_id) {
            if let Some(completed_at) = acc.completed_at {
                if now.saturating_sub(completed_at) > 600 {
                    to_remove.push(session_id.clone());
                }
            }
        }
    }
}
```

#### Updated Process Detector

The process detector loop must also use `handle_status_change()`:

```rust
// crates/server/src/live/manager.rs — process detector loop (every 5s)
for (session_id, session) in sessions.iter_mut() {
    if let Some(acc) = accumulators.get_mut(session_id) {
        let seconds_since = seconds_since_modified_from_timestamp(session.last_activity_at);
        let (running, pid) = has_running_process(&processes, &session.project_path);
        let new_status = derive_status(acc.last_line.as_ref(), seconds_since, running);

        if session.status != new_status || session.pid != pid {
            // Trigger transition detection + classification
            manager.handle_status_change(
                session_id, new_status, acc, running, seconds_since,
            ).await;

            session.status = new_status;
            session.pid = pid;
            session.pause_classification = acc.pause_classification.clone();
            let _ = manager.tx.send(SessionEvent::SessionUpdated {
                session: session.clone(),
            });
        }
    }
}
```

#### derive_status Tests

```rust
// crates/server/src/live/state.rs — add to existing tests module

#[test]
fn test_new_status_paused_no_data() {
    let status = derive_status(None, 0, false);
    assert_eq!(status, SessionStatus::Paused);
}

#[test]
fn test_new_status_working_streaming_recent() {
    let last = make_live_line(LineType::Assistant, vec![], None);
    let status = derive_status(Some(&last), 10, true);
    assert_eq!(status, SessionStatus::Working);
}

#[test]
fn test_new_status_working_tool_use_recent() {
    let last = make_live_line(LineType::Assistant, vec!["Bash".to_string()], None);
    let status = derive_status(Some(&last), 25, true);
    assert_eq!(status, SessionStatus::Working);
}

#[test]
fn test_new_status_paused_end_turn_recent() {
    let last = make_live_line(LineType::Assistant, vec![], Some("end_turn"));
    let status = derive_status(Some(&last), 10, true);
    assert_eq!(status, SessionStatus::Paused);
}

#[test]
fn test_new_status_paused_at_31s() {
    let last = make_live_line(LineType::Assistant, vec![], None);
    let status = derive_status(Some(&last), 31, true);
    assert_eq!(status, SessionStatus::Paused);
}

#[test]
fn test_new_status_paused_at_60s_with_process() {
    let last = make_live_line(LineType::Assistant, vec![], Some("end_turn"));
    let status = derive_status(Some(&last), 61, true);
    assert_eq!(status, SessionStatus::Paused, "Process still running keeps it from Done");
}

#[test]
fn test_new_status_done_at_61s_no_process() {
    let last = make_live_line(LineType::Assistant, vec![], Some("end_turn"));
    let status = derive_status(Some(&last), 61, false);
    assert_eq!(status, SessionStatus::Done);
}

#[test]
fn test_new_status_working_progress_recent() {
    let last = make_live_line(LineType::Progress, vec![], None);
    let status = derive_status(Some(&last), 5, true);
    assert_eq!(status, SessionStatus::Working);
}

#[test]
fn test_new_status_paused_user_message() {
    let last = make_live_line(LineType::User, vec![], None);
    let status = derive_status(Some(&last), 5, true);
    assert_eq!(status, SessionStatus::Paused);
}
```

### Frontend Changes

#### Updated Types

```typescript
// src/types/live.ts

export type LiveDisplayStatus = 'working' | 'paused' | 'done'

export type PauseReason = 'needsInput' | 'taskComplete' | 'midWork' | 'error'

export interface PauseClassification {
  reason: PauseReason
  label: string
  confidence: number
  source: 'structural' | 'ai' | 'fallback'
}

// With 3-state backend, toDisplayStatus is nearly a passthrough.
// Kept for backward safety if unknown statuses arrive.
export function toDisplayStatus(status: string): LiveDisplayStatus {
  switch (status) {
    case 'working':
      return 'working'
    case 'done':
      return 'done'
    case 'paused':
    default:
      return 'paused'
  }
}

export const DISPLAY_STATUS_ORDER: Record<LiveDisplayStatus, number> = {
  working: 0,
  paused: 1,
  done: 2,
}
```

#### Updated LiveSession Interface

```typescript
// src/hooks/use-live-sessions.ts

import type { PauseClassification } from '../types/live'

export interface LiveSession {
  id: string
  project: string
  projectDisplayName: string
  projectPath: string
  filePath: string
  status: 'working' | 'paused' | 'done'  // was: 5-state union
  gitBranch: string | null
  pid: number | null
  title: string
  lastUserMessage: string
  currentActivity: string
  turnCount: number
  startedAt: number | null
  lastActivityAt: number
  model: string | null
  tokens: { /* unchanged */ }
  contextWindowTokens: number
  cost: { /* unchanged */ }
  cacheStatus: 'warm' | 'cold' | 'unknown'
  pauseClassification?: PauseClassification | null  // NEW
}

export interface LiveSummary {
  workingCount: number    // was: activeCount
  pausedCount: number     // was: waitingCount + idleCount
  doneCount: number       // NEW
  totalCostTodayUsd: number
  totalTokensToday: number
}
```

Add SSE payload validation for `pauseClassification`:

```typescript
// In use-live-sessions.ts, session_updated handler:
es.addEventListener('session_updated', (e: MessageEvent) => {
  try {
    const data = JSON.parse(e.data)
    const session = data.session ?? data
    if (session?.id) {
      // Validate pauseClassification if present
      if (session.pauseClassification && !session.pauseClassification.reason) {
        console.warn('[SSE] Invalid pauseClassification:', session.pauseClassification)
        session.pauseClassification = null
      }
      setSessions(prev => new Map(prev).set(session.id, session))
      setLastUpdate(new Date())
    }
  } catch { /* ignore */ }
})
```

#### Updated KanbanView

```typescript
// src/components/live/KanbanView.tsx

const COLUMNS: {
  title: string
  status: LiveDisplayStatus
  accentColor: string
  emptyMessage: string
}[] = [
  {
    title: 'Working',
    status: 'working',
    accentColor: 'bg-green-500',
    emptyMessage: 'No active sessions',
  },
  {
    title: 'Paused',
    status: 'paused',
    accentColor: 'bg-amber-500',
    emptyMessage: 'No paused sessions',
  },
  {
    title: 'Done',
    status: 'done',
    accentColor: 'bg-blue-500',
    emptyMessage: 'No completed sessions',
  },
]
```

Updated grouping with actionability-based sub-sorting for Paused column:

```typescript
function pausedSortKey(session: LiveSession): number {
  if (!session.pauseClassification) return 2
  switch (session.pauseClassification.reason) {
    case 'needsInput': return 0   // Most urgent — you need to respond
    case 'error': return 1         // Something broke — investigate
    case 'midWork': return 2       // Paused mid-task
    case 'taskComplete': return 3  // Done, just needs acknowledgment
  }
}

const grouped = useMemo(() => {
  const groups: Record<LiveDisplayStatus, LiveSession[]> = {
    working: [],
    paused: [],
    done: [],
  }
  for (const s of sessions) {
    groups[toDisplayStatus(s.status)].push(s)
  }
  for (const [status, arr] of Object.entries(groups)) {
    if (status === 'paused') {
      // Paused: sort by actionability first, then recency
      arr.sort((a, b) => {
        const keyCmp = pausedSortKey(a) - pausedSortKey(b)
        if (keyCmp !== 0) return keyCmp
        return b.lastActivityAt - a.lastActivityAt
      })
    } else {
      arr.sort((a, b) => b.lastActivityAt - a.lastActivityAt)
    }
  }
  return groups
}, [sessions])
```

#### Pause Reason Visual Treatment

Each pause reason gets a distinct visual indicator on the card:

| Reason | Badge Color | Icon | Label Example |
|--------|------------|------|---------------|
| `needsInput` | Red/Amber | `MessageCircle` | "Asked which auth approach to use" |
| `error` | Red | `AlertTriangle` | "Build failed — 3 type errors" |
| `midWork` | Gray | `Pause` | "Paused during file refactor" |
| `taskComplete` | Green | `CheckCircle` | "Committed auth feature to main" |

```tsx
// In SessionCard.tsx
import { GitBranch, MessageCircle, AlertTriangle, Pause, CheckCircle } from 'lucide-react'
import type { PauseClassification } from '../../types/live'

function PauseReasonBadge({ classification }: { classification: PauseClassification }) {
  const config = {
    needsInput: {
      color: 'bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-300',
      icon: MessageCircle,
    },
    error: {
      color: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300',
      icon: AlertTriangle,
    },
    midWork: {
      color: 'bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400',
      icon: Pause,
    },
    taskComplete: {
      color: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300',
      icon: CheckCircle,
    },
  }

  const { color, icon: Icon } = config[classification.reason]

  return (
    <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${color}`}>
      <Icon className="h-3 w-3" />
      {classification.label}
    </span>
  )
}
```

#### PauseReasonBadge Placement in SessionCard

Replace the current activity line with the pause badge when `status === 'paused'`:

```tsx
// SessionCard.tsx — replace current activity section (approx lines 88-92)
{/* Pause reason badge (when paused) OR current activity (when working) */}
{toDisplayStatus(session.status) === 'paused' ? (
  <div className="mb-2">
    {session.pauseClassification ? (
      <PauseReasonBadge classification={session.pauseClassification} />
    ) : (
      <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400">
        <Pause className="h-3 w-3" />
        Paused
      </span>
    )}
  </div>
) : session.currentActivity ? (
  <div className="text-xs text-green-600 dark:text-green-400 truncate mb-2">
    {session.currentActivity}
  </div>
) : null}
```

#### Updated STATUS_CONFIG in SessionCard

```typescript
// SessionCard.tsx
const STATUS_CONFIG = {
  working: { color: 'bg-green-500', label: 'Working', pulse: true },
  paused: { color: 'bg-amber-500', label: 'Paused', pulse: false },
  done: { color: 'bg-zinc-700', label: 'Done', pulse: false },
} as const
```

#### Paused Column Header

```tsx
// src/components/live/KanbanColumn.tsx
// Replace the column header with conditional rendering:

<div className="px-3 py-2 flex items-center justify-between">
  {status === 'paused' ? (
    <PausedColumnHeader sessions={sessions} />
  ) : (
    <>
      <span className="text-sm font-medium text-gray-700 dark:text-gray-300">{title}</span>
      <span className="text-xs text-gray-400 dark:text-gray-500">({sessions.length})</span>
    </>
  )}
</div>

function PausedColumnHeader({ sessions }: { sessions: LiveSession[] }) {
  const needsInput = sessions.filter(
    s => s.pauseClassification?.reason === 'needsInput'
  ).length

  return (
    <div className="flex items-center gap-2">
      <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Paused</span>
      <span className="text-xs bg-zinc-200 dark:bg-zinc-700 px-1.5 py-0.5 rounded-full">
        {sessions.length}
      </span>
      {needsInput > 0 && (
        <span className="text-xs bg-amber-200 dark:bg-amber-800 text-amber-800 dark:text-amber-200 px-1.5 py-0.5 rounded-full">
          {needsInput} need input
        </span>
      )}
    </div>
  )
}
```

#### Updated StatusDot

```typescript
// src/components/live/StatusDot.tsx
import type { LiveDisplayStatus } from '../../types/live'

interface StatusDotProps {
  status: LiveDisplayStatus  // was: hardcoded 4-state union
  size?: 'sm' | 'md'
  pulse?: boolean
}

const STATUS_COLORS: Record<LiveDisplayStatus, string> = {
  working: 'bg-green-500',
  paused: 'bg-amber-500',
  done: 'bg-blue-500',
}
```

#### Updated ContextGauge

```typescript
// src/components/live/ContextGauge.tsx
// Update inactive status check:
const isInactive = status === 'paused' || status === 'done'
// was: status === 'idle' || status === 'complete'
```

#### Updated LiveCommandPalette

```typescript
// src/components/live/LiveCommandPalette.tsx
// Update status filter options:
const statusFilters: { status: string; label: string }[] = [
  { status: 'working', label: 'Show working sessions' },
  { status: 'paused', label: 'Show paused sessions' },
  // removed: 'waiting' and 'idle' options
]
```

#### Updated MissionControlPage SummaryBar

```tsx
// src/pages/MissionControlPage.tsx
// Update SummaryBar to use new field names:
<div>
  <span className="text-green-500 font-medium">{summary.workingCount}</span>
  <span className="text-gray-500 dark:text-gray-400 ml-1">working</span>
</div>
<div>
  <span className="text-amber-500 font-medium">{summary.pausedCount}</span>
  <span className="text-gray-500 dark:text-gray-400 ml-1">paused</span>
</div>
```

#### Updated ListView Pause Reason Display

```tsx
// src/components/live/ListView.tsx
// In the Activity column, show pause badge when paused:
<td className="px-2 py-2">
  {toDisplayStatus(session.status) === 'paused' && session.pauseClassification ? (
    <PauseReasonBadge classification={session.pauseClassification} />
  ) : (
    <span className="text-xs text-gray-700 dark:text-gray-300 truncate block">
      {activityText}
    </span>
  )}
</td>
```

### SSE Event Changes

The `LiveSession` payload now includes `pauseClassification` (camelCase, matching the rest of the API):

```json
{
  "type": "session_updated",
  "session": {
    "id": "uuid-123",
    "status": "paused",
    "pauseClassification": {
      "reason": "needsInput",
      "label": "Asked which auth approach to use",
      "confidence": 0.99,
      "source": "structural"
    }
  }
}
```

**Null behavior:** REST endpoints (`/api/live/sessions`, `/api/live/sessions/:id`) will include `"pauseClassification": null` for Working/Done sessions. This is intentional — frontend consumers must handle null gracefully.

**Async Tier 2:** When AI classification arrives asynchronously, a second `session_updated` event is sent with the enriched classification. The async task re-checks `session.status == Paused` before broadcasting to avoid stale updates on sessions that already transitioned back to Working.

### Files Changed

| File | Change | Phase |
|------|--------|-------|
| **Backend — Core** | | |
| `crates/server/src/live/state.rs` | Replace 5-state enum with 3-state. Update `derive_status()`. Add `pause_classification` to `LiveSession`. Update `SessionEvent::Summary` fields | MVP |
| `crates/server/src/live/classifier.rs` | **New file.** `SessionStateClassifier` with Tier 1 structural + Tier 2 AI logic | MVP |
| `crates/server/src/live/manager.rs` | Add `classifier` field to manager. Add `handle_status_change()` shared method. Add message history tracking to accumulator. Update process detector + cleanup task | MVP |
| `crates/server/src/routes/live.rs` | Update `build_summary()` match for 3-state enum. New field names: `workingCount`, `pausedCount`, `doneCount` | MVP |
| **Backend — LLM** | | |
| `crates/core/src/llm/provider.rs` | **Extend** trait: add `complete()` method alongside existing `classify()`. DO NOT remove `classify()` | MVP |
| `crates/core/src/llm/types.rs` | Add `CompletionRequest`, `CompletionResponse`, `ResponseFormat` types | MVP |
| `crates/core/src/llm/claude_cli.rs` | Add `complete()` method implementation to existing `ClaudeCliProvider` | MVP |
| `crates/core/src/llm/config.rs` | Add `LlmConfig`, `ProviderType` (may already exist partially) | MVP |
| `crates/core/src/llm/factory.rs` | **New file.** Provider factory with fallback | MVP |
| `crates/core/src/llm/anthropic_api.rs` | **New file.** Direct Anthropic API provider | Phase 2 |
| `crates/core/src/llm/openai_api.rs` | **New file.** OpenAI-compatible provider | Phase 2 |
| `crates/core/src/llm/ollama.rs` | **New file.** Ollama local provider | Phase 2 |
| `crates/core/src/llm/custom.rs` | **New file.** Custom endpoint provider | Phase 2 |
| **Frontend — Types** | | |
| `src/types/live.ts` | 3-state `LiveDisplayStatus`, `PauseClassification` type, `PauseReason` type, update `toDisplayStatus()`, update `DISPLAY_STATUS_ORDER` | MVP |
| **Frontend — Hooks** | | |
| `src/hooks/use-live-sessions.ts` | Update `LiveSession.status` union (3 states). Add `pauseClassification` field. Update `LiveSummary` fields. Add SSE validation | MVP |
| `src/hooks/use-live-session-filters.ts` | Update status filter options (remove idle/waiting) | MVP |
| **Frontend — Components** | | |
| `src/components/live/KanbanView.tsx` | 3 columns. Add `pausedSortKey()`. Update grouped state keys | MVP |
| `src/components/live/SessionCard.tsx` | Add `PauseReasonBadge` component. Update `STATUS_CONFIG` to 3 states. Add badge placement logic. Add icon imports | MVP |
| `src/components/live/KanbanColumn.tsx` | Add `PausedColumnHeader` with sub-count. Conditional header rendering | MVP |
| `src/components/live/StatusDot.tsx` | Update type from hardcoded 4-state to `LiveDisplayStatus`. Update `STATUS_COLORS` map to 3 states | MVP |
| `src/components/live/ContextGauge.tsx` | Update `isInactive` check: `'paused' \| 'done'` instead of `'idle' \| 'complete'` | MVP |
| `src/components/live/ListView.tsx` | Add pause reason display in Activity column for paused sessions | MVP |
| `src/components/live/LiveCommandPalette.tsx` | Update status filter options (remove waiting/idle, add paused) | MVP |
| `src/pages/MissionControlPage.tsx` | Update SummaryBar: `workingCount`/`pausedCount` instead of `activeCount`/`waitingCount`/`idleCount` | MVP |
| `src/lib/live-filter.ts` | Update comment referencing 4 display statuses → 3 | MVP |

### Migration

This is a breaking SSE schema change. Since Mission Control has no persisted state (all in-memory), the migration is clean:

1. Deploy backend with new status enum + classifier
2. Deploy frontend with new column layout
3. All connected clients auto-reconnect and receive new schema

No database migration needed. No backward compatibility shims.

### Breaking Changes Checklist

Before declaring this done, verify:

**Backend (Rust):**
- [ ] `SessionStatus` enum has 3 variants (Working, Paused, Done)
- [ ] `derive_status()` returns the new 3 variants with correct thresholds
- [ ] All `SessionStatus::Streaming` / `ToolUse` / `WaitingForUser` / `Idle` / `Complete` references are gone
- [ ] `build_summary()` in `routes/live.rs` matches on 3 states, emits `workingCount`/`pausedCount`/`doneCount`
- [ ] Cleanup task checks `SessionStatus::Done` (not `Complete`)
- [ ] Process detector calls `handle_status_change()` for transition detection
- [ ] `LlmProvider` trait retains `classify()` AND adds `complete()`
- [ ] `PauseClassification` serializes with camelCase (matching LiveSession convention)
- [ ] Run `cargo test -p vibe-recall-server` — all new + existing tests pass

**Frontend (TypeScript):**
- [ ] `LiveDisplayStatus` has 3 values: `'working' | 'paused' | 'done'`
- [ ] `LiveSession.status` union has 3 values
- [ ] `LiveSummary` has `workingCount`, `pausedCount`, `doneCount`
- [ ] `PauseReason` uses camelCase values: `'needsInput' | 'taskComplete' | 'midWork' | 'error'`
- [ ] `KanbanView` has 3 columns, grouped state has 3 keys
- [ ] `StatusDot` color map has 3 entries
- [ ] `ContextGauge` checks `'paused' | 'done'` for inactive
- [ ] `LiveCommandPalette` filter options updated
- [ ] `MissionControlPage` SummaryBar uses new field names
- [ ] `SessionCard` handles `pauseClassification: null` gracefully
- [ ] Run `bun run typecheck` — no errors

### Future Extensions

1. **"Mark as Done" button** — manual override on paused cards
2. **Settings UI** — provider/model selection at `/settings`
3. **Classification history** — track how accurately the AI classifies over time
4. **Custom classification prompts** — power users can tweak the prompt
5. **Batch classification** — when app starts, classify all existing paused sessions

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | LlmProvider trait removed existing `classify()` method | Blocker | Changed to EXTEND trait — `complete()` added alongside `classify()` |
| 2 | PauseReason/ClassificationSource used snake_case, LiveSession uses camelCase | Warning | Changed to per-variant `#[serde(rename)]` with camelCase values |
| 3 | Process detector didn't trigger classification (only `process_jsonl_update` did) | Blocker | Extracted `handle_status_change()` shared method, called from both paths |
| 4 | `prev_status` variable didn't exist in manager | Minor | Added `last_status: Option<SessionStatus>` to `SessionAccumulator` |
| 5 | SessionAccumulator had no message history tracking | Warning | Added `recent_messages: VecDeque<MessageSummary>` with accumulation logic |
| 6 | Summary event fields not updated (waiting_count/idle_count) | Blocker | Updated to `working_count`/`paused_count`/`done_count` |
| 7 | Cleanup task still checked `SessionStatus::Complete` | Blocker | Changed to `SessionStatus::Done` |
| 8 | Async Tier 2 could broadcast stale classification | Warning | Added status re-check before broadcasting |
| 9 | No tests for new `derive_status()` behavior | Warning | Added 9 test cases covering all transitions and thresholds |
| 10 | Missing files from Files Changed table | Minor | Added StatusDot, ContextGauge, LiveCommandPalette, MissionControlPage, ListView, live-filter |
| 11 | Frontend null handling for pauseClassification | Blocker | Added null guards in SessionCard, SSE validation in hook |
| 12 | PauseReasonBadge placement in SessionCard unspecified | Warning | Added explicit placement: replaces current activity when paused |
| 13 | ListView missing pause reason display | Warning | Added pause badge in Activity column for paused sessions |
| 14 | Classifier storage/initialization not specified | Minor | Added `classifier` field to LiveSessionManager, startup code |
| 15 | Phase 2 provider files not marked as optional | Minor | Added Phase column to Files Changed table |
| 16 | Done threshold change (300s→60s) not documented | Warning | Added breaking behavior warning callout |
| 17 | KanbanColumn header integration not shown | Warning | Added conditional rendering code |
| 18 | StatusDot hardcoded 4-state type | Blocker | Updated to use `LiveDisplayStatus` import, 3-state color map |
| 19 | ContextGauge checked for `'idle'` status | Warning | Changed to `'paused' \| 'done'` |
| 20 | LiveCommandPalette had 4-state filter options | Blocker | Updated to 2 options: working, paused |
| 21 | MissionControlPage SummaryBar had waitingCount/idleCount | Blocker | Updated to workingCount/pausedCount |
| 22 | Missing icon imports in SessionCard | Minor | Added MessageCircle, AlertTriangle, Pause, CheckCircle imports |
| 23 | SSE payload validation missing | Warning | Added runtime validation in session_updated handler |
