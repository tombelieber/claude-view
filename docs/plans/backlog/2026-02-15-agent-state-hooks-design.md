---
status: approved
date: 2026-02-15
supersedes: 2026-02-15-intelligent-session-states.md
---

# Agent State via Hooks + JSONL Hybrid

## Problem

Mission Control tried to classify session states using:

1. **JSONL file parsing with structural heuristics (Tier 1)** -- works but limited to ~70% of cases
2. **Claude CLI spawning for AI classification (Tier 2)** -- total failure

Tier 2 failed catastrophically: with 40+ JSONL files discovered simultaneously on startup, every Paused session triggered a concurrent `claude -p` process. This caused recursive spawning loops (server itself runs inside a Claude Code session), 40 concurrent processes hitting rate limits, timeouts, and an effective infinite retry loop. The approach was fundamentally unworkable.

The 3-state model (Working/Paused/Done) was also too limited -- organized around Claude's lifecycle, not the operator's needs. An operator doesn't care if Claude is "paused" -- they care whether it needs their attention, is working autonomously, or has delivered results.

## New Approach: Hooks + JSONL Hybrid

**Claude Code hooks** fire shell commands at precise moments (tool use, stop, permission request, etc.). Each hook sends an HTTP POST to our server with ground-truth state data. No JSONL guessing, no AI classification, no recursive spawning.

**JSONL watching** stays for continuous monitoring: token/cost tracking, streaming detection, context window usage, git branch, etc.

This isn't just a session monitor -- it's a **remote agent command center**. The end-game:
- Operator connects from mobile (via Tailscale/Cloudflare tunnel)
- Sees all agents: which need attention, which are working, which delivered results
- Can respond to questions, approve plans, grant permissions from phone
- Agents work 24/7 in background
- Like OpenClaw but zero setup -- just Claude Code + this dashboard
- Next-gen workflow builder where the workflow IS the conversation

## Core State Model

### Three Fixed Groups (the operator's mental model)

| Group | Meaning | Accent |
|-------|---------|--------|
| **Needs You** | Agent blocked until human acts | Amber/Red |
| **Autonomous** | Agent working, no action needed | Green |
| **Delivered** | Output ready, no urgency | Blue/Gray |

### Open Sub-States Within Each Group

Sub-states are strings, not a closed enum. New states can be added without schema changes.

**v1 sub-states** (what hooks + JSONL can detect today):

```
Needs You:
  awaiting_input      "Asked you a question"           (PostToolUse -> AskUserQuestion hook)
  awaiting_approval   "Plan ready for review"           (PostToolUse -> ExitPlanMode hook)
  needs_permission    "Needs permission to proceed"     (PermissionRequest hook)
  error               "Tool failed, needs attention"    (PostToolUseFailure hook)
  idle                "Waiting for your next prompt"     (Stop hook + no new activity)

Autonomous:
  thinking            "Generating response..."           (JSONL: assistant streaming, no end_turn)
  acting              "Running Bash / Editing file"      (JSONL: tool_use blocks or PreToolUse hook)
  delegating          "Running subagent"                 (SubagentStart hook)

Delivered:
  task_complete       "Finished: added auth feature"     (TaskCompleted hook)
  session_ended       "Session closed"                   (SessionEnd hook / process exit)
```

**End-game sub-states** (future, when more signals available):

```
Needs You:            + awaiting_review, awaiting_resources, budget_exceeded, conflict_resolution
Autonomous:           + queued, retrying, learning, waiting_for_agent
Delivered:            + succeeded, failed, cancelled, archived
```

## Two-Layer Architecture (Core + Domain)

```
+---------------------------------------------+
|  Domain Layer (pluggable, future)            |
|  "PR ready for review" / "Design mockup"     |
+---------------------------------------------+
|  Core Layer (universal, stable)              |
|  state: awaiting_approval                    |
|  group: needs_you                            |
|  context: { tool: "ExitPlanMode", ... }      |
+---------------------------------------------+
```

- **Core layer**: Fixed groups + open sub-states + raw context. This is the protocol -- never breaks.
- **Domain layer**: Optional interpreter that reads context and overrides labels/icons/urgency for specific workflows (SDLC, creative, research).
- **Example**: Core says `task_complete` with `context.tools_used = ["git commit"]`. SDLC domain renders "Committed to feature/auth". Creative domain might render "Draft exported".

Properties this gives us:

| Property | How |
|----------|-----|
| **Extensibility** | Domain layers are plugins. Adding SDLC labels never touches core enum |
| **Correctness** | Core states from ground-truth signals. Domain only affects display |
| **Robustness** | If domain plugin missing/buggy, core label still renders. Three fallback layers: hook -> JSONL -> generic |
| **Stability** | Core enum is frozen contract. All evolution happens above it |

## Data Structures

### Backend (Rust)

```rust
use serde::{Serialize, Deserialize};

/// The universal agent state -- core protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentState {
    /// Which UI group this belongs to (fixed, never changes)
    pub group: AgentStateGroup,
    /// Sub-state within group (open string -- new states added freely)
    pub state: String,
    /// Human-readable label (domain layer can override)
    pub label: String,
    /// How confident we are in this classification
    pub confidence: f32,
    /// How this state was determined
    pub source: SignalSource,
    /// Raw context for domain layers to interpret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStateGroup {
    NeedsYou,
    Autonomous,
    Delivered,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalSource {
    Hook,
    Jsonl,
    Fallback,
}
```

### Frontend (TypeScript)

