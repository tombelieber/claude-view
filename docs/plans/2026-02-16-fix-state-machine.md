---
status: draft
date: 2026-02-16
audit: round 4 — fixed 7 remaining issues (4 important, 3 warning) to reach 100/100 (see Changelog at bottom)
---

# Fix State Machine: Stop Flickering & Clarify Delivered

## Problem Statement

The current 3-state model (needs_you, autonomous, delivered) has two critical issues that break observability in Mission Control:

### Issue 1: needs_you <-> autonomous Flickering

**Current behavior:**
```
Time    Event                 State                      Display
────    ─────                 ─────                      ───────
 t0     PostToolUse hook      autonomous/acting          ✓ Agent working
 t5     Process detector      re-derives Paused→MidWork  🔴 FLIP → idle (NeedsYou!)
 t10    Process detector      re-derives Paused→MidWork  🔴 FLIP → idle
 t30    New hook arrives      autonomous/acting          ✓ Agent working
 t35    Process detector      re-derives Paused→MidWork  🔴 FLIP → idle
```

**Root cause (CORRECTED):** There are TWO independent paths that set `session.agent_state`:

1. **Hook path** (`hooks.rs:handle_hook`) — Sets `session.agent_state` directly when a hook arrives. Correct.
2. **Process detector path** (`manager.rs:spawn_process_detector`, lines 283-331) — Runs every 5 seconds, re-derives `SessionStatus` from JSONL timing via `derive_status()`, then calls `handle_status_change()` which overwrites `acc.agent_state` via the JSONL classifier, then copies to `session.agent_state = acc.agent_state.clone()` (line 323).

The process detector **does not consult `state_resolver.resolve()`** — it blindly overwrites hook-derived state with JSONL-derived state. So even if a hook correctly set the session to `Autonomous/acting`, the next process detector tick (5 seconds later) can overwrite it to `NeedsYou/idle` based on JSONL timing alone.

**Note:** There IS an existing anti-flicker hack at `manager.rs:652-663` — when `MidWork + has_running_process`, it overrides to `Autonomous/thinking` instead of `NeedsYou/idle`. But this only fires when the fallback classifier returns `MidWork` AND a Claude process is detected. If `derive_status()` returns `Paused` (JSONL >30s stale) and the structural classifier matches (e.g., `end_turn` detected), it can still produce a NeedsYou state that overrides the hook.

**Critical detail:** `state_resolver.resolve()` is **dead code** — it is NEVER called anywhere. The StateResolver's `update_from_hook()` is called from `hooks.rs`, but `resolve()` is never used by any consumer. The hook handler sets `session.agent_state` directly and the process detector uses `acc.agent_state` directly.

**Impact:**
- Operators can't trust dashboard — it shows wrong state constantly
- Can't distinguish between "agent is busy, don't interrupt" vs "agent needs my decision"
- Breaks real-time monitoring and trust

### Issue 2: "Delivered" is Vague

(Same as before — this analysis is correct.)

**Current behavior:**
```
task_complete   → Task finished? Session finished? Ready for action?
session_ended   → Agent disconnected. Good outcome? Bad outcome?
awaiting_approval → Is this "needs_you" or is it "delivered and waiting"?
```

---

## Proposed Solution: Two-Part Fix

### Part A: Fix Flickering (the actual bug)

Make the process detector loop **consult the StateResolver** before overwriting agent_state. Hook-derived states take priority over JSONL-derived states. The StateResolver already implements this logic — it just needs to be called.

**Key insight:** The fix is in `manager.rs`, NOT in `state_resolver.rs`. The resolver's expiry logic is already reasonable (60s for transient states). The problem is that `resolve()` is never called.

### Part B: Enriched State Model (deferred)

Adding new states (`work_delivered`, `user_paused`, new hook events like `WorkOutputReady`) is a **separate concern** from the flickering bug. These should be Phase 2 after the flickering is fixed. Claude Code does NOT emit `WorkOutputReady`, `PauseRequested`, or `ResumeRequested` hooks — these are fictional hook names. Adding phantom state handling won't help.

---

## Implementation Steps

### Step 1: Wire StateResolver into the Process Detector Loop (THE FIX)

**File:** `crates/server/src/live/manager.rs`

The `LiveSessionManager` needs access to the `StateResolver`. Currently, the StateResolver lives on `AppState` but the manager doesn't have a reference to it.

**1a. Add StateResolver to LiveSessionManager:**

```rust
// In LiveSessionManager struct definition (manager.rs ~line 117):
pub struct LiveSessionManager {
    sessions: LiveSessionMap,
    tx: broadcast::Sender<SessionEvent>,
    finders: Arc<TailFinders>,
    accumulators: Arc<RwLock<HashMap<String, SessionAccumulator>>>,
    processes: Arc<RwLock<HashMap<PathBuf, ClaudeProcess>>>,
    pricing: Arc<HashMap<String, cost::ModelPricing>>,
    classifier: Arc<SessionStateClassifier>,
    state_resolver: StateResolver,  // NEW
}
```

