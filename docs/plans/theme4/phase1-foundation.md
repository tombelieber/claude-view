---
status: done
date: 2026-02-05
purpose: Theme 4 Phase 1 — Foundation for Chat Insights & Pattern Discovery
---

# Phase 1: Foundation

> **Goal:** Establish database schema, Claude CLI integration, and background job infrastructure for Theme 4.
>
> **Estimated effort:** 2-3 days
>
> **Dependencies:** None (first phase)

---

## Overview

Phase 1 creates the foundational infrastructure that all other Theme 4 phases depend on:

1. **Database schema changes** — New columns on `sessions` for classification + new tables for tracking jobs
2. **Claude CLI integration** — Spawn `claude` CLI process, parse JSON output, handle errors
3. **Background job runner** — Async job execution with progress tracking, SSE streaming, persistence

This phase produces no user-visible UI changes, but enables all downstream features.

---

## Tasks

### 1.1 Add Session Classification Columns

Add columns to the `sessions` table for storing LLM classification results.

**SQL (Migration 13):**

```sql
-- 13a: Classification hierarchy
ALTER TABLE sessions ADD COLUMN category_l1 TEXT;           -- 'code_work', 'support_work', 'thinking_work'
ALTER TABLE sessions ADD COLUMN category_l2 TEXT;           -- 'feature', 'bug_fix', 'refactor', etc.
ALTER TABLE sessions ADD COLUMN category_l3 TEXT;           -- 'new-component', 'error-fix', etc.
ALTER TABLE sessions ADD COLUMN category_confidence REAL;   -- 0.0-1.0
ALTER TABLE sessions ADD COLUMN category_source TEXT;       -- 'llm:claude-haiku', 'llm:gpt-4o-mini'
ALTER TABLE sessions ADD COLUMN classified_at TEXT;         -- ISO timestamp (RFC 3339)

-- 13b: Behavioral metrics for pattern detection
ALTER TABLE sessions ADD COLUMN prompt_word_count INTEGER;  -- Word count of first user prompt
ALTER TABLE sessions ADD COLUMN correction_count INTEGER DEFAULT 0;   -- Count of "fix", "no", "wrong" follow-ups
ALTER TABLE sessions ADD COLUMN same_file_edit_count INTEGER DEFAULT 0; -- Re-edits on same file in session
```

**Index for filtering by classification:**

```sql
CREATE INDEX IF NOT EXISTS idx_sessions_category_l1 ON sessions(category_l1) WHERE category_l1 IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_sessions_classified ON sessions(classified_at) WHERE classified_at IS NOT NULL;
```

**Subtasks:**
- [ ] Add migration statements to `crates/db/src/migrations.rs`
- [ ] Update `SessionInfo` struct in `crates/core/src/types.rs`
- [ ] Update `get_session_by_id` and `get_sessions_for_project` queries to include new columns
- [ ] Write migration test in `crates/db/src/migrations.rs`

**Files to modify:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/db/src/migrations.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/core/src/types.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/db/src/queries.rs`

---

### 1.2 Add classification_jobs Table

Track background classification job state, allowing jobs to survive server restarts.

**SQL (Migration 13c):**

```sql
CREATE TABLE IF NOT EXISTS classification_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,               -- ISO timestamp (RFC 3339)
    completed_at TEXT,                       -- ISO timestamp, NULL if running
    total_sessions INTEGER NOT NULL,         -- Total sessions to classify
    classified_count INTEGER DEFAULT 0,      -- Successfully classified
    skipped_count INTEGER DEFAULT 0,         -- Already classified (skip)
    failed_count INTEGER DEFAULT 0,          -- Classification failed
    provider TEXT NOT NULL,                  -- 'claude-cli', 'anthropic-api', 'openai-compatible'
    model TEXT NOT NULL,                     -- 'claude-haiku-4', 'gpt-4o-mini'
    status TEXT DEFAULT 'running',           -- 'running', 'completed', 'cancelled', 'failed'
    error_message TEXT,                      -- Error details if status='failed'
    cost_estimate_cents INTEGER,             -- Pre-calculated cost estimate
    actual_cost_cents INTEGER,               -- Actual cost (if available)
    tokens_used INTEGER,                     -- Total tokens consumed
    CONSTRAINT valid_status CHECK (status IN ('running', 'completed', 'cancelled', 'failed'))
);