```typescript
type AgentStateGroup = 'needs_you' | 'autonomous' | 'delivered'

interface AgentState {
  group: AgentStateGroup
  state: string           // open string -- v1 states listed above, more added over time
  label: string
  confidence: number
  source: 'hook' | 'jsonl' | 'fallback'
  context?: Record<string, unknown>
}

// Required imports from lucide-react:
// import { MessageCircle, FileCheck, Shield, AlertTriangle, Clock,
//          Sparkles, Terminal, GitBranch, CheckCircle, Power,
//          Bell, Loader, Archive } from 'lucide-react'

// v1 known states (for icon/color mapping, but unknown states render with generic style)
const KNOWN_STATES = {
  // Needs You
  awaiting_input: { icon: MessageCircle, color: 'amber' },
  awaiting_approval: { icon: FileCheck, color: 'amber' },
  needs_permission: { icon: Shield, color: 'red' },
  error: { icon: AlertTriangle, color: 'red' },
  idle: { icon: Clock, color: 'gray' },
  // Autonomous
  thinking: { icon: Sparkles, color: 'green' },
  acting: { icon: Terminal, color: 'green' },
  delegating: { icon: GitBranch, color: 'green' },
  // Delivered
  task_complete: { icon: CheckCircle, color: 'blue' },
  session_ended: { icon: Power, color: 'gray' },
} as const

// Unknown states get a generic icon/color for their group
const GROUP_DEFAULTS = {
  needs_you: { icon: Bell, color: 'amber' },
  autonomous: { icon: Loader, color: 'green' },
  delivered: { icon: Archive, color: 'gray' },
} as const
```

## Signal Architecture

Two signal paths feed one state resolver:

```
Claude Code Hooks                    JSONL File Watcher
     |                                      |
     |  HTTP POST /api/live/hook             |  File change -> tail parse
     |  {session_id, event, context}         |  {line_type, tools, stop_reason, tokens}
     |                                      |
     +----------+            +--------------+
                v            v
           +---------------------+
           |   State Resolver    |
           |                     |
           |  Hook signal wins   |
           |  if available.      |
           |  JSONL fills gaps.  |
           |  Fallback if both   |
           |  are silent.        |
           +---------+-----------+
                     |
                     v
              AgentState {
                group, state,
                label, confidence,
                source, context
              }
                     |
                     v
              SSE broadcast -> Frontend
```

**Priority rule**: Hook signals override JSONL-derived states for the same session. A `PostToolUse("AskUserQuestion")` hook immediately sets `awaiting_input`, even if JSONL hasn't caught up.

**Staleness**: Each signal carries a timestamp. Expiration is **state-category-aware**:

| Category | States | Expiration |
|----------|--------|------------|
| **Terminal** | `task_complete`, `session_ended` | Never expire — these are final states |
| **Blocking** | `awaiting_input`, `awaiting_approval`, `needs_permission`, `error`, `idle` | Persist until next hook event (user may take minutes to respond) |
| **Transient** | `thinking`, `acting`, `delegating` | Expire after 60s with no new signal — degrade to JSONL fallback |

A blanket 60s timeout would incorrectly expire blocking states (user takes 3 min to answer a question) and terminal states (session ended = permanent).

**Token/cost/context-window data**: Always from JSONL (hooks don't carry usage metrics). Displayed as card metadata, independent of state model.

## Hook Configuration

### Recommended Approach: Forwarding Script

The user creates a small script that every hook calls. The script reads the standard hook JSON payload from stdin and forwards it to the Mission Control server:

```bash
#!/bin/bash
# ~/.claude/hooks/notify-mission-control.sh
# Reads hook payload from stdin, forwards to Mission Control server.
# Runs curl in foreground with tight timeout (1s) so errors are visible.
# Do NOT background (&) — it suppresses exit codes and hides failures.
curl -s --max-time 1 -X POST "http://localhost:47892/api/live/hook" \
  -H "Content-Type: application/json" \
  -d @- >/dev/null 2>&1
# Exit 0 even on curl failure — don't block Claude Code if server is down
exit 0
```

This approach is simple: every hook calls the same script, and the server parses the standard hook JSON payload (which already contains session_id, hook_event_name, tool_name, etc.).

### Claude Code Settings

The user adds this to their Claude Code settings (`~/.claude/settings.json` or project `.claude/settings.json`):

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "AskUserQuestion",
        "hooks": [{
          "type": "command",
          "command": "~/.claude/hooks/notify-mission-control.sh"
        }]
      },
      {
        "matcher": "ExitPlanMode",
        "hooks": [{
          "type": "command",
          "command": "~/.claude/hooks/notify-mission-control.sh"
        }]
      }
    ],
    "PostToolUseFailure": [
      {
        "hooks": [{
          "type": "command",
          "command": "~/.claude/hooks/notify-mission-control.sh"
        }]
      }
    ],
    "Stop": [
      {
        "hooks": [{
          "type": "command",
          "command": "~/.claude/hooks/notify-mission-control.sh"
        }]
      }
    ],
    "PermissionRequest": [
      {
        "hooks": [{
          "type": "command",
          "command": "~/.claude/hooks/notify-mission-control.sh"
        }]
      }
    ],
    "SubagentStart": [
      {
        "hooks": [{
          "type": "command",
          "command": "~/.claude/hooks/notify-mission-control.sh"
        }]
      }
    ],
    "SubagentStop": [
      {
        "hooks": [{
          "type": "command",
          "command": "~/.claude/hooks/notify-mission-control.sh"
        }]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [{
          "type": "command",
          "command": "~/.claude/hooks/notify-mission-control.sh"
        }]
      }
    ],
    "TaskCompleted": [
      {
        "hooks": [{
          "type": "command",
          "command": "~/.claude/hooks/notify-mission-control.sh"
        }]
      }
    ]
  }
}
```

> **NOTE:** Matchers use string regex (e.g., `"matcher": "AskUserQuestion"`), NOT JSON objects. Hook payloads arrive via stdin as JSON with common fields: `session_id`, `hook_event_name`, `cwd`, `transcript_path`, `permission_mode`.

## Hook Endpoint

Backend receives hook events and maps them to AgentState:

```rust
// POST /api/live/hook
// Receives the raw Claude Code hook payload (passed through from stdin)