**1b. Pass StateResolver into `start()`:**

```rust
// Update start() signature (manager.rs ~line 139):
pub fn start(
    pricing: HashMap<String, ModelPricing>,
    state_resolver: StateResolver,  // NEW parameter
) -> (Arc<Self>, LiveSessionMap, broadcast::Sender<SessionEvent>) {
    // ... existing code ...
    let manager = Arc::new(Self {
        sessions: sessions.clone(),
        tx: tx.clone(),
        finders: Arc::new(TailFinders::new()),
        accumulators: Arc::new(RwLock::new(HashMap::new())),
        processes: Arc::new(RwLock::new(HashMap::new())),
        pricing: Arc::new(core_pricing),
        classifier,
        state_resolver,  // NEW
    });
    // ...
}
```

**1c. Update callers of `start()` in `lib.rs` — REPLACE existing StateResolver::new():**

```rust
// In create_app_full() (lib.rs ~line 134):
// IMPORTANT: Create ONE resolver, share it between manager and AppState.
// You MUST replace the existing `state_resolver: StateResolver::new()` at ~line 153
// with `state_resolver: resolver.clone()`. Do NOT keep both — that creates two
// separate resolvers and the fix becomes silently ineffective.
let resolver = StateResolver::new();
let (_manager, live_sessions, live_tx) =
    LiveSessionManager::start(pricing.clone(), resolver.clone());

let state = Arc::new(state::AppState {
    // ... all existing fields ...
    state_resolver: resolver,  // REPLACES: StateResolver::new()
    // ...
});
```

**1d. In process detector loop, consult StateResolver OUTSIDE the lock scope:**

> **Lock discipline:**
> - **Phase 1** (under `sessions.write()` + `accumulators.write()`): Sync work ONLY. No `.await`.
>   Holding both guards across `.await` blocks all hook delivery AND JSONL updates.
> - **Phase 2** (no locks): Async `update_from_jsonl()` calls. Safe — only resolver's internal locks.
> - **Phase 3** (under `sessions.write()` only): Calls `resolve().await` — this IS an `.await`
>   under a write guard, but is intentionally safe because:
>   (a) `resolve()` only acquires `hook_states.read()` / `jsonl_states.read()` (microseconds).
>   (b) Hook handler releases `hook_states.write()` BEFORE acquiring `sessions.write()` —
>       no circular dependency, no deadlock.
>   (c) This prevents the TOCTOU race that would occur if resolve ran in Phase 2.
>   (d) For <20 active sessions (typical), total Phase 3 lock time is ~milliseconds.

```rust
// In spawn_process_detector() (manager.rs ~line 283):
// Replace the existing for loop body with this collect-then-resolve pattern:

tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;

        let new_processes = tokio::task::spawn_blocking(detect_claude_processes)
            .await
            .unwrap_or_default();

        {
            let mut processes = manager.processes.write().await;
            *processes = new_processes;
        }

        // Phase 1: Collect changes under locks, then drop locks.
        let mut pending_updates: Vec<(String, SessionStatus, Option<u32>, AgentState)> = Vec::new();

        {
            let processes = manager.processes.read().await;
            let mut sessions = manager.sessions.write().await;
            let mut accumulators = manager.accumulators.write().await;

            for (session_id, session) in sessions.iter_mut() {
                if let Some(acc) = accumulators.get_mut(session_id) {
                    let seconds_since = seconds_since_modified_from_timestamp(
                        session.last_activity_at,
                    );
                    let (running, pid) =
                        has_running_process(&processes, &session.project_path);
                    let new_status =
                        derive_status(acc.last_line.as_ref(), seconds_since, running);

                    if session.status != new_status || session.pid != pid {
                        // JSONL-based classification (sync, no .await)
                        manager.handle_status_change(
                            session_id, new_status.clone(), acc, running, seconds_since,
                        );

                        // Collect: (session_id, new_status, pid, jsonl_derived_state)
                        pending_updates.push((
                            session_id.clone(),
                            new_status,
                            pid,
                            acc.agent_state.clone(),
                        ));
                    }
                }
            }
        }
        // All locks dropped here.

        // Phase 2: Feed JSONL states into resolver (async, no external locks held).
        // update_from_jsonl() only needs StateResolver's internal lock.
        for (session_id, _, _, ref jsonl_state) in &pending_updates {
            manager.state_resolver.update_from_jsonl(session_id, jsonl_state.clone()).await;
        }

        // Phase 3: Resolve and apply under sessions lock.
        // CRITICAL: resolve() is called HERE (not Phase 2) to prevent TOCTOU race.
        // If a hook arrives between Phase 2 and Phase 3, resolving in Phase 2 would
        // produce a stale result that Phase 3 would overwrite the hook's fresh state with.
        // Calling resolve() under sessions.write() guarantees we see the latest hook signal.
        // This is safe from deadlock: hook handler releases hook_states.write() BEFORE
        // acquiring sessions.write(), so no circular lock dependency exists.
        if !pending_updates.is_empty() {
            let mut sessions = manager.sessions.write().await;
            for (session_id, new_status, pid, _) in pending_updates {
                if let Some(session) = sessions.get_mut(&session_id) {
                    let resolved = manager.state_resolver.resolve(&session_id).await;
                    session.status = new_status;
                    session.pid = pid;
                    session.agent_state = resolved;
                    let _ = manager.tx.send(SessionEvent::SessionUpdated {
                        session: session.clone(),
                    });
                }
            }
        }
    }
});
```

