---
status: pending
date: 2026-02-05
depends_on: phase1-foundation
parallelizable_with: [phase3-system-page, phase4-pattern-engine]
---

# Theme 4 Phase 2: Classification System

> **Goal:** Build the LLM-powered session classification system that categorizes sessions into the 30-category taxonomy.

## Overview

This phase implements the core classification system that powers the Insights page. It includes:
1. LLM prompt design for batch classification
2. Background job execution with progress tracking
3. SSE streaming for real-time UI updates
4. Provider configuration (Claude CLI default, BYOK API)
5. React components for triggering and monitoring classification

**Dependencies:**
- Phase 1 (Foundation) must be complete:
  - `classification_jobs` table exists
  - Session columns (`category_l1`, `category_l2`, `category_l3`) exist
  - Background job runner infrastructure exists
  - Claude CLI integration works

---

## Classification Taxonomy (30 Categories)

### Hierarchy Structure

```
Code Work (12 leaves)
├── Feature
│   ├── new-component      # Creating new UI components, pages, modules
│   ├── add-functionality  # Adding capabilities to existing code
│   └── integration        # Connecting systems, APIs, services
├── Bug Fix
│   ├── error-fix          # Fixing errors, exceptions, crashes
│   ├── logic-fix          # Correcting business logic issues
│   └── performance-fix    # Resolving performance problems
├── Refactor
│   ├── cleanup            # Code style, formatting, dead code removal
│   ├── pattern-migration  # Moving to new patterns, architectures
│   └── dependency-update  # Updating packages, versions
└── Testing
    ├── unit-tests         # Adding/fixing unit tests
    ├── integration-tests  # E2E, API tests
    └── test-fixes         # Fixing flaky/broken tests

Support Work (9 leaves)
├── Docs
│   ├── code-comments      # Inline documentation, JSDoc
│   ├── readme-guides      # README, tutorials, guides
│   └── api-docs           # API documentation, schemas
├── Config
│   ├── env-setup          # Environment variables, local setup
│   ├── build-tooling      # Webpack, Vite, tsconfig
│   └── dependencies       # Package.json, Cargo.toml
└── Ops
    ├── ci-cd              # GitHub Actions, pipelines
    ├── deployment         # Docker, K8s, hosting
    └── monitoring         # Logging, alerts, metrics

Thinking Work (9 leaves)
├── Planning
│   ├── brainstorming      # Idea exploration, options analysis
│   ├── design-doc         # Technical specs, PRDs
│   └── task-breakdown     # Breaking work into tasks
├── Explanation
│   ├── code-understanding # "How does X work?"
│   ├── concept-learning   # Learning new tech/concepts
│   └── debug-investigation# Diagnosing issues, root cause
└── Architecture
    ├── system-design      # High-level architecture
    ├── data-modeling      # Database schemas, data flow
    └── api-design         # API contracts, protocols
```

### Category IDs (for database storage)

```
L1          L2             L3
─────────────────────────────────────────────────
code        feature        new-component
code        feature        add-functionality
code        feature        integration
code        bugfix         error-fix
code        bugfix         logic-fix
code        bugfix         performance-fix
code        refactor       cleanup
code        refactor       pattern-migration
code        refactor       dependency-update
code        testing        unit-tests
code        testing        integration-tests
code        testing        test-fixes
support     docs           code-comments
support     docs           readme-guides
support     docs           api-docs
support     config         env-setup
support     config         build-tooling
support     config         dependencies
support     ops            ci-cd
support     ops            deployment
support     ops            monitoring
thinking    planning       brainstorming
thinking    planning       design-doc
thinking    planning       task-breakdown
thinking    explanation    code-understanding
thinking    explanation    concept-learning
thinking    explanation    debug-investigation
thinking    architecture   system-design
thinking    architecture   data-modeling
thinking    architecture   api-design
```

---

## Tasks

### 2.1 Classification Prompt Design

**Goal:** Design and test the LLM prompt that classifies sessions.

**Requirements:**
- Accept batch of 5 sessions (efficiency vs context window)
- Return structured JSON (parseable without regex)
- Include full taxonomy in system prompt
- Handle edge cases: non-English, empty prompts, ambiguous

**File changes:**
- `crates/db/src/classification.rs` (new) — prompt template, response parsing

**Subtasks:**
- [ ] 2.1.1 Write system prompt with taxonomy reference
- [ ] 2.1.2 Write user prompt template for batch classification
- [ ] 2.1.3 Define JSON response schema
- [ ] 2.1.4 Implement response parser with error handling
- [ ] 2.1.5 Add unit tests with sample responses
- [ ] 2.1.6 Test with Claude CLI locally (manual validation)

---

### 2.2 POST /api/classify Endpoint

**Goal:** Trigger classification job.

**File changes:**
- `crates/server/src/routes/classify.rs` (new)
- `crates/server/src/routes/mod.rs` — add module
- `crates/db/src/lib.rs` — export classification module