// NOTE: No serde rename_all here -- Claude Code sends snake_case field names
// which already match Rust's default naming convention.
#[derive(Debug, Deserialize)]
pub struct HookPayload {
    pub session_id: String,
    pub hook_event_name: String,
    // Common fields (sent by ALL hook events)
    pub cwd: Option<String>,
    pub transcript_path: Option<String>,
    pub permission_mode: Option<String>,
    // Tool-specific fields (PostToolUse, PostToolUseFailure, PermissionRequest)
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub tool_response: Option<serde_json::Value>,
    pub tool_use_id: Option<String>,
    // Error field (PostToolUseFailure)
    pub error: Option<String>,
    pub is_interrupt: Option<bool>,
    // Subagent fields (SubagentStart, SubagentStop)
    pub agent_type: Option<String>,
    pub agent_id: Option<String>,
    // Session fields (SessionEnd)
    pub reason: Option<String>,
    // Task fields (TaskCompleted)
    pub task_id: Option<String>,
    pub task_subject: Option<String>,
    pub task_description: Option<String>,
    // Additional optional fields from Claude Code docs (safe to ignore if absent)
    pub stop_hook_active: Option<bool>,
    pub agent_transcript_path: Option<String>,
    pub teammate_name: Option<String>,
    pub team_name: Option<String>,
    pub source: Option<String>,
}

fn resolve_state_from_hook(payload: &HookPayload) -> AgentState {
    match payload.hook_event_name.as_str() {
        "PostToolUse" => match payload.tool_name.as_deref() {
            Some("AskUserQuestion") => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "awaiting_input".into(),
                label: "Asked you a question".into(),
                confidence: 0.99,
                source: SignalSource::Hook,
                context: payload.tool_input.clone(),
            },
            Some("ExitPlanMode") => AgentState {
                group: AgentStateGroup::NeedsYou,
                state: "awaiting_approval".into(),
                label: "Plan ready for review".into(),
                confidence: 0.99,
                source: SignalSource::Hook,
                context: None,
            },
            _ => AgentState {
                group: AgentStateGroup::Autonomous,
                state: "acting".into(),
                label: format!("Used {}", payload.tool_name.as_deref().unwrap_or("tool")),
                confidence: 0.9,
                source: SignalSource::Hook,
                context: None,
            },
        },
        "PostToolUseFailure" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "error".into(),
            label: format!("Failed: {}", payload.tool_name.as_deref().unwrap_or("tool")),
            confidence: 0.95,
            source: SignalSource::Hook,
            context: payload.error.as_ref().map(|e| serde_json::json!({"error": e})),
        },
        "PermissionRequest" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "needs_permission".into(),
            label: format!("Needs permission: {}", payload.tool_name.as_deref().unwrap_or("tool")),
            confidence: 0.99,
            source: SignalSource::Hook,
            context: None,
        },
        "Stop" => AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "idle".into(),
            label: "Waiting for your next prompt".into(),
            confidence: 0.8,
            source: SignalSource::Hook,
            context: None,
        },
        "SubagentStart" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "delegating".into(),
            label: format!("Running {} subagent", payload.agent_type.as_deref().unwrap_or("unknown")),
            confidence: 0.99,
            source: SignalSource::Hook,
            context: None,
        },
        "SubagentStop" => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: format!("Subagent {} finished", payload.agent_type.as_deref().unwrap_or("unknown")),
            confidence: 0.9,
            source: SignalSource::Hook,
            context: None,
        },
        "SessionEnd" => AgentState {
            group: AgentStateGroup::Delivered,
            state: "session_ended".into(),
            label: "Session closed".into(),
            confidence: 0.99,
            source: SignalSource::Hook,
            context: None,
        },
        "TaskCompleted" => AgentState {
            group: AgentStateGroup::Delivered,
            state: "task_complete".into(),
            label: payload.task_subject.clone().unwrap_or_else(|| "Task completed".into()),
            confidence: 0.99,
            source: SignalSource::Hook,
            context: None,
        },
        _ => AgentState {
            group: AgentStateGroup::Autonomous,
            state: "acting".into(),
            label: format!("Event: {}", payload.hook_event_name),
            confidence: 0.5,
            source: SignalSource::Hook,
            context: None,
        },
    }
}
```

## State Resolver: Merging Hook + JSONL Signals

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};
use super::state::{AgentState, AgentStateGroup, SignalSource};

/// Resolves the current AgentState for a session by merging hook and JSONL signals.
/// Hook signals take priority over JSONL-derived states.
///
/// Thread-safe: all internal state is behind Arc<RwLock<>> because the resolver
/// is accessed concurrently from: (1) hook POST endpoint, (2) JSONL watcher task,
/// (3) process detector task, (4) SSE handler reads.
///
/// NOTE: Consider integrating these fields directly into LiveSessionManager
/// (which already owns Arc<RwLock<HashMap<>>> for sessions and accumulators)
/// instead of creating a standalone struct, to avoid dual-source-of-truth.
/// If standalone, store as Arc<StateResolver> in AppState.
#[derive(Clone)]
pub struct StateResolver {
    /// Most recent hook-derived state per session
    hook_states: Arc<RwLock<HashMap<String, (AgentState, Instant)>>>,
    /// JSONL-derived state (from existing derive_status + structural_classify)
    jsonl_states: Arc<RwLock<HashMap<String, AgentState>>>,
}

impl StateResolver {
    pub fn new() -> Self {
        Self {
            hook_states: Arc::new(RwLock::new(HashMap::new())),
            jsonl_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Called when a hook event arrives (from POST /api/live/hook handler)
    pub async fn update_from_hook(&self, session_id: &str, state: AgentState) {
        self.hook_states.write().await
            .insert(session_id.to_string(), (state, Instant::now()));
    }

    /// Called when JSONL processing produces a new state
    pub async fn update_from_jsonl(&self, session_id: &str, state: AgentState) {
        self.jsonl_states.write().await
            .insert(session_id.to_string(), state);
    }

    /// Get resolved state: hook wins if fresh, else JSONL, else fallback.
    ///
    /// Uses **state-category-aware expiration**:
    /// - Terminal states (task_complete, session_ended): never expire
    /// - Blocking states (awaiting_input, awaiting_approval, needs_permission, error, idle): persist until next hook
    /// - Transient states (thinking, acting, delegating): expire after 60s
    pub async fn resolve(&self, session_id: &str) -> AgentState {
        if let Some((hook_state, timestamp)) = self.hook_states.read().await.get(session_id) {
            let dominated = match Self::state_category(&hook_state.state) {
                StateCategory::Terminal => false,  // never expire
                StateCategory::Blocking => false,  // persist until next hook replaces it
                StateCategory::Transient => timestamp.elapsed() > Duration::from_secs(60),
            };
            if !dominated {
                return hook_state.clone();
            }
        }

        // JSONL-derived state as baseline
        if let Some(jsonl_state) = self.jsonl_states.read().await.get(session_id) {
            return jsonl_state.clone();
        }

        // Fallback
        AgentState {
            group: AgentStateGroup::Autonomous,
            state: "unknown".into(),
            label: "Status unavailable".into(),
            confidence: 0.0,
            source: SignalSource::Fallback,
            context: None,
        }
    }

    /// Remove stale hook states (called periodically from cleanup task).
    /// Only removes **transient** states that exceed max_age.
    /// Terminal and blocking states are never cleaned up by time alone.
    pub async fn cleanup_stale(&self, max_age: Duration) {
        let mut states = self.hook_states.write().await;
        states.retain(|_, (state, ts)| {
            match Self::state_category(&state.state) {
                StateCategory::Terminal => true,   // never remove
                StateCategory::Blocking => true,   // never remove by time
                StateCategory::Transient => ts.elapsed() < max_age,
            }
        });
    }

    /// Classify a state string into its expiration category.
    fn state_category(state: &str) -> StateCategory {
        match state {
            "task_complete" | "session_ended" => StateCategory::Terminal,
            "awaiting_input" | "awaiting_approval" | "needs_permission" | "error" | "idle"
                => StateCategory::Blocking,
            _ => StateCategory::Transient,  // thinking, acting, delegating, unknown
        }
    }
}

/// How a state expires. Used by StateResolver for category-aware staleness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateCategory {
    /// Final states that never expire (task_complete, session_ended)
    Terminal,
    /// States where the agent is blocked on human action — persist until next hook
    Blocking,
    /// Active work states that expire after 60s of silence
    Transient,
}
```

