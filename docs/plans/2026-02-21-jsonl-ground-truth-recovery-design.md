# JSONL Ground Truth Recovery — Design (v2)

**Date:** 2026-02-21
**Status:** Approved
**Supersedes:** v1 (dropped — over-engineered mtime guards and staleness thresholds)

## Problem

When the server restarts, snapshot-recovered sessions may have stale `agent_state`. The Stop hook fires via `curl` to localhost — if the server is down, `|| true` swallows the failure and the state transition is lost. The session's PID is still alive (idle at prompt), so the reconciliation loop keeps it as "Autonomous" forever.

The previous fix (300s `last_activity_at` staleness timeout) was a band-aid that could misfire during long-running tool executions (>5 min builds).

## Key Insight

The JSONL file is the complete, authoritative record of what Claude did — independent of hooks. If Claude finished work and went idle, the JSONL ends with `assistant end_turn` or `result`, regardless of whether the Stop hook reached the server. No time-based guards needed.

## Design

**At startup:** Read the last JSONL line to derive the correct agent_state. Override the snapshot's stale value.

**At runtime:** Remove the 300s staleness check entirely. PID liveness + hooks cover all real failure modes.

### State Derivation Rules

| Last meaningful JSONL line | Derived state |
|---|---|
| `type: "result"` | NeedsYou/idle |
| `assistant` + `stop_reason: "end_turn"` | NeedsYou/idle |
| `assistant` + `stop_reason: "tool_use"` or `tool_use` blocks in content | Autonomous/acting |
| `user` + `tool_result` content | Autonomous/thinking |
| `user` (real prompt, no tool_result) | Autonomous/thinking |
| No meaningful line / empty file | NeedsYou/idle (safe default) |

"Meaningful" = skip `progress`, `system`, `summary`, `other` line types.

### Why No Mtime Guard

v1 proposed a 60s mtime guard to downgrade "stale" autonomous sessions. This is wrong because:

- During tool execution (e.g., `cargo build`), the JSONL is not written to. A 60s+ tool run would be incorrectly downgraded.
- The JSONL content already encodes the correct state. If a tool finished, Claude wrote subsequent lines. If it's still running, Autonomous is correct.
- Hooks self-correct on the next event. The window of incorrectness (if any) is seconds.

### Why No Runtime Staleness Check

The 300s staleness check existed because the server had no way to know the true state after a missed Stop hook. Now that startup recovery reads the JSONL, the root cause is addressed. Remaining runtime scenarios:

- **Hook curl timeout during GC pause:** The next hook (seconds later) self-corrects.
- **Hook process killed:** Same — next event self-corrects.
- **All hooks fail persistently:** Extremely unlikely. If it happens, PID liveness still catches process death.

Net: the staleness check was masking a startup recovery gap. With that gap closed, it's unnecessary complexity.

### Changes

1. **Make `parse_single_line` public** in `live_parser.rs`
   - Needed to parse individual tail lines in `manager.rs`

2. **Add `derive_agent_state_from_jsonl`** in `manager.rs`
   - `tail_lines(path, 10)` → scan from end → find last meaningful line → apply derivation rules
   - Returns `Option<AgentState>` (None = keep safe default)

3. **Wire into startup recovery** (`spawn_file_watcher` step 3)
   - After `build_recovered_session`, call derive function and override snapshot agent_state

4. **Remove 300s staleness check** from reconciliation loop
   - Delete `AUTONOMOUS_STALE_SECS`, `now_secs`, and the entire "1b. Staleness downgrade" block
   - Delete 3 associated tests
   - PID liveness check (1a) stays

### Proven at Scale

- **Event sourcing / log-derived state:** Kafka, Event Store, Datomic all derive current state from the event log. The JSONL is our event log.
- **PID liveness as health check:** Standard Unix process monitoring (systemd, supervisord, Kubernetes liveness probes).
- **Hooks as primary state authority:** Webhook-driven architecture (GitHub webhooks, Stripe events) — the hook is the real-time signal, the log is the recovery mechanism.