**Subtasks:**
- [ ] 2.2.1 Create `ClassifyRequest` struct
- [ ] 2.2.2 Create `ClassifyResponse` struct (with ts-rs export)
- [ ] 2.2.3 Implement job creation logic
- [ ] 2.2.4 Calculate estimated cost (based on session count + avg tokens)
- [ ] 2.2.5 Spawn background task
- [ ] 2.2.6 Return job ID immediately (202 Accepted)
- [ ] 2.2.7 Add route to router
- [ ] 2.2.8 Write integration tests

---

### 2.3 GET /api/classify/status Endpoint

**Goal:** Poll classification job status.

**File changes:**
- `crates/server/src/routes/classify.rs`

**Subtasks:**
- [ ] 2.3.1 Create `ClassifyStatusResponse` struct
- [ ] 2.3.2 Query current job from database
- [ ] 2.3.3 Calculate progress percentage and ETA
- [ ] 2.3.4 Include last run info (duration, cost)
- [ ] 2.3.5 Write integration tests

---

### 2.3b POST /api/classify/cancel Endpoint

**Goal:** Cancel a running classification job.

**File changes:**
- `crates/server/src/routes/classify.rs`

**Subtasks:**
- [ ] 2.3b.1 Create `CancelResponse` struct
- [ ] 2.3b.2 Check if job is running, return 404 if not
- [ ] 2.3b.3 Set cancellation flag in `ClassifyState`
- [ ] 2.3b.4 Update job status to `cancelled` in database
- [ ] 2.3b.5 Return cancellation confirmation
- [ ] 2.3b.6 Write integration tests

---

### 2.4 SSE /api/classify/stream Endpoint

**Goal:** Real-time progress streaming during classification.

**File changes:**
- `crates/server/src/routes/classify.rs`
- `crates/server/src/classify_state.rs` (new) — atomic state for job progress

**Subtasks:**
- [ ] 2.4.1 Create `ClassifyState` (similar to `IndexingState`)
- [ ] 2.4.2 Implement SSE handler with event types
- [ ] 2.4.3 Emit `progress` events on batch completion
- [ ] 2.4.4 Emit `batch` events with date range info
- [ ] 2.4.5 Emit `complete` event with final stats
- [ ] 2.4.6 Emit `error` event with retry info
- [ ] 2.4.7 Write integration tests

---

### 2.5 Provider Configuration

**Goal:** Support multiple LLM providers for classification.

**File changes:**
- `crates/db/src/settings.rs` (new or extend) — provider config storage
- `crates/db/src/classification.rs` — provider abstraction
- `crates/server/src/routes/settings.rs` (extend or new)

**Subtasks:**
- [ ] 2.5.1 Define `ProviderConfig` enum (ClaudeCli, AnthropicApi, OpenAiCompatible)
- [ ] 2.5.2 Add `classification_settings` table (migration)
- [ ] 2.5.3 Implement Claude CLI provider (spawn process, parse JSON)
- [ ] 2.5.4 Implement Anthropic API provider (reqwest, async)
- [ ] 2.5.5 Implement OpenAI-compatible provider (configurable endpoint)
- [ ] 2.5.6 Add GET/PUT /api/settings/classification endpoints
- [ ] 2.5.7 Write integration tests per provider

---

### 2.6 Classification UI

**Goal:** React components for classification management.

**File changes:**
- `src/components/ClassificationStatus.tsx` (new)
- `src/components/ClassificationProgress.tsx` (new)
- `src/components/ProviderSettings.tsx` (new)
- `src/hooks/use-classification.ts` (new)
- `src/types/generated/` — auto-generated types

**Subtasks:**
- [ ] 2.6.1 Create `useClassification` hook (trigger, status, stream)
- [ ] 2.6.2 Create `ClassificationStatus` card component
- [ ] 2.6.3 Create `ClassificationProgress` modal component
- [ ] 2.6.4 Create `ProviderSettings` form component
- [ ] 2.6.5 Integrate into System page (placeholder until Phase 3)
- [ ] 2.6.6 Write component tests

---

## Rate Limiting Strategy

### Retry Behavior

| Parameter | Value | Notes |
|-----------|-------|-------|
| Max retries per batch | 3 | After 3 failures, batch is skipped and logged |
| Backoff algorithm | Exponential | Starting at 1s, doubles each retry |
| Initial backoff | 1 second | First retry after 1s |
| Max backoff | 60 seconds | Cap at 60s regardless of retry count |
| Jitter | ±20% | Random jitter to prevent thundering herd |

### Circuit Breaker

| Parameter | Value | Notes |
|-----------|-------|-------|
| Failure threshold | 5 consecutive batches | If 5 batches fail in a row, pause job |
| Pause duration | 5 minutes | Wait before resuming classification |
| Max pauses | 3 | After 3 pauses, fail the entire job |

### Backoff Formula