**1e. Same fix in `process_jsonl_update()` — resolve AFTER dropping accumulators lock:**

```rust
// In process_jsonl_update() (manager.rs ~line 580-609):
// The LiveSession is built BEFORE the drops, using acc.agent_state.clone().
// We make it `mut` so we can overwrite agent_state with the resolved value after drops.

let mut live_session = LiveSession {
    // ... all existing fields unchanged ...
    agent_state: acc.agent_state.clone(),  // Temporarily uses JSONL-derived state
    // ...
};

// Drop the accumulators lock before acquiring sessions lock
// (this is the EXISTING pattern at lines 603-604)
drop(processes);
drop(accumulators);

// NEW: Feed JSONL state to resolver, then resolve (hook wins if fresh).
// These calls only acquire StateResolver's internal locks, safe without external locks.
self.state_resolver.update_from_jsonl(&session_id, live_session.agent_state.clone()).await;
let resolved_state = self.state_resolver.resolve(&session_id).await;
live_session.agent_state = resolved_state;  // Overwrite with resolved state

// Update the shared session map (existing code)
let mut sessions = self.sessions.write().await;
sessions.insert(session_id, live_session);
```

> **WHY `mut live_session`:** The LiveSession must be constructed while `accumulators` is still
> locked (it reads ~15 fields from `acc`). But resolving requires dropping locks first. So we
> build the struct, drop locks, resolve, then mutate `agent_state` before inserting.

### Step 2: Spawn Background Cleanup for StateResolver

**File:** `crates/server/src/live/manager.rs`

The `cleanup_stale()` method exists on StateResolver but is never called. Add it to the cleanup task.

```rust
// In spawn_cleanup_task() (manager.rs ~line 338), add after existing cleanup:
// Clean up stale hook states (entries older than 10 minutes)
manager.state_resolver.cleanup_stale(Duration::from_secs(600)).await;
```

### Step 3: Extract Expiry Constant and Testable `is_expired()` Function

**File:** `crates/server/src/live/state_resolver.rs`

The current 60s timeout for Transient states is correct. Extract it as a named constant and extract the expiry logic into a testable `pub(crate)` function. This enables pure-function testing of expiry behavior without manipulating `Instant` values (which is unsafe on short-uptime CI VMs).

```rust
// At top of state_resolver.rs:
const TRANSIENT_EXPIRY_SECS: u64 = 60;

// Add testable expiry function (no Instant manipulation needed in tests):
impl StateResolver {
    /// Check if a hook state should be considered expired given its elapsed time.
    /// Extracted as pub(crate) for unit testing without Instant manipulation.
    pub(crate) fn is_expired(state: &str, elapsed: Duration) -> bool {
        match Self::state_category(state) {
            StateCategory::Terminal => false,
            StateCategory::Blocking => false,
            StateCategory::Transient => elapsed > Duration::from_secs(TRANSIENT_EXPIRY_SECS),
        }
    }
}

// Update resolve() to use the extracted function:
// Replace the inline match with:
let expired = Self::is_expired(&hook_state.state, timestamp.elapsed());
```

### Step 4: Add `work_delivered` State to KNOWN_STATES (Frontend Only)

**File:** `src/components/live/types.ts`

Add the new state to the KNOWN_STATES map. No new `AgentStateGroup` variant needed — `work_delivered` maps to the existing `delivered` group.

```typescript
// Add to KNOWN_STATES (after session_ended):
work_delivered: { icon: 'CheckCircle', color: 'blue' },
```

**Do NOT add a `paused` group yet.** That requires hook support from Claude Code which doesn't exist.

### Step 5: Add `work_delivered` State to Backend Classifier

**File:** `crates/server/src/live/classifier.rs`

**5a. Add `WorkDelivered` variant to `PauseReason`:**

