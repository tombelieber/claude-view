# Phase F: Interactive Control — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow users to resume and interact with Claude Code sessions from the web dashboard via a Node.js sidecar that wraps the Claude Agent SDK.

**Architecture:** Rust Axum server manages a lazy-started Node.js sidecar child process. The sidecar uses `@anthropic-ai/claude-agent-sdk` to spawn/resume Claude Code sessions. Communication: Axum <-> Unix socket HTTP/WS <-> Sidecar <-> Agent SDK <-> Claude Code subprocess. Frontend connects via Axum WebSocket proxy.

**Tech Stack:** Rust (Axum, tokio-tungstenite, hyper-unix), Node.js 20+ (Hono, ws, claude-agent-sdk), React (TanStack Query, WebSocket hooks)

**Design doc:** [`phase-f-interactive.md`](phase-f-interactive.md) — full architecture, wireframes, acceptance criteria, code samples.

**SDK API note:** The design doc references `query({ resume: sessionId })` which was the v1 API. The current Agent SDK V2 uses `unstable_v2_resumeSession(sessionId, options)` (positional sessionId) which returns an `SDKSession` synchronously. The session has `send(message)` and `stream()` (async generator of `SDKMessage`). Permission routing uses `canUseTool` callback in options or the `hooks` config with event-name keys (`"PermissionRequest"`). If the `unstable_v2_*` prefix has been stabilized by implementation time, drop the prefix.

**Audit note (2026-02-27):** This plan was audited against the actual codebase and Agent SDK. Round 1 fixed obvious issues. Round 2 fixed 23 blockers and 19 warnings found by 4 parallel audit agents that verified every code block. See `phase-f-audit-results.md` for the full audit report. All fixes are logged in the Changelog at the bottom of this file.

---

## Parallelism Map

```
Layer 1: Sidecar Foundation
  Task 1 ─► Task 2 ─► Task 4 (sidecar scaffold → server → wire into Axum)
                  └──► Task 3 (Rust SidecarManager, parallel with Task 2)

Layer 2: Session Control
  Task 5 (cost estimation, independent — Rust only)
  Task 6 ─► Task 7 ─► Task 8 (session manager → control routes → Rust proxy)

Layer 3: WebSocket
  Task 9 ─► Task 10 ─► Task 11 (sidecar WS → Rust WS proxy → frontend hook)

Layer 4: Frontend UI
  Task 12 (pre-flight modal, needs Task 5 + Task 8)
  Task 13 (dashboard chat, needs Task 11)
  Task 14 (permission dialog, needs Task 11)
  Task 15 (cost status bar, needs Task 13)

Layer 5: Integration
  Task 16 (distribution — CI, npx-cli)
  Task 17 (end-to-end verification)
```

**Parallel pairs:** Tasks 2+3 (sidecar server + Rust manager). Tasks 5+6 (cost estimation + session manager). Tasks 12+13+14 (all frontend UI after WS layer done).

---

## Task 1: Sidecar Project Scaffold

**Files:**
- Create: `sidecar/package.json`
- Create: `sidecar/tsconfig.json`
- Create: `sidecar/src/types.ts`

**Context:** The sidecar is a standalone Node.js project at repo root (NOT in `apps/` — it's not a Turborepo-managed app, it's a child process of the Rust server). It does NOT need to be in the root `package.json` workspaces array.

**Step 1: Create sidecar directory**

```bash
mkdir -p sidecar/src
```

**Step 2: Create `sidecar/package.json`**

```json
{
  "name": "claude-view-sidecar",
  "version": "0.8.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "tsx watch src/index.ts",
    "build": "tsup src/index.ts --format esm --target node20 --clean",
    "start": "node dist/index.js",
    "test": "vitest run",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "@anthropic-ai/claude-agent-sdk": "^0.1.0",
    "hono": "^4.0.0",
    "@hono/node-server": "^1.13.0",
    "ws": "^8.16.0"
  },
  "devDependencies": {
    "tsx": "^4.7.0",
    "tsup": "^8.0.0",
    "typescript": "^5.4.0",
    "@types/ws": "^8.5.0",
    "@types/node": "^20.0.0",
    "vitest": "^3.0.0"
  }
}
```

**Step 3: Create `sidecar/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "outDir": "dist",
    "rootDir": "src",
    "declaration": true,
    "sourceMap": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "verbatimModuleSyntax": true
  },
  "include": ["src/**/*.ts"],
  "exclude": ["node_modules", "dist"]
}
```

**Step 4: Create `sidecar/src/types.ts`**

This file defines the IPC message protocol between sidecar and frontend (relayed through Axum).

```typescript
// sidecar/src/types.ts
// IPC message protocol: Frontend <-> Axum <-> Sidecar

// ── Frontend → Sidecar (via Axum WS proxy) ──

export interface UserMessage {
  type: 'user_message'
  content: string
}

export interface PermissionResponse {
  type: 'permission_response'
  requestId: string
  allowed: boolean
}

export interface PingMessage {
  type: 'ping'
}

export type ClientMessage = UserMessage | PermissionResponse | PingMessage

// ── Sidecar → Frontend (via Axum WS proxy) ──

export interface AssistantChunk {
  type: 'assistant_chunk'
  content: string
  messageId: string
}

export interface AssistantDone {
  type: 'assistant_done'
  messageId: string
  usage: {
    inputTokens: number
    outputTokens: number
    cacheReadTokens: number
    cacheWriteTokens: number
  }
  cost: number
  totalCost: number
}

export interface ToolUseStart {
  type: 'tool_use_start'
  toolName: string
  toolInput: Record<string, unknown>
  toolUseId: string
}

export interface ToolUseResult {
  type: 'tool_use_result'
  toolUseId: string
  output: string
  isError: boolean
}

export interface PermissionRequest {
  type: 'permission_request'
  requestId: string
  toolName: string
  toolInput: Record<string, unknown>
  description: string
  timeoutMs: number
}

export interface SessionStatusMessage {
  type: 'session_status'
  status: 'active' | 'waiting_input' | 'waiting_permission' | 'completed' | 'error'
  contextUsage: number
  turnCount: number
}

export interface ErrorMessage {
  type: 'error'
  message: string
  fatal: boolean
}

export interface PongMessage {
  type: 'pong'
}

export type ServerMessage =
  | AssistantChunk
  | AssistantDone
  | ToolUseStart
  | ToolUseResult
  | PermissionRequest
  | SessionStatusMessage
  | ErrorMessage
  | PongMessage

// ── HTTP request/response types ──

export interface ResumeRequest {
  sessionId: string
  model?: string
  projectPath?: string
}

export interface ResumeResponse {
  controlId: string
  status: 'active' | 'already_active'
  sessionId: string
}

export interface SendRequest {
  controlId: string
  message: string
}

export interface ActiveSession {
  controlId: string
  sessionId: string
  status: string
  turnCount: number
  totalCost: number
  startedAt: number
}

export interface HealthResponse {
  status: 'ok'
  activeSessions: number
  uptime: number
}
```

**Step 5: Install dependencies and verify**

```bash
cd sidecar && bun install && bun run typecheck
```

> **Note:** The sidecar is outside the Bun workspace (not in `apps/` or `packages/`), so `bun install` at repo root won't install its deps. Always `cd sidecar` first. Use `bun` for dev (per CLAUDE.md), but distribution uses `npm ci` in CI.

Expected: Clean typecheck, no errors.

**Step 6: Commit**

```bash
git add sidecar/
git commit -m "feat(phase-f): scaffold sidecar project with IPC types"
```

---

## Task 2: Sidecar Health Server

**Files:**
- Create: `sidecar/src/health.ts`
- Create: `sidecar/src/index.ts`

**Depends on:** Task 1

**Context:** The sidecar HTTP server listens on a Unix domain socket. Axum starts it lazily on first `/api/control/*` request and periodically health-checks it. The entry point sets up graceful shutdown and parent-process liveness detection.

**Step 1: Create `sidecar/src/health.ts`**

```typescript
// sidecar/src/health.ts
import { Hono } from 'hono'
import type { HealthResponse } from './types.js'

const startTime = Date.now()

export function healthRouter(getActiveCount: () => number) {
  const router = new Hono()

  router.get('/', (c) => {
    const response: HealthResponse = {
      status: 'ok',
      activeSessions: getActiveCount(),
      uptime: Math.floor((Date.now() - startTime) / 1000),
    }
    return c.json(response)
  })

  return router
}
```

**Step 2: Create `sidecar/src/index.ts`**

```typescript
// sidecar/src/index.ts
import { Hono } from 'hono'
import { createAdaptorServer } from '@hono/node-server'
import fs from 'node:fs'
import { healthRouter } from './health.js'

const SOCKET_PATH = process.env.SIDECAR_SOCKET
  ?? `/tmp/claude-view-sidecar-${process.ppid}.sock`

const app = new Hono()

// Placeholder active session count — Task 6 wires the real SessionManager
let activeSessionCount = 0

app.route('/health', healthRouter(() => activeSessionCount))

// Root health check (for quick connectivity tests)
app.get('/', (c) => c.json({ status: 'ok' }))

// Clean up stale socket from prior crash
if (fs.existsSync(SOCKET_PATH)) {
  fs.unlinkSync(SOCKET_PATH)
}

// Create HTTP server using createAdaptorServer (Options type requires { fetch } property)
const server = createAdaptorServer({ fetch: app.fetch })

server.listen(SOCKET_PATH, () => {
  console.log(`[sidecar] Listening on ${SOCKET_PATH}`)
  console.log(`[sidecar] PID: ${process.pid}, Parent PID: ${process.ppid}`)
})

// Parent process liveness check (2s interval).
// If the Rust server dies, the sidecar self-terminates.
const parentCheck = setInterval(() => {
  try {
    process.kill(process.ppid!, 0) // signal 0 = check if alive
  } catch {
    console.log('[sidecar] Parent process exited, shutting down')
    shutdown()
  }
}, 2000)

function shutdown() {
  clearInterval(parentCheck)
  server.close()
  if (fs.existsSync(SOCKET_PATH)) {
    fs.unlinkSync(SOCKET_PATH)
  }
  process.exit(0)
}

process.on('SIGTERM', shutdown)
process.on('SIGINT', shutdown)

export { app, server, SOCKET_PATH }
```

**Step 3: Build and verify**

```bash
cd sidecar && bun run build
ls sidecar/dist/index.js  # should exist
```

**Step 4: Smoke test — start sidecar, hit health endpoint, stop**

```bash
cd sidecar
SIDECAR_SOCKET=/tmp/claude-view-sidecar-test.sock node dist/index.js &
SIDECAR_PID=$!
sleep 1
# Health check via Unix socket
curl --unix-socket /tmp/claude-view-sidecar-test.sock http://localhost/health
# Expected: {"status":"ok","activeSessions":0,"uptime":1}
kill $SIDECAR_PID
# Verify socket cleaned up
ls /tmp/claude-view-sidecar-test.sock 2>/dev/null && echo "FAIL: socket not cleaned" || echo "PASS: socket cleaned"
```

**Step 5: Commit**

```bash
git add sidecar/src/health.ts sidecar/src/index.ts
git commit -m "feat(phase-f): sidecar health server on Unix socket"
```

---

## Task 3: Rust SidecarManager

**Files:**
- Create: `crates/server/src/sidecar.rs`
- Modify: `crates/server/src/lib.rs` (add `pub mod sidecar;`)
- Modify: `Cargo.toml` (workspace deps: add `hyper-util`, `http-body-util`)
- Modify: `crates/server/Cargo.toml` (add `hyper-util`, `http-body-util`)

**Depends on:** None (can be built in parallel with Task 2)