```
delay = min(initial_backoff * 2^(retry_count - 1), max_backoff) * (1 ± jitter)

Example:
  Retry 1: 1s * (1 ± 0.2) = 0.8-1.2s
  Retry 2: 2s * (1 ± 0.2) = 1.6-2.4s
  Retry 3: 4s * (1 ± 0.2) = 3.2-4.8s
```

---

## Session Preview Truncation

### Truncation Rules

| Parameter | Value | Notes |
|-----------|-------|-------|
| Max preview length | 2000 characters | Truncate first user message to this length |
| Truncation suffix | `... [truncated]` | Appended when preview is truncated |
| Max skills per session | 10 | Include only first 10 skills in prompt |

### Implementation

```rust
fn truncate_preview(preview: &str) -> String {
    const MAX_LEN: usize = 2000;
    const SUFFIX: &str = "... [truncated]";

    if preview.len() <= MAX_LEN {
        preview.to_string()
    } else {
        // Truncate at word boundary if possible
        let truncate_at = MAX_LEN - SUFFIX.len();
        let boundary = preview[..truncate_at]
            .rfind(char::is_whitespace)
            .unwrap_or(truncate_at);
        format!("{}{}", &preview[..boundary], SUFFIX)
    }
}
```

---

## Persistence Strategy

### Commit Timing

| Event | Action | Notes |
|-------|--------|-------|
| Batch completion | Commit to SQLite | Classifications saved after each batch of 5 |
| Job progress | Update `classification_jobs` table | Progress saved every batch |
| Error | Log to `classification_errors` | Individual failures logged with session ID |

### Database Updates Per Batch

```sql
-- 1. Update classified sessions (within transaction)
BEGIN;
UPDATE sessions SET
  category_l1 = ?, category_l2 = ?, category_l3 = ?,
  classification_confidence = ?,
  classified_at = strftime('%s', 'now')
WHERE id = ?;
-- ... (repeat for each session in batch)

-- 2. Update job progress
UPDATE classification_jobs SET
  classified_sessions = classified_sessions + ?,
  actual_cost = actual_cost + ?,
  updated_at = strftime('%s', 'now')
WHERE id = ?;
COMMIT;
```

### Resume Behavior on Restart

| Scenario | Behavior |
|----------|----------|
| Server restart during job | Job status remains `running` in database |
| On startup | Check for `running` jobs older than 5 minutes |
| Stale job detected | Mark as `failed` with message "Server restart interrupted job" |
| User can retry | POST /api/classify will process remaining unclassified sessions |

### Job Recovery Query

```sql
-- Find and mark stale jobs on startup
UPDATE classification_jobs
SET status = 'failed',
    error_message = 'Server restart interrupted job',
    completed_at = strftime('%s', 'now')
WHERE status = 'running'
  AND updated_at < strftime('%s', 'now') - 300;  -- 5 minute timeout
```

---

## LLM Prompt

### System Prompt

```
You are a session classifier for a coding assistant analytics tool. Your job is to categorize AI coding sessions based on the user's first prompt (the initial question or request).

## Taxonomy

Classify each session into exactly ONE category from this hierarchy:

### Code Work
- **feature/new-component**: Creating new UI components, pages, modules from scratch
- **feature/add-functionality**: Adding capabilities to existing code
- **feature/integration**: Connecting systems, APIs, external services
- **bugfix/error-fix**: Fixing errors, exceptions, crashes
- **bugfix/logic-fix**: Correcting business logic, wrong behavior
- **bugfix/performance-fix**: Resolving slowness, memory issues, optimization
- **refactor/cleanup**: Code style, formatting, removing dead code
- **refactor/pattern-migration**: Moving to new patterns, architectures
- **refactor/dependency-update**: Updating packages, libraries, versions
- **testing/unit-tests**: Adding or fixing unit tests
- **testing/integration-tests**: E2E, API, browser tests
- **testing/test-fixes**: Fixing flaky or broken tests

### Support Work
- **docs/code-comments**: Inline documentation, JSDoc, docstrings
- **docs/readme-guides**: README, tutorials, user guides
- **docs/api-docs**: API documentation, OpenAPI specs
- **config/env-setup**: Environment variables, local dev setup
- **config/build-tooling**: Webpack, Vite, tsconfig, build configs
- **config/dependencies**: package.json, Cargo.toml, requirements.txt
- **ops/ci-cd**: GitHub Actions, CI pipelines, automation
- **ops/deployment**: Docker, K8s, hosting, infrastructure
- **ops/monitoring**: Logging, alerting, metrics, observability

### Thinking Work
- **planning/brainstorming**: Exploring ideas, comparing options
- **planning/design-doc**: Writing technical specs, PRDs
- **planning/task-breakdown**: Breaking work into subtasks
- **explanation/code-understanding**: "How does X work?", code walkthroughs
- **explanation/concept-learning**: Learning new technologies, concepts
- **explanation/debug-investigation**: Diagnosing issues, finding root cause
- **architecture/system-design**: High-level system architecture
- **architecture/data-modeling**: Database schemas, data structures
- **architecture/api-design**: API contracts, protocols, interfaces

## Classification Rules

1. Use ONLY the user's first prompt to classify (ignore later context)
2. If a prompt spans multiple categories, pick the PRIMARY intent
3. For ambiguous prompts, prefer the more specific category
4. If truly unclear, use the closest match with confidence < 0.7
5. Non-English prompts: Classify based on detected intent, or "unknown" if unreadable

## Response Format

Return ONLY a JSON object (no markdown, no explanation):
{
  "classifications": [
    {
      "sessionId": "abc123",
      "category": "code/feature/new-component",
      "confidence": 0.92,
      "reasoning": "User asks to create a new React component"
    }
  ]
}

Confidence levels:
- 0.9-1.0: Very confident, clear intent
- 0.7-0.9: Confident, but some ambiguity
- 0.5-0.7: Uncertain, best guess
- <0.5: Very uncertain (consider "unknown")
```