CREATE INDEX IF NOT EXISTS idx_classification_jobs_status ON classification_jobs(status);
CREATE INDEX IF NOT EXISTS idx_classification_jobs_started ON classification_jobs(started_at DESC);
```

**Subtasks:**
- [ ] Add migration statements to `crates/db/src/migrations.rs`
- [ ] Create `ClassificationJob` struct in `crates/core/src/types.rs`
- [ ] Create `ClassificationJobStatus` enum
- [ ] Add CRUD functions in `crates/db/src/queries.rs`:
  - `create_classification_job()`
  - `get_active_classification_job()` — returns running job if exists
  - `update_classification_job_progress()`
  - `complete_classification_job()`
  - `cancel_classification_job()`
  - `get_recent_classification_jobs()` — last 10 jobs
- [ ] Write tests for job lifecycle

**Files to modify:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/db/src/migrations.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/core/src/types.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/db/src/queries.rs`

---

### 1.3 Add index_runs Table

Track indexing history for the System page.

**SQL (Migration 13d):**

```sql
CREATE TABLE IF NOT EXISTS index_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,               -- ISO timestamp (RFC 3339)
    completed_at TEXT,                       -- ISO timestamp, NULL if running
    type TEXT NOT NULL,                      -- 'full', 'incremental', 'deep'
    sessions_before INTEGER,                 -- Session count before run
    sessions_after INTEGER,                  -- Session count after run
    duration_ms INTEGER,                     -- Total duration
    throughput_mb_per_sec REAL,             -- Processing speed (for display)
    status TEXT DEFAULT 'running',           -- 'running', 'completed', 'failed'
    error_message TEXT,                      -- Error details if status='failed'
    CONSTRAINT valid_type CHECK (type IN ('full', 'incremental', 'deep')),
    CONSTRAINT valid_status CHECK (status IN ('running', 'completed', 'failed'))
);

CREATE INDEX IF NOT EXISTS idx_index_runs_started ON index_runs(started_at DESC);
```

**Subtasks:**
- [ ] Add migration statements to `crates/db/src/migrations.rs`
- [ ] Create `IndexRun` struct in `crates/core/src/types.rs`
- [ ] Create `IndexRunType` and `IndexRunStatus` enums
- [ ] Add CRUD functions in `crates/db/src/queries.rs`:
  - `create_index_run()`
  - `complete_index_run()`
  - `get_recent_index_runs()` — last 20 runs
- [ ] Integrate with existing `run_background_index()` in `crates/db/src/indexer_parallel.rs`
- [ ] Write tests

**Files to modify:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/db/src/migrations.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/core/src/types.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/db/src/queries.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/db/src/indexer_parallel.rs`

---

### 1.4 Claude CLI Integration

Create a module for spawning Claude CLI and parsing its JSON output.

**Location:** `crates/core/src/llm/`

**Module structure:**

```
crates/core/src/llm/
├── mod.rs           # Module exports
├── provider.rs      # LlmProvider trait
├── claude_cli.rs    # ClaudeCliProvider implementation
└── types.rs         # Response types
```

**ClaudeCliProvider implementation:**

```rust
// crates/core/src/llm/provider.rs
use async_trait::async_trait;
use crate::llm::types::{ClassificationRequest, ClassificationResponse, LlmError};

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Classify a session's first prompt.
    async fn classify(&self, request: ClassificationRequest) -> Result<ClassificationResponse, LlmError>;

    /// Check if the provider is available (CLI installed, API key set, etc.)
    async fn health_check(&self) -> Result<(), LlmError>;

    /// Provider name for logging/display
    fn name(&self) -> &str;

    /// Model identifier
    fn model(&self) -> &str;
}
```

```rust
// crates/core/src/llm/claude_cli.rs
use std::process::Command;
use tokio::process::Command as TokioCommand;