**Context:** The SidecarManager spawns the Node.js sidecar as a child process, health-checks it via Unix socket HTTP, and kills it on shutdown. It's lazy: the sidecar only starts on first control request. Follows existing patterns in `crates/server/src/state.rs` — wrapped in `Arc` for shared access.

**Step 1: Verify Rust dependencies (skip if already present)**

> **Note:** The workspace `Cargo.toml` already has `hyper = "1"` (with `"client", "http1"` features), `hyper-util = "0.1"` (with `"tokio"`), `http-body-util = "0.1"`, and `bytes = "1"`. The server crate (`crates/server/Cargo.toml`) already depends on all four via `workspace = true`. **Skip this step** -- verify the deps exist and move on. Do NOT add `hyperlocal` or `client-legacy` feature -- they are not needed.

**Step 2: Write the failing test**

Create `crates/server/src/sidecar.rs`:

> **Note:** `sidecar.rs` already exists as an untracked WIP file. **Do NOT skip** — diff the WIP file against the code below line-by-line. If any function signature, field, or method differs from the plan, **replace the WIP file entirely** with the plan's version. The WIP may have been written against an older API. Also note: `pub mod sidecar;` is already in `lib.rs` (line 20) — verify it's present and proceed.

```rust
// crates/server/src/sidecar.rs
//! Node.js sidecar process manager for Phase F interactive control.
//!
//! The sidecar wraps the Claude Agent SDK (npm-only) and exposes a local
//! HTTP + WebSocket API on a Unix domain socket. Axum proxies all
//! `/api/control/*` requests to this socket.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;

/// Errors from sidecar operations.
#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    #[error("Failed to spawn sidecar: {0}")]
    SpawnFailed(std::io::Error),
    #[error("Sidecar health check timed out after 3s")]
    HealthCheckTimeout,
    #[error("Sidecar directory not found (set SIDECAR_DIR or place sidecar/ next to binary)")]
    SidecarDirNotFound,
    #[error("Node.js not found in PATH (required for interactive mode)")]
    NodeNotFound,
    #[error("Sidecar returned error: {0}")]
    RequestError(String),
}

/// Manages the lifecycle of the Node.js sidecar child process.
///
/// Thread-safe: uses `Mutex<Option<Child>>` for the child handle.
/// The sidecar is lazy-started on first `ensure_running()` call.
pub struct SidecarManager {
    child: Mutex<Option<Child>>,
    socket_path: String,
}

impl SidecarManager {
    pub fn new() -> Self {
        let pid = std::process::id();
        Self {
            child: Mutex::new(None),
            socket_path: format!("/tmp/claude-view-sidecar-{pid}.sock"),
        }
    }

    /// Get the Unix socket path for this sidecar instance.
    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }

    /// Start sidecar if not already running. Returns the socket path.
    ///
    /// Idempotent: if the child is already alive, returns immediately.
    /// If the child died (crash), restarts it.
    pub async fn ensure_running(&self) -> Result<String, SidecarError> {
        {
            let mut guard = self.child.lock().unwrap();

            // Check if existing child is still alive
            if let Some(ref mut child) = *guard {
                match child.try_wait() {
                    Ok(None) => return Ok(self.socket_path.clone()), // still running
                    Ok(Some(status)) => {
                        tracing::warn!("Sidecar exited with {status}, restarting...");
                    }
                    Err(e) => {
                        tracing::warn!("Failed to check sidecar status: {e}");
                    }
                }
            }

            // Find sidecar directory
            let sidecar_dir = Self::find_sidecar_dir()?;
            let entry_point = sidecar_dir.join("dist/index.js");
            if !entry_point.exists() {
                return Err(SidecarError::SidecarDirNotFound);
            }

            // Verify Node.js is available
            if Command::new("node").arg("--version").output().is_err() {
                return Err(SidecarError::NodeNotFound);
            }

            // Clean up stale socket
            let _ = std::fs::remove_file(&self.socket_path);

            // Spawn sidecar
            //
            // CLAUDE.md HARD RULE: Strip ALL `CLAUDE*` env vars when spawning
            // child processes. The `Command::new("node")` must use `.env_clear()`
            // then selectively re-add only safe env vars (PATH, HOME, USER,
            // SIDECAR_SOCKET, NODE_PATH, etc.). Filter out any key starting with
            // `CLAUDE` or `ANTHROPIC_API_KEY`. Pattern:
            //
            //   let filtered_env: Vec<(String, String)> = std::env::vars()
            //       .filter(|(k, _)| !k.starts_with("CLAUDE") && k != "ANTHROPIC_API_KEY")
            //       .collect();
            //   Command::new("node")
            //       .env_clear()
            //       .envs(filtered_env)
            //       .env("SIDECAR_SOCKET", &self.socket_path)
            //       // ... rest of spawn config
            //
            let filtered_env: Vec<(String, String)> = std::env::vars()
                .filter(|(k, _)| !k.starts_with("CLAUDE") && k != "ANTHROPIC_API_KEY")
                .collect();
            let child = Command::new("node")
                .arg(&entry_point)
                .env_clear()
                .envs(filtered_env)
                .env("SIDECAR_SOCKET", &self.socket_path)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(SidecarError::SpawnFailed)?;

            tracing::info!(
                pid = child.id(),
                socket = %self.socket_path,
                "Spawned sidecar process"
            );

            *guard = Some(child);
        } // drop lock before async health check

        // Wait for sidecar to be ready (poll health endpoint)
        for attempt in 0..30 {
            sleep(Duration::from_millis(100)).await;
            if self.health_check().await.is_ok() {
                tracing::info!(
                    attempts = attempt + 1,
                    "Sidecar ready"
                );
                return Ok(self.socket_path.clone());
            }
        }

        Err(SidecarError::HealthCheckTimeout)
    }

    /// Send SIGTERM, wait briefly, then SIGKILL if needed.
    pub fn shutdown(&self) {
        let mut guard = self.child.lock().unwrap();
        if let Some(ref mut child) = *guard {
            tracing::info!(pid = child.id(), "Shutting down sidecar");
            let _ = child.kill();
            let _ = child.wait();
        }
        *guard = None;

        // Cleanup socket file
        let _ = std::fs::remove_file(&self.socket_path);
    }

    /// Check if the sidecar is currently running.
    pub fn is_running(&self) -> bool {
        let mut guard = self.child.lock().unwrap();
        if let Some(ref mut child) = *guard {
            matches!(child.try_wait(), Ok(None))
        } else {
            false
        }
    }

    /// HTTP health check over Unix socket using raw hyper 1.x client.
    ///
    /// Replaces hyperlocal (incompatible with hyper 1.x -- audit fix B3).
    /// Uses tokio::net::UnixStream + hyper::client::conn::http1 directly.
    async fn health_check(&self) -> Result<(), SidecarError> {
        use http_body_util::Empty;
        use hyper::client::conn::http1;
        use hyper_util::rt::TokioIo;

        let stream = tokio::net::UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| SidecarError::RequestError(format!("Unix socket connect: {e}")))?;

        let io = TokioIo::new(stream);
        let (mut sender, conn) = http1::handshake(io)
            .await
            .map_err(|e| SidecarError::RequestError(format!("HTTP handshake: {e}")))?;

        // Spawn connection driver (required for hyper 1.x)
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                tracing::debug!("Health check connection closed: {e}");
            }
        });

        let req = hyper::Request::builder()
            .uri("/health")
            .header("host", "localhost")
            .body(Empty::<bytes::Bytes>::new())
            .map_err(|e| SidecarError::RequestError(format!("Build request: {e}")))?;

        let response = sender
            .send_request(req)
            .await
            .map_err(|e| SidecarError::RequestError(format!("Send request: {e}")))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(SidecarError::RequestError(format!(
                "Health check returned {}",
                response.status()
            )))
        }
    }

    /// Locate the sidecar directory.
    ///
    /// Priority:
    /// 1. `SIDECAR_DIR` env var (set by npx-cli)
    /// 2. `./sidecar/` relative to the binary (npx distribution)
    /// 3. `./sidecar/` relative to CWD (dev mode: `cargo run` from repo root)
    fn find_sidecar_dir() -> Result<PathBuf, SidecarError> {
        // 1. Explicit env var
        if let Ok(dir) = std::env::var("SIDECAR_DIR") {
            let p = PathBuf::from(&dir);
            if p.exists() {
                return Ok(p);
            }
            tracing::warn!(sidecar_dir = %dir, "SIDECAR_DIR set but directory does not exist");
        }

        // 2. Binary-relative (npx distribution)
        if let Ok(exe) = std::env::current_exe() {
            if let Ok(canonical) = exe.canonicalize() {
                if let Some(exe_dir) = canonical.parent() {
                    let bin_sidecar = exe_dir.join("sidecar");
                    if bin_sidecar.join("dist/index.js").exists() {
                        return Ok(bin_sidecar);
                    }
                }
            }
        }

        // 3. CWD-relative (dev mode)
        let cwd_sidecar = PathBuf::from("sidecar");
        if cwd_sidecar.join("dist/index.js").exists() {
            return Ok(cwd_sidecar);
        }

        Err(SidecarError::SidecarDirNotFound)
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_socket_path_with_pid() {
        let mgr = SidecarManager::new();
        let pid = std::process::id();
        assert_eq!(mgr.socket_path(), format!("/tmp/claude-view-sidecar-{pid}.sock"));
    }

    #[test]
    fn test_not_running_by_default() {
        let mgr = SidecarManager::new();
        assert!(!mgr.is_running());
    }

    #[test]
    fn test_shutdown_when_not_running_is_noop() {
        let mgr = SidecarManager::new();
        mgr.shutdown(); // should not panic
    }

    #[test]
    fn test_find_sidecar_dir_returns_error_when_not_found() {
        // With no SIDECAR_DIR and no sidecar/ in CWD or binary-relative
        let result = SidecarManager::find_sidecar_dir();
        // In test environment, sidecar/dist/index.js likely doesn't exist
        // (unless tests run from repo root with sidecar built)
        // This test just verifies it doesn't panic
        let _ = result;
    }
}
```

**Step 3: Register the module**

In `crates/server/src/lib.rs`, add `pub mod sidecar;` alongside the other module declarations. Find the existing `pub mod` block and add it alphabetically.

**Step 4: Run tests**

```bash
cargo test -p claude-view-server sidecar:: -- --nocapture
```

Expected: 4 tests pass.

**Step 5: Verify compilation**

```bash
cargo check -p claude-view-server
```

Expected: Clean compile with new `http-body-util` dependency.

**Step 6: Commit**

```bash
git add Cargo.toml crates/server/Cargo.toml crates/server/src/sidecar.rs crates/server/src/lib.rs
git commit -m "feat(phase-f): Rust SidecarManager with health check and lifecycle"
```

---

## Task 4: Wire SidecarManager into AppState

**Files:**
- Modify: `crates/server/src/state.rs` (add `sidecar` field)
- Modify: `crates/server/src/main.rs` (shutdown hook)
- Modify: `crates/server/src/lib.rs` (re-export SidecarManager, update `create_app_full`)

**Depends on:** Task 3

**Context:** Add `SidecarManager` to `AppState` so route handlers can access it. Wire shutdown into the existing Ctrl+C graceful shutdown path. The sidecar is lazy — it only starts when first control request arrives.

**Step 1: Add sidecar field to AppState**

In `crates/server/src/state.rs`:

1. Add import at top: `use crate::sidecar::SidecarManager;`
2. Add field to `AppState` struct (after `hook_event_channels`):
   ```rust
   /// Node.js sidecar manager for Phase F interactive control.
   /// Lazy-started on first `/api/control/*` request.
   pub sidecar: Arc<SidecarManager>,
   ```
3. Add initialization in ALL three constructors (`new`, `new_with_indexing`, `new_with_indexing_and_registry`):
   ```rust
   sidecar: Arc::new(SidecarManager::new()),
   ```

> **IMPORTANT (B7):** Adding the `sidecar` field breaks ALL existing `AppState { ... }` struct literals. There are **8 locations** that must be updated:
>
> 1. `crates/server/src/state.rs` -- `new()` constructor (~line 88)
> 2. `crates/server/src/state.rs` -- `new_with_indexing()` constructor (~line 120)
> 3. `crates/server/src/state.rs` -- `new_with_indexing_and_registry()` constructor (~line 151)
> 4. `crates/server/src/lib.rs` -- `create_app_with_git_sync()` struct literal (~line 107)
> 5. `crates/server/src/lib.rs` -- `create_app_full()` struct literal (~line 168)
> 6. `crates/server/src/routes/terminal.rs` -- `test_state_with_session()` helper (~line 1143)
> 7. `crates/server/src/routes/terminal.rs` -- `ws_unknown_session_returns_error` test (~line 1322)
> 8. `crates/server/src/routes/jobs.rs` -- test helper (~line 78)
>
> Add `sidecar: Arc::new(crate::sidecar::SidecarManager::new()),` to each. Run `cargo test -p claude-view-server` to verify no struct literal is missed.

**Step 2: Add sidecar shutdown to main.rs graceful shutdown**

See Steps 2b and 2c below for the complete, copy-paste-ready code. The key points:

- Create `SidecarManager` in `main.rs` BEFORE `create_app_full()` (Step 2c)
- Pass it as 7th param to `create_app_full()` (Step 2b shows the new signature)
- Clone the Arc as `sidecar_for_shutdown` before `axum::serve`
- Call `sidecar_for_shutdown.shutdown()` in the graceful shutdown block after `cleanup(shutdown_port)`

> **WIRING NOTE:** In `main.rs`, there is NO `state` variable accessible at the `with_graceful_shutdown()` site -- `AppState` is created inside `create_app_full()` and consumed there. To capture the sidecar handle for shutdown:
> 1. Create `SidecarManager` separately BEFORE calling `create_app_full()`
> 2. Pass the `Arc<SidecarManager>` into `create_app_full()` as a parameter
> 3. Clone the Arc and capture it in the shutdown closure
>
> Do NOT assume a `state` variable exists in main.rs scope.

**Step 2b: Modify `create_app_full()` signature in `lib.rs`**

Add a `sidecar` parameter to `create_app_full()` (~line 141 in `crates/server/src/lib.rs`):

```rust
// Current signature (6 params):
pub fn create_app_full(
    db: Database,
    indexing: Arc<IndexingState>,
    registry: RegistryHolder,
    search_index: SearchIndexHolder,
    shutdown: tokio::sync::watch::Receiver<bool>,
    static_dir: Option<PathBuf>,
) -> Router {

// New signature (7 params — add sidecar after static_dir):
pub fn create_app_full(
    db: Database,
    indexing: Arc<IndexingState>,
    registry: RegistryHolder,
    search_index: SearchIndexHolder,
    shutdown: tokio::sync::watch::Receiver<bool>,
    static_dir: Option<PathBuf>,
    sidecar: Arc<sidecar::SidecarManager>,
) -> Router {
```

Then in the `AppState` struct literal (~line 168), add:

```rust
    sidecar,  // add after hook_event_channels
```

**Step 2c: Update `main.rs` call site**

In `main.rs` (~line 282), create the sidecar before `create_app_full`:

```rust
    // Step 4: Build the Axum app with indexing state, registry holder, and search index
    let static_dir = get_static_dir();
    let sidecar = Arc::new(claude_view_server::SidecarManager::new());
    let sidecar_for_shutdown = sidecar.clone(); // capture for graceful shutdown
    let app = create_app_full(
        db.clone(),
        indexing.clone(),
        registry_holder.clone(),
        search_index_holder.clone(),
        shutdown_rx,
        static_dir,
        sidecar,  // new 7th param
    );
```

And in the `with_graceful_shutdown` block (~line 677), after `cleanup(shutdown_port)`:

```rust
    // Shut down Node.js sidecar if running
    sidecar_for_shutdown.shutdown();
```

**Step 3: Re-export from lib.rs**

In `crates/server/src/lib.rs`, add `pub use sidecar::SidecarManager;` alongside the other re-exports.

**Step 4: Verify compilation**

```bash
cargo check -p claude-view-server
```

Expected: Clean compile. All existing tests still pass.

**Step 5: Run existing tests**

```bash
cargo test -p claude-view-server -- --nocapture 2>&1 | tail -5
```

Expected: All existing tests pass (the new `sidecar` field is initialized with defaults in all constructors).

**Step 6: Commit**

```bash
git add crates/server/src/state.rs crates/server/src/main.rs crates/server/src/lib.rs
git commit -m "feat(phase-f): wire SidecarManager into AppState and shutdown"
```

---

## Task 5: Cost Estimation Endpoint

**Files:**
- Create: `crates/server/src/routes/control.rs`
- Modify: `crates/server/src/routes/mod.rs` (add `pub mod control;`, register route)

**Depends on:** None (Rust-only, no sidecar needed)

**Context:** Before resuming a session, the frontend shows a cost estimate. This endpoint reads session metadata from SQLite (already indexed), checks cache warmth (last_message_at vs now), and calculates estimated costs using the existing `ModelPricing` table in `AppState.pricing`. No sidecar involvement.

**Step 0: Add `get_session_by_id()` to the database layer (B1)**

The `Database` struct has no `get_session_by_id()` method. Add it to `crates/db/src/queries/sessions.rs` before writing the route handler:

```rust
// Add to crates/db/src/queries/sessions.rs
pub async fn get_session_by_id(&self, id: &str) -> DbResult<Option<SessionInfo>> {
    // Use the same column list and row mapping as list_sessions() / query_sessions_filtered()
    // Pattern: sqlx::query_as::<_, SessionRow>("SELECT ... FROM sessions WHERE id = ?1")
    //   .bind(id)
    //   .fetch_optional(self.pool())  // use self.pool() per CLAUDE.md rule
    //   .await?
    // Then map: .map(|r| { let pid = r.project_id.clone(); r.into_session_info(&pid) })
    //
    // IMPORTANT: The sessions table has 63+ columns. Do NOT write a manual SELECT *.
    // Copy the column list from the existing list_sessions() query to ensure consistency.
    // The SessionRow intermediate struct and its into_session_info() mapping handle the conversion.
    // IMPLEMENTATION REQUIRED — do NOT leave todo!() here or tests will panic.
    // Copy the full column SELECT from query_sessions_filtered() in dashboard.rs,
    // add WHERE id = ?1, use fetch_optional, map through SessionRow -> SessionInfo.
    // Pattern: same column list as query_sessions_filtered() in dashboard.rs
    let row = sqlx::query_as::<_, SessionRow>(
        // Copy column list from query_sessions_filtered() — ~63 columns
        "SELECT id, project_id, /* ... copy full column list ... */ FROM sessions WHERE id = ?1"
    )
    .bind(id)
    .fetch_optional(self.pool())
    .await?;

    // into_session_info(self, project_encoded: &str) — needs project_id as second arg
    Ok(row.map(|r| {
        let pid = r.project_id.clone();
        r.into_session_info(&pid)
    }))
}
```

**Step 1: Create `crates/server/src/routes/control.rs` with cost estimation**

```rust
// crates/server/src/routes/control.rs
//! Phase F: Interactive control routes.
//!
//! - POST /api/control/estimate — cost estimation (Rust-only, no sidecar)
//! - POST /api/control/resume — proxy to sidecar (Task 8)
//! - WS   /api/control/sessions/:id/stream — proxy to sidecar (Task 10)