### User Prompt Template

```
Classify these coding sessions:

{{#each sessions}}
---
Session ID: {{id}}
First Prompt: {{preview}}
Skills Used: {{skills_used}}
---
{{/each}}

Return JSON with classifications for all {{sessions.length}} sessions.
```

### Expected Response Format

```json
{
  "classifications": [
    {
      "sessionId": "01JMQW8V7J...",
      "category": "code/feature/new-component",
      "confidence": 0.92,
      "reasoning": "User asks to create a new React component for dashboard"
    },
    {
      "sessionId": "01JMQW9A2K...",
      "category": "thinking/explanation/debug-investigation",
      "confidence": 0.78,
      "reasoning": "User debugging a failing test, investigating root cause"
    }
  ]
}
```

---

## API Specification

### POST /api/classify

Trigger a new classification job.

**Request:**
```typescript
interface ClassifyRequest {
  /** Which sessions to classify */
  mode: 'unclassified' | 'all';
  /** Override default provider (optional) */
  provider?: 'claude-cli' | 'anthropic-api' | 'openai-compatible';
  /** Dry run - calculate cost without executing (optional) */
  dryRun?: boolean;
}
```

**Response (202 Accepted):**
```typescript
interface ClassifyResponse {
  /** Unique job identifier */
  jobId: string;
  /** Number of sessions to classify */
  totalSessions: number;
  /** Estimated cost in USD (based on token estimates) */
  estimatedCost: number;
  /** Estimated duration in seconds */
  estimatedDurationSecs: number;
  /** Status of the job */
  status: 'queued' | 'running';
}
```

**Response (409 Conflict):**
```typescript
interface ErrorResponse {
  error: string;
  details: string;
}
```

**Example:**
```bash
# Start classification of unclassified sessions
curl -X POST http://localhost:47892/api/classify \
  -H "Content-Type: application/json" \
  -d '{"mode": "unclassified"}'

# Response
{
  "jobId": "cls_01JMQWA3B5...",
  "totalSessions": 5865,
  "estimatedCost": 1.82,
  "estimatedDurationSecs": 480,
  "status": "running"
}
```

---

### POST /api/classify/cancel

Cancel a running classification job.

**Request:** (empty body)

**Response (200 OK):**
```typescript
interface CancelResponse {
  /** Job ID that was cancelled */
  jobId: string;
  /** Number of sessions classified before cancellation */
  classified: number;
  /** Job status after cancellation */
  status: 'cancelled';
}
```

**Response (404 Not Found):**
```typescript
interface ErrorResponse {
  error: string;
  details: string;  // "No classification job is currently running"
}
```

**Example:**
```bash
# Cancel running classification
curl -X POST http://localhost:47892/api/classify/cancel

# Response
{
  "jobId": "cls_01JMQWA3B5...",
  "classified": 1247,
  "status": "cancelled"
}
```

---

### GET /api/classify/status

Get current classification job status.

**Response:**
```typescript
interface ClassifyStatusResponse {
  /** Current status */
  status: 'idle' | 'running' | 'completed' | 'failed' | 'cancelled';
  /** Current job ID (if running or recently completed) */
  jobId?: string;
  /** Progress info (if running) */
  progress?: {
    classified: number;
    total: number;
    percentage: number;
    /** Estimated time remaining */
    eta: string;
    /** Current batch being processed */
    currentBatch?: string;
  };
  /** Last completed run info */
  lastRun?: {
    jobId: string;
    completedAt: string;
    duration: string;
    sessionsClassified: number;
    cost: number;
    errorCount: number;
  };
  /** Error info (if failed) */
  error?: {
    message: string;
    retryable: boolean;
  };
}
```