pub struct ClaudeCliProvider {
    model: String,        // "haiku" | "sonnet" | "opus"
    timeout_secs: u64,    // Default: 30
}

impl ClaudeCliProvider {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            timeout_secs: 30,
        }
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Spawn Claude CLI and parse JSON response.
    ///
    /// Command: `claude -p --output-format json --dangerously-skip-permissions --model {model} "{prompt}"`
    async fn spawn_and_parse(&self, prompt: &str) -> Result<serde_json::Value, LlmError> {
        use tokio::time::{timeout, Duration};

        let timeout_duration = Duration::from_secs(self.timeout_secs);

        let future = TokioCommand::new("claude")
            .args([
                "-p",
                "--output-format", "json",
                "--dangerously-skip-permissions",
                "--model", &self.model,
                prompt,
            ])
            .output();

        let output = timeout(timeout_duration, future)
            .await
            .map_err(|_| LlmError::Timeout(self.timeout_secs))?
            .map_err(|e| LlmError::SpawnFailed(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LlmError::CliError(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&stdout)
            .map_err(|e| LlmError::ParseFailed(e.to_string()))
    }
}

#[async_trait]
impl LlmProvider for ClaudeCliProvider {
    async fn classify(&self, request: ClassificationRequest) -> Result<ClassificationResponse, LlmError> {
        let prompt = build_classification_prompt(&request);
        let json = self.spawn_and_parse(&prompt).await?;
        parse_classification_response(json)
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        // Quick check: run `claude --version`
        let output = TokioCommand::new("claude")
            .arg("--version")
            .output()
            .await
            .map_err(|e| LlmError::SpawnFailed(format!("claude not found: {}", e)))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(LlmError::NotAvailable("claude --version failed".into()))
        }
    }

    fn name(&self) -> &str { "claude-cli" }
    fn model(&self) -> &str { &self.model }
}
```

```rust
// crates/core/src/llm/types.rs
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationRequest {
    pub session_id: String,
    pub first_prompt: String,
    pub files_touched: Vec<String>,
    pub skills_used: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResponse {
    pub category_l1: String,      // "code_work" | "support_work" | "thinking_work"
    pub category_l2: String,      // "feature" | "bug_fix" | etc.
    pub category_l3: String,      // "new-component" | "error-fix" | etc.
    pub confidence: f64,          // 0.0-1.0
    pub reasoning: Option<String>, // Optional explanation
}

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("Failed to spawn LLM process: {0}")]
    SpawnFailed(String),

    #[error("CLI returned error: {0}")]
    CliError(String),

    #[error("Failed to parse response: {0}")]
    ParseFailed(String),

    #[error("Provider not available: {0}")]
    NotAvailable(String),

    #[error("Rate limited, retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },

    #[error("Invalid response format: {0}")]
    InvalidFormat(String),

    #[error("Timeout after {0} seconds")]
    Timeout(u64),
}
```

**Classification prompt template:**

```rust
fn build_classification_prompt(request: &ClassificationRequest) -> String {
    format!(
        r#"Classify this Claude Code session. Respond with JSON only.

First prompt:
```
{}
```

Files touched: {}
Skills used: {}

Categories:
L1: code_work | support_work | thinking_work
L2 (if code_work): feature | bug_fix | refactor | testing
L2 (if support_work): docs | config | ops
L2 (if thinking_work): planning | explanation | architecture
L3 (by L2):
  - feature: new-component | new-endpoint | new-integration | enhancement
  - bug_fix: error-fix | logic-fix | regression-fix | crash-fix
  - refactor: rename | extract | restructure | cleanup
  - testing: unit-test | integration-test | e2e-test | test-fix
  - docs: readme | api-docs | inline-docs | changelog
  - config: env-config | build-config | ci-config | deps
  - ops: deploy | monitoring | migration | backup
  - planning: design-doc | spike | estimation | roadmap
  - explanation: how-it-works | debugging-help | code-review | comparison
  - architecture: system-design | data-model | api-design | infra

Respond:
{{"category_l1": "...", "category_l2": "...", "category_l3": "...", "confidence": 0.0-1.0}}"#,
        request.first_prompt,
        request.files_touched.join(", "),
        request.skills_used.join(", ")
    )
}
```

**Subtasks:**
- [ ] Create `crates/core/src/llm/mod.rs`
- [ ] Create `crates/core/src/llm/provider.rs` with `LlmProvider` trait
- [ ] Create `crates/core/src/llm/types.rs` with request/response/error types
- [ ] Create `crates/core/src/llm/claude_cli.rs` with `ClaudeCliProvider`
- [ ] Add `pub mod llm;` to `crates/core/src/lib.rs`
- [ ] Write unit tests for prompt building
- [ ] Write integration test (requires Claude CLI installed)
- [ ] Add `async-trait` to `crates/core/Cargo.toml`
- [ ] Add `thiserror` to `crates/core/Cargo.toml`

**Files to create:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/core/src/llm/mod.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/core/src/llm/provider.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/core/src/llm/types.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/core/src/llm/claude_cli.rs`