```rust
// In PauseReason enum (classifier.rs ~line 54):
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PauseReason {
    #[serde(rename = "needsInput")]
    NeedsInput,
    #[serde(rename = "taskComplete")]
    TaskComplete,
    #[serde(rename = "workDelivered")]
    WorkDelivered,  // NEW: work output ready for review (PR, artifact, etc.)
    #[serde(rename = "midWork")]
    MidWork,
    #[serde(rename = "error")]
    Error,
}
```

**5b. Add PR detection pattern in `structural_classify()`:**

```rust
// In structural_classify(), add BEFORE line ~120 (the commit/push pattern check).
// PR pattern is more specific than commit/push — if placed after, a PR creation
// mentioning "commit" would match the commit pattern first with the wrong label.

// PR creation pattern
if last_msg.tool_names.iter().any(|t| t == "Bash")
    && (content.contains("pull request") || content.contains("pr created")
        || content.contains("gh pr create"))
    && ctx.last_stop_reason.as_deref() == Some("end_turn")
{
    return Some(PauseClassification {
        reason: PauseReason::WorkDelivered,  // Typed variant, no string matching
        label: "PR ready for review".into(),
        confidence: 0.90,
        source: ClassificationSource::Structural,
    });
}
```

**5c. Update `pause_classification_to_agent_state()` in `manager.rs` — type-safe mapping:**

```rust
fn pause_classification_to_agent_state(c: &PauseClassification) -> AgentState {
    let (group, state) = match c.reason {
        PauseReason::NeedsInput => (AgentStateGroup::NeedsYou, "awaiting_input"),
        PauseReason::TaskComplete => (AgentStateGroup::Delivered, "task_complete"),
        PauseReason::WorkDelivered => (AgentStateGroup::Delivered, "work_delivered"),
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
}
```

> **Why `PauseReason::WorkDelivered` instead of string matching:** The previous design
> checked `c.label.contains("PR ready")` which breaks silently if the label text changes
> in classifier.rs. A typed enum variant is compile-time safe — if you add a new reason,
> the exhaustive match forces you to handle it everywhere.

---

## Testing Strategy

### Unit Tests

**File:** `crates/server/src/live/state_resolver.rs`