## LiveSession Struct Change

The `LiveSession` struct in `state.rs` (line 31-72) must replace `pause_classification` with `agent_state`:

```rust
// In state.rs LiveSession struct:

// REMOVE this field (line 45):
// pub pause_classification: Option<PauseClassification>,

// ADD this field:
pub agent_state: AgentState,
```

**Default value** for sessions discovered before any hook fires:

```rust
// In SessionAccumulator / LiveSession::new() / wherever sessions are first created:
agent_state: AgentState {
    group: AgentStateGroup::Autonomous,
    state: "thinking".into(),
    label: "Discovered...".into(),
    confidence: 0.3,
    source: SignalSource::Fallback,
    context: None,
},
```

Also update `SessionAccumulator` in `manager.rs` (line 61) — replace `pause_classification: Option<PauseClassification>` with `agent_state: AgentState`.

## JSONL to AgentState Mapping

The existing `derive_status()` + `structural_classify()` logic maps to AgentState:

```rust
/// Convert current JSONL-derived status into an AgentState.
/// This is the JSONL fallback when hooks aren't configured.
fn jsonl_to_agent_state(
    status: &SessionStatus,  // Working/Paused/Done from derive_status()
    classification: Option<&PauseClassification>,  // from structural_classify()
    last_line: Option<&LiveLine>,
) -> AgentState {
    match status {
        SessionStatus::Working => {
            let (state, label) = if let Some(ll) = last_line {
                if let Some(last_tool) = ll.tool_names.last() {
                    ("acting".into(), format!("Running {}", last_tool))
                } else {
                    ("thinking".into(), "Generating response...".into())
                }
            } else {
                ("thinking".into(), "Working...".into())
            };
            AgentState {
                group: AgentStateGroup::Autonomous,
                state,
                label,
                confidence: 0.7,
                source: SignalSource::Jsonl,
                context: None,
            }
        }
        SessionStatus::Paused => {
            if let Some(c) = classification {
                let (group, state) = match c.reason {
                    PauseReason::NeedsInput => (AgentStateGroup::NeedsYou, "awaiting_input"),
                    PauseReason::TaskComplete => (AgentStateGroup::Delivered, "task_complete"),
                    PauseReason::MidWork => (AgentStateGroup::NeedsYou, "idle"),
                    PauseReason::Error => (AgentStateGroup::NeedsYou, "error"),
                };
                AgentState {
                    group,
                    state: state.into(),
                    label: c.label.clone(),
                    confidence: c.confidence,
                    source: SignalSource::Jsonl,
                    context: None,
                }
            } else {
                AgentState {
                    group: AgentStateGroup::NeedsYou,
                    state: "idle".into(),
                    label: "Session paused".into(),
                    confidence: 0.3,
                    source: SignalSource::Fallback,
                    context: None,
                }
            }
        }
        SessionStatus::Done => AgentState {
            group: AgentStateGroup::Delivered,
            state: "session_ended".into(),
            label: "Session ended".into(),
            confidence: 0.9,
            source: SignalSource::Jsonl,
            context: None,
        },
    }
}
```

## Interaction Model

Each "Needs You" state maps to an operator action:

| Agent State | Operator Action | Implementation Phase |
|------------|----------------|---------------------|
| awaiting_input | Type answer in card | Phase 4 (Agent SDK) |
| awaiting_approval | Tap "Approve" / "Reject" | Phase 4 (Agent SDK) |
| needs_permission | Tap "Allow" / "Deny" | Phase 4 (via hook response) |
| error | Tap "Retry" / type guidance | Phase 4 (Agent SDK) |
| idle | Type next prompt / "Dismiss" | Phase 4 (Agent SDK) |