**Files to modify:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/core/src/lib.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/core/Cargo.toml`

---

### 1.5 Background Job Runner

Create infrastructure for running long-running async jobs with progress tracking.

**Location:** `crates/server/src/jobs/`

**Module structure:**

```
crates/server/src/jobs/
├── mod.rs           # Module exports, JobRunner type
├── runner.rs        # JobRunner implementation
├── state.rs         # JobState (atomic progress tracking)
└── types.rs         # Job trait, JobHandle, JobStatus
```

**JobRunner design:**

```rust
// crates/server/src/jobs/types.rs
use std::sync::Arc;
use tokio::sync::oneshot;

/// Unique identifier for a running job.
pub type JobId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Failed(String),
}

/// Handle to a running job, used for cancellation.
pub struct JobHandle {
    pub id: JobId,
    cancel_tx: Option<oneshot::Sender<()>>,
}

impl JobHandle {
    pub fn cancel(mut self) -> bool {
        if let Some(tx) = self.cancel_tx.take() {
            tx.send(()).is_ok()
        } else {
            false
        }
    }
}

/// Progress update sent via SSE.
#[derive(Debug, Clone, Serialize)]
pub struct JobProgress {
    pub job_id: JobId,
    pub job_type: String,
    pub status: String,
    pub current: u64,
    pub total: u64,
    pub message: Option<String>,
    pub timestamp: String,
}
```

```rust
// crates/server/src/jobs/state.rs
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::RwLock;
use tokio::sync::broadcast;
use super::types::{JobProgress, JobStatus};

/// Atomic state for a single job.
pub struct JobState {
    id: u64,
    job_type: String,
    status: AtomicU8,
    current: AtomicU64,
    total: AtomicU64,
    message: RwLock<Option<String>>,
    progress_tx: broadcast::Sender<JobProgress>,
}

impl JobState {
    pub fn new(id: u64, job_type: String, total: u64) -> Self {
        let (progress_tx, _) = broadcast::channel(64);
        Self {
            id,
            job_type,
            status: AtomicU8::new(JobStatus::Pending as u8),
            current: AtomicU64::new(0),
            total: AtomicU64::new(total),
            message: RwLock::new(None),
            progress_tx,
        }
    }

    pub fn set_running(&self) {
        self.status.store(JobStatus::Running as u8, Ordering::Relaxed);
        self.broadcast_progress();
    }

    pub fn increment(&self) -> u64 {
        let new = self.current.fetch_add(1, Ordering::Relaxed) + 1;
        self.broadcast_progress();
        new
    }

    pub fn set_message(&self, msg: impl Into<String>) {
        if let Ok(mut guard) = self.message.write() {
            *guard = Some(msg.into());
        }
        self.broadcast_progress();
    }

    pub fn complete(&self) {
        self.status.store(JobStatus::Completed as u8, Ordering::Relaxed);
        self.broadcast_progress();
    }