**Do NOT change field visibility.** The `hook_states` field stays private. Instead, tests use:
1. **Pure function tests** via `is_expired()` — test expiry logic with `Duration` values, no `Instant` manipulation.
2. **Integration tests** via the public API — test priority and cleanup using short real-time durations + `tokio::time::sleep`. No `Instant::checked_sub()`, which panics on CI VMs with <2h uptime.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::state::{AgentState, AgentStateGroup, SignalSource};

    fn make_hook_state(state: &str, group: AgentStateGroup) -> AgentState {
        AgentState {
            group,
            state: state.into(),
            label: "test".into(),
            confidence: 0.99,
            source: SignalSource::Hook,
            context: None,
        }
    }

    fn make_jsonl_state(state: &str, group: AgentStateGroup) -> AgentState {
        AgentState {
            group,
            state: state.into(),
            label: "test".into(),
            confidence: 0.5,
            source: SignalSource::Jsonl,
            context: None,
        }
    }

    // ==========================================================================
    // Pure function tests: expiry logic (no Instant, no async, no panics)
    // ==========================================================================

    #[test]
    fn transient_not_expired_before_threshold() {
        assert!(!StateResolver::is_expired("acting", Duration::from_secs(59)));
        assert!(!StateResolver::is_expired("thinking", Duration::from_secs(0)));
        assert!(!StateResolver::is_expired("delegating", Duration::from_secs(30)));
    }

    #[test]
    fn transient_expired_after_threshold() {
        assert!(StateResolver::is_expired("acting", Duration::from_secs(61)));
        assert!(StateResolver::is_expired("thinking", Duration::from_secs(120)));
        assert!(StateResolver::is_expired("delegating", Duration::from_secs(3600)));
    }

    #[test]
    fn blocking_never_expires() {
        // Blocking states: no matter how much time passes, is_expired returns false
        assert!(!StateResolver::is_expired("awaiting_input", Duration::from_secs(7200)));
        assert!(!StateResolver::is_expired("awaiting_approval", Duration::from_secs(86400)));
        assert!(!StateResolver::is_expired("needs_permission", Duration::from_secs(604800)));
        assert!(!StateResolver::is_expired("error", Duration::from_secs(7200)));
        assert!(!StateResolver::is_expired("idle", Duration::from_secs(7200)));
    }

    #[test]
    fn terminal_never_expires() {
        assert!(!StateResolver::is_expired("task_complete", Duration::from_secs(86400)));
        assert!(!StateResolver::is_expired("session_ended", Duration::from_secs(604800)));
    }

    // ==========================================================================
    // Integration tests: priority and cleanup (real-time, short durations)
    // ==========================================================================

    #[tokio::test]
    async fn hook_state_takes_priority_over_jsonl() {
        let resolver = StateResolver::new();

        // JSONL says NeedsYou
        resolver.update_from_jsonl(
            "s1", make_jsonl_state("idle", AgentStateGroup::NeedsYou)
        ).await;

        // Hook says Autonomous (more recent = wins)
        resolver.update_from_hook(
            "s1", make_hook_state("acting", AgentStateGroup::Autonomous)
        ).await;

        let resolved = resolver.resolve("s1").await;
        assert_eq!(resolved.state, "acting");
        assert_eq!(resolved.group, AgentStateGroup::Autonomous);
    }

    #[tokio::test]
    async fn jsonl_state_used_when_no_hook() {
        let resolver = StateResolver::new();
        resolver.update_from_jsonl(
            "s1", make_jsonl_state("idle", AgentStateGroup::NeedsYou)
        ).await;

        let resolved = resolver.resolve("s1").await;
        assert_eq!(resolved.state, "idle");
    }

    #[tokio::test]
    async fn fallback_when_no_hook_or_jsonl() {
        let resolver = StateResolver::new();
        let resolved = resolver.resolve("nonexistent").await;
        assert_eq!(resolved.state, "unknown");
        assert_eq!(resolved.group, AgentStateGroup::Autonomous);
    }

    #[tokio::test]
    async fn cleanup_stale_removes_old_transient_keeps_blocking() {
        let resolver = StateResolver::new();
        resolver.update_from_hook(
            "s1", make_hook_state("acting", AgentStateGroup::Autonomous)
        ).await;
        resolver.update_from_hook(
            "s2", make_hook_state("awaiting_input", AgentStateGroup::NeedsYou)
        ).await;

        // Wait long enough for entries to exceed a tiny max_age
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Clean up entries older than 10ms (both are older, but only transient is removed)
        resolver.cleanup_stale(Duration::from_millis(10)).await;

        // Verify via resolve() behavior (no internal field access needed):
        // s1 was transient+cleaned → resolve should fall through to JSONL/fallback
        let s1 = resolver.resolve("s1").await;
        assert_eq!(s1.state, "unknown"); // Fell through to fallback (no JSONL state either)

        // s2 was blocking → still returns hook state
        let s2 = resolver.resolve("s2").await;
        assert_eq!(s2.state, "awaiting_input");
    }
}
```

> **Why no `Instant` manipulation:** `Instant::now().checked_sub(Duration::from_secs(7200))`
> returns `None` on systems with <2h uptime (e.g., freshly provisioned CI VMs), causing
> `.unwrap()` to panic. By testing expiry logic via pure-function `is_expired()` with
> `Duration` values, and testing cleanup/priority via short real-time durations (50ms sleep
> + 10ms threshold), all tests are safe regardless of system uptime.

### Integration Test Scenarios

**Scenario 1: No flicker during normal execution**
```
1. Hook arrives (PostToolUse) → session.agent_state = Autonomous/acting
2. Process detector runs at t=5s → derives Paused from JSONL
   → calls handle_status_change → acc.agent_state = NeedsYou/idle
   → calls state_resolver.update_from_jsonl(NeedsYou/idle)
   → calls state_resolver.resolve() → returns Autonomous/acting (hook wins!)
   → session.agent_state = Autonomous/acting ✓ NO FLICKER
3. Hook arrives at t=30s → refreshes hook timestamp
4. Process detector at t=35s → same flow → hook still wins ✓
```

**Scenario 2: Hook expires, JSONL correctly takes over**
```
1. Hook arrives (PostToolUse) → Autonomous/acting
2. No more hooks for 61 seconds
3. Process detector at t=65s → state_resolver.resolve()
   → hook is 65s old, Transient, expires
   → falls through to JSONL state → NeedsYou/idle
   → THIS IS CORRECT — agent genuinely stopped
```

**Scenario 3: Blocking hook never expires**
```
1. AskUserQuestion hook → NeedsYou/awaiting_input
2. User goes to lunch (2 hours)
3. Process detector every 5s → state_resolver.resolve()
   → hook state is Blocking, NEVER expires
   → keeps returning awaiting_input ✓
```

---

## Files to Modify

1. **`crates/server/src/live/manager.rs`** — THE MAIN FIX
   - Add `state_resolver: StateResolver` field to `LiveSessionManager`
   - Update `start()` to accept and store `StateResolver`
   - In `spawn_process_detector()`: call `resolve()` instead of using `acc.agent_state` directly
   - In `process_jsonl_update()`: same pattern — resolve after classification
   - In `spawn_cleanup_task()`: call `state_resolver.cleanup_stale()`

2. **`crates/server/src/lib.rs`** — Wire the shared StateResolver
   - In `create_app_full()`: create `StateResolver`, pass to both `LiveSessionManager::start()` and `AppState`

3. **`crates/server/src/live/state_resolver.rs`** — Extract testable function + add tests
   - Add `TRANSIENT_EXPIRY_SECS` constant
   - Extract `pub(crate) fn is_expired()` for pure-function testing (NO `pub(crate)` on fields)
   - Add comprehensive unit tests (pure function + short real-time integration)

4. **`crates/server/src/live/classifier.rs`** — Add `WorkDelivered` variant + PR detection
   - Add `PauseReason::WorkDelivered` variant to the enum (with `#[serde(rename = "workDelivered")]`)
   - Add structural pattern for "PR created" → `WorkDelivered`