**Example:**
```bash
curl http://localhost:47892/api/classify/status

# Response (running)
{
  "status": "running",
  "jobId": "cls_01JMQWA3B5...",
  "progress": {
    "classified": 1247,
    "total": 5865,
    "percentage": 21.3,
    "eta": "6m 42s",
    "currentBatch": "Oct 15-18, 2025"
  },
  "lastRun": {
    "jobId": "cls_01JMQV9Z2A...",
    "completedAt": "2026-02-04T23:15:00Z",
    "duration": "8m 23s",
    "sessionsClassified": 5120,
    "cost": 1.58,
    "errorCount": 3
  }
}
```

---

### GET /api/classify/stream

SSE endpoint for real-time progress.

**Event Types:**

```typescript
// Progress update (every batch)
event: progress
data: { classified: number, total: number, percentage: number, eta: string }

// Batch completion info
event: batch
data: { batch: string, count: number, duration: string }

// Job completed
event: complete
data: { jobId: string, duration: string, cost: number, classified: number }

// Error occurred
event: error
data: { message: string, retrying: boolean, retryIn?: string }

// Job cancelled
event: cancelled
data: { jobId: string, classified: number }
```

**Example stream:**
```
event: progress
data: {"classified":50,"total":5865,"percentage":0.9,"eta":"9m 12s"}

event: batch
data: {"batch":"Jan 1-5, 2026","count":50,"duration":"5.2s"}

event: progress
data: {"classified":100,"total":5865,"percentage":1.7,"eta":"9m 5s"}

event: error
data: {"message":"Rate limit exceeded","retrying":true,"retryIn":"30s"}

event: progress
data: {"classified":150,"total":5865,"percentage":2.6,"eta":"8m 45s"}

...

event: complete
data: {"jobId":"cls_01JMQWA3B5...","duration":"8m 23s","cost":1.82,"classified":5865}
```

---

### PUT /api/settings/classification

Update classification provider settings.

**Request:**
```typescript
interface ClassificationSettingsRequest {
  /** Active provider */
  provider: 'claude-cli' | 'anthropic-api' | 'openai-compatible';
  /** Claude CLI settings */
  claudeCli?: {
    path: string;  // default: 'claude'
    model: string; // default: 'haiku'
  };
  /** Anthropic API settings */
  anthropicApi?: {
    apiKey: string;
    model: string; // default: 'claude-3-haiku-20240307'
  };
  /** OpenAI-compatible settings */
  openaiCompatible?: {
    endpoint: string;
    apiKey: string;
    model: string;
  };
}
```

**Response (200 OK):**
```typescript
interface ClassificationSettingsResponse {
  provider: string;
  status: 'valid' | 'unconfigured' | 'invalid';
  message?: string;
}
```

---

### GET /api/settings/classification

Get current classification provider settings.

**Response:**
```typescript
interface ClassificationSettingsResponse {
  /** Active provider */
  provider: 'claude-cli' | 'anthropic-api' | 'openai-compatible';
  /** Provider status (has it been tested?) */
  providerStatus: 'valid' | 'unconfigured' | 'invalid';
  /** Claude CLI config (path masked) */
  claudeCli?: {
    path: string;
    model: string;
    available: boolean;
  };
  /** Anthropic API config (key masked) */
  anthropicApi?: {
    apiKeySet: boolean;
    model: string;
  };
  /** OpenAI-compatible config (key masked) */
  openaiCompatible?: {
    endpoint: string;
    apiKeySet: boolean;
    model: string;
  };
}
```

---

## UI Mockups

### ClassificationStatus Card (for /system page)

```
┌──────────────────────────────────────────────────────────────┐
│  Classification                                    [Classify]│
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Status: Ready                                               │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ Sessions classified   5,120 / 5,865  (87.3%)           │ │
│  │ ████████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░│ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  Last run: Feb 4, 2026 11:15 PM                             │
│  Duration: 8m 23s  ·  Cost: $1.58  ·  Errors: 3            │
│                                                              │
│  Provider: Claude CLI (haiku)                    [Settings] │
└──────────────────────────────────────────────────────────────┘
```

When classification is running:

