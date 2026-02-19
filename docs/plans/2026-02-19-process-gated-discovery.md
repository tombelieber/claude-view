# Process-Gated Session Discovery Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Prevent dead sessions from appearing as "running" by making hooks the sole session lifecycle authority, with process detection as the only recovery path on server restart.

**Architecture:** Remove the JSONL watcher's ability to create sessions. Reorder startup so the process detector runs first. Gate initial-scan session creation on process existence. JSONL watcher only enriches existing sessions.

**Tech Stack:** Rust, Axum, tokio, sysinfo

---

### Task 1: Remove the fallback session creation from `process_jsonl_update`

**Files:**
- Modify: `crates/server/src/live/manager.rs:805-845`

**Step 1: Delete the `else` branch**

In `process_jsonl_update`, replace the session-map update block (lines 805-845) with:

```rust
        // Update the shared session map — metadata only, hooks own agent_state/status.
        // NEVER create sessions here. Only hooks (SessionStart) and startup recovery
        // (process-gated) create sessions. If no session exists, the accumulator holds
        // the metadata until a hook or recovery creates the session entry.
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            apply_jsonl_metadata(session, &metadata, &file_path_str, &project, &project_display_name, &project_path);
        }
        // else: no session in map — accumulator is populated, metadata will be applied
        // when SessionStart hook or startup recovery creates the session entry.
```

**Step 2: Run tests to verify nothing breaks**

Run: `cargo test -p vibe-recall-server -- manager`
Expected: All existing tests pass (they test helper functions, not session creation).

**Step 3: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "fix(live): remove fallback session creation from JSONL watcher

The JSONL watcher no longer creates sessions with fabricated
Autonomous/unknown state. It only enriches sessions that already
exist in the live map (created by hooks or startup recovery)."
```

---

### Task 2: Extract eager process scan into a reusable method

**Files:**
- Modify: `crates/server/src/live/manager.rs`

**Step 1: Add `run_eager_process_scan` method**

Add this method to the `impl LiveSessionManager` block, after `remove_accumulator` (line 224):

```rust
    /// Run a one-shot process table scan and store results.
    ///
    /// Called at startup BEFORE the initial JSONL scan so that process-gated
    /// session recovery can check for live processes.
    async fn run_eager_process_scan(&self) {
        let new_processes = tokio::task::spawn_blocking(detect_claude_processes)
            .await
            .unwrap_or_default();
        let count = new_processes.len();
        let mut processes = self.processes.write().await;
        *processes = new_processes;
        info!("Eager process scan found {} Claude processes", count);
    }
```

**Step 2: Run tests**

Run: `cargo test -p vibe-recall-server -- manager`
Expected: PASS (new method, not yet called).

**Step 3: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "refactor(live): extract eager process scan method"
```

---

### Task 3: Add `recover_session_from_metadata` helper

**Files:**
- Modify: `crates/server/src/live/manager.rs`

**Step 1: Add the recovery helper method**

Add this method to `impl LiveSessionManager`, after `run_eager_process_scan`:

```rust
    /// Create a recovered session from JSONL metadata + a confirmed live process.
    ///
    /// Used at startup to recover sessions that were running before a server restart.
    /// State is always NeedsYou/idle — the next hook corrects it.
    fn create_recovered_session(
        metadata: &JsonlMetadata,
        session_id: &str,
        file_path: &str,
        project: &str,
        project_display_name: &str,
        project_path: &str,
    ) -> LiveSession {
        let recovery_state = AgentState {
            group: AgentStateGroup::NeedsYou,
            state: "idle".into(),
            label: "Waiting for your next prompt".into(),
            context: None,
        };
        LiveSession {
            id: session_id.to_string(),
            project: project.to_string(),
            project_display_name: project_display_name.to_string(),
            project_path: project_path.to_string(),
            file_path: file_path.to_string(),
            status: status_from_agent_state(&recovery_state),
            agent_state: recovery_state,
            git_branch: metadata.git_branch.clone(),
            pid: metadata.pid,
            title: metadata.title.clone(),
            last_user_message: metadata.last_user_message.clone(),
            current_activity: "Waiting for your next prompt".into(),
            turn_count: metadata.turn_count,
            started_at: metadata.started_at,
            last_activity_at: metadata.last_activity_at,
            model: metadata.model.clone(),
            tokens: metadata.tokens.clone(),
            context_window_tokens: metadata.context_window_tokens,
            cost: metadata.cost.clone(),
            cache_status: metadata.cache_status.clone(),
            current_turn_started_at: metadata.current_turn_started_at,
            last_turn_task_seconds: metadata.last_turn_task_seconds,
            sub_agents: metadata.sub_agents.clone(),
            progress_items: metadata.progress_items.clone(),
            state_epoch: 0,
            epoch_active: false, // not active until hook says so
        }
    }
```