    pub fn fail(&self, error: impl Into<String>) {
        self.status.store(JobStatus::Failed as u8, Ordering::Relaxed);
        self.set_message(error);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<JobProgress> {
        self.progress_tx.subscribe()
    }

    fn broadcast_progress(&self) {
        let progress = self.snapshot();
        let _ = self.progress_tx.send(progress);
    }

    pub fn snapshot(&self) -> JobProgress {
        JobProgress {
            job_id: self.id,
            job_type: self.job_type.clone(),
            status: self.status_string(),
            current: self.current.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
            message: self.message.read().ok().and_then(|g| g.clone()),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn status_string(&self) -> String {
        match self.status.load(Ordering::Relaxed) {
            0 => "pending".into(),
            1 => "running".into(),
            2 => "completed".into(),
            3 => "cancelled".into(),
            _ => "failed".into(),
        }
    }
}
```

```rust
// crates/server/src/jobs/runner.rs
use std::collections::HashMap;
use std::sync::{Arc, RwLock, atomic::{AtomicU64, Ordering}};
use tokio::sync::{oneshot, broadcast};
use super::types::{JobId, JobHandle, JobStatus, JobProgress};
use super::state::JobState;

/// Central job runner that manages all background jobs.
pub struct JobRunner {
    next_id: AtomicU64,
    jobs: RwLock<HashMap<JobId, Arc<JobState>>>,
    global_tx: broadcast::Sender<JobProgress>,
}

impl JobRunner {
    pub fn new() -> Self {
        let (global_tx, _) = broadcast::channel(256);
        Self {
            next_id: AtomicU64::new(1),
            jobs: RwLock::new(HashMap::new()),
            global_tx,
        }
    }

    /// Start a new job. Returns a handle for cancellation.
    pub fn start_job<F, Fut>(
        &self,
        job_type: impl Into<String>,
        total: u64,
        f: F,
    ) -> JobHandle
    where
        F: FnOnce(Arc<JobState>, oneshot::Receiver<()>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let state = Arc::new(JobState::new(id, job_type.into(), total));

        // Store state
        if let Ok(mut jobs) = self.jobs.write() {
            jobs.insert(id, Arc::clone(&state));
        }

        // Create cancellation channel
        let (cancel_tx, cancel_rx) = oneshot::channel();

        // Forward job progress to global channel
        let global_tx = self.global_tx.clone();
        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            let mut rx = state_clone.subscribe();
            while let Ok(progress) = rx.recv().await {
                let _ = global_tx.send(progress);
            }
        });

        // Spawn the job
        let state_for_task = Arc::clone(&state);
        tokio::spawn(async move {
            state_for_task.set_running();
            match f(state_for_task.clone(), cancel_rx).await {
                Ok(()) => state_for_task.complete(),
                Err(e) => state_for_task.fail(e),
            }
        });

        JobHandle {
            id,
            cancel_tx: Some(cancel_tx),
        }
    }

    /// Subscribe to all job progress updates (for SSE).
    pub fn subscribe(&self) -> broadcast::Receiver<JobProgress> {
        self.global_tx.subscribe()
    }

    /// Get current status of a job.
    pub fn get_job(&self, id: JobId) -> Option<JobProgress> {
        self.jobs.read().ok()
            .and_then(|jobs| jobs.get(&id).map(|s| s.snapshot()))
    }

    /// Get all active jobs.
    pub fn active_jobs(&self) -> Vec<JobProgress> {
        self.jobs.read().ok()
            .map(|jobs| jobs.values().map(|s| s.snapshot()).collect())
            .unwrap_or_default()
    }
}

impl Default for JobRunner {
    fn default() -> Self {
        Self::new()
    }
}
```

**Integration with AppState:**

```rust
// Update crates/server/src/state.rs
pub struct AppState {
    pub start_time: Instant,
    pub db: Database,
    pub indexing: Arc<IndexingState>,
    pub registry: RegistryHolder,
    pub jobs: Arc<JobRunner>,  // NEW
}
```

**SSE endpoint for job progress:**

```rust
// crates/server/src/routes/jobs.rs
use axum::{
    extract::State,
    response::sse::{Event, Sse},
};
use futures::stream::Stream;
use std::sync::Arc;
use crate::state::AppState;

/// GET /api/jobs/stream — SSE stream of all job progress updates.
pub async fn stream_jobs(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.jobs.subscribe();

    let stream = async_stream::stream! {
        let mut rx = rx;
        while let Ok(progress) = rx.recv().await {
            let json = serde_json::to_string(&progress).unwrap_or_default();
            yield Ok(Event::default().data(json));
        }
    };

    Sse::new(stream)
}

/// GET /api/jobs — List all active jobs.
pub async fn list_jobs(
    State(state): State<Arc<AppState>>,
) -> axum::Json<Vec<JobProgress>> {
    axum::Json(state.jobs.active_jobs())
}

pub fn router() -> axum::Router<Arc<AppState>> {
    use axum::routing::get;
    axum::Router::new()
        .route("/jobs", get(list_jobs))
        .route("/jobs/stream", get(stream_jobs))
}
```

**Subtasks:**
- [ ] Create `crates/server/src/jobs/mod.rs`
- [ ] Create `crates/server/src/jobs/types.rs`
- [ ] Create `crates/server/src/jobs/state.rs`
- [ ] Create `crates/server/src/jobs/runner.rs`
- [ ] Add `pub mod jobs;` to `crates/server/src/lib.rs`
- [ ] Update `AppState` to include `JobRunner`
- [ ] Create `crates/server/src/routes/jobs.rs`
- [ ] Update `crates/server/src/routes/mod.rs` to include jobs router
- [ ] Add `async-stream` to `crates/server/Cargo.toml`
- [ ] Write unit tests for JobRunner
- [ ] Write integration test for SSE streaming

**Files to create:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/server/src/jobs/mod.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/server/src/jobs/types.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/server/src/jobs/state.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/server/src/jobs/runner.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/server/src/routes/jobs.rs`

**Files to modify:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/server/src/lib.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/server/src/state.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/server/src/routes/mod.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/server/src/main.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/server/Cargo.toml`

---

## Data Model

### Full Migration 13 SQL

```sql
-- Migration 13: Theme 4 Foundation
-- 13a: Classification columns on sessions
ALTER TABLE sessions ADD COLUMN category_l1 TEXT;
ALTER TABLE sessions ADD COLUMN category_l2 TEXT;
ALTER TABLE sessions ADD COLUMN category_l3 TEXT;
ALTER TABLE sessions ADD COLUMN category_confidence REAL;
ALTER TABLE sessions ADD COLUMN category_source TEXT;
ALTER TABLE sessions ADD COLUMN classified_at TEXT;

-- 13b: Behavioral metrics columns on sessions
ALTER TABLE sessions ADD COLUMN prompt_word_count INTEGER;
ALTER TABLE sessions ADD COLUMN correction_count INTEGER DEFAULT 0;
ALTER TABLE sessions ADD COLUMN same_file_edit_count INTEGER DEFAULT 0;

-- 13c: classification_jobs table
CREATE TABLE IF NOT EXISTS classification_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    total_sessions INTEGER NOT NULL,
    classified_count INTEGER DEFAULT 0,
    skipped_count INTEGER DEFAULT 0,
    failed_count INTEGER DEFAULT 0,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    status TEXT DEFAULT 'running',
    error_message TEXT,
    cost_estimate_cents INTEGER,
    actual_cost_cents INTEGER,
    tokens_used INTEGER,
    CONSTRAINT valid_status CHECK (status IN ('running', 'completed', 'cancelled', 'failed'))
);