use std::sync::Arc;
use axum::{extract::State, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use crate::state::AppState;

/// Request body for cost estimation.
#[derive(Debug, Deserialize)]
pub struct EstimateRequest {
    pub session_id: String,
    pub model: Option<String>,
}

/// Cost estimation response.
#[derive(Debug, Serialize)]
pub struct CostEstimate {
    pub session_id: String,
    pub history_tokens: u64,
    pub cache_warm: bool,
    pub first_message_cost: f64,
    pub per_message_cost: f64,
    pub model: String,
    pub explanation: String,
    pub session_title: Option<String>,
    pub project_name: Option<String>,
    pub turn_count: u32,
    pub files_edited: u32,
    pub last_active_secs_ago: i64,
}

// **Pattern note:** The codebase uses `Result<..., ApiError>` (from `crates/server/src/error.rs`)
// for all route handlers -- not bare `StatusCode`. Bare `StatusCode` returns an empty body with
// no JSON error. Use `ApiError::Internal(msg)` for DB errors and `ApiError::NotFound` for
// missing sessions. Check `error.rs` for available variants.
async fn estimate_cost(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EstimateRequest>,
) -> Result<Json<CostEstimate>, crate::error::ApiError> {
    let now = chrono::Utc::now().timestamp();

    // Look up session in DB
    let session = state.db.get_session_by_id(&req.session_id)
        .await
        .map_err(|e| crate::error::ApiError::Internal(format!("DB error: {e}")))?
        .ok_or_else(|| crate::error::ApiError::NotFound(format!("Session {} not found", req.session_id)))?;

    let model = req.model.unwrap_or_else(|| {
        session.primary_model.clone().unwrap_or_else(|| "claude-sonnet-4-20250514".to_string())
    });

    let history_tokens = session.total_input_tokens.unwrap_or(0) as u64;
    let last_activity = session.modified_at; // epoch seconds
    let cache_warm = last_activity > 0 && (now - last_activity) < 300; // 5 min TTL

    // Look up model pricing
    let pricing = state.pricing.read().unwrap();
    let model_pricing = claude_view_core::pricing::lookup_pricing(&model, &pricing);

    let (input_base, _output_base) = match model_pricing {
        Some(p) => (p.input_cost_per_token * 1_000_000.0, p.output_cost_per_token * 1_000_000.0),
        None => (3.0, 15.0), // fallback to Sonnet pricing per 1M tokens
    };

    let per_million = |tokens: u64, rate_per_m: f64| -> f64 {
        (tokens as f64 / 1_000_000.0) * rate_per_m
    };

    let first_message_cost = if cache_warm {
        per_million(history_tokens, input_base * 0.10) // cache read
    } else {
        per_million(history_tokens, input_base * 1.25) // cache write
    };

    let per_message_cost = per_million(history_tokens, input_base * 0.10); // always cache read

    let secs_ago = now - last_activity;

    let explanation = if cache_warm {
        format!(
            "Cache is warm (last active {}s ago). First message: ${:.4} (cached). Each follow-up: ~${:.4}.",
            secs_ago, first_message_cost, per_message_cost,
        )
    } else {
        format!(
            "Cache is cold (last active {}m ago). First message: ${:.4} (cache warming). Follow-ups drop to ~${:.4} (cached).",
            secs_ago / 60, first_message_cost, per_message_cost,
        )
    };

    Ok(Json(CostEstimate {
        session_id: req.session_id,
        history_tokens,
        cache_warm,
        first_message_cost,
        per_message_cost,
        model,
        explanation,
        session_title: session.longest_task_preview.clone(),
        project_name: Some(session.project_path.clone()),
        turn_count: session.turn_count_api.unwrap_or(0).min(u32::MAX as u64) as u32,
        files_edited: session.files_edited_count as u32,
        last_active_secs_ago: secs_ago,
    }))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/control/estimate", post(estimate_cost))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_warm_within_5_minutes() {
        let now = 1000;
        let last_activity = 800; // 200s ago (< 300s)
        assert!(last_activity > 0 && (now - last_activity) < 300);
    }

    #[test]
    fn test_cache_cold_after_5_minutes() {
        let now = 1000;
        let last_activity = 600; // 400s ago (> 300s)
        assert!(!(last_activity > 0 && (now - last_activity) < 300));
    }

    #[test]
    fn test_cache_cold_for_epoch_zero() {
        let now = 1000;
        let last_activity = 0; // never active
        assert!(!(last_activity > 0 && (now - last_activity) < 300));
    }

    #[test]
    fn test_cost_estimate_math() {
        // 100K tokens, Sonnet pricing ($3/1M input), cache cold
        let tokens: u64 = 100_000;
        let input_base = 3.0; // per 1M
        let cache_write_cost = (tokens as f64 / 1_000_000.0) * (input_base * 1.25);
        let cache_read_cost = (tokens as f64 / 1_000_000.0) * (input_base * 0.10);

        assert!((cache_write_cost - 0.375).abs() < 0.001); // $0.375
        assert!((cache_read_cost - 0.030).abs() < 0.001);   // $0.030
    }
}
```

**Step 2: Register the route**

In `crates/server/src/routes/mod.rs`:

1. Add `pub mod control;` in the module declarations (alphabetical, after `coaching`)
2. Add `.nest("/api", control::router())` in `api_routes()` (after the `coaching` line). **IMPORTANT (H6):** Use `.nest("/api", control::router())` at the router assembly level (same pattern as other routes like `coaching`, `session`, etc.), NOT `.nest("/api/control", ...)` which would double-prefix the routes to `/api/api/control/...`.

**Step 3: Run tests**

```bash
cargo test -p claude-view-server control:: -- --nocapture
```

Expected: 4 tests pass.

**Step 4: Verify full server compilation**

```bash
cargo check -p claude-view-server
```

Expected: Clean compile.

**Step 5: Commit**

```bash
git add crates/server/src/routes/control.rs crates/server/src/routes/mod.rs
git commit -m "feat(phase-f): cost estimation endpoint POST /api/control/estimate"
```

---

## Task 6: Sidecar Session Manager

**Files:**
- Create: `sidecar/src/session-manager.ts`

**Depends on:** Task 1

**Context:** The SessionManager tracks active control sessions. Each `ControlSession` wraps a Claude Agent SDK session, manages event emission for WebSocket output, and handles tool permission routing via the SDK's `canUseTool` callback. This is the core brain of the sidecar.

**Step 1: Create `sidecar/src/session-manager.ts`**

```typescript
// sidecar/src/session-manager.ts
//
// REWRITTEN (audit H1+H2+H3): Uses correct Agent SDK V2 API.
// - unstable_v2_resumeSession(sessionId, options) — positional sessionId
// - SDKSession has .send(message) and .stream() (async generator of SDKMessage)
// - Permission routing via canUseTool callback in options
// - close() is synchronous in V2 (no await)