v1: Cards are read-only (see state, see what's needed). Full bidirectional control requires Agent SDK integration.

## Frontend Layout

The UI groups sessions by AgentStateGroup, with rich sub-state rendering:

**Desktop**: Three sections (can be columns or rows based on view mode):
- "Needs You" section: amber/red accent, sorted by urgency (needs_permission > awaiting_input > error > awaiting_approval > idle)
- "Running" section: green accent, sorted by most recent activity
- "Done" section: blue/gray accent, sorted by completion time (newest first)

**Mobile**: New `MobileStatusTabs` component with three tabs: "Needs You (3)" / "Running (2)" / "Done (5)" with count badges. This is **separate from** the existing `MobileTabBar` (which is a view mode switcher for Grid/Board/List).

Each card shows:
- Session title (first user message)
- State badge (icon + label from sub-state)
- Time in state ("2m ago")
- Project/branch context
- Token/cost summary (from JSONL data, independent of state)

Unknown sub-states (future ones) render with the group's default icon/color -- no frontend changes needed to support new sub-states.

## What Changes from Current Implementation

### Keep (adapt)

- `classifier.rs` -- Tier 1 structural classification becomes the JSONL->AgentState mapper
- `manager.rs` -- JSONL watching + process detection + SessionAccumulator (all stay)
- `state.rs` -- 3-state SessionStatus enum stays as internal JSONL status, mapped to AgentState
- Module scoping -- all MC frontend under `src/components/live/`

### Add

- `POST /api/live/hook` endpoint -- receives hook payloads
- `StateResolver` -- merges hook + JSONL signals
- `AgentState` struct -- replaces `PauseClassification` on LiveSession
- Hook configuration docs/script for users
- Frontend group-based layout (replacing 3-column Kanban with 3 groups)

### Remove

- **Tier 2 AI classification via Claude CLI** -- gone entirely
- `LlmProvider.complete()` method (added for session classification, no longer needed for THIS feature)
- `spawn_ai_classification()` in manager.rs -- already removed (confirmed by audit), clean up any remaining references
- `LlmConfig`/`ProviderType`/`factory.rs` for THIS feature (keep if used elsewhere)

> **NOTE:** The existing `LlmProvider` trait with `classify()` is used by the AI Fluency Score feature (session work-type classification). That stays. We only remove the `complete()` method and factory code that was added specifically for intelligent session states.

## Implementation Phases

### Phase 1: Hook Endpoint + State Model (MVP)

**Implementation order matters** — later steps depend on earlier ones compiling:

1. Add `AgentState`, `AgentStateGroup`, `SignalSource`, `StateCategory` types to `state.rs`
2. Replace `pause_classification` with `agent_state` on `LiveSession` + `SessionAccumulator`
3. Add `StateResolver` to `state_resolver.rs`, wire into `AppState`
4. Add `jsonl_to_agent_state()` mapper, wire into manager's async callers
5. Add `POST /api/live/hook` endpoint + route registration
6. Update `build_summary()` and `SessionEvent::Summary` to use group counts
7. Update frontend types, SSE listener, and all `toDisplayStatus()` / `pauseClassification` consumers **atomically**
8. Create hook notification script (`~/.claude/hooks/notify-mission-control.sh`)
9. Remove Tier 2 AI classification code (classifier.rs `ai_classify()`, `LlmProvider.complete()`, factory code)
10. **Verify** Open Question #5 (AskUserQuestion PostToolUse) with test hook before declaring done

### Phase 2: Subagent Tracking

- Handle SubagentStart/Stop hook events
- Show subagents as nested cards under parent session
- Track subagent lifecycle (spawned -> running -> completed)

### Phase 3: Mobile Remote Access

- Tailscale/Cloudflare tunnel setup for remote access
- Responsive mobile UI (tab bar with count badges)
- Push notifications for "Needs You" state changes

### Phase 4: Bidirectional Control (Agent SDK)

- "Respond from dashboard" -- type answers to AskUserQuestion
- "Approve/Reject" buttons for ExitPlanMode
- "Allow/Deny" for permission requests
- "Resume session" -- continue from dashboard
- This is the "workflow builder without the builder" -- full remote agent control

### Phase 5: Domain Plugins

- SDLC domain layer (git-aware labels, PR states, test status)
- Creative domain layer (draft/revision/feedback labels)
- Custom domain layer API for power users

## Migration from Superseded Plan

The existing intelligent session states code (partially implemented in `2026-02-15-intelligent-session-states.md`) transitions:

| Current | New | Notes |
|---------|-----|-------|
| `classifier.rs` Tier 1 structural patterns | Reused in `jsonl_to_agent_state()` mapper | Keep structural heuristics |
| `classifier.rs` Tier 2 AI code | Deleted | Replaced by hooks |
| `PauseClassification` on `LiveSession` | Replaced by `AgentState` | Richer model |
| `PauseReason` enum | Mapped to AgentState sub-states | Open string vs closed enum |
| `SessionStatus` 3-state enum | Kept internally for JSONL, mapped to `AgentStateGroup` | Internal vs external model |
| Frontend `PauseReasonBadge` | Replaced by universal `StateBadge` using `KNOWN_STATES` config | Extensible |
| Frontend 3-column Kanban | 3-group layout (same number, different semantics) | Operator-centric grouping |

The frontend column layout doesn't change dramatically -- it's still 3 groups. What changes is the richness of what each card shows and the extensibility of the state system.

## Files Changed (MVP -- Phase 1)

### Backend -- New

| File | Description |
|------|-------------|
| `crates/server/src/routes/hooks.rs` | `POST /api/live/hook` endpoint |
| `crates/server/src/live/state_resolver.rs` | StateResolver merging hook + JSONL |

### Backend -- Modified

| File | Change |
|------|--------|
| `crates/server/src/live/state.rs` | Add `AgentState`, `AgentStateGroup`, `SignalSource`. Keep `SessionStatus` for internal JSONL. Add `jsonl_to_agent_state()` |
| `crates/server/src/live/manager.rs` | Replace `PauseClassification` with `AgentState` on session/accumulator. Wire `StateResolver`. Remove `spawn_ai_classification()` references. **Integration point**: In the async callers of `handle_status_change()` (e.g., `process_jsonl_update()` at line 367, process detector loop), AFTER `handle_status_change()` returns, call `jsonl_to_agent_state()` and then `state_resolver.update_from_jsonl().await` |
| `crates/server/src/live/classifier.rs` | Remove Tier 2 AI methods. Keep Tier 1 structural (used by JSONL mapper). Remove `LlmProvider` dependency |
| `crates/server/src/live/mod.rs` | Add `pub mod state_resolver;` |
| `crates/server/src/routes/mod.rs` | Add `pub mod hooks;` declaration AND register route in `api_routes()`. Route path: `hooks::router()` defines `/live/hook`, nested under `/api` → final URL is `POST /api/live/hook`. Matches the curl in the forwarding script |
| `crates/server/src/routes/live.rs` | Update `build_summary()` to count by `AgentStateGroup` |
| `crates/server/src/lib.rs` or `main.rs` | Wire hook endpoint, create StateResolver |

### Frontend -- New

| File | Description |
|------|-------------|
| `src/components/live/MobileStatusTabs.tsx` | Mobile tab bar for "Needs You" / "Running" / "Done" status groups with count badges |

### Frontend -- Modified

| File | Change |
|------|--------|
| `src/components/live/types.ts` | Replace `LiveDisplayStatus` with `AgentStateGroup`. Add `AgentState` type. Add `KNOWN_STATES` and `GROUP_DEFAULTS` config. **Remove `toDisplayStatus()`** — used in 5 files (KanbanView, ListView, MissionControlPage, live-filter.ts, types.ts). Replace all calls with `session.agentState.group`. Also remove `DISPLAY_STATUS_ORDER` and `PauseReason`/`PauseClassification` types |
| `src/components/live/use-live-sessions.ts` | Update `LiveSession` interface: `agentState: AgentState` replaces `pauseClassification`. Update `LiveSummary` with group counts (`needsYouCount`/`autonomousCount`/`deliveredCount`). **CRITICAL**: SSE listener at line ~126 uses `data.workingCount !== undefined` to detect summary events — must change to `data.needsYouCount !== undefined` |
| `src/components/live/KanbanView.tsx` | Group by `agentState.group` instead of `toDisplayStatus()`. Replace `pausedSortKey()` (reads `session.pauseClassification?.reason`) with urgency sort on `session.agentState.state`. Rename columns |
| `src/components/live/SessionCard.tsx` | Replace `PauseReasonBadge` with universal `StateBadge`. Render icon/label from `KNOWN_STATES` or `GROUP_DEFAULTS` |
| `src/components/live/KanbanColumn.tsx` | Update header to show group name + sub-counts. **Replace `PausedColumnHeader`** (reads `s.pauseClassification?.reason === 'needsInput'`) with group-aware header using `session.agentState` |
| `src/components/live/StatusDot.tsx` | Color from `AgentStateGroup` instead of `LiveDisplayStatus` |
| `src/components/live/ContextGauge.tsx` | Use `agentState.group` for inactive check |
| `src/components/live/ListView.tsx` | Show state badge in activity column |
| `src/components/live/LiveFilterBar.tsx` | Filter by group (needs_you, autonomous, delivered) |
| `src/components/live/LiveCommandPalette.tsx` | Update filter options |
| `src/components/live/MobileTabBar.tsx` | **No change** — this is a view mode switcher (Grid/Board/List), NOT status group tabs. Leave as-is. |
| `src/components/live/MobileStatusTabs.tsx` *(new)* | **New component**: Mobile tab bar for status groups "Needs You (3)" / "Running (2)" / "Done (5)" with count badges. Displayed above view content on mobile. |
| `src/components/live/live-filter.ts` | Update status filter values from working/paused/done to needs_you/autonomous/delivered. Replace `toDisplayStatus()` calls with `session.agentState.group` |
| `src/components/live/use-live-session-filters.ts` | Update filter options to match new groups |
| `src/pages/MissionControlPage.tsx` | Update summary bar with group counts (`needsYouCount`/`autonomousCount`/`deliveredCount`). Replace `toDisplayStatus()` call with `session.agentState.group` |

### User Setup

| File | Description |
|------|-------------|
| `~/.claude/hooks/notify-mission-control.sh` | Hook script that forwards stdin to server |

## Testing Strategy

1. **State resolver unit tests**: Hook priority > JSONL, **state-category-aware staleness** (terminal never expires, blocking persists, transient expires at 60s), `cleanup_stale()` preserves terminal/blocking states, fallback behavior
2. **Hook endpoint integration test**: Send mock hook payloads, verify AgentState resolution
3. **JSONL->AgentState mapping tests**: Each SessionStatus + PauseReason combo maps correctly
4. **Frontend snapshot tests**: Each sub-state renders correct icon/color/label
5. **Unknown state test**: Future sub-state string renders with group default (no crash)
6. **E2E: Hook flow**: Configure hook -> trigger tool use -> verify card state updates in browser

## Updated SessionEvent::Summary

The `SessionEvent` enum in `state.rs` must update its `Summary` variant to use group counts:

```rust
SessionEvent::Summary {
    #[serde(rename = "needsYouCount")]
    needs_you_count: usize,
    #[serde(rename = "autonomousCount")]
    autonomous_count: usize,
    #[serde(rename = "deliveredCount")]
    delivered_count: usize,
    #[serde(rename = "totalCostTodayUsd")]
    total_cost_today_usd: f64,
    #[serde(rename = "totalTokensToday")]
    total_tokens_today: u64,
},
```

## StateResolver Wiring Notes

**Calling async StateResolver from sync contexts:** The existing `handle_status_change()` in `manager.rs` is synchronous (`fn`, not `async fn`). Do NOT call `state_resolver.update_from_jsonl().await` from within it. Instead:

1. Call `state_resolver.update_from_jsonl()` from the async callers of `handle_status_change()` — i.e., from `process_jsonl_update()` and the process detector loop, both of which are already async.
2. Call `state_resolver.update_from_hook()` from the `POST /api/live/hook` route handler (already async).

**AppState integration:** Add `state_resolver: StateResolver` field to `AppState` in `crates/server/src/state.rs`. The `StateResolver` struct already derives `Clone` and uses `Arc<RwLock<>>` internally, so it can be stored directly (no extra `Arc` wrapper needed):

```rust
// In state.rs AppState struct:
pub state_resolver: StateResolver,

// In all 3 constructors (new, new_with_indexing, new_with_indexing_and_registry):
state_resolver: StateResolver::new(),
```

**Wiring in create_app_full() (lib.rs):**

```rust
// After creating AppState (which now has state_resolver):
// The hook endpoint accesses state_resolver via State(Arc<AppState>)
// The manager's async callers use state.state_resolver.update_from_jsonl()

// In process_jsonl_update() (already async):
// After calling handle_status_change() which updates session fields,
// also update the state resolver:
let agent_state = jsonl_to_agent_state(&status, classification.as_ref(), last_line.as_ref());
state.state_resolver.update_from_jsonl(&session_id, agent_state).await;
```

**Periodic cleanup task** (add to manager startup alongside existing cleanup):

```rust
// In LiveSessionManager::start() or create_app_full():
let resolver_for_cleanup = app_state.state_resolver.clone();
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(120));
    loop {
        interval.tick().await;
        // 60s matches the transient state expiration in resolve()
        resolver_for_cleanup.cleanup_stale(Duration::from_secs(60)).await;
    }
});
```

## Updated build_summary()

After Phase 1, `build_summary()` in `routes/live.rs` must count by `AgentStateGroup`:

```rust
fn build_summary(map: &HashMap<String, LiveSession>) -> serde_json::Value {
    let mut needs_you_count = 0usize;
    let mut autonomous_count = 0usize;
    let mut delivered_count = 0usize;
    let mut total_cost = 0.0f64;
    let mut total_tokens = 0u64;

    for session in map.values() {
        match session.agent_state.group {
            AgentStateGroup::NeedsYou => needs_you_count += 1,
            AgentStateGroup::Autonomous => autonomous_count += 1,
            AgentStateGroup::Delivered => delivered_count += 1,
        }
        total_cost += session.cost.total_usd;
        total_tokens += session.tokens.total_tokens;
    }

    serde_json::json!({
        "needsYouCount": needs_you_count,
        "autonomousCount": autonomous_count,
        "deliveredCount": delivered_count,
        "totalCostTodayUsd": total_cost,
        "totalTokensToday": total_tokens,
    })
}
```

## Open Questions (to resolve during implementation)

1. ~~**Session ID correlation**~~: **RESOLVED** — `extract_session_id()` in `manager.rs` (line 663) strips `.jsonl` from the file path to get the UUID. Hook payloads include `session_id` which is this same UUID. No mapping table needed: `hook.session_id == extract_session_id(jsonl_path)`
2. **Hook auto-configuration**: Should `claude-view` auto-install hooks when it starts? Or require manual user setup?
3. **Subagent card nesting**: Should subagents be separate cards or nested under parent? (Phase 2 decision)
4. **PermissionRequest hook response**: Phase 1 hooks are read-only (post-event). Blocking hooks like `PermissionRequest` can return decisions to Claude Code -- this requires Phase 4 (bidirectional control) infrastructure
5. **AskUserQuestion as PostToolUse tool_name**: Strong evidence suggests YES — GitHub issue [#13439](https://github.com/anthropics/claude-code/issues/13439) shows PostToolUse hooks fire for AskUserQuestion (marked "Works" in test matrix). PreToolUse had a stdin/stdout conflict bug (fixed in v2.0.76+), but PostToolUse was unaffected. **However, the official docs don't explicitly list internal tools.** Verification step before implementing:
   ```bash
   # Add to ~/.claude/settings.json temporarily:
   "PostToolUse": [{ "hooks": [{ "type": "command", "command": "jq '.tool_name' >> /tmp/hook-tools.log" }] }]
   # Then trigger AskUserQuestion in a session and check /tmp/hook-tools.log
   ```
   **Fallback if it doesn't fire**: Detect `awaiting_input` via the `Stop` hook — when Claude stops after using AskUserQuestion, the Stop hook fires. Parse `transcript_path` JSONL to find the last tool used. This is slower (seconds vs instant) but reliable

## Changelog of Fixes Applied (Audit)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Hook matcher syntax used JSON object `{ "tool_name": "AskUserQuestion" }` -- Claude Code expects a string regex | Blocker | Changed all matchers to string format: `"matcher": "AskUserQuestion"` |
| 2 | StateResolver used raw `HashMap` -- not thread-safe for concurrent access from hook endpoint, JSONL watcher, and process detector | Blocker | Wrapped in `Arc<RwLock<>>`, made methods `async`, added `Clone` derive, added `cleanup_stale()` method |
| 3 | `build_summary()` update not shown -- plan said "count by AgentStateGroup" but had no code | Blocker | Added explicit `build_summary()` code section with `needs_you_count`/`autonomous_count`/`delivered_count` |
| 4 | HookPayload missing `permission_mode` (common field on ALL hooks) | Warning | Added `permission_mode: Option<String>` to HookPayload |
| 5 | HookPayload missing `tool_use_id`, `is_interrupt`, `task_description` fields | Warning | Added all three fields with correct types |
| 6 | Hook forwarding script used `&` backgrounding -- suppresses exit codes and hides curl failures | Warning | Removed `&`, run curl in foreground with `--max-time 1`, explicit `exit 0` to not block Claude Code |
| 7 | Hook forwarding script used `INPUT=$(cat)` then `-d "$INPUT"` -- unnecessary variable, breaks on special chars | Warning | Changed to `curl -d @-` to pipe stdin directly |
| 8 | Plan said "Remove spawn_ai_classification()" but it was already removed | Minor | Updated note to say "already removed, clean up references" |
| 9 | Open question #1 about "Hook stdin format" was already answered by audit | Minor | Removed -- format is confirmed: JSON via stdin with documented fields |
| 10 | Missing note about PermissionRequest being a blocking hook that can return decisions | Minor | Added open question #4 about Phase 4 requirement for hook responses |
| 11 | `HookPayload` used `#[serde(rename_all = "camelCase")]` but Claude Code sends snake_case | Blocker | Removed `rename_all` attribute — Rust field names already match Claude Code's snake_case JSON |
| 12 | `SubagentStop` had no match arm in `resolve_state_from_hook()` — fell through to generic "acting" | Warning | Added explicit `SubagentStop` match arm with "Subagent finished" label |
| 13 | `StateResolver` async methods can't be called from sync `handle_status_change()` | Blocker | Added "StateResolver Wiring Notes" section — call from async callers, not from sync method |
| 14 | `SessionEvent::Summary` variant fields not updated in plan | Blocker | Added "Updated SessionEvent::Summary" section with new field names |
| 15 | `AppState` needs `state_resolver` field — not addressed | Warning | Added note in "StateResolver Wiring Notes" section |
| 16 | Missing `HashMap` import in StateResolver code block | Minor | Added `use std::collections::HashMap` |
| 17 | Missing `MobileTabBar.tsx`, `live-filter.ts`, `use-live-session-filters.ts` from frontend file list | Warning | Added all three to "Frontend -- Modified" table |
| 18 | `AskUserQuestion` as PostToolUse tool_name is unverified — may not fire as PostToolUse hook | Warning | Added to open questions — needs experimental verification |
| 19 | `StateResolver::resolve()` used blanket 60s expiration for ALL states — breaks blocking states (`awaiting_input`: user takes 3min to answer → state lost) and terminal states (`session_ended`: permanent state expires) | **Blocker** | Added state-category-aware expiration: Terminal=never, Blocking=until next hook, Transient=60s. Added `StateCategory` enum and `state_category()` helper |
| 20 | `cleanup_stale()` removed ALL states older than max_age including valid terminal/blocking states | **Blocker** | `cleanup_stale()` now only removes Transient states; Terminal and Blocking are retained |
| 21 | Open Question #1 (Session ID correlation) already answered — `extract_session_id()` strips `.jsonl` extension = same UUID as hook `session_id` | Minor | Marked as RESOLVED with explanation |
| 22 | SSE listener in `use-live-sessions.ts` uses `data.workingCount !== undefined` to detect summary events — must change atomically with backend | **Blocker** | Added CRITICAL note to `use-live-sessions.ts` file change entry: must change to `data.needsYouCount !== undefined` |
| 23 | `MobileTabBar.tsx` is a view mode switcher (Grid/Board/List), NOT status group tabs — plan said to "update" it for "Needs You / Running / Done" tabs | **Blocker** | Marked MobileTabBar as **no change**. Added new `MobileStatusTabs.tsx` component for status group tabs. Updated Frontend Layout section. Added to Frontend -- New file list |
| 24 | HookPayload missing 5 optional fields from Claude Code docs: `stop_hook_active`, `agent_transcript_path`, `teammate_name`, `team_name`, `source` | Warning | Added all 5 fields as `Option<>` to HookPayload struct |
| 25 | Plan said "replace PauseClassification with AgentState" but never showed the actual struct field change on `LiveSession` or `SessionAccumulator` | **Blocker** | Added "LiveSession Struct Change" section with explicit field add/remove, default value, and SessionAccumulator note |
| 26 | `routes/mod.rs` not updated — missing `pub mod hooks;` declaration and route registration in `api_routes()` | **Blocker** | Updated file change table to specify both `pub mod hooks;` AND `.nest("/api", hooks::router())` |
| 27 | AppState wiring incomplete — didn't specify `StateResolver` vs `Arc<StateResolver>` type, no creation code, no cleanup_stale() periodic task | **Blocker** | Expanded wiring notes with explicit type (no Arc needed, Clone+internal Arc), constructor code, process_jsonl_update wiring, and periodic cleanup task |
| 28 | Frontend GROUP_DEFAULTS uses Bell, Loader, Archive icons but import list only had 10 icons | Warning | Added explicit import comment listing all 13 required Lucide icons |
| 29 | `jsonl_to_agent_state()` used `unwrap_or(&"tool".to_string())` inside `!is_empty()` guard — dead code, confusing | Minor | Rewrote to `if let Some(last_tool) = ll.tool_names.last()` pattern |
| 30 | Fallback label "Connecting..." is misleading for stale/unknown states | Minor | Changed to "Status unavailable" |
| 31 | StateResolver code block missing `use super::state::{AgentState, AgentStateGroup, SignalSource}` import | Warning | Added import |
| 32 | `toDisplayStatus()` used in 5 frontend files but plan never said to remove/replace it | **Blocker** | Added explicit removal note to `types.ts` entry. Added `session.agentState.group` replacement note to KanbanView, live-filter.ts, MissionControlPage entries |
| 33 | Open Question #5 had no evidence or fallback — just "verify experimentally" | Warning | Added GitHub issue #13439 evidence (PostToolUse "Works" for AskUserQuestion). Added concrete verification command. Added Stop hook + transcript_path fallback mechanism |
| 34 | `cleanup_stale()` periodic task used `max_age: 300s` but staleness table says transient states expire at 60s | Warning | Changed to `Duration::from_secs(60)` to match resolve() expiration |
| 35 | Hook route path ambiguous — `/api/hook` vs `/api/live/hook` | Minor | Clarified: `hooks::router()` defines `/live/hook`, nested under `/api` → `POST /api/live/hook` |
| 36 | `jsonl_to_agent_state()` integration point in manager.rs never specified | **Blocker** | Added explicit note: call from async callers of `handle_status_change()` (process_jsonl_update, process detector) |
| 37 | AgentState code block missing `use serde::{Serialize, Deserialize};` import | Minor | Added import |
| 38 | `KanbanView.tsx` has `pausedSortKey()` reading `pauseClassification?.reason` — not called out for replacement | Warning | Added explicit note to replace with urgency sort on `agentState.state` |
| 39 | `KanbanColumn.tsx` has `PausedColumnHeader` reading `pauseClassification?.reason === 'needsInput'` — not called out | Warning | Added explicit note to replace with group-aware header using `agentState` |
| 40 | Implementation Phase 1 had no ordering — later steps depend on earlier ones compiling | **Blocker** | Replaced bullet list with numbered sequence (10 steps) with dependency reasoning |