**Step 2: Write test for recovery state**

Add to `mod tests` at the bottom of `manager.rs`:

```rust
    #[test]
    fn test_recovered_session_is_paused() {
        use vibe_recall_core::cost::{CacheStatus, CostBreakdown, TokenUsage};

        let metadata = JsonlMetadata {
            git_branch: Some("main".into()),
            pid: Some(1234),
            title: "test session".into(),
            last_user_message: "hello".into(),
            turn_count: 5,
            started_at: Some(1000),
            last_activity_at: 2000,
            model: Some("claude-opus-4-6".into()),
            tokens: TokenUsage::default(),
            context_window_tokens: 0,
            cost: CostBreakdown::default(),
            cache_status: CacheStatus::Unknown,
            current_turn_started_at: None,
            last_turn_task_seconds: None,
            sub_agents: Vec::new(),
            progress_items: Vec::new(),
        };

        let session = LiveSessionManager::create_recovered_session(
            &metadata, "test-id", "/path/to/file.jsonl",
            "project", "project", "/decoded/project",
        );

        assert_eq!(session.status, SessionStatus::Paused);
        assert_eq!(session.agent_state.group, AgentStateGroup::NeedsYou);
        assert_eq!(session.agent_state.state, "idle");
        assert!(!session.epoch_active);
        assert_eq!(session.pid, Some(1234));
        assert_eq!(session.title, "test session");
    }
```

**Step 3: Run test**

Run: `cargo test -p vibe-recall-server -- manager::tests::test_recovered_session_is_paused`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "feat(live): add create_recovered_session helper for startup recovery"
```

---

### Task 4: Rewrite `spawn_file_watcher` startup sequence with process-gated recovery

**Files:**
- Modify: `crates/server/src/live/manager.rs:231-264` (the initial scan section of `spawn_file_watcher`)

**Step 1: Rewrite the initial scan to use process-gated recovery**

Replace lines 234-264 (inside the `tokio::spawn` block, from `// Initial scan` through the discovery loop) with:

```rust
        tokio::spawn(async move {
            // --- Startup recovery sequence ---
            // 1. Eager process scan FIRST — builds the process table before JSONL scan
            manager.run_eager_process_scan().await;

            // 2. Initial JSONL scan — build accumulators, gate session creation on process
            let projects_dir = match dirs::home_dir() {
                Some(home) => home.join(".claude").join("projects"),
                None => {
                    warn!("Could not determine home directory; skipping initial scan");
                    return;
                }
            };

            let initial_paths = {
                let dir = projects_dir.clone();
                tokio::task::spawn_blocking(move || initial_scan(&dir))
                    .await
                    .unwrap_or_default()
            };

            info!("Initial scan found {} recent JSONL files", initial_paths.len());

            // Process each JSONL file: builds accumulators for all,
            // but only creates sessions for those with a live process.
            let mut recovered_count = 0u32;
            for path in &initial_paths {
                // process_jsonl_update populates the accumulator but (after Task 1)
                // no longer creates sessions. We handle recovery here.
                manager.process_jsonl_update(path).await;

                let session_id = extract_session_id(path);

                // Check if a session was already created (e.g., a hook arrived
                // between process scan and now — race-safe)
                let already_exists = {
                    let sessions = manager.sessions.read().await;
                    sessions.contains_key(&session_id)
                };
                if already_exists {
                    continue;
                }

                // Process-gated: only create session if a live process exists
                let (project, project_display_name, project_path) = extract_project_info(path);
                let has_process = {
                    let processes = manager.processes.read().await;
                    has_running_process(&processes, &project_path).0
                };

                if has_process {
                    // Read the accumulated metadata to build the recovered session
                    let accumulators = manager.accumulators.read().await;
                    if let Some(acc) = accumulators.get(&session_id) {
                        let file_path_str = path.to_str().unwrap_or("").to_string();
                        let last_activity_at = std::fs::metadata(path)
                            .and_then(|m| m.modified())
                            .ok()
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);
                        let seconds_since = seconds_since_modified_from_timestamp(last_activity_at);
                        let cost = vibe_recall_core::cost::calculate_live_cost(
                            &acc.tokens, acc.model.as_deref(), &manager.pricing,
                        );
                        let cache_status = vibe_recall_core::cost::derive_cache_status(Some(seconds_since));
                        let processes = manager.processes.read().await;
                        let (_, pid) = has_running_process(&processes, &project_path);
                        drop(processes);

                        let metadata = JsonlMetadata {
                            git_branch: acc.git_branch.clone(),
                            pid,
                            title: acc.first_user_message.clone(),
                            last_user_message: acc.last_user_message.clone(),
                            turn_count: acc.user_turn_count,
                            started_at: acc.started_at,
                            last_activity_at,
                            model: acc.model.clone(),
                            tokens: acc.tokens.clone(),
                            context_window_tokens: acc.context_window_tokens,
                            cost,
                            cache_status,
                            current_turn_started_at: acc.current_turn_started_at,
                            last_turn_task_seconds: acc.last_turn_task_seconds,
                            sub_agents: acc.sub_agents.clone(),
                            progress_items: {
                                let mut items = acc.todo_items.clone();
                                items.extend(acc.task_items.clone());
                                items
                            },
                        };
                        drop(accumulators);

                        let session = LiveSessionManager::create_recovered_session(
                            &metadata, &session_id, &file_path_str,
                            &project, &project_display_name, &project_path,
                        );
                        let mut sessions = manager.sessions.write().await;
                        sessions.insert(session_id.clone(), session.clone());
                        drop(sessions);

                        let _ = manager.tx.send(SessionEvent::SessionDiscovered { session });
                        recovered_count += 1;
                    }
                }
            }

            info!(
                "Startup recovery: {} sessions recovered from {} JSONL files",
                recovered_count,
                initial_paths.len()
            );
```