```
┌──────────────────────────────────────────────────────────────┐
│  Classification                                     [Cancel] │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Status: Classifying...                        ETA: 6m 42s  │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ Progress   1,247 / 5,865  (21.3%)                      │ │
│  │ █████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  Current batch: Oct 15-18, 2025 (50 sessions)              │
│  Rate: 2.5 sessions/sec                                     │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

---

### ClassificationProgress Modal

```
┌──────────────────────────────────────────────────────────────┐
│                    Classification in Progress                │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│                        ○ ○ ○                                │
│                   (spinner animation)                        │
│                                                              │
│         Classifying 5,865 sessions with Claude haiku        │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ █████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│ │
│  │            1,247 / 5,865 (21.3%)                       │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  Time elapsed: 1m 41s         Estimated remaining: 6m 42s   │
│  Cost so far: $0.39           Estimated total: $1.82        │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ Recent batches:                                      │   │
│  │   ✓ Oct 15-18, 2025 (50 sessions) - 5.2s            │   │
│  │   ✓ Oct 19-22, 2025 (50 sessions) - 4.8s            │   │
│  │   → Oct 23-26, 2025 (50 sessions) - processing...   │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│                                          [Run in Background] │
│                                          [Cancel]            │
└──────────────────────────────────────────────────────────────┘
```

---

### ProviderSettings Form

```
┌──────────────────────────────────────────────────────────────┐
│  Classification Provider                                     │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Provider                                                    │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ ◉ Claude CLI (uses your existing subscription)       │   │
│  │ ○ Anthropic API (bring your own key)                 │   │
│  │ ○ OpenAI-compatible (custom endpoint)                │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ── Claude CLI Settings ──────────────────────────────────  │
│                                                              │
│  CLI Path          [claude                              ]   │
│  Model             [haiku                          ▼    ]   │
│                                                              │
│  Status: ✓ Claude CLI detected at /usr/local/bin/claude    │
│                                                              │
│                                                              │
│                            [Test Connection]    [Save]      │
└──────────────────────────────────────────────────────────────┘
```

When "Anthropic API" is selected:

```
│  ── Anthropic API Settings ───────────────────────────────  │
│                                                              │
│  API Key           [sk-ant-•••••••••••••••••••••••••   ]   │
│  Model             [claude-3-haiku-20240307        ▼    ]   │
│                                                              │
│  Status: ⚠ API key not configured                          │
```

When "OpenAI-compatible" is selected:

```
│  ── OpenAI-compatible Settings ───────────────────────────  │
│                                                              │
│  Endpoint          [http://localhost:11434/v1           ]   │
│  API Key           [(optional for local models)         ]   │
│  Model             [llama3                              ]   │
│                                                              │
│  Status: ✓ Connected to Ollama                             │
```

---

## Rust Types

### Core Types (`crates/db/src/classification.rs`)

```rust
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Classification taxonomy levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "lowercase")]
pub enum CategoryL1 {
    Code,
    Support,
    Thinking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "lowercase")]
pub enum CategoryL2 {
    // Code
    Feature,
    Bugfix,
    Refactor,
    Testing,
    // Support
    Docs,
    Config,
    Ops,
    // Thinking
    Planning,
    Explanation,
    Architecture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "kebab-case")]
pub enum CategoryL3 {
    // Feature
    NewComponent,
    AddFunctionality,
    Integration,
    // Bugfix
    ErrorFix,
    LogicFix,
    PerformanceFix,
    // Refactor
    Cleanup,
    PatternMigration,
    DependencyUpdate,
    // Testing
    UnitTests,
    IntegrationTests,
    TestFixes,
    // Docs
    CodeComments,
    ReadmeGuides,
    ApiDocs,
    // Config
    EnvSetup,
    BuildTooling,
    Dependencies,
    // Ops
    CiCd,
    Deployment,
    Monitoring,
    // Planning
    Brainstorming,
    DesignDoc,
    TaskBreakdown,
    // Explanation
    CodeUnderstanding,
    ConceptLearning,
    DebugInvestigation,
    // Architecture
    SystemDesign,
    DataModeling,
    ApiDesign,
}

/// A single classification result from the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub session_id: String,
    pub category_l1: CategoryL1,
    pub category_l2: CategoryL2,
    pub category_l3: CategoryL3,
    pub confidence: f32,
    pub reasoning: String,
}

/// LLM response for batch classification.
#[derive(Debug, Clone, Deserialize)]
pub struct ClassificationResponse {
    pub classifications: Vec<ClassificationResult>,
}

/// Input for classification (session ID + preview).
#[derive(Debug, Clone)]
pub struct ClassificationInput {
    pub session_id: String,
    pub preview: String,
    pub skills_used: Vec<String>,
}
```

### Job Types (`crates/db/src/classification.rs`)

```rust
/// Classification job status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "lowercase")]
pub enum ClassificationJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Classification job record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationJob {
    pub id: String,
    pub status: ClassificationJobStatus,
    pub total_sessions: i64,
    pub classified_sessions: i64,
    pub error_count: i64,
    pub estimated_cost: f64,
    pub actual_cost: Option<f64>,
    pub provider: String,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub error_message: Option<String>,
}
```

### Provider Types (`crates/db/src/settings.rs`)

```rust
/// LLM provider for classification.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ClassificationProvider {
    ClaudeCli {
        path: String,
        model: String,
    },
    AnthropicApi {
        api_key: String,
        model: String,
    },
    OpenAiCompatible {
        endpoint: String,
        api_key: Option<String>,
        model: String,
    },
}

impl Default for ClassificationProvider {
    fn default() -> Self {
        Self::ClaudeCli {
            path: "claude".to_string(),
            model: "haiku".to_string(),
        }
    }
}
```

### State Types (`crates/server/src/classify_state.rs`)

```rust
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::RwLock;

/// Status of the current classification job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ClassifyStatus {
    Idle = 0,
    Running = 1,
    Completed = 2,
    Failed = 3,
    Cancelled = 4,
}

