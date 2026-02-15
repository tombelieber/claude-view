---
status: draft
date: 2026-02-15
---

# Full-Chat AI Fluency Score — Design

## Problem

The current classification system is severely limited:

1. **Only 200 characters** of the first user prompt are analyzed (`discovery.rs:695` → `truncate_preview(&preview, 200)`)
2. **Output is a 3-level category** (`code_work > bugfix > error-fix`) — no insight into _how well_ the user worked
3. **Uses Claude CLI** (15-20s per call, eats user's subscription quota)
4. **No full-chat analysis** — the system never reads the actual conversation, just a truncated first prompt

Users want to know: _"How well am I using AI? What should I do differently?"_ — not just _"What category was this session?"_

## Vision

**The SWE-bench for humans using AI.**

LLM benchmarks (MMLU, HumanEval, SWE-bench) score AI on standardized tasks. The AI Fluency Score flips this: **score humans on how effectively they wield AI**, using their real Claude Code session history as the test data.

## The 7-Dimension Scoring Framework

Each session is scored 1-10 across 7 dimensions, rolling up to an overall Fluency Score (1-100).

| # | Dimension | What It Measures | Source | Weight |
|---|-----------|-----------------|--------|--------|
| 1 | **Prompt Clarity** | Did you give clear, complete instructions? | AI analysis of user messages | 20% |
| 2 | **Task Scoping** | Right-sized chunks? Or context-overflow dumps? | Hybrid: token metrics + AI judgment | 15% |
| 3 | **Course Correction** | When AI went wrong, how fast did you redirect? | AI analysis of error→fix arcs | 15% |
| 4 | **Tool Leverage** | Used skills, MCP, features effectively? | Local: skill/tool usage patterns | 10% |
| 5 | **Iteration Efficiency** | Re-edit rate, churn cycles | Local: re-edit rate, files re-edited / files edited | 15% |
| 6 | **Outcome Quality** | Did work reach completion? Commit? Ship? | Hybrid: commits/tests + AI judgment | 15% |
| 7 | **Context Economy** | Managed the context window well? | Local: tokens used vs value delivered | 10% |

### Scoring Tiers

| Tier | Score | Meaning |
|------|-------|---------|
| Expert | 90-100 | You wield AI like an extension of your mind |
| Proficient | 70-89 | Solid habits, room to optimize |
| Developing | 50-69 | Common patterns holding you back |
| Beginner | <50 | Significant coaching opportunities |

### Why Not All AI?

3 of 7 dimensions are **computed locally for free** (Tool Leverage, Iteration Efficiency, Context Economy). 2 are **hybrid** (Task Scoping, Outcome Quality — local metrics + AI judgment). Only 2 are **purely AI-dependent** (Prompt Clarity, Course Correction).

This means:
- Local scores are instant and free for every session
- AI analysis is only triggered on user action (analyze button)
- Cost is ~$0.005-0.015 per session (only the AI portion)

## Technical Architecture

### Step 1: Conversation Digest Preprocessor

**Module:** `crates/core/src/digest.rs` (NEW)

Reads the raw JSONL file and produces a structured conversation digest that's ~90% smaller than the raw file. This runs in Rust — zero cost, instant, no API needed.

**Input:** Raw JSONL file (e.g., 200K tokens, 50MB)
**Output:** `ConversationDigest` struct (~20-30K tokens)

```rust
// crates/core/src/digest.rs

pub struct ConversationDigest {
    /// Session metadata
    pub session_id: String,
    pub duration_seconds: u64,
    pub turn_count: u32,
    pub model: Option<String>,
    pub git_branch: Option<String>,

    /// Pre-computed local metrics
    pub local_metrics: LocalMetrics,

    /// The conversation flow (stripped of tool payloads)
    pub conversation: Vec<DigestMessage>,

    /// Approximate token count of the digest
    pub digest_tokens: u64,
}

pub struct LocalMetrics {
    pub files_edited: u32,
    pub files_re_edited: u32,
    pub re_edit_rate: f32,           // files_re_edited / files_edited
    pub tools_used: HashMap<String, u32>,  // tool_name → count
    pub skills_invoked: Vec<String>,
    pub context_tokens_used: u64,
    pub context_window_max: u64,
    pub context_utilization: f32,    // used / max
    pub commit_count: u32,
    pub tests_run: bool,
    pub test_failures_initial: u32,
    pub test_failures_final: u32,
    pub error_count: u32,            // errors encountered during session
}

pub struct DigestMessage {
    pub role: DigestRole,
    pub content: String,             // Full text for user, trimmed for assistant
    pub tool_names: Vec<String>,     // Just names, no payloads
    pub tool_outcomes: Vec<String>,  // "success", "error: <brief message>"
    pub timestamp_offset_secs: u64,  // Seconds since session start
}

pub enum DigestRole {
    User,
    Assistant,
    Error,      // Captured errors (build failures, test failures, permission denials)
}
```

**Preprocessing rules:**

| JSONL Content | Action |
|--------------|--------|
| User messages | **Keep full text** (this is what we're scoring) |
| Assistant messages | Keep text, **strip thinking blocks** |
| Tool inputs | **Strip entirely** (file contents, search queries) → replace with tool name |
| Tool outputs | **Strip payloads** → keep only tool name + success/error signal |
| System messages | **Strip entirely** (boilerplate) |
| Streaming chunks | **Deduplicate** → combine into single message |
| Error signals | **Keep** (build errors, test failures, permission denials) |

**Data extraction strategy:** `digest.rs` MUST parse the raw JSONL file directly (reuse existing `crates/core/src/parser.rs` async parser). Do NOT rely on `SessionInfo` fields from the DB — several `LocalMetrics` fields are not pre-aggregated:

| LocalMetrics Field | Source | Notes |
|-------------------|--------|-------|
| `files_edited` | ✅ `SessionInfo.files_edited_count` | Available in DB |
| `files_re_edited` | ✅ `SessionInfo.reedited_files_count` | Available in DB |
| `commit_count` | ✅ `SessionInfo.commit_count` | Available in DB |
| `skills_invoked` | ✅ `SessionInfo.skills_used` | Available in DB |
| `tools_used` HashMap | ❌ Parse from JSONL | `SessionInfo.tool_counts` only has 4 hardcoded tools (edit/read/bash/write) |
| `context_tokens_used` | ⚠️ `SessionInfo.total_input_tokens + total_output_tokens` | Available but `Option<u64>` |
| `context_window_max` | ❌ Infer from model name | Not stored; use lookup table (Haiku/Sonnet/Opus → 200K) |
| `context_utilization` | ❌ Compute | `context_tokens_used / context_window_max` |
| `tests_run` | ❌ Parse from JSONL | Detect `cargo test`, `pytest`, `npm test` in Bash tool calls |
| `test_failures_initial/final` | ❌ Parse from JSONL | Parse Bash output for failure counts |
| `error_count` | ❌ Parse from JSONL | Count tool outputs with error status (NOT `api_error_count`) |

**Recommended approach:** Call `parser::parse_session(path)` to get a `ParsedSession`, then transform into `ConversationDigest` + compute `LocalMetrics` from the parsed messages. This reuses the battle-tested JSONL parser and avoids duplicate parsing logic.

**Why this works:** A 200K-token session might contain ~150K tokens of tool I/O (file contents, search results, build output) and ~30K of thinking blocks. After stripping, the conversation digest is ~20-30K tokens — easily fits any LLM provider.

### Step 2: Local Metric Scoring

**Module:** `crates/core/src/scoring.rs` (NEW)

Computes the 3 locally-scored dimensions from the digest's `LocalMetrics`:

```rust
// crates/core/src/scoring.rs

pub struct LocalScores {
    pub tool_leverage: DimensionScore,
    pub iteration_efficiency: DimensionScore,
    pub context_economy: DimensionScore,
}

pub struct DimensionScore {
    pub score: u8,          // 1-10
    pub reasoning: String,  // Human-readable explanation
}

pub fn score_local_metrics(metrics: &LocalMetrics) -> LocalScores {
    LocalScores {
        tool_leverage: score_tool_leverage(metrics),
        iteration_efficiency: score_iteration_efficiency(metrics),
        context_economy: score_context_economy(metrics),
    }
}
```

**Scoring logic (examples):**

**Tool Leverage (1-10):**
- 9-10: Used 5+ distinct tools, skills invoked, efficient tool selection
- 7-8: Good tool variety (3-4 tools), some skill usage
- 5-6: Basic tool usage (Read, Edit, Bash only)
- 3-4: Minimal tool usage, manual workarounds
- 1-2: Almost no tool usage

**Iteration Efficiency (1-10):**
- 9-10: Re-edit rate < 10%. Minimal churn.
- 7-8: Re-edit rate 10-25%. Acceptable iteration.
- 5-6: Re-edit rate 25-40%. Moderate churn.
- 3-4: Re-edit rate 40-60%. High churn — unclear specs or poor review.
- 1-2: Re-edit rate > 60%. Severe churn.

**Context Economy (1-10):**
- 9-10: Context utilization 20-60%. Efficient use of space.
- 7-8: Utilization 60-80%. Healthy but could optimize.
- 5-6: Utilization < 20% (wasted session) or 80-90% (pushing limits).
- 3-4: Utilization > 90%. Context pressure likely hurt quality.
- 1-2: Hit context limit or nearly empty session.

### Step 3: AI Analysis

**Uses the existing `LlmProvider` trait** — add a new `analyze()` method:

```rust
// crates/core/src/llm/provider.rs — extend existing trait
// IMPORTANT: Use a default impl so existing ClaudeCliProvider doesn't break.
// Current trait has: classify(), complete(), health_check(), name(), model()

#[async_trait]
pub trait LlmProvider: Send + Sync {
    // ... existing 5 methods (classify, complete, health_check, name, model) ...

    /// Analyze a full session digest and produce dimension scores + insights.
    /// Used by: AI Fluency Score (full-chat analysis).
    /// Default returns NotAvailable — only AnthropicApi/OpenAiCompat implement this.
    async fn analyze(&self, _request: AnalysisRequest) -> Result<AnalysisResponse, LlmError> {
        Err(LlmError::NotAvailable(
            format!("analyze() not implemented for provider '{}'", self.name())
        ))
    }
}
```

**New `LlmError` variants** needed in `crates/core/src/llm/types.rs` (existing variants: `SpawnFailed`, `CliError`, `ParseFailed`, `NotAvailable`, `RateLimited`, `InvalidFormat`, `Timeout`):

```rust
// Add to existing LlmError enum in types.rs:
    #[error("HTTP request failed: {0}")]
    HttpError(String),        // reqwest::Error → LlmError conversion

    #[error("Authentication failed: {0}")]
    Unauthorized(String),     // API key invalid/expired (401/403)
```

The providers convert `reqwest::Error` via `.map_err(|e| LlmError::HttpError(e.to_string()))`.

**New `ApiError` variant** needed in `crates/server/src/error.rs` (currently has: `SessionNotFound`, `ProjectNotFound`, `Parse`, `Discovery`, `Database`, `Internal`, `BadRequest`, `Conflict`):

```rust
// Add to existing ApiError enum:
    #[error("LLM provider error: {0}")]
    Llm(String),              // Manual conversion, NOT #[from] (avoids vibe_recall_core dep)

// Add to IntoResponse impl:
    ApiError::Llm(msg) => {
        tracing::error!(message = %msg, "LLM provider error");
        (StatusCode::SERVICE_UNAVAILABLE, ErrorResponse::new("LLM provider error"))
    }
```

**New types** in `crates/core/src/llm/types.rs`:

```rust
pub struct AnalysisRequest {
    pub session_id: String,
    pub digest_text: String,       // Rendered conversation digest
    pub local_metrics_text: String, // Pre-computed metrics for context
    pub max_tokens: u32,           // For response (default 1000)
    pub temperature: f32,          // 0.0 for consistency
}

// CRITICAL: The LLM scoring prompt returns camelCase JSON keys
// (promptClarity, taskScoping, etc.). This derive ensures correct
// deserialization from camelCase → snake_case struct fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResponse {
    pub prompt_clarity: AiDimensionScore,
    pub task_scoping: AiDimensionScore,
    pub course_correction: AiDimensionScore,
    pub outcome_quality: AiDimensionScore,
    pub summary: String,             // 2-3 sentence summary
    pub session_narrative: String,   // Paragraph-length flow description
    pub coaching_tips: Vec<String>,  // Actionable coaching tips
    // NOTE: input_tokens, output_tokens, latency_ms are NOT in the LLM response.
    // They are populated by the provider implementation after the API call returns.
    #[serde(skip)]
    pub input_tokens: Option<u64>,
    #[serde(skip)]
    pub output_tokens: Option<u64>,
    #[serde(skip)]
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDimensionScore {
    pub score: u8,       // 1-10
    pub reasoning: String,
}
```

**The scoring prompt (system prompt sent to LLM):**

```text
You are an AI Fluency Coach analyzing a Claude Code session.

Score the human operator on these dimensions. Each score is 1-10.

## Scoring Rubric

### Prompt Clarity (1-10)
- 9-10: First prompt fully specifies intent, constraints, and success criteria.
        Zero clarification needed. Example: "Add JWT auth to /api/users using
        RS256, store refresh tokens in httpOnly cookies, add tests."
- 7-8:  Clear intent with minor ambiguity. One clarification round at most.
- 5-6:  Vague direction requiring 2-3 rounds to pin down requirements.
- 3-4:  Unclear or contradictory instructions causing wasted AI turns.
- 1-2:  No meaningful direction given; AI had to guess entirely.

### Task Scoping (1-10)
- 9-10: Task fits comfortably in one session. Single clear objective.
- 7-8:  Good scope, maybe slightly ambitious but manageable.
- 5-6:  Task is large, required pivots or context pressure.
- 3-4:  Overloaded with too many objectives, context exhausted.
- 1-2:  Massive dump with no prioritization, AI couldn't make progress.

### Course Correction (1-10)
- 9-10: Caught AI errors instantly, gave precise redirections.
        Or: AI made no errors (score 10 — no correction needed is perfect).
- 7-8:  Corrected errors within 1-2 turns with clear feedback.
- 5-6:  Noticed problems but gave vague corrections ("that's wrong").
- 3-4:  Let AI go down wrong path for many turns before correcting.
- 1-2:  Never corrected, or corrections made things worse.

### Outcome Quality (1-10)
(You have local metrics as context. Factor them in alongside the conversation.)
- 9-10: Task completed successfully. Code committed. Tests passing.
- 7-8:  Task mostly complete. Minor items remaining.
- 5-6:  Partial completion. Some objectives met, others deferred.
- 3-4:  Minimal progress. Most objectives unmet.
- 1-2:  Session abandoned or ended in error state.

## Instructions

- Score based ONLY on what you observe in the conversation
- Provide specific reasoning with examples from the conversation
- Coaching tips should be actionable and specific (not generic advice)
- Summary should describe what happened, not judge quality
- If the session is very short (1-2 turns), score generously on dimensions
  that don't apply (e.g., Course Correction = 10 for a single-turn Q&A)

Respond ONLY in JSON:
{
  "promptClarity": { "score": 8, "reasoning": "Clear initial prompt..." },
  "taskScoping": { "score": 7, "reasoning": "..." },
  "courseCorrection": { "score": 9, "reasoning": "..." },
  "outcomeQuality": { "score": 8, "reasoning": "..." },
  "summary": "Refactored auth module...",
  "sessionNarrative": "The session began with...",
  "coachingTips": ["Consider specifying test expectations upfront...", "..."]
}
```

### Step 4: Merge & Store

**Module:** `crates/core/src/analysis.rs` (NEW)

Combines local scores + AI scores into the final fluency score:

```rust
pub struct FluencyAnalysis {
    pub session_id: String,
    // AI-scored (from LLM)
    pub prompt_clarity: DimensionScore,
    pub task_scoping: DimensionScore,
    pub course_correction: DimensionScore,
    pub outcome_quality: DimensionScore,
    // Local-scored (from Rust)
    pub tool_leverage: DimensionScore,
    pub iteration_efficiency: DimensionScore,
    pub context_economy: DimensionScore,
    // Overall
    pub fluency_score: u8,  // 1-100, weighted average
    // Rich text
    pub summary: String,
    pub session_narrative: String,
    pub coaching_tips: Vec<String>,
    // Metadata
    pub provider: String,
    pub model: String,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost_cents: Option<f64>,
}

// IMPORTANT: Array order MUST match FluencyAnalysis struct field order above.
// Struct order: prompt_clarity[0], task_scoping[1], course_correction[2],
//               outcome_quality[3], tool_leverage[4], iteration_efficiency[5],
//               context_economy[6]
const WEIGHTS: [f32; 7] = [
    0.20,  // [0] Prompt Clarity
    0.15,  // [1] Task Scoping
    0.15,  // [2] Course Correction
    0.15,  // [3] Outcome Quality    ← was incorrectly 0.10 (Tool Leverage)
    0.10,  // [4] Tool Leverage      ← was incorrectly 0.15 (Iteration Eff.)
    0.15,  // [5] Iteration Efficiency
    0.10,  // [6] Context Economy
];  // Sum = 1.00 ✓

pub fn compute_fluency_score(scores: &[u8; 7]) -> u8 {
    let weighted: f32 = scores.iter()
        .zip(WEIGHTS.iter())
        .map(|(&score, &weight)| score as f32 * weight)
        .sum();
    // Scale from 1-10 → 1-100
    (weighted * 10.0).round().clamp(1.0, 100.0) as u8
}
```

## Multi-Provider API Architecture

Two new provider implementations from day 1, both implementing the existing `LlmProvider` trait.

### Provider 1: Anthropic API Direct

**Module:** `crates/core/src/llm/anthropic_api.rs` (NEW)

```rust
pub struct AnthropicApiProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    timeout_secs: u64,
}
```

- HTTPS POST to `https://api.anthropic.com/v1/messages`
- Headers: `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`
- Model: `claude-haiku-4-5-20251001` (default) or user-selected
- Auth: BYO API key from Settings UI
- Cost: ~$0.005-0.015 per session analysis (20-30K token digest)
- Latency: ~1-3s (vs 15-20s for CLI)

### Provider 2: OpenAI-Compatible

**Module:** `crates/core/src/llm/openai_compat.rs` (NEW)

```rust
pub struct OpenAiCompatProvider {
    client: reqwest::Client,
    base_url: String,     // Default: "https://api.openai.com/v1"
    api_key: Option<String>,
    model: String,
    timeout_secs: u64,
}
```

One implementation covers ALL OpenAI-compatible providers:

| Service | base_url | API Key | Model |
|---------|----------|---------|-------|
| OpenAI | `https://api.openai.com/v1` | Required | `gpt-4o-mini` |
| DeepSeek | `https://api.deepseek.com/v1` | Required | `deepseek-chat` |
| Mistral | `https://api.mistral.ai/v1` | Required | `mistral-small-latest` |
| Groq | `https://api.groq.com/openai/v1` | Required | `llama-3.1-70b-versatile` |
| Together | `https://api.together.xyz/v1` | Required | `meta-llama/Llama-3-70b-chat-hf` |
| Ollama (local) | `http://localhost:11434/v1` | None | `llama3.1` |
| Custom | User-specified | Optional | User-specified |

All use the same request/response format:
```json
POST {base_url}/chat/completions
{
  "model": "...",
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."}
  ],
  "temperature": 0.0,
  "max_tokens": 1000,
  "response_format": {"type": "json_object"}
}
```

### Updated Factory

```rust
// crates/core/src/llm/factory.rs
// NOTE: ProviderType already has: ClaudeCli, AnthropicApi, OpenAi, Ollama, Custom
// OpenAi/Ollama/Custom all use the same OpenAiCompatProvider (same HTTP format).

pub fn create_provider(config: &LlmConfig) -> Result<Arc<dyn LlmProvider>, LlmError> {
    match config.provider {
        ProviderType::ClaudeCli => Ok(Arc::new(ClaudeCliProvider::new(&config.model))),
        ProviderType::AnthropicApi => {
            let key = config.api_key.as_deref()
                .ok_or_else(|| LlmError::NotAvailable("Anthropic API key required".into()))?;
            Ok(Arc::new(AnthropicApiProvider::new(key, &config.model)))
        }
        // OpenAi, Ollama, and Custom all use the same OpenAI-compatible HTTP format.
        // Ollama defaults to localhost:11434, Custom uses user-provided endpoint.
        ProviderType::OpenAi | ProviderType::Ollama | ProviderType::Custom => {
            let default_url = match config.provider {
                ProviderType::Ollama => "http://localhost:11434/v1",
                _ => "https://api.openai.com/v1",
            };
            let base_url = config.endpoint.as_deref().unwrap_or(default_url);
            Ok(Arc::new(OpenAiCompatProvider::new(
                base_url,
                config.api_key.as_deref(),
                &config.model,
            )))
        }
    }
}
```

### New Dependency

Add `reqwest` to **workspace** deps first (root `Cargo.toml`), then reference in core:

```toml
# Root Cargo.toml — add to [workspace.dependencies]
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
```

```toml
# crates/core/Cargo.toml — reference workspace dep
[dependencies]
reqwest = { workspace = true }
```

Use `rustls-tls` (not `native-tls`) — no OpenSSL dependency, simpler cross-compilation.

### Provider Configuration

**MVP: Environment variables** (no `settings` table exists yet — defer to Settings UI phase):

```bash
# Required for API providers:
export LLM_PROVIDER=anthropic_api   # or: openai, ollama, custom, claude_cli (default)
export LLM_MODEL=claude-haiku-4-5-20251001
export LLM_API_KEY=sk-ant-...       # BYO key
export LLM_BASE_URL=                # optional, for custom endpoints
export LLM_TIMEOUT_SECS=30          # optional, default 30
```

```rust
// crates/core/src/llm/config.rs — add from_env() constructor
impl LlmConfig {
    pub fn from_env() -> Self {
        let provider = match std::env::var("LLM_PROVIDER").as_deref() {
            Ok("anthropic_api") => ProviderType::AnthropicApi,
            Ok("openai") => ProviderType::OpenAi,
            Ok("ollama") => ProviderType::Ollama,
            Ok("custom") => ProviderType::Custom,
            _ => ProviderType::ClaudeCli,
        };
        Self {
            provider,
            model: std::env::var("LLM_MODEL")
                .unwrap_or_else(|_| "claude-haiku-4-5-20251001".into()),
            api_key: std::env::var("LLM_API_KEY").ok(),
            endpoint: std::env::var("LLM_BASE_URL").ok(),
            enabled: true,
            timeout_secs: std::env::var("LLM_TIMEOUT_SECS")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(30),
        }
    }
}
```

**Future:** Settings UI phase creates a `settings` table and reads config from there (env vars as override).

**Fallback chain:** If configured provider fails health check → fall back to Claude CLI → fall back to local-only scoring (no AI dimensions).

## Database Schema

### New Table: `session_analysis`

```sql
CREATE TABLE IF NOT EXISTS session_analysis (
    session_id            TEXT PRIMARY KEY REFERENCES sessions(id),
    -- AI-scored dimensions (from LLM)
    prompt_clarity        INTEGER NOT NULL CHECK (prompt_clarity BETWEEN 1 AND 10),
    task_scoping          INTEGER NOT NULL CHECK (task_scoping BETWEEN 1 AND 10),
    course_correction     INTEGER NOT NULL CHECK (course_correction BETWEEN 1 AND 10),
    outcome_quality       INTEGER NOT NULL CHECK (outcome_quality BETWEEN 1 AND 10),
    -- Local-scored dimensions (computed in Rust)
    tool_leverage         INTEGER NOT NULL CHECK (tool_leverage BETWEEN 1 AND 10),
    iteration_efficiency  INTEGER NOT NULL CHECK (iteration_efficiency BETWEEN 1 AND 10),
    context_economy       INTEGER NOT NULL CHECK (context_economy BETWEEN 1 AND 10),
    -- Overall
    fluency_score         INTEGER NOT NULL CHECK (fluency_score BETWEEN 1 AND 100),
    -- Rich text
    summary               TEXT NOT NULL,
    session_narrative     TEXT,
    coaching_tips         TEXT,           -- JSON array: ["tip1", "tip2"]
    reasoning             TEXT,           -- JSON object: per-dimension reasoning
    -- Metadata
    provider              TEXT NOT NULL,
    model                 TEXT NOT NULL,
    input_tokens          INTEGER,
    output_tokens         INTEGER,
    cost_cents            REAL,
    analyzed_at           TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_session_analysis_score ON session_analysis(fluency_score);
CREATE INDEX idx_session_analysis_date ON session_analysis(analyzed_at);
```

### Relationship to Existing Classification

The existing `sessions.category_l1/l2/l3` columns (Theme 4 classification) remain unchanged. `session_analysis` is a separate, richer analysis that includes the category as part of the AI analysis context but produces a completely different output (7-dimension scores vs 3-level category).

**Migration path:** When a session is analyzed (full fluency score), also update `sessions.category_l1/l2/l3` from the AI response — so the category becomes a free byproduct of the richer analysis. This means: no need to run classification separately once analysis is available.

## API Endpoints

### POST /api/analyze/single/:session_id

Analyze a single session. Returns the full fluency analysis.

```json
// Response
{
  "sessionId": "uuid-123",
  "fluencyScore": 78,
  "dimensions": {
    "promptClarity": { "score": 8, "reasoning": "Clear initial prompt..." },
    "taskScoping": { "score": 7, "reasoning": "..." },
    "courseCorrection": { "score": 9, "reasoning": "..." },
    "outcomeQuality": { "score": 8, "reasoning": "..." },
    "toolLeverage": { "score": 6, "reasoning": "..." },
    "iterationEfficiency": { "score": 7, "reasoning": "..." },
    "contextEconomy": { "score": 8, "reasoning": "..." }
  },
  "summary": "Refactored auth module, encountered 3 test failures, resolved all, committed.",
  "sessionNarrative": "The session began with a clear request to...",
  "coachingTips": [
    "Consider specifying test expectations upfront to reduce iteration cycles",
    "You caught the middleware bug quickly — good course correction"
  ],
  "wasCached": false,
  "provider": "anthropic_api",
  "model": "claude-haiku-4-5-20251001"
}
```

### POST /api/analyze (batch)

Analyze multiple sessions. Returns job ID, progress via SSE (reuse existing classify stream pattern).

### GET /api/analyze/status

Get analysis job status (same pattern as classify/status).

### GET /api/analyze/stream

SSE stream of analysis progress (same pattern as classify/stream).

### GET /api/analyze/summary

Aggregate fluency stats across all analyzed sessions:

```json
{
  "totalAnalyzed": 342,
  "averageFluencyScore": 72,
  "scoreDistribution": { "expert": 12, "proficient": 180, "developing": 130, "beginner": 20 },
  "dimensionAverages": {
    "promptClarity": 7.2,
    "taskScoping": 6.8,
    "courseCorrection": 8.1,
    "outcomeQuality": 7.4,
    "toolLeverage": 5.9,
    "iterationEfficiency": 6.5,
    "contextEconomy": 7.8
  },
  "topCoachingThemes": [
    { "theme": "Prompt specificity", "frequency": 89, "tip": "Include success criteria in your first message" },
    { "theme": "Tool discovery", "tip": "Use skills like /commit instead of manual git commands" }
  ],
  "trend": {
    "last7Days": 74,
    "last30Days": 71,
    "allTime": 72
  }
}
```

## Pipeline Flow

```
User clicks "Analyze" (single or batch)
         │
         ▼
┌─ Step 1: Read JSONL (instant, free) ────────────┐
│  Read full JSONL file from disk                  │
│  ~/.claude/projects/{hash}/{session_id}.jsonl    │
└──────────────────────────────────────────────────┘
         │
         ▼
┌─ Step 2: Build Digest (instant, free) ──────────┐
│  JSONL → ConversationDigest                      │
│  Strip tool payloads, thinking blocks, system    │
│  Keep user messages (full), assistant text,      │
│  tool names, error signals                       │
│  ~200K tokens → ~20-30K tokens                   │
└──────────────────────────────────────────────────┘
         │
         ▼
┌─ Step 3: Local Scoring (instant, free) ─────────┐
│  Compute from digest.local_metrics:              │
│  → tool_leverage (1-10)                          │
│  → iteration_efficiency (1-10)                   │
│  → context_economy (1-10)                        │
└──────────────────────────────────────────────────┘
         │
         ▼
┌─ Step 4: AI Analysis (1-3s, ~$0.01/session) ────┐
│  Render digest to text + scoring rubric          │
│  Send to configured LLM provider                 │
│  → prompt_clarity (1-10 + reasoning)             │
│  → task_scoping (1-10 + reasoning)               │
│  → course_correction (1-10 + reasoning)          │
│  → outcome_quality (1-10 + reasoning)            │
│  → summary, narrative, coaching_tips             │
└──────────────────────────────────────────────────┘
         │
         ▼
┌─ Step 5: Merge & Store ─────────────────────────┐
│  Combine local + AI scores                       │
│  Compute weighted fluency_score (1-100)          │
│  INSERT INTO session_analysis                    │
│  Also update sessions.category_l1/l2/l3         │
└──────────────────────────────────────────────────┘
```

**Batch mode:** Semaphore-bounded to 5 concurrent API calls. Progress via SSE (reuse `classify/stream` pattern). Estimated throughput: ~100 sessions/minute with Anthropic API.

## Cost Estimates

Based on research data (3,060 sessions, 168.5M raw tokens):

| Provider | Cost / session | Total (3,060) | Speed |
|----------|---------------|---------------|-------|
| Claude Haiku 4.5 (Anthropic API) | ~$0.010 | ~$30 | 1-3s |
| GPT-4o-mini (OpenAI) | ~$0.008 | ~$24 | 1-2s |
| DeepSeek V3 | ~$0.004 | ~$12 | 2-4s |
| Gemini 2.0 Flash | ~$0.004 | ~$12 | 1-2s |
| Ollama (local) | $0 | $0 | 5-15s |

Costs are for the ~20-30K token digest (not the raw 55K average), so significantly cheaper than sending full JSONL.

## Files Changed

| File | Change | Status |
|------|--------|--------|
| **New Rust modules** | | |
| `crates/core/src/digest.rs` | Conversation digest preprocessor | NEW |
| `crates/core/src/scoring.rs` | Local metric scoring (3 dimensions) | NEW |
| `crates/core/src/analysis.rs` | Analysis pipeline orchestrator | NEW |
| `crates/core/src/llm/anthropic_api.rs` | Anthropic API provider | NEW |
| `crates/core/src/llm/openai_compat.rs` | OpenAI-compatible provider | NEW |
| **Modified Rust modules** | | |
| `crates/core/src/llm/provider.rs` | Add `analyze()` method to trait | MODIFY |
| `crates/core/src/llm/types.rs` | Add `AnalysisRequest`, `AnalysisResponse`, `AiDimensionScore` | MODIFY |
| `crates/core/src/llm/claude_cli.rs` | No changes needed (trait default impl handles `analyze()`) | UNCHANGED |
| `crates/core/src/llm/config.rs` | Add `from_env()` to `LlmConfig` (ProviderType already has all needed variants) | MODIFY |
| `crates/core/src/llm/factory.rs` | Add `AnthropicApi` + `OpenAi`/`Ollama`/`Custom` creation | MODIFY |
| `crates/core/src/llm/mod.rs` | Export new modules and types | MODIFY |
| `crates/core/src/lib.rs` | Export `digest`, `scoring`, `analysis` modules | MODIFY |
| `crates/core/Cargo.toml` | Add `reqwest` dependency | MODIFY |
| **Server routes** | | |
| `crates/server/src/routes/analyze.rs` | Analysis API endpoints | NEW |
| `crates/server/src/routes/mod.rs` | Mount analyze routes | MODIFY |
| `crates/server/src/error.rs` | Add `Llm(String)` variant to `ApiError` | MODIFY |
| **Database** | | |
| `crates/db/src/migrations.rs` | Add `session_analysis` table as next migration (after `fluency_scores`) | MODIFY |
| `crates/db/src/queries/session_analysis.rs` | Analysis CRUD queries (NOT `analysis.rs` — avoids confusion with `fluency.rs`) | NEW |
| **Frontend (future, not in this plan)** | | |
| Settings UI for provider configuration | — | DEFERRED |
| Fluency Score dashboard | — | DEFERRED |
| Per-session analysis view | — | DEFERRED |

## What NOT to Build Yet

- **No Settings UI** — configure via env vars for MVP (`LLM_PROVIDER`, `LLM_API_KEY`, etc.), UI comes later
- **No `settings` table** — no such table exists; defer to Settings UI phase
- **No dashboard** — build the scoring pipeline first, visualize later
- **No auto-analyze** — user-triggered only (button click)
- **No free trial metering** — just BYO key for now
- **No Gemini provider** — Anthropic + OpenAI-compat covers 95% of providers
- **No streaming analysis responses** — batch is fine, SSE for progress only

## Implementation Order

**Phase 0: Foundation (must be done first)**
0a. Add `LlmError::HttpError` + `LlmError::Unauthorized` variants to `types.rs`
0b. Add default `analyze()` method to `LlmProvider` trait in `provider.rs`
0c. Add `AnalysisRequest`, `AnalysisResponse`, `AiDimensionScore` types to `types.rs`
0d. Add `reqwest` to workspace deps (`Cargo.toml`) and core deps
0e. Add `LlmConfig::from_env()` to `config.rs`
0f. Add `ApiError::Llm` variant to `crates/server/src/error.rs`
0g. Add `session_analysis` table migration to `crates/db/src/migrations.rs`

**Phase 1: Core pipeline**
1. **Digest preprocessor** (`digest.rs`) — this is the foundation, everything depends on it
2. **Local scoring** (`scoring.rs`) — free, instant, can ship without any API integration
3. **Anthropic API provider** (`anthropic_api.rs`) — first real API provider
4. **OpenAI-compatible provider** (`openai_compat.rs`) — covers the long tail

**Phase 2: Integration**
5. **Analysis pipeline** (`analysis.rs`) — orchestrates digest → score → store
6. **DB queries** (`queries/session_analysis.rs`) — CRUD for session_analysis table
7. **API endpoints** (`routes/analyze.rs`) — expose to frontend, wire into router

## Naming Conflict: Existing FluencyScore vs New FluencyAnalysis

The codebase already has a `FluencyScore` system (`crates/core/src/fluency_score.rs` + `crates/db/src/queries/fluency.rs` + `fluency_scores` table). This is a **different concept** — a weekly aggregate score computed from session facets (achievement_rate, friction_rate, etc.).

The new system introduced by this plan is `FluencyAnalysis` — a **per-session** 7-dimension analysis. The two are complementary:

| | Existing `FluencyScore` | New `FluencyAnalysis` |
|---|---|---|
| Scope | Weekly aggregate | Per-session |
| Input | Session facets from DB | Raw JSONL conversation |
| AI needed? | No (pure math) | Yes (4 of 7 dimensions) |
| Table | `fluency_scores` | `session_analysis` |
| Module | `fluency_score.rs` | `analysis.rs` |

**Do NOT rename or merge these.** They serve different purposes and both remain.

## AppState Strategy

The analysis endpoints use **on-demand provider creation** (matching the existing `classify.rs` pattern). No changes to `AppState` needed:

```rust
// In routes/analyze.rs handler:
let config = LlmConfig::from_env();
let provider = create_provider(&config)?;
let response = provider.analyze(request).await
    .map_err(|e| ApiError::Llm(e.to_string()))?;
```

This avoids adding `Arc<dyn LlmProvider>` to `AppState` and the associated lifecycle/refresh complexity. Defer cached providers to a future optimization if latency is a concern.

## Dependencies on Existing Work

- **Phase B2 (Intelligent Session States)** is in-flight on this branch with 30 uncommitted files. This plan builds on the same `LlmProvider` trait extensions. Recommend: commit Phase B2 first, then start this work.
- **The `complete()` method** added in Phase B2 remains for pause classification. The new `analyze()` method is separate.
- **`reqwest` dependency** is new for `crates/core` — currently only uses `tokio` for async.

## Open Questions

1. **Should the scoring rubric be user-customizable?** Power users might want different weights or criteria. For now: hardcode, add customization later.
2. **How to handle sessions with no JSONL file on disk?** Some old sessions may have been cleaned up. Skip with a warning.
3. **Should analysis results be re-computable?** If we improve the rubric, users may want to re-analyze. Yes — `REPLACE INTO session_analysis` allows re-analysis.

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `analyze()` on `LlmProvider` trait breaks existing `ClaudeCliProvider` | Blocker | Changed to default impl returning `NotAvailable` — no change needed in `claude_cli.rs` |
| 2 | `AnalysisResponse` serde mismatch — LLM returns camelCase, struct uses snake_case | Blocker | Added `#[serde(rename_all = "camelCase")]` and `#[serde(skip)]` for non-LLM fields |
| 3 | Missing `LlmError::HttpError` + `Unauthorized` variants for HTTP providers | Blocker | Added both new variants to types.rs section |
| 4 | Missing `From<LlmError>` for `ApiError` — routes can't use `?` operator | Blocker | Added `ApiError::Llm(String)` variant and `IntoResponse` mapping |
| 5 | `ProviderType::OpenAiCompat` doesn't exist — codebase has `OpenAi`/`Ollama`/`Custom` | Blocker | Rewrote factory to use existing enum variants with `OpenAi \| Ollama \| Custom` match arm |
| 6 | No `settings` table — plan referenced non-existent table | Blocker | Changed to env vars for MVP (`LLM_PROVIDER`, `LLM_API_KEY`, etc.) with `from_env()` constructor |
| 7 | `LocalMetrics` fields not extractable from `SessionInfo` alone | Blocker | Added data extraction table showing which fields come from DB vs JSONL parsing |
| 8 | WEIGHTS array indices didn't match `FluencyAnalysis` struct field order | Blocker | Fixed: outcome_quality[3]=0.15, tool_leverage[4]=0.10. Simplified to `[f32; 7]` |
| 9 | Existing `FluencyScore` naming conflict with new `FluencyAnalysis` | Warning | Added "Naming Conflict" section documenting both systems are different and complementary |
| 10 | `reqwest` not in workspace deps | Warning | Added two-step: workspace `Cargo.toml` then `crates/core/Cargo.toml` |
| 11 | `queries/fluency.rs` already exists — potential overlap | Warning | Renamed new file to `queries/session_analysis.rs` to avoid confusion |
| 12 | AppState provider strategy undefined | Warning | Added "AppState Strategy" section — on-demand creation matching existing classify pattern |
| 13 | Migration number not specified | Warning | Clarified: "next migration after `fluency_scores`" |
| 14 | Files Changed table had stale entries | Minor | Updated: `claude_cli.rs` → UNCHANGED, `config.rs` description fixed, added `error.rs` |
| 15 | Implementation order missing foundation steps | Minor | Added Phase 0 (7 foundation tasks) before Phase 1 (core pipeline) |