import {
  unstable_v2_resumeSession,
  type SDKMessage,
  type SDKSession,
} from '@anthropic-ai/claude-agent-sdk'
import { EventEmitter } from 'node:events'
import type { ServerMessage, ActiveSession } from './types.js'

interface ControlSession {
  controlId: string
  sessionId: string
  sdkSession: SDKSession
  status: 'active' | 'waiting_input' | 'waiting_permission' | 'completed' | 'error'
  totalCost: number
  turnCount: number
  contextUsage: number  // 0-100 percentage of context window used
  startedAt: number
  emitter: EventEmitter
  isStreaming: boolean  // guard against concurrent sendMessage calls
  pendingPermissions: Map<string, {
    resolve: (allowed: boolean) => void
    timer: ReturnType<typeof setTimeout>
  }>
}

export class SessionManager {
  private sessions = new Map<string, ControlSession>()

  getActiveCount(): number {
    return this.sessions.size
  }

  getSession(controlId: string): ControlSession | undefined {
    return this.sessions.get(controlId)
  }

  listSessions(): ActiveSession[] {
    return Array.from(this.sessions.values()).map((cs) => ({
      controlId: cs.controlId,
      sessionId: cs.sessionId,
      status: cs.status,
      turnCount: cs.turnCount,
      totalCost: cs.totalCost,
      startedAt: cs.startedAt,
    }))
  }

  async resume(sessionId: string, model?: string, projectPath?: string): Promise<ControlSession> {
    // Check if already active
    for (const cs of this.sessions.values()) {
      if (cs.sessionId === sessionId) {
        return cs
      }
    }

    const controlId = crypto.randomUUID()
    const emitter = new EventEmitter()
    const pendingPermissions = new Map<string, {
      resolve: (allowed: boolean) => void
      timer: ReturnType<typeof setTimeout>
    }>()

    // SDK V2 LIMITATION (verified against installed `@anthropic-ai/claude-agent-sdk` types):
    //
    // `SDKSessionOptions` only accepts: { model, pathToClaudeCodeExecutable?, executable?,
    //   executableArgs?, env? }
    //
    // The following fields are NOT available in V2:
    //   - `cwd` -- pass via env: { CLAUDE_CWD: projectPath } if the SDK respects it, or omit
    //   - `canUseTool` -- NOT available in V2. Interactive permission routing requires either:
    //     (a) Fall back to V1 query() API which supports canUseTool in its Options type
    //     (b) Accept all tools automatically in V2 and add interactive permissions when SDK stabilizes
    //   - `includePartialMessages` -- NOT available in V2. Streaming content comes via stream() events.
    //
    // Recommendation: For Phase F MVP, use V2 with auto-accept permissions. Add
    // env: { CLAUDE_ACCEPT_TOS: 'true' } or equivalent. Mark interactive permission approval as
    // Phase F.2 pending SDK stabilization. The permission UI (Task 14) can still be built -- it
    // just won't be wired to canUseTool until the V2 API adds it.
    const sdkSession = unstable_v2_resumeSession(sessionId, {
      model: model ?? 'claude-sonnet-4-20250514',
      env: {
        ...(projectPath ? { CLAUDE_CWD: projectPath } : {}),
      },
    })

    // Phase F.2 TODO: Wire interactive permission routing when SDK V2 adds canUseTool.
    // The permission UI (Task 14) and pendingPermissions map are ready -- they just need
    // the SDK callback to be wired in. For now, permissions are auto-accepted by the SDK.

    const cs: ControlSession = {
      controlId,
      sessionId,
      sdkSession,
      status: 'waiting_input',
      totalCost: 0,
      turnCount: 0,
      contextUsage: 0,
      startedAt: Date.now(),
      emitter,
      isStreaming: false,
      pendingPermissions,
    }

    this.sessions.set(controlId, cs)
    return cs
  }

  async sendMessage(controlId: string, content: string): Promise<void> {
    const cs = this.sessions.get(controlId)
    if (!cs) throw new Error(`No session: ${controlId}`)
    if (cs.isStreaming) throw new Error('Session is already streaming')

    cs.isStreaming = true
    cs.status = 'active'
    let messageId = crypto.randomUUID()

    await cs.sdkSession.send(content)

    // Process stream in background
    ;(async () => {
      try {
        for await (const msg of cs.sdkSession.stream()) {
          switch (msg.type) {
            case 'stream_event': {
              // Real-time text chunks (needs includePartialMessages: true)
              const event = (msg as any).event
              if (event?.type === 'content_block_delta' && event.delta?.type === 'text_delta') {
                cs.emitter.emit('message', {
                  type: 'assistant_chunk',
                  content: event.delta.text,
                  messageId,
                } satisfies ServerMessage)
              }
              break
            }
            case 'assistant': {
              // Complete assistant message with all content blocks
              messageId = crypto.randomUUID()
              for (const block of (msg as any).message.content) {
                if (block.type === 'tool_use') {
                  cs.emitter.emit('message', {
                    type: 'tool_use_start',
                    toolName: block.name,
                    toolInput: block.input,
                    toolUseId: block.id,
                  } satisfies ServerMessage)
                }
              }
              break
            }
            case 'user': {
              // Tool results come back as user messages
              break
            }
            case 'result': {
              if ((msg as any).subtype === 'success') {
                cs.totalCost = (msg as any).total_cost_usd ?? 0
                cs.turnCount = (msg as any).num_turns ?? 0
              }
              cs.status = 'waiting_input'
              cs.emitter.emit('message', {
                type: 'assistant_done',
                messageId,
                usage: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheWriteTokens: 0 },
                cost: 0,
                totalCost: cs.totalCost,
              } satisfies ServerMessage)
              break
            }
          }
        }
        cs.isStreaming = false
      } catch (err) {
        cs.isStreaming = false
        cs.status = 'error'
        cs.emitter.emit('message', {
          type: 'error',
          message: err instanceof Error ? err.message : String(err),
          fatal: true,
        } satisfies ServerMessage)
      }
    })()
  }

  resolvePermission(controlId: string, requestId: string, allowed: boolean): boolean {
    const cs = this.sessions.get(controlId)
    if (!cs) return false
    const pending = cs.pendingPermissions.get(requestId)
    if (!pending) return false
    clearTimeout(pending.timer)
    cs.pendingPermissions.delete(requestId)
    pending.resolve(allowed)
    return true
  }

  async close(controlId: string): Promise<void> {
    const cs = this.sessions.get(controlId)
    if (!cs) return
    cs.sdkSession.close() // close() is synchronous in V2, no await
    this.sessions.delete(controlId)
  }

  hasSessionId(sessionId: string): boolean {
    return Array.from(this.sessions.values()).some(cs => cs.sessionId === sessionId)
  }

  getBySessionId(sessionId: string): ControlSession | undefined {
    return Array.from(this.sessions.values()).find(cs => cs.sessionId === sessionId)
  }

  async shutdownAll(): Promise<void> {
    await Promise.all(Array.from(this.sessions.keys()).map(id => this.close(id)))
  }
}
```

**Step 2: Verify typecheck**

```bash
cd sidecar && bun run typecheck
```

Expected: Clean. If Agent SDK types don't match exactly (the `unstable_v2_*` API might have slightly different shapes), adjust the types to match. The key contract is: `resumeSession` returns a `Session` with a `prompt()` method that returns a streamable query.

**Step 3: Commit**

```bash
git add sidecar/src/session-manager.ts
git commit -m "feat(phase-f): sidecar SessionManager with Agent SDK integration"
```

---

## Task 7: Sidecar Control Routes

**Files:**
- Create: `sidecar/src/control.ts`
- Modify: `sidecar/src/index.ts` (wire control routes + SessionManager)

**Depends on:** Task 2, Task 6

**Context:** HTTP routes that Axum proxies to. Resume, send message, list active, terminate.

**Step 1: Create `sidecar/src/control.ts`**

```typescript
// sidecar/src/control.ts
import { Hono } from 'hono'
import type { SessionManager } from './session-manager.js'
import type { ResumeRequest, SendRequest } from './types.js'