/// Lock-free state for classification progress tracking.
pub struct ClassifyState {
    status: AtomicU8,
    total: AtomicU64,
    classified: AtomicU64,
    errors: AtomicU64,
    cancel_requested: AtomicBool,
    job_id: RwLock<Option<String>>,
    current_batch: RwLock<Option<String>>,
    error_message: RwLock<Option<String>>,
    started_at: AtomicU64,
    cost_cents: AtomicU64,
}

impl ClassifyState {
    pub fn new() -> Self {
        Self {
            status: AtomicU8::new(ClassifyStatus::Idle as u8),
            total: AtomicU64::new(0),
            classified: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            cancel_requested: AtomicBool::new(false),
            job_id: RwLock::new(None),
            current_batch: RwLock::new(None),
            error_message: RwLock::new(None),
            started_at: AtomicU64::new(0),
            cost_cents: AtomicU64::new(0),
        }
    }

    pub fn status(&self) -> ClassifyStatus { /* ... */ }
    pub fn set_running(&self, job_id: String, total: u64) { /* ... */ }
    pub fn increment_classified(&self, count: u64) { /* ... */ }
    pub fn set_current_batch(&self, batch: String) { /* ... */ }
    pub fn set_completed(&self, cost_cents: u64) { /* ... */ }
    pub fn set_failed(&self, message: String) { /* ... */ }
    pub fn set_cancelled(&self) { /* ... */ }
    pub fn is_cancel_requested(&self) -> bool { /* ... */ }
    pub fn request_cancel(&self) { /* ... */ }
    pub fn reset(&self) { /* ... */ }
}
```

---

## React Components

### Component Tree

```
src/
├── components/
│   ├── ClassificationStatus.tsx      # Card for /system page
│   ├── ClassificationProgress.tsx    # Modal during classification
│   └── ProviderSettings.tsx          # Provider config form
├── hooks/
│   └── use-classification.ts         # Classification hook
└── types/generated/
    ├── CategoryL1.ts                 # ts-rs generated
    ├── CategoryL2.ts
    ├── CategoryL3.ts
    ├── ClassificationJobStatus.ts
    └── ClassificationProvider.ts