5. **`crates/server/src/live/manager.rs`** — Type-safe `work_delivered` mapping
   - Update `pause_classification_to_agent_state()` match arm for `PauseReason::WorkDelivered`

6. **`src/components/live/types.ts`** — Add `work_delivered` to KNOWN_STATES

---

## What This Plan Does NOT Do (Deferred)

| Deferred Item | Why |
|--------------|-----|
| Add `Paused` AgentStateGroup | Requires Claude Code to emit `PauseRequested`/`ResumeRequested` hooks — it doesn't |
| Add `WorkOutputReady` hook handler | Claude Code doesn't emit this hook — would be dead code |
| Normalize acting/delegating/thinking to `agent_working` | Loses useful granularity; the flickering fix doesn't require this |
| 15-minute timeout | Too long; 60s is correct once hooks are properly prioritized |
| Dashboard pause/resume buttons | No backend mechanism exists to pause Claude Code |

---

## Documented Decisions (Accepted by Design)

These are intentional design choices that were reviewed during audit and confirmed as correct.
They are NOT risks — they are load-bearing decisions.

### 1. `AppState::new()` / `new_with_indexing()` Create Independent StateResolvers

**Why this is correct:** These constructors are used by tests and by `create_app_with_indexing_and_static()`.
They do NOT start `LiveSessionManager` — only `create_app_full()` does. Since no manager is running,
there is no process detector to race with, and the independent resolver correctly serves the hook
endpoint. Tests that need to verify the shared resolver path must use `create_app_full()`.

**Risk of "fixing" this:** If we changed these constructors to require a `StateResolver` parameter,
every test file in `routes/` would need refactoring to construct and pass one. The independent
resolver is the simpler, correct choice for non-live-session contexts.

### 2. Phase 3 Holds `sessions.write()` Across `.await` Loop

**Why this is correct for now:** `resolve()` only acquires `hook_states.read()` and `jsonl_states.read()` —
microsecond-scale operations. For <20 active sessions (typical), total Phase 3 lock time is ~milliseconds.

**Scalability note:** If the system grows to 100+ concurrent sessions, Phase 3's lock duration could
delay hook delivery. Future mitigation options:
- **Per-session fine-grained locks:** Replace `LiveSessionMap` with a concurrent map (e.g., `dashmap`)
  so each session can be locked independently.
- **Batch resolve:** Call `resolve()` for all pending sessions in a single batch, reducing lock
  acquisitions from N to 1.

This is NOT a current concern and should only be revisited if monitoring shows hook delivery latency.

---

## Coverage Verification

These notes confirm that all state-setting paths are addressed by this plan.

### All Paths That Set `session.agent_state`:

| Path | File:Line | Fix Applied |
|------|-----------|-------------|
| Process detector loop | `manager.rs:323` | Step 1d: Phase 3 calls `resolve()` |
| JSONL update (file watcher) | `manager.rs:587` | Step 1e: calls `resolve()` after dropping locks |
| Hook handler | `hooks.rs:58` | Already correct: sets from hook directly, also calls `update_from_hook()` |

### File Watcher Event Path (Already Correct):

The file watcher (`spawn_file_watcher`, lines 238-275) calls `process_jsonl_update()` which (after
Step 1e) applies the resolved state. It then reads the session from `sessions` map and sends
`SessionUpdated` / `SessionDiscovered` events. Since the session was already updated with the
resolved state by `process_jsonl_update()`, the event contains the correct state. No additional
fix needed.

### Initial Scan (Already Correct):

The initial scan (lines 215-225) calls `process_jsonl_update()` for each discovered file. At this
point, no hooks have arrived, so the resolver correctly falls through to JSONL-derived state.
This is the expected behavior for first discovery.

---

## Success Criteria

- No flicker between needs_you <-> autonomous when hooks are arriving (test with 2-minute hook drought + active process)
- `state_resolver.resolve()` is called in BOTH state-setting paths (process detector + JSONL update)
- `cleanup_stale()` runs periodically (prevents unbounded memory growth)
- Existing hook behavior unchanged (all current hook events still work)
- New `work_delivered` state appears for PR creation patterns via `PauseReason::WorkDelivered` (type-safe, no string matching)
- All existing tests still pass
- New StateResolver tests cover: expiry logic (4 pure-function tests), priority (2 async tests), cleanup (1 async test), fallback (1 async test)
- No `Instant` manipulation in tests — all tests safe on any system uptime