-- 13d: index_runs table
CREATE TABLE IF NOT EXISTS index_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    type TEXT NOT NULL,
    sessions_before INTEGER,
    sessions_after INTEGER,
    duration_ms INTEGER,
    throughput_mb_per_sec REAL,
    status TEXT DEFAULT 'running',
    error_message TEXT,
    CONSTRAINT valid_type CHECK (type IN ('full', 'incremental', 'deep')),
    CONSTRAINT valid_status CHECK (status IN ('running', 'completed', 'failed'))
);

-- 13e: Indexes
CREATE INDEX IF NOT EXISTS idx_sessions_category_l1 ON sessions(category_l1) WHERE category_l1 IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_sessions_classified ON sessions(classified_at) WHERE classified_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_classification_jobs_status ON classification_jobs(status);
CREATE INDEX IF NOT EXISTS idx_classification_jobs_started ON classification_jobs(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_index_runs_started ON index_runs(started_at DESC);
```

---

## Rust Types

### Core Types (crates/core/src/types.rs additions)

```rust
// Add to SessionInfo struct
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    // ... existing fields ...

    // Theme 4: Classification
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_l1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_l2: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_l3: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_confidence: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classified_at: Option<String>,

    // Theme 4: Behavioral metrics
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_word_count: Option<u32>,
    #[serde(default)]
    pub correction_count: u32,
    #[serde(default)]
    pub same_file_edit_count: u32,
}
```

### Classification Job Types (crates/core/src/types.rs)

```rust
/// Status of a classification job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum ClassificationJobStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
}