```

### Hook Interface (`use-classification.ts`)

```typescript
import { useState, useCallback, useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'

export interface ClassificationProgress {
  classified: number
  total: number
  percentage: number
  eta: string
  currentBatch?: string
}

export interface ClassificationLastRun {
  jobId: string
  completedAt: string
  duration: string
  sessionsClassified: number
  cost: number
  errorCount: number
}

export interface UseClassificationResult {
  // Status
  status: 'idle' | 'running' | 'completed' | 'failed' | 'cancelled'
  jobId: string | null
  progress: ClassificationProgress | null
  lastRun: ClassificationLastRun | null
  error: string | null

  // Actions
  startClassification: (mode: 'unclassified' | 'all') => Promise<boolean>
  cancelClassification: () => Promise<boolean>

  // SSE
  isStreaming: boolean
  connectStream: () => void
  disconnectStream: () => void
}

export function useClassification(): UseClassificationResult {
  // Implementation connects to:
  // - GET /api/classify/status (polling)
  // - POST /api/classify (trigger)
  // - GET /api/classify/stream (SSE)
  // - POST /api/classify/cancel (cancel)
}
```

### ClassificationStatus Component

```typescript
interface ClassificationStatusProps {
  onConfigure: () => void  // Opens provider settings
}

export function ClassificationStatus({ onConfigure }: ClassificationStatusProps) {
  const {
    status,
    progress,
    lastRun,
    startClassification,
    cancelClassification,
    connectStream,
  } = useClassification()

  // Renders:
  // - Progress bar (sessions classified / total)
  // - Last run info
  // - Classify/Cancel button
  // - Settings link
}
```

### ClassificationProgress Component

```typescript
interface ClassificationProgressProps {
  isOpen: boolean
  onClose: () => void
  onRunInBackground: () => void
}

export function ClassificationProgress({
  isOpen,
  onClose,
  onRunInBackground,
}: ClassificationProgressProps) {
  const { progress, status, error, cancelClassification } = useClassification()

  // Renders:
  // - Modal overlay
  // - Spinner animation
  // - Progress bar with percentage
  // - Time elapsed / remaining
  // - Cost info
  // - Recent batches list
  // - Run in Background / Cancel buttons
}
```

### ProviderSettings Component

```typescript
interface ProviderSettingsProps {
  isOpen: boolean
  onClose: () => void
  onSave: () => void
}

export function ProviderSettings({
  isOpen,
  onClose,
  onSave,
}: ProviderSettingsProps) {
  // State for provider selection and config
  const [provider, setProvider] = useState<'claude-cli' | 'anthropic-api' | 'openai-compatible'>('claude-cli')
  const [claudeCliPath, setClaudeCliPath] = useState('claude')
  const [claudeCliModel, setClaudeCliModel] = useState('haiku')
  // ... other provider configs

  // Renders:
  // - Radio group for provider selection
  // - Conditional config fields per provider
  // - Test Connection button
  // - Save button
}
```

---

## Database Migration

Add to `crates/db/src/migrations.rs`:

```rust
// Migration 13: Classification settings table
r#"
CREATE TABLE IF NOT EXISTS classification_settings (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    provider TEXT NOT NULL DEFAULT 'claude-cli',
    claude_cli_path TEXT NOT NULL DEFAULT 'claude',
    claude_cli_model TEXT NOT NULL DEFAULT 'haiku',
    anthropic_api_key TEXT,
    anthropic_model TEXT DEFAULT 'claude-3-haiku-20240307',
    openai_endpoint TEXT,
    openai_api_key TEXT,
    openai_model TEXT,
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
"#,
r#"INSERT OR IGNORE INTO classification_settings (id) VALUES (1);"#,
```

Note: `classification_jobs` table is created in Phase 1.

---

## Testing Strategy

### Unit Tests

| Test | Location | Coverage |
|------|----------|----------|
| Prompt rendering | `crates/db/src/classification.rs` | Template variables, escaping |
| Response parsing | `crates/db/src/classification.rs` | Valid JSON, malformed JSON, partial results |
| Category validation | `crates/db/src/classification.rs` | All 30 categories parse correctly |
| Provider config | `crates/db/src/settings.rs` | Serialization, defaults |

### Integration Tests

| Test | Location | Coverage |
|------|----------|----------|
| POST /api/classify | `crates/server/src/routes/classify.rs` | 202 response, job creation |
| GET /api/classify/status | `crates/server/src/routes/classify.rs` | Idle, running, completed states |
| GET /api/classify/stream | `crates/server/src/routes/classify.rs` | SSE event format, stream completion |
| Concurrent job prevention | `crates/server/src/routes/classify.rs` | 409 when job running |
| Provider settings CRUD | `crates/server/src/routes/settings.rs` | GET/PUT round-trip |

### Component Tests

| Test | Location | Coverage |
|------|----------|----------|
| ClassificationStatus | `src/components/ClassificationStatus.test.tsx` | Idle, running, completed states |
| ClassificationProgress | `src/components/ClassificationProgress.test.tsx` | Progress updates, cancel |
| ProviderSettings | `src/components/ProviderSettings.test.tsx` | Provider switching, validation |
| useClassification | `src/hooks/use-classification.test.ts` | SSE handling, state transitions |

### Manual Tests

- [ ] Claude CLI classification works with real sessions
- [ ] SSE stream updates in real-time
- [ ] Cancel stops classification gracefully
- [ ] Provider switching persists correctly
- [ ] Cost estimates are reasonable

---

## Acceptance Criteria

### Must Pass

- [ ] **AC-2.1:** Classification prompt returns valid JSON for batch of 5 sessions
- [ ] **AC-2.2:** POST /api/classify returns 202 and creates job record
- [ ] **AC-2.3:** GET /api/classify/status returns correct progress during classification
- [ ] **AC-2.4:** SSE stream emits progress, batch, complete events
- [ ] **AC-2.5:** SSE stream emits error events with retry info
- [ ] **AC-2.6:** Concurrent POST /api/classify returns 409 Conflict
- [ ] **AC-2.6b:** POST /api/classify/cancel stops job and returns classified count
- [ ] **AC-2.7:** Classification updates session category_l1/l2/l3 columns
- [ ] **AC-2.8:** Provider settings persist across server restarts
- [ ] **AC-2.9:** Claude CLI provider works with default settings
- [ ] **AC-2.10:** UI shows real-time progress during classification

### Nice to Have

- [ ] **AC-2.11:** Anthropic API provider works with valid key
- [ ] **AC-2.12:** OpenAI-compatible provider works with Ollama
- [ ] **AC-2.13:** Cost tracking matches actual API usage
- [ ] **AC-2.14:** Classification handles non-English prompts gracefully

---

## Estimated Effort

| Task | Effort | Dependencies |
|------|--------|--------------|
| 2.1 Prompt design | 4 hours | None |
| 2.2 POST /api/classify | 3 hours | Phase 1 |
| 2.3 GET /api/classify/status | 2 hours | 2.2 |
| 2.3b POST /api/classify/cancel | 1 hour | 2.2 |
| 2.4 SSE stream | 4 hours | 2.2 |
| 2.5 Provider config | 6 hours | 2.2 |
| 2.6 Classification UI | 8 hours | 2.2, 2.3, 2.3b, 2.4 |
| **Total** | **28 hours** | |

---

## Related Documents

- Master design: `../2026-02-05-theme4-chat-insights-design.md`
- Phase 1 (Foundation): `phase1-foundation.md`
- Phase 3 (System Page): `phase3-system-page.md`
- Progress tracker: `PROGRESS.md`