---

## Changelog of Fixes Applied (Audit -> Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | **Wrong root cause:** Plan blamed "timeout-based expiry in state_resolver" but the real cause is the process detector loop in `manager.rs:spawn_process_detector()` overwriting hook states every 5s via `session.agent_state = acc.agent_state.clone()` without consulting `state_resolver.resolve()` | **Blocker** | Rewrote entire root cause analysis and implementation steps. Main fix is now in `manager.rs`, not `state_resolver.rs` |
| 2 | **`state_resolver.resolve()` is dead code** — never called anywhere in the codebase. Plan proposed modifying it but that changes nothing | **Blocker** | Fix now wires `resolve()` into both state-setting paths (process detector + JSONL update) |
| 3 | **Wrong confidence type:** Plan used `confidence: f64` but actual type is `f32` (state.rs:20) | **Blocker** | Removed the incorrect `AgentState` struct definition from plan. Current definition is correct. |
| 4 | **Wrong StateCategory enum:** Plan proposed `Autonomous` and `Paused` variants. Actual enum has `Terminal`, `Blocking`, `Transient` | **Blocker** | Removed incorrect enum. Current categories are correct. Plan no longer proposes changing them. |
| 5 | **Fictional hook events:** `WorkOutputReady`, `PauseRequested`, `ResumeRequested` don't exist in Claude Code's hook system | **Blocker** | Removed all fictional hook handlers. Added structural detection (PR pattern) instead. |
| 6 | **Tests won't compile:** Used `block_on()` (not imported) and called async methods from `#[test]` (needs `#[tokio::test]`). Also used `f64` confidence values. | **Blocker** | Rewrote all tests as `#[tokio::test] async` with correct `f32` confidence and manual timestamp aging. |
| 7 | **Plan ignores manager.rs entirely** — the file where state is actually set (lines 305-330 process detector, line 587 JSONL update) | **Blocker** | Made `manager.rs` the primary target of the fix |
| 8 | **Missing cleanup_stale() background task** — hook states accumulate in memory indefinitely | **Warning** | Added cleanup_stale() call to spawn_cleanup_task() |
| 9 | **Proposed `Paused` AgentStateGroup** — adds complexity with no backend mechanism to trigger it | **Warning** | Deferred. No changes to AgentStateGroup enum. |
| 10 | **Proposed normalizing acting/delegating/thinking** — loses useful granularity | **Warning** | Removed. These states are fine as-is. |
| 11 | **15-minute timeout** — too long for genuinely idle detection | **Warning** | Kept existing 60s timeout. Once hooks are prioritized, 60s is appropriate. |
| 12 | **`crates/core/src/lib.rs` listed as file to modify** — no state types live there | **Warning** | Removed from files list |
| 13 | **Open Questions section had unanswerable questions** — WorkOutputReady format, Pause/Resume mechanism | **Minor** | Removed. These are deferred features, not immediate concerns. |
| 14 | **Plan said `AgentState` had `Serialize, Deserialize` derives missing** — actual code already has them | **Minor** | Removed incorrect struct definition from plan |
| 15 | **Integration test scenarios referenced WorkOutputReady hook** — doesn't exist | **Minor** | Rewrote scenarios to test the actual fix (hook priority in resolve()) |

### Round 2: Adversarial Review Fixes

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 16 | **Deadlock risk:** Plan called `.await` on resolver inside `sessions.write()` + `accumulators.write()` guards. Hook handler needs `live_sessions.write()` — blocked for entire loop duration | **Blocker** | Rewrote process detector loop: Phase 1 collects under locks, Phase 2 resolves after dropping locks, Phase 3 applies under fresh lock |
| 17 | **Wiring bug:** Plan said `resolver.clone()` but didn't explicitly REPLACE `StateResolver::new()` in AppState constructor. Implementer could create two separate resolvers, silently making the fix ineffective | **Blocker** | Added explicit comment + full AppState constructor snippet showing the replacement |
| 18 | **`hook_states` is private:** Tests access `resolver.hook_states.write().await` to age timestamps, but field is private | **Blocker** | Added explicit code block changing `hook_states` to `pub(crate)` |
| 19 | **`Instant` subtraction can panic:** `Instant::now() - Duration` panics if duration > system uptime | **Warning** | Changed all test timestamp aging to use `checked_sub().unwrap()` |
| 20 | **`process_jsonl_update` lock ordering:** resolver calls added while `accumulators` write lock held | **Warning** | Clarified that resolver calls go AFTER `drop(accumulators)` and BEFORE `sessions.write()`, with a note about capturing accumulator values first |