/// A classification job record.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ClassificationJob {
    pub id: i64,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub total_sessions: i64,
    pub classified_count: i64,
    pub skipped_count: i64,
    pub failed_count: i64,
    pub provider: String,
    pub model: String,
    pub status: ClassificationJobStatus,
    pub error_message: Option<String>,
    pub cost_estimate_cents: Option<i64>,
    pub actual_cost_cents: Option<i64>,
    pub tokens_used: Option<i64>,
}
```

### Index Run Types (crates/core/src/types.rs)

```rust
/// Type of index run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum IndexRunType {
    Full,
    Incremental,
    Deep,
}

/// Status of an index run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum IndexRunStatus {
    Running,
    Completed,
    Failed,
}

/// An index run record.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct IndexRun {
    pub id: i64,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub run_type: IndexRunType,
    pub sessions_before: Option<i64>,
    pub sessions_after: Option<i64>,
    pub duration_ms: Option<i64>,
    pub throughput_mb_per_sec: Option<f64>,
    pub status: IndexRunStatus,
    pub error_message: Option<String>,
}
```

---

## API Changes

### New Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/jobs` | List all active background jobs |
| GET | `/api/jobs/stream` | SSE stream of job progress updates |

### Modified Endpoints

| Endpoint | Change |
|----------|--------|
| `GET /api/sessions` | Add `category_l1`, `category_l2`, `category_l3` fields to response |
| `GET /api/sessions/:id` | Add classification fields to response |
| `GET /api/projects/:id/sessions` | Add classification fields to response |

---

## File Changes Summary

### Files to Create

| File | Purpose |
|------|---------|
| `crates/core/src/llm/mod.rs` | LLM module exports |
| `crates/core/src/llm/provider.rs` | LlmProvider trait |
| `crates/core/src/llm/types.rs` | Request/response/error types |
| `crates/core/src/llm/claude_cli.rs` | ClaudeCliProvider implementation |
| `crates/server/src/jobs/mod.rs` | Jobs module exports |
| `crates/server/src/jobs/types.rs` | Job types |
| `crates/server/src/jobs/state.rs` | JobState (atomic progress) |
| `crates/server/src/jobs/runner.rs` | JobRunner |
| `crates/server/src/routes/jobs.rs` | Jobs API routes |

### Files to Modify

| File | Change |
|------|--------|
| `crates/db/src/migrations.rs` | Add Migration 13 |
| `crates/core/src/types.rs` | Add classification/job types to SessionInfo |
| `crates/core/src/lib.rs` | Export llm module |
| `crates/core/Cargo.toml` | Add async-trait dependency |
| `crates/db/src/queries.rs` | Add job CRUD functions, update session queries |
| `crates/db/src/indexer_parallel.rs` | Integrate index_runs tracking |
| `crates/server/src/lib.rs` | Export jobs module |
| `crates/server/src/state.rs` | Add JobRunner to AppState |
| `crates/server/src/main.rs` | Initialize JobRunner |
| `crates/server/src/routes/mod.rs` | Add jobs router |
| `crates/server/Cargo.toml` | Add async-stream dependency |

---

## Testing Strategy

### Unit Tests

