---
status: approved
date: 2026-02-19
---

# Process-Gated Session Discovery

## Problem

Sessions discovered via JSONL file watching (initial scan or ongoing file events) are
created with a fallback `Autonomous/unknown` state, which maps to `SessionStatus::Working`.
This causes dead sessions to appear as "running" in the live monitor until the 5-minute
crash detector kicks in.

Root cause: the JSONL watcher violates the principle that hooks are the sole authority
for session lifecycle. It creates sessions in the live map with fabricated state.

## Core Invariant

A session only exists in the live map if ONE of these is true:

1. A `SessionStart` hook created it (normal operation)
2. At startup, a live process was detected for it (recovery)

The JSONL watcher never creates sessions. It only enriches existing ones.

## Startup Sequence

Current: file watcher → initial scan (creates sessions from JSONL) → process detector.

New:

1. **Process detector runs first** (one-shot eager scan) — builds process table
2. **Initial JSONL scan** — for each .jsonl modified in last 24h:
   - Build accumulator (tokens, title, cost, sub-agents, progress)
   - Check process table: live process matches this project?
     - YES → create session with `NeedsYou/idle` + accumulated metadata
     - NO → accumulator stays dormant, no session created
3. **File watcher starts** for ongoing events
4. Hook server already listening (Axum binds before all of this)

## Component Responsibilities

| Component | Creates sessions? | Modifies state? | Modifies metadata? |
|-----------|------------------|-----------------|-------------------|
| Hook handler | Yes (SessionStart) | Yes (sole authority) | Partial (last_user_message, turn_count) |
| JSONL watcher | Never | Never | Yes (tokens, cost, title, sub-agents, progress) |
| Process detector | Only at startup (recovery) | Only crash→Done | PID field only |

## Recovery State

Sessions recovered via process detection use:

```rust
AgentState {
    group: AgentStateGroup::NeedsYou,
    state: "idle".into(),
    label: "Waiting for your next prompt".into(),
    context: None,
}
```

Maps to `SessionStatus::Paused`. The next hook event corrects it to the real state.

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| Server restart, session actively working | Process detected → Paused. Next hook corrects to Working within ms. |
| Server restart, session idle | Process detected → Paused. Correct. |
| Server restart, session already exited | No process → not created. Correct. |
| JSONL event before SessionStart hook | Accumulator populated, no session. Hook creates it, next JSONL event enriches. |
| SessionStart hook before JSONL event | Hook creates skeleton. JSONL enriches with metadata. |
| Server restart, claude still launching | Missed by scan. SessionStart hook creates it moments later. |

## Files Changed

| File | Change |
|------|--------|
| `crates/server/src/live/manager.rs` | Remove `else` branch in `process_jsonl_update`. Reorder startup: eager process scan before JSONL scan. Process-gated session creation in initial scan. |