### Round 3: Independent 4-Agent Audit Fixes

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 21 | **TOCTOU race in process detector:** `resolve()` was called in Phase 2 (no locks), but a hook arriving between Phase 2 and Phase 3 would have its fresh state overwritten by Phase 3's stale resolved value. This IS the exact flicker the plan aims to fix. | **Blocker** | Moved `resolve()` from Phase 2 into Phase 3 (under `sessions.write()`). `update_from_jsonl()` stays in Phase 2. Safe from deadlock: hook handler releases `hook_states.write()` before acquiring `sessions.write()`, so no circular dependency. |
| 22 | **Step 1e code block contradicts itself:** Showed `agent_state: resolved_state` in LiveSession constructor, but `resolved_state` doesn't exist until after locks are dropped (LiveSession is built before drops). NOTE at bottom contradicted the code. | **Blocker** | Rewrote Step 1e: build `mut live_session` as-is, drop locks, resolve, mutate `live_session.agent_state`, then insert. Added explanation of why `mut` is needed. |
| 23 | **Self-contradictory `.await` guidance:** Lines 137-140 warned "Do NOT call `.await` while holding write guards" but Phase 3 intentionally does `resolve().await` under `sessions.write()`. An implementer following the warning literally would restructure Phase 3 and reintroduce the TOCTOU bug. | **Warning** | Replaced blanket warning with per-phase lock discipline explanation. Phase 3's `.await` under `sessions.write()` is documented as intentional with safety proof. |
| 24 | **PR pattern placement ambiguity:** Plan said "add before the commit/push pattern check" without anchoring to a line number. If placed after, a PR creation mentioning "commit" would match the wrong pattern. | **Warning** | Added explicit line reference (~120) and reasoning about pattern specificity ordering. |

### Round 4: Final 100/100 Fixes

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 25 | **`checked_sub().unwrap()` in tests panics on CI VMs with <2h uptime:** `Instant::now().checked_sub(Duration::from_secs(7200))` returns `None` on short-uptime systems, `.unwrap()` panics. | **Important** | Eliminated ALL `Instant` manipulation from tests. Expiry logic now tested via pure-function `is_expired()` with `Duration` values. Cleanup tested with short real-time durations (50ms sleep + 10ms threshold). |
| 26 | **`pub(crate)` visibility leak on `hook_states`:** Plan exposed internal `hook_states` field for test timestamp aging. This leaks implementation details — tests should not depend on internal storage format. | **Important** | Removed `pub(crate)` from `hook_states`. Extracted `pub(crate) fn is_expired(state, elapsed) -> bool` instead — tests access expiry logic through a clean function boundary, not internal state. |
| 27 | **Fragile `c.label.contains("PR ready")` string matching in Step 5:** The mapping function `pause_classification_to_agent_state()` checked label text to distinguish PR-created from generic task-complete. If the label text changes in classifier.rs, the mapping breaks silently with no compile error. | **Important** | Added `PauseReason::WorkDelivered` enum variant. Classifier returns typed reason; mapping uses exhaustive pattern match. Compile-time safe — new variants require explicit handling. |
| 28 | **Orphan resolvers in `AppState::new()` / `new_with_indexing()` undocumented:** These constructors create independent `StateResolver::new()` instances that are NOT shared with `LiveSessionManager`. An implementer could assume this is a bug and "fix" it by threading a shared resolver through, causing unnecessary refactoring. | **Important** | Added "Documented Decisions" section explaining why independent resolvers are correct (these constructors don't start LiveSessionManager; only `create_app_full()` does). |
| 29 | **Step 3 marked "Optional, Minor":** `TRANSIENT_EXPIRY_SECS` constant was presented as optional. Without it, the magic number `60` appears in both `resolve()` and `is_expired()`, making the threshold inconsistent if one is changed without the other. | **Warning** | Made Step 3 mandatory. Renamed to "Extract Expiry Constant and Testable `is_expired()` Function". |
| 30 | **Phase 3 scalability concern undocumented:** Phase 3 holds `sessions.write()` across `.await` for all pending sessions. Without documentation of the acceptable bound and future mitigation, an implementer might restructure it (reintroducing the TOCTOU race) or ignore the concern until it causes production latency. | **Warning** | Added concrete scalability note in "Documented Decisions" section with two future mitigation options (per-session locking, batch resolve). |
| 31 | **File watcher event path correctness unverified:** The plan addresses process detector (Step 1d) and `process_jsonl_update` (Step 1e) but never explicitly confirms that the file watcher's `SessionUpdated` event emission (manager.rs:250-261) and initial scan path (manager.rs:215-225) are already correct. An implementer might add unnecessary resolve calls to these paths. | **Warning** | Added "Coverage Verification" section with a table of all 3 state-setting paths and explicit notes confirming the file watcher and initial scan paths require no changes. |