Keep the rest of `spawn_file_watcher` unchanged (file watcher setup + event loop starting at line 266).

**Step 2: Also remove `is_new` logic from catch-up scan**

In the catch-up scan (lines 295-310), sessions are no longer created by `process_jsonl_update`, so `is_new` will always remain true but the session won't be in the map. Simplify the catch-up loop:

```rust
                    for path in &catchup_paths {
                        manager.process_jsonl_update(path).await;
                        // Sessions are only created by hooks — no discovery broadcast needed.
                        // If a hook already created the session, process_jsonl_update enriched it.
                    }
```

**Step 3: Simplify ongoing file event handling**

In the `FileEvent::Modified` handler (lines 313-335), `process_jsonl_update` no longer creates sessions, so `is_new` logic is unnecessary. Simplify:

```rust
                    FileEvent::Modified(path) => {
                        let session_id = extract_session_id(&path);
                        manager.process_jsonl_update(&path).await;

                        // Broadcast update if session exists (created by hook)
                        let sessions = manager.sessions.read().await;
                        if let Some(session) = sessions.get(&session_id) {
                            let _ = manager.tx.send(SessionEvent::SessionUpdated {
                                session: session.clone(),
                            });
                        }
                    }
```

**Step 4: Run full server tests**

Run: `cargo test -p vibe-recall-server`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add crates/server/src/live/manager.rs
git commit -m "feat(live): process-gated session recovery on startup

Rewritten startup sequence:
1. Eager process scan runs first
2. Initial JSONL scan builds accumulators for all recent files
3. Sessions only created for files with a matching live process
4. JSONL file events only enrich existing sessions (never create)

Dead sessions no longer appear as 'running' after server restart."
```

---

### Task 5: End-to-end verification

**Files:** None (manual testing)

**Step 1: Build the server**

Run: `cargo build -p vibe-recall-server`
Expected: Compiles without warnings.

**Step 2: Start the dev server**

Run: `bun run dev` (in a separate terminal)

**Step 3: Verify active sessions appear**

Open Mission Control in the browser. Sessions with running Claude processes should appear as Paused (NeedsYou/idle). When you submit a prompt in one of those sessions, it should transition to Working via the hook.

**Step 4: Verify dead sessions don't appear**

Session `26d80a19-c84b-419c-80c6-99452f7de8c0` (and any other dead sessions) should NOT appear in the live monitor.

**Step 5: Verify new sessions work normally**

Open a new Claude Code session. It should appear in Mission Control when the SessionStart hook fires, with correct state transitions.

**Step 6: Commit any fixups if needed**

---

### Task 6: Update design doc status

**Files:**
- Modify: `docs/plans/2026-02-19-process-gated-discovery-design.md` (frontmatter status → `done`)
- Modify: `docs/plans/PROGRESS.md` (update plan file index)

**Step 1: Update frontmatter**

Change `status: approved` to `status: done`.

**Step 2: Commit**

```bash
git add docs/plans/
git commit -m "docs: mark process-gated discovery as done"
```