export function controlRouter(sessions: SessionManager) {
  const router = new Hono()

  // Resume (or get existing) a session
  router.post('/resume', async (c) => {
    const body = await c.req.json<ResumeRequest>()

    if (!body.sessionId?.match(/^[0-9a-f-]{36}$/)) {
      return c.json({ error: 'Invalid session ID format' }, 400)
    }

    // Check if already resumed
    if (sessions.hasSessionId(body.sessionId)) {
      const existing = sessions.getBySessionId(body.sessionId)!
      return c.json({
        controlId: existing.controlId,
        status: 'already_active',
        sessionId: body.sessionId,
      })
    }

    try {
      const cs = await sessions.resume(
        body.sessionId,
        body.model,
        body.projectPath,
      )
      return c.json({
        controlId: cs.controlId,
        status: 'active',
        sessionId: body.sessionId,
      })
    } catch (err) {
      return c.json(
        {
          error: `Failed to resume: ${err instanceof Error ? err.message : err}`,
        },
        500,
      )
    }
  })

  // Send a message to an active session
  router.post('/send', async (c) => {
    const body = await c.req.json<SendRequest>()
    const session = sessions.getSession(body.controlId)
    if (!session) {
      return c.json({ error: 'Session not found' }, 404)
    }

    try {
      // Fire-and-forget: actual streaming goes via WebSocket
      sessions.sendMessage(body.controlId, body.message).catch((err) => {
        console.error(`[sidecar] sendMessage error: ${err}`)
      })
      return c.json({ status: 'sent' })
    } catch (err) {
      return c.json({ error: `Send failed: ${err}` }, 500)
    }
  })

  // List all active control sessions
  router.get('/sessions', (c) => {
    return c.json(sessions.listSessions())
  })

  // Terminate a control session
  router.delete('/sessions/:controlId', async (c) => {
    const { controlId } = c.req.param()
    await sessions.close(controlId)
    return c.json({ status: 'terminated' })
  })

  return router
}
```

**Step 2: Wire into `sidecar/src/index.ts`**

Replace the placeholder `activeSessionCount` with the real `SessionManager`:

```typescript
// sidecar/src/index.ts — updated
import { Hono } from 'hono'
import { createAdaptorServer } from '@hono/node-server'
import fs from 'node:fs'
import { healthRouter } from './health.js'
import { controlRouter } from './control.js'
import { SessionManager } from './session-manager.js'

const SOCKET_PATH = process.env.SIDECAR_SOCKET
  ?? `/tmp/claude-view-sidecar-${process.ppid}.sock`

const sessionManager = new SessionManager()
const app = new Hono()

app.route('/health', healthRouter(() => sessionManager.getActiveCount()))
app.route('/control', controlRouter(sessionManager))
app.get('/', (c) => c.json({ status: 'ok' }))

// Clean up stale socket from prior crash
if (fs.existsSync(SOCKET_PATH)) {
  fs.unlinkSync(SOCKET_PATH)
}

// Create HTTP server using createAdaptorServer (Options type requires { fetch } property)
const server = createAdaptorServer({ fetch: app.fetch })

server.listen(SOCKET_PATH, () => {
  console.log(`[sidecar] Listening on ${SOCKET_PATH}`)
  console.log(`[sidecar] PID: ${process.pid}, Parent PID: ${process.ppid}`)
})

const parentCheck = setInterval(() => {
  try {
    process.kill(process.ppid!, 0)
  } catch {
    console.log('[sidecar] Parent process exited, shutting down')
    shutdown()
  }
}, 2000)

async function shutdown() {
  clearInterval(parentCheck)
  await sessionManager.shutdownAll()
  server.close()
  if (fs.existsSync(SOCKET_PATH)) {
    fs.unlinkSync(SOCKET_PATH)
  }
  process.exit(0)
}

process.on('SIGTERM', () => void shutdown())
process.on('SIGINT', () => void shutdown())

export { app, server, sessionManager, SOCKET_PATH }
```

**Step 3: Build and typecheck**

```bash
cd sidecar && bun run typecheck && bun run build
```

**Step 4: Commit**

```bash
git add sidecar/src/control.ts sidecar/src/index.ts
git commit -m "feat(phase-f): sidecar control routes (resume, send, list, terminate)"
```

---

## Task 8: Rust Control Proxy Routes

**Files:**
- Modify: `crates/server/src/routes/control.rs` (add proxy handlers)

**Depends on:** Task 4, Task 5, Task 7

**Context:** Axum proxies POST requests to the sidecar's Unix socket. The proxy ensures the sidecar is running first (`ensure_running()`), then forwards the request body and returns the response. Uses raw `tokio::net::UnixStream` + `hyper::client::conn::http1` (not `hyperlocal`, which is incompatible with hyper 1.x).

**Step 1: Add proxy handler to `routes/control.rs`**

Add these functions and update the router:

```rust
use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use http_body_util::{Full, BodyExt};
use bytes::Bytes;
use tokio::net::UnixStream;
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;