| Test | Location | Scope |
|------|----------|-------|
| Migration 13 columns exist | `crates/db/src/migrations.rs` | Verify schema |
| Migration 13 constraints work | `crates/db/src/migrations.rs` | Verify CHECK constraints |
| Classification job CRUD | `crates/db/src/queries.rs` | Create, read, update, cancel |
| Index run CRUD | `crates/db/src/queries.rs` | Create, complete, list |
| LLM prompt building | `crates/core/src/llm/claude_cli.rs` | Prompt format |
| LLM response parsing | `crates/core/src/llm/claude_cli.rs` | JSON parsing |
| JobState atomics | `crates/server/src/jobs/state.rs` | Concurrent updates |
| JobRunner lifecycle | `crates/server/src/jobs/runner.rs` | Start, progress, complete |

### Integration Tests

| Test | Location | Scope |
|------|----------|-------|
| Claude CLI spawn (optional) | `crates/core/src/llm/claude_cli.rs` | Requires Claude CLI |
| SSE job streaming | `crates/server/src/routes/jobs.rs` | End-to-end SSE |
| Full migration up/down | `crates/db/tests/` | Database lifecycle |

### Test Commands

```bash
# Run all Phase 1 tests
cargo test -p db -- migration13
cargo test -p core -- llm
cargo test -p server -- jobs

# Run specific test file
cargo test -p db -- migrations::tests

# Integration test (requires Claude CLI)
CLAUDE_CLI_TEST=1 cargo test -p core -- llm::integration --ignored
```

---

## Acceptance Criteria

### Task 1.1: Session Classification Columns
- [ ] Migration 13a runs without error on fresh DB
- [ ] Migration 13a runs without error on existing DB (ALTER TABLE)
- [ ] `SessionInfo` struct includes new classification fields
- [ ] Session queries return classification fields (NULL initially)
- [ ] Partial index on `category_l1` works

### Task 1.2: classification_jobs Table
- [ ] Table created with all columns
- [ ] CHECK constraint enforces valid status values
- [ ] `create_classification_job()` works
- [ ] `get_active_classification_job()` returns running job or None
- [ ] `update_classification_job_progress()` increments counters
- [ ] `complete_classification_job()` sets status and completed_at
- [ ] `cancel_classification_job()` sets status to cancelled

### Task 1.3: index_runs Table
- [ ] Table created with all columns
- [ ] CHECK constraints enforce valid type/status values
- [ ] `create_index_run()` works
- [ ] `complete_index_run()` sets duration and status
- [ ] `get_recent_index_runs()` returns last 20 runs
- [ ] Existing indexer creates index_run record

### Task 1.4: Claude CLI Integration
- [ ] `ClaudeCliProvider::health_check()` detects if CLI is installed
- [ ] `spawn_and_parse()` executes `claude -p` with correct args
- [ ] JSON response parsed into `ClassificationResponse`
- [ ] Errors handled: spawn failure, CLI error, parse failure, timeout
- [ ] Classification prompt follows taxonomy

### Task 1.5: Background Job Runner
- [ ] `JobRunner::start_job()` spawns async task
- [ ] `JobState` tracks progress with atomics
- [ ] Progress updates broadcast via channel
- [ ] `GET /api/jobs` returns active jobs
- [ ] `GET /api/jobs/stream` returns SSE stream
- [ ] Cancellation via `JobHandle::cancel()` works
- [ ] Jobs cleaned up after completion

### Overall
- [ ] All 577+ existing backend tests pass
- [ ] No TypeScript compilation errors
- [ ] `cargo clippy` passes with no warnings
- [ ] Migration test verifies schema correctness

---

## Dependencies

This phase has no dependencies on other phases.

Phases 2, 3, and 4 all depend on Phase 1 completing before they can start.

---

## Notes

- **No UI changes in this phase** — all work is backend infrastructure
- **Claude CLI is optional** — health_check gracefully fails if not installed
- **Job runner is generic** — will be reused by classification (Phase 2) and future batch operations
- **TypeScript types auto-generated** — ts-rs exports will update frontend types
- **Follow existing patterns** — migrations like Migration 8, atomic state like IndexingState