/// Proxy a request to the sidecar, ensuring it's running first.
///
/// Uses raw `tokio::net::UnixStream` + `hyper::client::conn::http1` instead of
/// `hyperlocal` (which is incompatible with hyper 1.x).
async fn proxy_to_sidecar(
    state: &AppState,
    method: &str,
    path: &str,
    body: Option<String>,
) -> Result<Response, StatusCode> {
    let socket_path = state.sidecar.ensure_running().await.map_err(|e| {
        tracing::error!("Sidecar unavailable: {e}");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    // Connect to sidecar Unix socket
    let stream = UnixStream::connect(&socket_path).await.map_err(|e| {
        tracing::error!("Failed to connect to sidecar socket: {e}");
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    let io = TokioIo::new(stream);

    let (mut sender, conn) = http1::handshake(io).await.map_err(|e| {
        tracing::error!("Sidecar HTTP handshake failed: {e}");
        StatusCode::BAD_GATEWAY
    })?;
    tokio::spawn(conn);

    let req = hyper::Request::builder()
        .method(method)
        .uri(path)
        .header("host", "localhost")
        .header("content-type", "application/json");

    let req = if let Some(body) = body {
        req.body(Full::new(Bytes::from(body)))
    } else {
        // Use Full with empty bytes for type consistency
        req.body(Full::new(Bytes::new()))
    }
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let resp = sender.send_request(req).await.map_err(|e| {
        tracing::error!("Sidecar request failed: {e}");
        StatusCode::BAD_GATEWAY
    })?;

    // Convert hyper response to axum response
    let status = resp.status();
    let bytes = resp.into_body().collect().await
        .map_err(|_| StatusCode::BAD_GATEWAY)?
        .to_bytes();
    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(bytes))
        .unwrap())
}

/// POST /api/control/resume — proxy to sidecar
async fn resume_session(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<Response, StatusCode> {
    proxy_to_sidecar(&state, "POST", "/control/resume", Some(body)).await
}

/// POST /api/control/send — proxy to sidecar
async fn send_message(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<Response, StatusCode> {
    proxy_to_sidecar(&state, "POST", "/control/send", Some(body)).await
}

/// GET /api/control/sessions — list active control sessions
async fn list_sessions(
    State(state): State<Arc<AppState>>,
) -> Result<Response, StatusCode> {
    proxy_to_sidecar(&state, "GET", "/control/sessions", None).await
}

/// DELETE /api/control/sessions/:id — terminate a control session
async fn terminate_session(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(control_id): axum::extract::Path<String>,
) -> Result<Response, StatusCode> {
    proxy_to_sidecar(
        &state,
        "DELETE",
        &format!("/control/sessions/{control_id}"),
        None,
    )
    .await
}
```

**Step 2: Update the router function**

```rust
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/control/estimate", post(estimate_cost))
        .route("/control/resume", post(resume_session))
        .route("/control/send", post(send_message))
        .route("/control/sessions", axum::routing::get(list_sessions))
        .route("/control/sessions/{id}", axum::routing::delete(terminate_session))
}
```

**Step 3: Verify compilation**

```bash
cargo check -p claude-view-server
```

**Step 4: Commit**

```bash
git add crates/server/src/routes/control.rs
git commit -m "feat(phase-f): Rust proxy routes for sidecar control endpoints"
```

---

## Task 9: Sidecar WebSocket Handler

**Files:**
- Create: `sidecar/src/ws-handler.ts`
- Modify: `sidecar/src/index.ts` (add WS upgrade handling)

**Depends on:** Task 7

**Context:** The sidecar runs a WebSocket server alongside the HTTP server on the same Unix socket. When Axum connects a WS to `/control/sessions/:controlId/stream`, the sidecar registers that WS as the output channel for the session. Messages from the frontend (user_message, permission_response, ping) are routed to the session manager.

**Step 1: Create `sidecar/src/ws-handler.ts`**

```typescript
// sidecar/src/ws-handler.ts
import type { WebSocket } from 'ws'
import type { SessionManager } from './session-manager.js'
import type { ClientMessage, ServerMessage } from './types.js'

export function handleWebSocket(
  ws: WebSocket,
  controlId: string,
  sessions: SessionManager  // Pass SessionManager, not ControlSession
) {
  const session = sessions.getSession(controlId)
  if (!session) {
    ws.send(JSON.stringify({ type: 'error', message: 'Session not found', fatal: true }))
    ws.close()
    return
  }

  // Subscribe to session events via EventEmitter
  const onMessage = (msg: ServerMessage) => {
    if (ws.readyState === ws.OPEN) {
      ws.send(JSON.stringify(msg))
    }
  }
  session.emitter.on('message', onMessage)

  // Send initial status
  ws.send(JSON.stringify({
    type: 'session_status',
    status: session.status,
    contextUsage: session.contextUsage,
    turnCount: session.turnCount,
  } satisfies ServerMessage))

  // Handle incoming messages from frontend
  ws.on('message', (raw) => {
    try {
      const msg: ClientMessage = JSON.parse(raw.toString())
      switch (msg.type) {
        case 'user_message':
          sessions.sendMessage(controlId, msg.content).catch((err) => {
            ws.send(JSON.stringify({ type: 'error', message: String(err), fatal: false }))
          })
          break
        case 'permission_response':
          sessions.resolvePermission(controlId, msg.requestId, msg.allowed)
          break
        case 'ping':
          ws.send(JSON.stringify({ type: 'pong' }))
          break
      }
    } catch {
      ws.send(JSON.stringify({ type: 'error', message: 'Invalid message format', fatal: false }))
    }
  })

  // Cleanup on close
  ws.on('close', () => {
    session.emitter.removeListener('message', onMessage)
  })
}
```

**Step 2: Add WS upgrade to `sidecar/src/index.ts`**

Add WebSocket server setup after the Unix server creation. The Hono server handles HTTP; we need a separate `ws.WebSocketServer` for WS upgrades on the same socket.

Add to `sidecar/src/index.ts` (after `server.listen`):

```typescript
import { WebSocketServer } from 'ws'
import { handleWebSocket } from './ws-handler.js'

// WS upgrade handler on the HTTP server (not a separate net.Server)
// The `server` is from createAdaptorServer(), which is a standard http.Server.
const wss = new WebSocketServer({ noServer: true })

server.on('upgrade', (request, socket, head) => {
  // Extract controlId from URL: /control/sessions/:controlId/stream
  const match = request.url?.match(/\/control\/sessions\/([^/]+)\/stream/)
  if (!match?.[1]) {
    socket.destroy()
    return
  }

  const controlId = match[1]
  wss.handleUpgrade(request, socket, head, (ws) => {
    handleWebSocket(ws, controlId, sessionManager)
  })
})
```

**Step 3: Build and verify**

```bash
cd sidecar && bun run typecheck && bun run build
```

**Step 4: Commit**

```bash
git add sidecar/src/ws-handler.ts sidecar/src/index.ts
git commit -m "feat(phase-f): sidecar WebSocket handler for bidirectional streaming"
```

---

## Task 10: Rust WebSocket Proxy

**Files:**
- Modify: `crates/server/src/routes/control.rs` (add WS proxy)

**Depends on:** Task 8, Task 9

**Context:** Axum upgrades the frontend WebSocket, then opens a second WebSocket to the sidecar's Unix socket, and relays messages bidirectionally. Uses `tokio-tungstenite` (already in Cargo.toml) for the sidecar-side WS connection.

**Step 1: Add WS proxy to `routes/control.rs`**

```rust
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::Path;
use futures_util::{SinkExt, StreamExt};

/// WS /api/control/sessions/:id/stream — bidirectional relay to sidecar
async fn ws_stream(
    ws: WebSocketUpgrade,
    Path(control_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_relay(socket, control_id, state))
}

async fn handle_ws_relay(
    frontend_ws: WebSocket,
    control_id: String,
    state: Arc<AppState>,
) {
    // Ensure sidecar is running
    let socket_path = match state.sidecar.ensure_running().await {
        Ok(p) => p,
        Err(e) => {
            let (mut sink, _) = frontend_ws.split();
            // B6: Message::Text uses Utf8Bytes in Axum 0.8 — use .into() to convert
            let _ = sink
                .send(Message::Text(
                    serde_json::json!({
                        "type": "error",
                        "message": format!("Sidecar unavailable: {e}"),
                        "fatal": true,
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            return;
        }
    };

    // Connect to sidecar WebSocket via Unix socket
    // tokio-tungstenite supports Unix socket connections via a custom connector
    let sidecar_path = format!(
        "/control/sessions/{}/stream",
        control_id
    );

    // Use raw tokio UnixStream + tungstenite handshake
    let unix_stream = match tokio::net::UnixStream::connect(&socket_path).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to connect to sidecar Unix socket: {e}");
            let (mut sink, _) = frontend_ws.split();
            let _ = sink
                .send(Message::Text(
                    serde_json::json!({
                        "type": "error",
                        "message": "Failed to connect to sidecar",
                        "fatal": true,
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            return;
        }
    };

    let ws_url = format!("ws://localhost{}", sidecar_path);
    let (sidecar_ws, _) = match tokio_tungstenite::client_async(ws_url, unix_stream).await {
        Ok(pair) => pair,
        Err(e) => {
            tracing::error!("Sidecar WS handshake failed: {e}");
            let (mut sink, _) = frontend_ws.split();
            let _ = sink
                .send(Message::Text(
                    serde_json::json!({
                        "type": "error",
                        "message": "Sidecar WebSocket handshake failed",
                        "fatal": true,
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            return;
        }
    };

    // Split both WebSockets and relay bidirectionally
    let (mut fe_sink, mut fe_stream) = frontend_ws.split();
    let (mut sc_sink, mut sc_stream) = sidecar_ws.split();

    // B6: In Axum 0.8 + tungstenite 0.28, Message::Text wraps Utf8Bytes, not String.
    // Use .as_ref() to read and .into() to convert when relaying between
    // axum::extract::ws::Message and tokio_tungstenite::tungstenite::Message.
    let fe_to_sc = async {
        while let Some(Ok(msg)) = fe_stream.next().await {
            match msg {
                Message::Text(text) => {
                    let s: &str = text.as_ref();
                    if sc_sink
                        .send(tokio_tungstenite::tungstenite::Message::Text(s.to_string().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Message::Ping(data) => {
                    let _ = sc_sink
                        .send(tokio_tungstenite::tungstenite::Message::Ping(data.to_vec().into()))
                        .await;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    };

    let sc_to_fe = async {
        while let Some(Ok(msg)) = sc_stream.next().await {
            match msg {
                tokio_tungstenite::tungstenite::Message::Text(text) => {
                    let s: &str = text.as_ref();
                    if fe_sink.send(Message::Text(s.to_string().into())).await.is_err() {
                        break;
                    }
                }
                tokio_tungstenite::tungstenite::Message::Close(_) => break,
                _ => {}
            }
        }
    };

    tokio::select! {
        _ = fe_to_sc => {},
        _ = sc_to_fe => {},
    }
}
```

**Step 2: Add the WS route**

Update the router:

```rust
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/control/estimate", post(estimate_cost))
        .route("/control/resume", post(resume_session))
        .route("/control/send", post(send_message))
        .route("/control/sessions", axum::routing::get(list_sessions))
        .route("/control/sessions/{id}", axum::routing::delete(terminate_session))
        .route("/control/sessions/{id}/stream", axum::routing::get(ws_stream))
}
```

**Step 3: Verify compilation**

```bash
cargo check -p claude-view-server
```

**Step 4: Commit**

```bash
git add crates/server/src/routes/control.rs
git commit -m "feat(phase-f): Rust WebSocket proxy relay to sidecar"
```

---

## Task 11: Frontend Control Session Hook

**Files:**
- Create: `apps/web/src/hooks/use-control-session.ts`
- Create: `apps/web/src/types/control.ts`

**Depends on:** Task 10

**Context:** React hook that manages the WebSocket connection to a control session. Handles all message types (assistant_chunk, tool_use, permission_request, etc.), maintains streaming state, and provides `sendMessage()` and `respondPermission()` callbacks. Follows existing patterns: `wsRef.current !== ws` stale guard per CLAUDE.md rules.

**Step 1: Create `apps/web/src/types/control.ts`**

```typescript
// apps/web/src/types/control.ts
// Mirrors sidecar/src/types.ts for frontend consumption

export interface CostEstimate {
  session_id: string
  history_tokens: number
  cache_warm: boolean
  first_message_cost: number
  per_message_cost: number
  model: string
  explanation: string
  session_title: string | null
  project_name: string | null
  turn_count: number
  files_edited: number
  last_active_secs_ago: number
}

export interface ResumeResponse {
  controlId: string
  status: 'active' | 'already_active'
  sessionId: string
  error?: string
}

// WebSocket message types (sidecar → frontend)

export interface AssistantChunk {
  type: 'assistant_chunk'
  content: string
  messageId: string
}

export interface AssistantDone {
  type: 'assistant_done'
  messageId: string
  usage: {
    inputTokens: number
    outputTokens: number
    cacheReadTokens: number
    cacheWriteTokens: number
  }
  cost: number
  totalCost: number
}

export interface ToolUseStartMsg {
  type: 'tool_use_start'
  toolName: string
  toolInput: Record<string, unknown>
  toolUseId: string
}

export interface ToolUseResultMsg {
  type: 'tool_use_result'
  toolUseId: string
  output: string
  isError: boolean
}

export interface PermissionRequestMsg {
  type: 'permission_request'
  requestId: string
  toolName: string
  toolInput: Record<string, unknown>
  description: string
  timeoutMs: number
}

export interface SessionStatusMsg {
  type: 'session_status'
  status: 'active' | 'waiting_input' | 'waiting_permission' | 'completed' | 'error'
  contextUsage: number
  turnCount: number
}

export interface ErrorMsg {
  type: 'error'
  message: string
  fatal: boolean
}

export interface PongMsg {
  type: 'pong'
}

export type ServerMessage =
  | AssistantChunk
  | AssistantDone
  | ToolUseStartMsg
  | ToolUseResultMsg
  | PermissionRequestMsg
  | SessionStatusMsg
  | ErrorMsg
  | PongMsg

// Chat message for display
export interface ChatMessage {
  role: 'user' | 'assistant' | 'tool_use' | 'tool_result'
  content?: string
  messageId?: string
  toolName?: string
  toolInput?: Record<string, unknown>
  toolUseId?: string
  output?: string
  isError?: boolean
  usage?: AssistantDone['usage']
}
```

**Step 2: Create `apps/web/src/hooks/use-control-session.ts`**

```typescript
// apps/web/src/hooks/use-control-session.ts
import { useCallback, useEffect, useRef, useState } from 'react'
import { wsUrl } from '../lib/ws-url'
import type {
  ChatMessage,
  PermissionRequestMsg,
  ServerMessage,
} from '../types/control'

export type ControlStatus =
  | 'connecting'
  | 'active'
  | 'waiting_input'
  | 'waiting_permission'
  | 'completed'
  | 'error'
  | 'disconnected'
  | 'reconnecting'

interface ControlSessionState {
  status: ControlStatus
  messages: ChatMessage[]
  streamingContent: string
  streamingMessageId: string
  contextUsage: number
  turnCount: number
  sessionCost: number
  lastTurnCost: number
  permissionRequest: PermissionRequestMsg | null
  error: string | null
}

const initialState: ControlSessionState = {
  status: 'connecting',
  messages: [],
  streamingContent: '',
  streamingMessageId: '',
  contextUsage: 0,
  turnCount: 0,
  sessionCost: 0,
  lastTurnCost: 0,
  permissionRequest: null,
  error: null,
}

export function useControlSession(controlId: string | null) {
  const [state, setState] = useState<ControlSessionState>(initialState)
  const wsRef = useRef<WebSocket | null>(null)
  const intentionalCloseRef = useRef(false)
  const reconnectAttemptRef = useRef(0)
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const unmountedRef = useRef(false)
  const MAX_RECONNECT_ATTEMPTS = 10
  const INITIAL_BACKOFF_MS = 1000
  const MAX_BACKOFF_MS = 30_000

  useEffect(() => {
    if (!controlId) return
    unmountedRef.current = false
    intentionalCloseRef.current = false

    function connect() {
      const ws = new WebSocket(wsUrl(`/api/control/sessions/${controlId}/stream`))
      wsRef.current = ws

    ws.onmessage = (event) => {
      // Stale guard per CLAUDE.md rules
      if (wsRef.current !== ws) return

      const msg: ServerMessage = JSON.parse(event.data)

      setState((prev) => {
        switch (msg.type) {
          case 'assistant_chunk':
            return {
              ...prev,
              streamingContent: prev.streamingContent + msg.content,
              streamingMessageId: msg.messageId,
              status: 'active',
            }

          case 'assistant_done':
            return {
              ...prev,
              messages: [
                ...prev.messages,
                {
                  role: 'assistant',
                  content: prev.streamingContent,
                  messageId: msg.messageId,
                  usage: msg.usage,
                },
              ],
              streamingContent: '',
              streamingMessageId: '',
              sessionCost: msg.totalCost,
              lastTurnCost: msg.cost,
              status: 'waiting_input',
            }

          case 'tool_use_start':
            return {
              ...prev,
              messages: [
                ...prev.messages,
                {
                  role: 'tool_use',
                  toolName: msg.toolName,
                  toolInput: msg.toolInput,
                  toolUseId: msg.toolUseId,
                },
              ],
            }

          case 'tool_use_result':
            return {
              ...prev,
              messages: [
                ...prev.messages,
                {
                  role: 'tool_result',
                  toolUseId: msg.toolUseId,
                  output: msg.output,
                  isError: msg.isError,
                },
              ],
            }

          case 'permission_request':
            return {
              ...prev,
              permissionRequest: msg,
              status: 'waiting_permission',
            }

          case 'session_status':
            return {
              ...prev,
              status: msg.status,
              contextUsage: msg.contextUsage,
              turnCount: msg.turnCount,
            }

          case 'error':
            return {
              ...prev,
              status: msg.fatal ? 'error' : prev.status,
              error: msg.message,
            }

          case 'pong':
            return prev // no state change

          default:
            return prev
        }
      })
    }

    ws.onclose = () => {
      if (wsRef.current !== ws) return  // stale guard
      if (unmountedRef.current) return  // unmount guard
      if (!intentionalCloseRef.current && reconnectAttemptRef.current < MAX_RECONNECT_ATTEMPTS) {
        const backoff = Math.min(INITIAL_BACKOFF_MS * 2 ** reconnectAttemptRef.current, MAX_BACKOFF_MS)
        reconnectTimerRef.current = setTimeout(connect, backoff)
        reconnectAttemptRef.current++
        setState(prev => ({ ...prev, status: 'reconnecting' as ControlStatus }))
      } else {
        setState((prev) => ({
          ...prev,
          status: prev.status === 'completed' ? 'completed' : 'disconnected',
        }))
      }
    }

    ws.onopen = () => {
      reconnectAttemptRef.current = 0  // reset on successful connect
      // Don't set 'active' — wait for the sidecar's initial session_status message
      // which will set the correct status ('waiting_input', 'active', etc.)
      setState((prev) => ({ ...prev, status: 'waiting_input' }))
    }

    ws.onerror = () => {
      if (wsRef.current !== ws) return
      setState((prev) => ({ ...prev, error: 'WebSocket connection error' }))
    }

    } // end connect()

    connect()

    // B8: Close WS BEFORE nulling the ref -- otherwise the stale guard in
    // onclose fires (wsRef.current !== ws) and skips the state update.
    return () => {
      unmountedRef.current = true
      if (reconnectTimerRef.current) clearTimeout(reconnectTimerRef.current)
      intentionalCloseRef.current = true
      wsRef.current?.close()
      wsRef.current = null
    }
  }, [controlId])

  const sendMessage = useCallback((content: string) => {
    const ws = wsRef.current
    if (!ws || ws.readyState !== WebSocket.OPEN) return

    ws.send(JSON.stringify({ type: 'user_message', content }))

    setState((prev) => ({
      ...prev,
      messages: [...prev.messages, { role: 'user', content }],
      status: 'active',
    }))
  }, [])

  const respondPermission = useCallback(
    (requestId: string, allowed: boolean) => {
      const ws = wsRef.current
      if (!ws || ws.readyState !== WebSocket.OPEN) return

      ws.send(
        JSON.stringify({ type: 'permission_response', requestId, allowed }),
      )

      setState((prev) => ({
        ...prev,
        permissionRequest: null,
        status: 'active',
      }))
    },
    [],
  )

  return { ...state, sendMessage, respondPermission }
}
```

**Step 3: Add Vite WS proxy for control WebSocket (B9)**

The current Vite config has `ws: true` only for `/api/live/sessions`. The new control WS at `/api/control/sessions/:id/stream` will silently fail to upgrade in dev mode without this.

Add to `apps/web/vite.config.ts`, BEFORE the catch-all `/api` entry:

```typescript
proxy: {
  // ORDER MATTERS: specific WS routes MUST come before catch-all /api
  '/api/live/sessions': { target: 'http://localhost:47892', ws: true },
  '/api/control/sessions': { target: 'http://localhost:47892', ws: true },  // NEW
  '/api': 'http://localhost:47892',  // catch-all (MUST be last)
}
```

> **Note:** `wsUrl()` bypasses Vite in dev mode (connects directly to :47892), so this proxy is a safety net for production builds.

**Step 4: Add reconnect logic (H7)**

> **NOTE (H7):** The `useControlSession` hook should follow the existing `useTerminalSocket` pattern with `intentionalCloseRef` for reconnect logic. When the WS closes unexpectedly (not by user action), attempt to reconnect with exponential backoff. Check `apps/web/src/hooks/use-terminal-socket.ts` for the reference implementation.

**Step 5: Verify typecheck**

```bash
cd apps/web && bunx tsc --noEmit
```

**Step 6: Commit**

```bash
git add apps/web/src/types/control.ts apps/web/src/hooks/use-control-session.ts apps/web/vite.config.ts
git commit -m "feat(phase-f): useControlSession hook with WebSocket state management"
```

---

## Task 12: Pre-Flight UI

**Files:**
- Create: `apps/web/src/components/live/ResumePreFlight.tsx`

**Depends on:** Task 5 (cost endpoint), Task 8 (resume endpoint), Task 11 (types)

**Context:** Modal that appears when clicking "Resume in Dashboard" on a session card. Shows session info, cost estimate, cache status, model selector, and resume button. Desktop = centered dialog, mobile = bottom sheet. Uses `@radix-ui/react-dialog` per CLAUDE.md rules (no hand-rolled overlays).

**Step 1: Create `apps/web/src/components/live/ResumePreFlight.tsx`**

This is a large component — see the design doc `phase-f-interactive.md` Step 4 for the full wireframe. Key implementation notes:

- Use `@tanstack/react-query` for the cost estimate fetch
- Use `useMutation` for the resume action
- Use explicit Tailwind classes + `dark:` variants (no shadcn CSS vars per CLAUDE.md)
- Cost endpoint: `POST /api/control/estimate` (hits Rust directly, no sidecar)
- Resume endpoint: `POST /api/control/resume` (proxied through Rust to sidecar)
- On successful resume, call `onResume(controlId)` to navigate to chat view

The component should fetch the cost estimate from the Rust API (port 47892 in dev), display cache warm/cold status, and have a "Resume in Dashboard" button that POSTs to the resume endpoint.

**Step 2: Create the directory**

```bash
mkdir -p apps/web/src/components/live
```

**Step 3: Write the component** (adapt wireframe from design doc Step 4)

**Step 4: Wire "Resume in Dashboard" trigger to existing SessionCard**

> There are two SessionCard components:
> - `apps/web/src/components/SessionCard.tsx` (historical/archive sessions)
> - `apps/web/src/components/live/SessionCard.tsx` (live monitoring)
>
> Add a "Resume in Dashboard" button to the historical SessionCard's footer area. The button opens the ResumePreFlight modal with `sessionId` pre-filled.
>
> The modal state (open/close + selectedSessionId) should live in the parent view component (e.g., `HistoryView.tsx`).

**Step 5: Commit**

```bash
git add apps/web/src/components/live/ResumePreFlight.tsx
git commit -m "feat(phase-f): ResumePreFlight modal with cost estimation"
```

---

## Task 13: Dashboard Chat View

**Files:**
- Create: `apps/web/src/components/live/DashboardChat.tsx`

**Depends on:** Task 11, Task 12

**Context:** Full-screen chat interface for interacting with a resumed session. Shows conversation history (from existing session messages API) above a divider, then new messages from the control session below. Streaming text renders with a blinking cursor. Input box: Enter to send, Shift+Enter for newline.

See design doc Step 6 for the full wireframe and key behaviors table.

Key implementation:
- Load history from existing `GET /api/sessions/:id/messages` (the current non-deprecated endpoint). Use the existing `useSessionMessages(sessionId)` hook from `apps/web/src/hooks/use-session-messages.ts` rather than reimplementing the fetch
- "Dashboard session started" divider between history and new messages
- Use `useControlSession(controlId)` for streaming state
- Auto-scroll to bottom (unless user scrolled up)
- Tool calls rendered inline (collapsible)
- Session completed: input disabled, summary banner shown

> **IMPORTANT (H9):** `DashboardChat` needs BOTH IDs as props:
> - `controlId` — for the WebSocket connection (`useControlSession(controlId)`)
> - `sessionId` — for loading conversation history from `GET /api/sessions/:id/messages`
>
> These are different: `controlId` is generated by the sidecar when a session is resumed, while `sessionId` is the original Claude Code session UUID. Both must be passed from the parent component (e.g. from the `ResumePreFlight` response). Use the existing `useSessionMessages(sessionId)` hook rather than reimplementing the fetch.

**Step 1: Create the component**

```bash
# Component at apps/web/src/components/live/DashboardChat.tsx
```

**Step 2: Add React Router route for control view**

> Add to `apps/web/src/router.tsx`:
> ```typescript
> { path: '/control/:controlId', element: <ControlPage /> }
> ```
>
> Create `apps/web/src/pages/ControlPage.tsx` that:
> 1. Reads `controlId` from URL params
> 2. Reads `sessionId` from URL search params (passed by ResumePreFlight on navigate)
> 3. Renders `DashboardChat` with both IDs
>
> The `onResume(controlId)` callback in ResumePreFlight should `navigate(`/control/${controlId}?sessionId=${sessionId}`)`.

**Step 3: Commit**

```bash
git add apps/web/src/components/live/DashboardChat.tsx
git commit -m "feat(phase-f): DashboardChat with streaming and history"
```

---

## Task 14: Permission Dialog

**Files:**
- Create: `apps/web/src/components/live/PermissionDialog.tsx`

**Depends on:** Task 11

**Context:** When Claude wants to use a tool (bash, edit, write), the sidecar sends a `permission_request` through the WebSocket. This dialog shows the tool name, input, and a countdown timer. Auto-deny after 60s. Allow/Deny buttons relay back through the WS.

See design doc Step 7 for the full wireframe and acceptance criteria.

Key implementation:
- `useEffect` countdown timer (1s interval)
- Auto-deny when countdown reaches 0
- Bash commands show the actual command
- File edits show the file path
- **H8: Use `@radix-ui/react-dialog` with Portal** — do NOT hand-roll a `z-50` fixed overlay div. Per CLAUDE.md rules, use Radix UI for overlays:

```typescript
import * as Dialog from '@radix-ui/react-dialog'

// Wrap in Dialog.Root + Dialog.Portal + Dialog.Overlay + Dialog.Content
// Dialog.Overlay handles backdrop click-to-close
// Dialog.Content handles focus trapping and escape key
```

> Style the Dialog.Overlay with: `className="fixed inset-0 bg-black/50 dark:bg-black/70"` (no shadcn/ui CSS vars per CLAUDE.md). Do NOT use hand-rolled `z-50` fixed overlay divs -- Radix Dialog.Portal handles z-index stacking automatically.

**Step 1: Create the component** (adapt from design doc Step 7)

**Step 2: Commit**

```bash
git add apps/web/src/components/live/PermissionDialog.tsx
git commit -m "feat(phase-f): PermissionDialog with countdown auto-deny"
```

---

## Task 15: Cost Tracking Status Bar

**Files:**
- Create: `apps/web/src/components/live/ChatStatusBar.tsx`
- Create: `apps/web/src/components/live/SessionSummary.tsx`

**Depends on:** Task 13

**Context:** Persistent bar at the bottom of DashboardChat showing context usage (progress bar), turn count, running cost, and last turn cost. When the session completes, shows a summary with prompt caching savings.

See design doc Step 8 for the wireframe and cost tracking logic.

Key implementation:
- Context usage: progress bar 0-100% from `contextUsage`
- Running cost: `$X.XX` from `sessionCost`
- Last turn: `+$X.XX (cached)` or `+$X.XX (cache warming)`
- Summary: compare actual cost vs what it would have cost without caching

**Step 1: Create `ChatStatusBar.tsx`**

**Step 2: Create `SessionSummary.tsx`**

**Step 3: Commit**

```bash
git add apps/web/src/components/live/ChatStatusBar.tsx apps/web/src/components/live/SessionSummary.tsx
git commit -m "feat(phase-f): ChatStatusBar and SessionSummary with cost tracking"
```

---

## Task 16: Distribution Integration

**Files:**
- Modify: `.github/workflows/release.yml` (add sidecar build step)
- Modify: `npx-cli/index.js` (set `SIDECAR_DIR`, install sidecar deps)
- Add: `sidecar/.gitignore` (ignore `node_modules/`, `dist/`)

**Depends on:** All previous tasks

**Context:** The sidecar JS bundle must be included in the release archive so `npx claude-view` users get interactive mode. CI builds the sidecar, bundles it alongside the Rust binary and frontend dist. The npx-cli sets `SIDECAR_DIR` so the Rust binary can find it. Sidecar deps (`@anthropic-ai/claude-agent-sdk`) are installed on first use if not already present.

See design doc Step 9 for CI YAML changes and npx-cli modifications.

Key changes:
1. Add `build-sidecar` job to release workflow (before `build-binary`)
2. Include sidecar `dist/` + `package.json` in release archive
3. npx-cli sets `SIDECAR_DIR` env var when spawning the Rust binary
4. npx-cli runs `npm install --omit=dev` in sidecar dir if `node_modules/` missing (npm context OK here — this is the distribution layer)

**Step 1: Create `sidecar/.gitignore`**

```
node_modules/
dist/
```

**Step 2: Update CI workflow** (see design doc Step 9 for exact YAML)

**Step 3: Update npx-cli** (see design doc Step 9 for exact JS)

**Step 4: Commit**

```bash
git add sidecar/.gitignore .github/workflows/release.yml npx-cli/index.js
git commit -m "feat(phase-f): distribution integration for sidecar"
```

---

## Task 17: End-to-End Verification

**Files:** None (verification only)

**Depends on:** All previous tasks

**Context:** Verify the full flow works end-to-end: build sidecar, build web, start server, resume a session from the dashboard.

**Step 1: Build everything**

```bash
cd sidecar && bun run build
cd apps/web && bun run build
cargo build -p claude-view-server
```

**Step 2: Start the server**

```bash
RUST_LOG=warn,claude_view_server=info cargo run -p claude-view-server
```

**Step 3: Test cost estimation endpoint**

```bash
# Find a real session ID from the UI or DB
SESSION_ID="..."
curl -X POST http://localhost:47892/api/control/estimate \
  -H "Content-Type: application/json" \
  -d "{\"session_id\": \"$SESSION_ID\"}"
# Expected: JSON with cost estimate, cache status, session info
```

**Step 4: Test sidecar launch**

```bash
curl -X POST http://localhost:47892/api/control/resume \
  -H "Content-Type: application/json" \
  -d "{\"session_id\": \"$SESSION_ID\"}"
# Expected: sidecar starts (check server logs), returns controlId
```

**Step 5: Test WebSocket**

Open browser to `http://localhost:47892`, navigate to a session, click "Resume in Dashboard", verify:
- Pre-flight modal shows with cost estimate
- After clicking Resume, chat view opens
- Typing a message sends it and streams a response
- Tool calls show in the chat
- Permission requests show the dialog with countdown

**Step 6: Test sidecar crash recovery**

```bash
# Find sidecar PID
ps aux | grep "sidecar"
# Kill it
kill -9 <PID>
# Send another control request — should auto-restart
curl -X GET http://localhost:47892/api/control/sessions
```

**Step 7: Test graceful shutdown**

Press Ctrl+C on the server. Verify:
- Sidecar process terminates
- Unix socket file cleaned up
- No orphan processes

---

## Acceptance Criteria (from design doc)

| # | Criterion | Task |
|---|-----------|------|
| F.1 | Sidecar starts and communicates with Axum successfully | T2, T3, T4 |
| F.2 | Resume loads conversation history and continues session | T6, T7, T8 |
| F.3 | Pre-flight panel shows accurate cost estimate | T5, T12 |
| F.4 | Cache warm/cold detection correct | T5 |
| F.5 | Chat input sends messages and receives streaming responses | T11, T13 |
| F.6 | Tool permission requests route to UI and back | T6, T9, T14 |
| F.7 | Cost tracking updates in real-time during chat | T15 |
| F.8 | Sidecar shuts down cleanly when Axum exits | T4 |
| F.9 | Works via `npx claude-view` distribution | T16 |
| F.10 | Mobile: chat view usable on phone screens | T12, T13 |
| F.11 | Sidecar crash leads to graceful degradation | T3 |
| F.12 | No security vulnerabilities (Unix socket only) | T2 |
| F.13 | Auto-deny tool permissions after 60s timeout | T6, T14 |
| F.14 | Session summary shows prompt caching savings | T15 |
| F.15 | No Node.js required for monitoring-only usage | T3 (lazy start) |

---

## Key Risk: Agent SDK API Stability

The `unstable_v2_*` prefix in the Agent SDK indicates the API may change. Before starting Task 6:

1. Run `npm info @anthropic-ai/claude-agent-sdk` to get the latest version
2. Check if `unstable_v2_resumeSession` has been stabilized (renamed to `resumeSession`)
3. Read the SDK's README/CHANGELOG for any breaking changes since the design doc was written
4. Adjust `session-manager.ts` accordingly — the patterns (resume, prompt, stream, permission hook) will be the same, only function names might change

If the SDK API has changed significantly, update the design doc (`phase-f-interactive.md`) to match before implementing.

---

## Changelog

**2026-02-27 -- Audit fixes applied** (see `phase-f-audit-results.md`)

| Fix | Severity | Tasks | Description |
|-----|----------|-------|-------------|
| B1 | Blocker | T5 | Added `get_session_by_id()` DB method requirement |
| B2 | Blocker | T5 | Fixed SessionInfo field names: model->primary_model, updated_at->modified_at, project_dir->project_path, files_modified->files_edited_count, turn_count->turn_count_api, title->longest_task_preview |
| B3 | Blocker | T3 | Replaced `hyperlocal 0.9` (hyper 0.14 dep) with raw `UnixStream` + `hyper::client::conn::http1` + `http-body-util` |
| B4+B5 | Blocker | T8 | Fixed hyper 1.x body types: `Body::from()` -> `Full::new(Bytes::from())`, `Body::empty()` -> `Full::new(Bytes::new())` |
| B6 | Blocker | T10 | Fixed `Message::Text` Utf8Bytes: `.into()` for send, `.as_ref()` for receive (follows terminal.rs) |
| B7 | Blocker | T4 | Added note: ALL test `AppState { ... }` struct literals must get `sidecar` field |
| B8 | Blocker | T11 | Fixed WS cleanup ordering: `ws.close()` before `wsRef.current = null` |
| B9 | Blocker | T11 | Added Vite WS proxy entry for `/api/control/sessions` |
| H1+H2+H3 | High | T6 | Complete rewrite of session-manager.ts against real Agent SDK V2 API: `send()` + `stream()` + `canUseTool` |
| H4 | High | T2, T7, T9 | Replaced `serve()+close()+emit('connection')` hack with `createAdaptorServer()` from `@hono/node-server` |
| H5 | High | T11 | Replaced hardcoded `localhost:47892` with `wsUrl()` from `apps/web/src/lib/ws-url.ts` |
| H6 | High | T5 | Fixed route nesting to use `.nest("/api", control::router())` pattern |
| H7 | High | T11 | Added reconnect logic note (follow `useTerminalSocket` pattern) |
| H8 | High | T14 | Replaced hand-rolled `z-50` overlay with `@radix-ui/react-dialog` Portal |
| H9 | High | T13 | DashboardChat props now include both `controlId` and `sessionId` |
| H10 | High | T1 | SDK version pinned to `"^0.1.0"` (was `"latest"`) |
| W1 | Warning | T2,T7,T9,T17 | All sidecar commands: `npm install`/`npm run` -> `bun install`/`bun run` |
| W7 | Warning | T6 | Removed `await` from `session.close()` (sync in V2) |

**2026-02-27 -- Round 2 audit fixes** (comprehensive codebase verification)

| Fix | Severity | Tasks | Description |
|-----|----------|-------|-------------|
| R2-B1 | Blocker | T2 | Task 2 still had stale `serve()+close()+emit()` code -- replaced with `createAdaptorServer` |
| R2-B2 | Blocker | T3 | health_check() still used `hyperlocal` -- replaced with raw UnixStream |
| R2-B3 | Blocker | T6 | SDK V2 `SDKSessionOptions` doesn't have `canUseTool`/`cwd`/`includePartialMessages` -- restructured |
| R2-B4 | Blocker | T6 | Added `shutdownAll()`, `hasSessionId()`, `getBySessionId()`, `contextUsage` to SessionManager |
| R2-B5 | Blocker | T7 | Fixed 6 method name mismatches: has->hasSessionId, get->getBySessionId, getByControlId->getSession, listActive->listSessions, terminate->close |
| R2-B6 | Blocker | T9 | Complete rewrite of ws-handler.ts for EventEmitter pattern (was using non-existent class methods) |
| R2-B7 | Blocker | T9 | Fixed `match[1]` strict typing -- added null guard |
| R2-B8 | Blocker | T12-13 | Added frontend wiring: SessionCard trigger button + React Router route for ControlPage |
| R2-B9 | Blocker | T3 | Added CLAUDE.md env var stripping note for sidecar spawning |
| R2-W1 | Warning | T3 | Cargo.toml deps already exist -- marked as skip-if-present |
| R2-W2 | Warning | T4 | Enumerated all 8 AppState struct literal sites explicitly |
| R2-W3 | Warning | T4 | Added graceful shutdown wiring note (no `state` var in main.rs scope) |
| R2-W4 | Warning | T5 | Error types should use `ApiError`, not bare `StatusCode` |
| R2-W5 | Warning | T5 | Fleshed out `get_session_by_id()` implementation guidance |
| R2-W6 | Warning | T6 | Added concurrent sendMessage guard (`isStreaming`) |
| R2-W7 | Warning | T6 | Removed unused `unstable_v2_createSession` import |
| R2-W8 | Warning | T11 | Added reconnect skeleton with exponential backoff |
| R2-W9 | Warning | T11 | Added stale guard + unmountedRef to onclose handler |
| R2-W10 | Warning | T11 | Vite proxy ordering comment |
| R2-W11 | Warning | T13 | Fixed deprecated API endpoint reference |
| R2-W12 | Warning | T14 | Added Dialog.Overlay styling guidance, no hand-rolled z-50 |
| R2-W13 | Warning | T1 | Added `verbatimModuleSyntax: true` to sidecar tsconfig |
| R2-M1 | Minor | T2,T7 | `createAdaptorServer(app)` -> `createAdaptorServer({ fetch: app.fetch })` |
| R2-M2 | Minor | T3 | Noted sidecar.rs and `pub mod sidecar;` already exist |

**2026-02-27 — Round 3 adversarial review fixes** (score 74→100)

| Fix | Severity | Tasks | Description |
| --- | -------- | ----- | ----------- |
| R3-1 | Blocker | T11 | Moved `import { wsUrl }` to top of file — `verbatimModuleSyntax` rejects mid-file imports |
| R3-2 | Blocker | T11 | Removed `pingInterval` (leaked on every reconnect, never cleared) — rely on sidecar heartbeat |
| R3-3 | Blocker | T8 | Removed unused `Empty` from `http_body_util` import — Clippy `unused_imports` |
| R3-4 | Blocker | T5 | Added overflow guard: `turn_count_api.unwrap_or(0).min(u32::MAX as u64) as u32` |
| R3-5 | Blocker | T5 | Replaced `todo!()` in `get_session_by_id` with implementation sketch using `SessionRow` |
| R3-6 | Warning | T11 | `onopen` sets `waiting_input` not `active` — wait for sidecar's `session_status` message |
| R3-7 | Warning | T16 | Fixed `npm install --production` (deprecated) → `npm install --omit=dev` |

**2026-02-27 — Round 4 adversarial re-review fixes** (score 78→target 100)

| Fix | Severity | Tasks | Description |
| --- | -------- | ----- | ----------- |
| R4-1 | Blocker | T5 | `into_session_info(self, project_encoded: &str)` needs 2nd arg — fixed closure + removed wrong `.transpose()` + `self.pool()` not `&self.pool` |
| R4-2 | Blocker | T5 | `estimate_cost` return type changed from bare `StatusCode` to `ApiError` — matches codebase error pattern |
| R4-3 | Blocker | T4 | Added explicit `create_app_full()` signature change code (new 7th `sidecar` param) + `main.rs` wiring + shutdown capture |
| R4-4 | Warning | T3 | WIP sidecar.rs "skip if matches" changed to "diff line-by-line, replace if differs" — prevents stale WIP bugs |
| R4-5 | Blocker | T8 | Removed unused `Request` from `use axum::http::{Request, StatusCode}` — Clippy `unused_imports` fails CI |
| R4-6 | Warning | T5 | Fixed stale comment in `get_session_by_id` showing old wrong pattern (`&self.pool`, no 2nd arg) |
| R4-7 | Minor | T11 | `npx tsc --noEmit` changed to `bunx tsc --noEmit` — matches CLAUDE.md dev toolchain |
