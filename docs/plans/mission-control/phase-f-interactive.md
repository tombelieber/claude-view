---
status: pending
date: 2026-02-10
phase: F
depends_on: A
---

# Phase F: Interactive Control via Agent SDK Sidecar

> **Goal:** Allow users to resume and interact with Claude Code sessions directly from the web dashboard, using the Claude Agent SDK as the execution layer.

## Problem Statement

Users can monitor sessions (Phase A) and view them in various layouts (B-E), but cannot **act on** them. Resuming a session requires switching to a terminal. Phase F closes this gap: click "Resume in Dashboard" on any session card, see a cost estimate, then continue the conversation in-browser with full streaming, tool permission routing, and real-time cost tracking.

## Architecture Overview

The Claude Agent SDK (`@anthropic-ai/claude-agent-sdk`) is npm-only -- it cannot be called from Rust. Phase F introduces a **Node.js sidecar process** that the Rust server manages as a child process. The sidecar exposes a local HTTP+WebSocket API that Axum proxies to the frontend.

```
┌─────────────────────────────────────────────────────────────────────┐
│                         User's Browser                              │
│                                                                     │
│  ┌──────────────┐   ┌──────────────┐   ┌───────────────────────┐   │
│  │ Session Grid  │   │ Resume       │   │ Dashboard Chat View   │   │
│  │ (Phase A)     │──>│ Pre-Flight   │──>│ (streaming messages)  │   │
│  └──────────────┘   └──────────────┘   └───────────┬───────────┘   │
│                                                     │               │
└─────────────────────────────────────────────────────│───────────────┘
                                                      │
                                HTTP / WebSocket       │
                                                      │
┌─────────────────────────────────────────────────────│───────────────┐
│                    Axum Server (Rust)                │               │
│                                                     │               │
│  ┌────────────┐  ┌────────────┐  ┌─────────────────▼──────────┐   │
│  │ /api/...   │  │ SSE        │  │ /api/control/*              │   │
│  │ (existing) │  │ indexing   │  │ proxy to sidecar            │   │
│  └────────────┘  └────────────┘  └─────────────────┬──────────┘   │
│                                                     │               │
│                              Unix socket / HTTP     │               │
│                                                     │               │
│  ┌──────────────────────────────────────────────────▼──────────┐   │
│  │                  Node.js Sidecar Process                     │   │
│  │                                                              │   │
│  │  ┌──────────────────┐  ┌──────────────────────────────┐     │   │
│  │  │ HTTP Server      │  │ Claude Agent SDK              │     │   │
│  │  │ (Fastify/Hono)   │  │ @anthropic-ai/claude-agent-sdk│     │   │
│  │  └──────┬───────────┘  └──────────────┬───────────────┘     │   │
│  │         │                              │                     │   │
│  │         │    ┌─────────────┐           │                     │   │
│  │         └───>│ Session Mgr │<──────────┘                     │   │
│  │              │ (per-session │                                 │   │
│  │              │  state map)  │                                 │   │
│  │              └─────────────┘                                 │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
                         │
                         │ Spawns subprocess via Agent SDK
                         ▼
                ┌────────────────┐
                │  Claude Code   │
                │  (subprocess)  │
                │  reads JSONL   │
                │  from session  │
                └────────────────┘
```

## Key Constraint

The Agent SDK **cannot attach to an existing terminal session**. "Resume" means:
1. Read the existing session's JSONL conversation history
2. Spawn a **new** Claude Code subprocess via the SDK with `resume: sessionId`
3. The SDK loads the conversation into context, resuming where it left off
4. New messages flow through the sidecar, not the original terminal

This means the terminal session and dashboard session can coexist but are **independent**. The pre-flight panel warns users about this.

---

## Step 1: Sidecar Directory & Bootstrap

### Files

| File | Purpose |
|------|---------|
| `sidecar/package.json` | Dependencies: `@anthropic-ai/claude-agent-sdk`, `hono`, `ws` |
| `sidecar/tsconfig.json` | TypeScript config (strict, ESM) |
| `sidecar/src/index.ts` | HTTP server entry point |
| `sidecar/src/health.ts` | Health check handler |
| `sidecar/src/session-manager.ts` | Per-session state machine |
| `sidecar/src/types.ts` | Shared IPC message types |

### `sidecar/package.json`

```json
{
  "name": "claude-view-sidecar",
  "version": "0.3.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "tsx watch src/index.ts",
    "build": "tsup src/index.ts --format esm --target node20",
    "start": "node dist/index.js"
  },
  "dependencies": {
    "@anthropic-ai/claude-agent-sdk": "^0.1.0",
    "hono": "^4.0.0",
    "ws": "^8.16.0"
  },
  "devDependencies": {
    "tsx": "^4.7.0",
    "tsup": "^8.0.0",
    "typescript": "^5.4.0",
    "@types/ws": "^8.5.0"
  }
}
```

### Communication Protocol

Axum communicates with the sidecar via **Unix domain socket** on macOS/Linux, falling back to localhost TCP on Windows.

| Transport | Path/Port | Why |
|-----------|-----------|-----|
| Unix socket (primary) | `/tmp/claude-view-sidecar-{pid}.sock` | No port collision, faster than TCP, filesystem permissions |
| TCP fallback (Windows) | `127.0.0.1:47893` | Windows lacks Unix sockets |

### `sidecar/src/index.ts` (entry point)

```typescript
import { Hono } from 'hono'
import { serve } from '@hono/node-server'
import { healthRouter } from './health.js'
import { controlRouter } from './control.js'
import { SessionManager } from './session-manager.js'
import net from 'net'
import fs from 'fs'

const SOCKET_PATH = process.env.SIDECAR_SOCKET
  ?? `/tmp/claude-view-sidecar-${process.ppid}.sock`

const app = new Hono()
const sessionManager = new SessionManager()

app.route('/health', healthRouter)
app.route('/control', controlRouter(sessionManager))

// Cleanup socket on startup (stale from crash)
if (fs.existsSync(SOCKET_PATH)) {
  fs.unlinkSync(SOCKET_PATH)
}

// Listen on Unix socket
const server = serve({ fetch: app.fetch, port: 0 }, (info) => {
  console.log(`Sidecar listening on ${SOCKET_PATH}`)
})

// Override to listen on Unix socket instead of TCP
server.close()
const unixServer = net.createServer(/* ... Hono adapter */)
unixServer.listen(SOCKET_PATH)

// Graceful shutdown
process.on('SIGTERM', () => {
  sessionManager.shutdownAll()
  unixServer.close()
  if (fs.existsSync(SOCKET_PATH)) fs.unlinkSync(SOCKET_PATH)
  process.exit(0)
})

// Parent process exit detection
// If Rust server dies, sidecar should self-terminate
const parentCheckInterval = setInterval(() => {
  try {
    process.kill(process.ppid!, 0) // signal 0 = check if alive
  } catch {
    console.log('Parent process exited, shutting down sidecar')
    process.emit('SIGTERM' as any)
  }
}, 2000)
```

### Sidecar Lifecycle (Rust side)

```
┌──────────────────────────────────────────────────────┐
│                  Axum Server                          │
│                                                      │
│  main.rs                                             │
│  ┌────────────────────────────────────────────┐      │
│  │ Server starts on :47892                     │      │
│  │ Sidecar NOT started yet (lazy)              │      │
│  └────────────────────────────────────────────┘      │
│                                                      │
│  First POST /api/control/resume                      │
│  ┌────────────────────────────────────────────┐      │
│  │ SidecarManager::ensure_running()            │      │
│  │  1. Check if child process alive            │      │
│  │  2. If not: spawn `node sidecar/dist/...`   │      │
│  │  3. Wait for /health to return 200          │      │
│  │  4. Store Child handle in AppState          │      │
│  └────────────────────────────────────────────┘      │
│                                                      │
│  On Axum shutdown (Ctrl+C / SIGTERM):                │
│  ┌────────────────────────────────────────────┐      │
│  │ SidecarManager::shutdown()                  │      │
│  │  1. Send SIGTERM to sidecar child           │      │
│  │  2. Wait 3s for clean exit                  │      │
│  │  3. SIGKILL if still alive                  │      │
│  └────────────────────────────────────────────┘      │
└──────────────────────────────────────────────────────┘
```

### Rust: `crates/server/src/sidecar.rs`

```rust
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;

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

    /// Start sidecar if not already running. Returns socket path.
    pub async fn ensure_running(&self) -> Result<String, SidecarError> {
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

        // Spawn sidecar
        let sidecar_dir = Self::find_sidecar_dir()?;
        let child = Command::new("node")
            .arg(sidecar_dir.join("dist/index.js"))
            .env("SIDECAR_SOCKET", &self.socket_path)
            .spawn()
            .map_err(SidecarError::SpawnFailed)?;

        *guard = Some(child);
        drop(guard); // release lock during health check

        // Wait for sidecar to be ready (health check)
        for attempt in 0..30 {
            sleep(Duration::from_millis(100)).await;
            if self.health_check().await.is_ok() {
                tracing::info!("Sidecar ready after {}ms", (attempt + 1) * 100);
                return Ok(self.socket_path.clone());
            }
        }

        Err(SidecarError::HealthCheckTimeout)
    }

    /// Send SIGTERM, wait, then SIGKILL if needed.
    pub fn shutdown(&self) {
        let mut guard = self.child.lock().unwrap();
        if let Some(ref mut child) = *guard {
            let _ = child.kill(); // SIGKILL as fallback
            let _ = child.wait();
        }
        *guard = None;

        // Cleanup socket file
        let _ = std::fs::remove_file(&self.socket_path);
    }

    async fn health_check(&self) -> Result<(), SidecarError> {
        // Connect to Unix socket, send GET /health, expect 200
        // Uses hyper UnixConnector or tokio UnixStream
        todo!("HTTP-over-Unix-socket health check")
    }

    fn find_sidecar_dir() -> Result<std::path::PathBuf, SidecarError> {
        // Priority:
        // 1. SIDECAR_DIR env var
        // 2. ./sidecar/ relative to binary
        // 3. npx cache: ~/.cache/claude-view/bin/sidecar/
        todo!()
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    #[error("Failed to spawn sidecar: {0}")]
    SpawnFailed(std::io::Error),
    #[error("Sidecar health check timed out after 3s")]
    HealthCheckTimeout,
    #[error("Sidecar directory not found")]
    SidecarDirNotFound,
    #[error("Sidecar returned error: {0}")]
    SidecarError(String),
}
```

### Health Check

| Endpoint | Method | Response |
|----------|--------|----------|
| `/health` | GET | `{ "status": "ok", "activeSessions": 0, "uptime": 12345 }` |

Axum calls this on startup and periodically (every 30s) to detect sidecar crashes.

### Acceptance Criteria (Step 1)

| # | Criterion | Pass |
|---|-----------|------|
| 1.1 | Sidecar starts as child process of Rust binary | [ ] |
| 1.2 | Health check returns 200 within 3s of spawn | [ ] |
| 1.3 | Sidecar shuts down when Rust process exits (SIGTERM) | [ ] |
| 1.4 | Sidecar self-terminates if parent PID disappears (crash) | [ ] |
| 1.5 | Stale socket file cleaned up on restart | [ ] |
| 1.6 | Sidecar only starts on first `/api/control/*` request (lazy) | [ ] |
| 1.7 | Sidecar auto-restarts if it crashes mid-session | [ ] |
| 1.8 | Socket listens only on localhost (no network exposure) | [ ] |

---

## Step 2: Pre-flight Cost Estimation

Before resuming, the frontend shows a cost estimate. This runs **entirely in Rust** -- no sidecar needed.

### Cost Model

The Anthropic API is **stateless**: every request sends the full conversation history. Prompt caching reduces repeat cost.

```
┌─────────────────────────────────────────────────────────────────┐
│                    Prompt Caching Economics                      │
│                                                                 │
│  Cache Status    │ Input Cost        │ When                     │
│  ────────────────┼───────────────────┼────────────────────────  │
│  Cache WRITE     │ 1.25x base rate   │ First msg after >5min    │
│  Cache READ      │ 0.10x base rate   │ Subsequent msgs <5min    │
│  No cache        │ 1.00x base rate   │ Cache disabled           │
│                                                                 │
│  TTL: 5 minutes from last API call                              │
│  Max cacheable: 4 context blocks (system + conversation)        │
└─────────────────────────────────────────────────────────────────┘
```

### Estimation Algorithm

```rust
/// Cost estimation for resuming a session.
pub struct CostEstimate {
    /// Estimated input tokens from conversation history
    pub history_tokens: u64,
    /// Whether prompt cache is likely warm (last activity < 5min ago)
    pub cache_warm: bool,
    /// Estimated cost for the first message (USD)
    pub first_message_cost: f64,
    /// Estimated cost per subsequent message (USD, assuming cache stays warm)
    pub per_message_cost: f64,
    /// Model pricing tier used for estimate
    pub model: String,
    /// Explanation string for UI
    pub explanation: String,
}

pub fn estimate_resume_cost(
    session: &SessionInfo,
    model: &str,
    now: i64,
) -> CostEstimate {
    let history_tokens = session.total_input_tokens.unwrap_or(0) as u64;
    let last_activity = session.last_message_at;
    let cache_warm = last_activity > 0 && (now - last_activity) < 300; // 5 min

    // Pricing per 1M tokens (as of 2026-02 for Sonnet 4)
    let (input_base, output_base) = match model {
        m if m.contains("opus") => (15.0, 75.0),
        m if m.contains("sonnet") => (3.0, 15.0),
        m if m.contains("haiku") => (0.25, 1.25),
        _ => (3.0, 15.0), // default to Sonnet pricing
    };

    let per_million = |tokens: u64, rate: f64| -> f64 {
        (tokens as f64 / 1_000_000.0) * rate
    };

    let first_message_cost = if cache_warm {
        per_million(history_tokens, input_base * 0.10) // cache read
    } else {
        per_million(history_tokens, input_base * 1.25) // cache write
    };

    // Subsequent messages: always cache read (within 5-min window)
    let per_message_cost = per_million(history_tokens, input_base * 0.10);

    let explanation = if cache_warm {
        format!(
            "Cache is warm (last active {}s ago). First message: ${:.4} (cached). \
             Each follow-up: ~${:.4}.",
            now - last_activity,
            first_message_cost,
            per_message_cost,
        )
    } else {
        format!(
            "Cache is cold (last active {}m ago). First message: ${:.4} (cache warming). \
             Follow-ups drop to ~${:.4} (cached).",
            (now - last_activity) / 60,
            first_message_cost,
            per_message_cost,
        )
    };

    CostEstimate {
        history_tokens,
        cache_warm,
        first_message_cost,
        per_message_cost,
        model: model.to_string(),
        explanation,
    }
}
```

### API Endpoint

| Method | Path | Body | Response |
|--------|------|------|----------|
| POST | `/api/control/estimate` | `{ "session_id": "abc", "model": "sonnet-4" }` | `CostEstimate` JSON |

This endpoint runs in Rust only. It reads session metadata from SQLite (already indexed) and calculates the estimate. No sidecar involved.

### Acceptance Criteria (Step 2)

| # | Criterion | Pass |
|---|-----------|------|
| 2.1 | Estimate returns within 50ms (SQLite lookup + math) | [ ] |
| 2.2 | Cache warm detection correct: < 5min since last_message_at | [ ] |
| 2.3 | Cache cold detection correct: >= 5min since last_message_at | [ ] |
| 2.4 | Epoch-zero `last_message_at` treated as cold (never active) | [ ] |
| 2.5 | Model pricing tiers correct for opus/sonnet/haiku | [ ] |
| 2.6 | Explanation string is human-readable | [ ] |

---

## Step 3: Resume Endpoint

### Flow

```
Browser                Axum                    Sidecar               Claude Code
   │                    │                         │                       │
   │ POST /api/control  │                         │                       │
   │    /resume         │                         │                       │
   │ {session_id,model} │                         │                       │
   │───────────────────>│                         │                       │
   │                    │ ensure_running()         │                       │
   │                    │────────────────────────->│                       │
   │                    │         200 OK           │                       │
   │                    │<────────────────────────-│                       │
   │                    │                         │                       │
   │                    │ POST /control/resume     │                       │
   │                    │ {session_id, model,      │                       │
   │                    │  project_path}           │                       │
   │                    │────────────────────────->│                       │
   │                    │                         │ SDK: query({           │
   │                    │                         │   resume: sessionId    │
   │                    │                         │ })                     │
   │                    │                         │───────────────────────>│
   │                    │                         │                       │
   │                    │ { control_id, status }   │                       │
   │                    │<────────────────────────-│                       │
   │  { control_id,     │                         │                       │
   │    status: "active"}│                         │                       │
   │<───────────────────│                         │                       │
```

### Sidecar Resume Handler

```typescript
// sidecar/src/control.ts
import { query, type AgentSession } from '@anthropic-ai/claude-agent-sdk'
import { SessionManager } from './session-manager.js'
import { Hono } from 'hono'

export function controlRouter(sessions: SessionManager) {
  const router = new Hono()

  router.post('/resume', async (c) => {
    const { sessionId, model, projectPath } = await c.req.json()

    // Validate session_id format (UUID)
    if (!sessionId?.match(/^[0-9a-f-]{36}$/)) {
      return c.json({ error: 'Invalid session ID' }, 400)
    }

    // Check if already resumed
    if (sessions.has(sessionId)) {
      return c.json({
        controlId: sessions.get(sessionId)!.controlId,
        status: 'already_active',
      })
    }

    try {
      // Spawn Claude Code subprocess via Agent SDK
      const session = await query({
        options: {
          resume: sessionId,
          model: model ?? undefined,
        },
        cwd: projectPath,
      })

      const controlId = crypto.randomUUID()
      sessions.register(controlId, sessionId, session)

      return c.json({
        controlId,
        status: 'active',
        sessionId,
      })
    } catch (err) {
      return c.json({
        error: `Failed to resume session: ${err instanceof Error ? err.message : err}`,
      }, 500)
    }
  })

  router.post('/send', async (c) => {
    const { controlId, message } = await c.req.json()
    const session = sessions.getByControlId(controlId)
    if (!session) {
      return c.json({ error: 'Session not found' }, 404)
    }

    try {
      await session.agentSession.send(message)
      return c.json({ status: 'sent' })
    } catch (err) {
      return c.json({ error: `Send failed: ${err}` }, 500)
    }
  })

  router.get('/sessions', (c) => {
    return c.json(sessions.listActive())
  })

  router.delete('/sessions/:controlId', async (c) => {
    const { controlId } = c.req.param()
    sessions.terminate(controlId)
    return c.json({ status: 'terminated' })
  })

  return router
}
```

### Axum Proxy

```rust
// crates/server/src/routes/control.rs

/// Proxy all /api/control/* requests to the sidecar.
pub async fn proxy_to_sidecar(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
) -> Result<impl IntoResponse, ApiError> {
    let socket_path = state.sidecar.ensure_running().await?;

    // Forward the request to sidecar via Unix socket
    let client = hyper_util::client::legacy::Client::builder(
        hyper_util::rt::TokioExecutor::new()
    ).build(hyperlocal::UnixConnector);

    let uri = hyperlocal::Uri::new(&socket_path, req.uri().path());
    let proxy_req = hyper::Request::builder()
        .method(req.method())
        .uri(uri)
        .body(req.into_body())?;

    let response = client.request(proxy_req).await?;
    Ok(response)
}
```

### Acceptance Criteria (Step 3)

| # | Criterion | Pass |
|---|-----------|------|
| 3.1 | Resume spawns a new Claude Code subprocess | [ ] |
| 3.2 | Conversation history loaded from existing JSONL | [ ] |
| 3.3 | Duplicate resume returns existing control_id (idempotent) | [ ] |
| 3.4 | Invalid session_id returns 400 | [ ] |
| 3.5 | SDK spawn failure returns 500 with descriptive error | [ ] |
| 3.6 | Axum proxy correctly forwards to sidecar Unix socket | [ ] |

---

## Step 4: Pre-flight UI Panel

### Component: `src/components/live/ResumePreFlight.tsx`

A modal/bottom-sheet that appears when clicking "Resume in Dashboard" on a session card.

### Wireframe

```
Desktop (centered modal):
┌─────────────────────────────────────────────────────┐
│  ↩ Resume Session                              [X]  │
│─────────────────────────────────────────────────────│
│                                                     │
│  Session: "Fix auth middleware"                      │
│  Project: claude-view                               │
│  Last active: 12 minutes ago                        │
│                                                     │
│  ┌─────────────────────────────────────────────┐    │
│  │  47 turns  ·  142K tokens  ·  23 files      │    │
│  └─────────────────────────────────────────────┘    │
│                                                     │
│  Cost Estimate                                      │
│  ┌─────────────────────────────────────────────┐    │
│  │  Cache: COLD (last active 12m ago)          │    │
│  │                                              │    │
│  │  First message:  ~$0.18  (cache warming)     │    │
│  │  Follow-ups:     ~$0.01  (cached)            │    │
│  │                                              │    │
│  │  ▸ What's prompt caching?                    │    │
│  │    ┌───────────────────────────────────────┐ │    │
│  │    │ Anthropic caches your conversation    │ │    │
│  │    │ history for 5 minutes. The first msg  │ │    │
│  │    │ warms the cache (1.25x input cost).   │ │    │
│  │    │ Follow-ups read from cache (0.1x).    │ │    │
│  │    │                                       │ │    │
│  │    │ Tip: Keep conversations flowing to    │ │    │
│  │    │ stay in the cache window.             │ │    │
│  │    └───────────────────────────────────────┘ │    │
│  └─────────────────────────────────────────────┘    │
│                                                     │
│  ⚠ If this session is open in a terminal, close     │
│    it first to avoid conflicts.                     │
│                                                     │
│  Model: [Sonnet 4 ▾]                               │
│                                                     │
│  ┌──────────┐  ┌──────────────────────────────┐     │
│  │  Cancel   │  │  ↩ Resume in Dashboard →     │     │
│  └──────────┘  └──────────────────────────────┘     │
└─────────────────────────────────────────────────────┘

Mobile (bottom sheet):
┌─────────────────────────────────┐
│  ── (drag handle)               │
│                                 │
│  ↩ Resume: "Fix auth..."       │
│  47 turns · 142K tokens        │
│                                 │
│  Cache: COLD · ~$0.18 first    │
│  ▸ What's prompt caching?      │
│                                 │
│  ⚠ Close terminal session      │
│    first                        │
│                                 │
│  [Cancel]  [Resume →]          │
└─────────────────────────────────┘
```

### Component Implementation

```tsx
// src/components/live/ResumePreFlight.tsx
import { useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { ArrowLeftRight, AlertTriangle, ChevronDown, ChevronRight } from 'lucide-react'
import { Dialog, DialogContent, DialogHeader } from '@/components/ui/dialog'
import { Sheet, SheetContent } from '@/components/ui/sheet'
import { useMediaQuery } from '@/hooks/use-media-query'

interface ResumePreFlightProps {
  sessionId: string
  isOpen: boolean
  onClose: () => void
  onResume: (controlId: string) => void
}

export function ResumePreFlight({ sessionId, isOpen, onClose, onResume }: ResumePreFlightProps) {
  const [model, setModel] = useState('sonnet-4')
  const [showCacheExplainer, setShowCacheExplainer] = useState(false)
  const isMobile = useMediaQuery('(max-width: 640px)')

  const { data: estimate, isLoading: estimateLoading } = useQuery({
    queryKey: ['cost-estimate', sessionId, model],
    queryFn: () => fetchCostEstimate(sessionId, model),
    enabled: isOpen,
  })

  const resumeMutation = useMutation({
    mutationFn: () => resumeSession(sessionId, model),
    onSuccess: (data) => {
      onResume(data.controlId)
    },
  })

  const content = (
    <div className="space-y-4">
      {/* Session info */}
      <div>
        <h3 className="font-semibold">{estimate?.sessionTitle ?? 'Loading...'}</h3>
        <p className="text-sm text-muted-foreground">{estimate?.projectName}</p>
        <p className="text-sm text-muted-foreground">
          Last active: {estimate?.lastActiveRelative}
        </p>
      </div>

      {/* Stats bar */}
      <div className="flex gap-3 text-sm bg-muted rounded-md px-3 py-2">
        <span>{estimate?.turnCount} turns</span>
        <span>·</span>
        <span>{formatTokens(estimate?.historyTokens ?? 0)} tokens</span>
        <span>·</span>
        <span>{estimate?.filesEdited} files</span>
      </div>

      {/* Cost estimate */}
      <div className="border rounded-lg p-4 space-y-2">
        <h4 className="text-sm font-medium">Cost Estimate</h4>
        <div className="flex items-center gap-2">
          <span className={`text-xs px-2 py-0.5 rounded-full ${
            estimate?.cacheWarm
              ? 'bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300'
              : 'bg-amber-100 text-amber-700 dark:bg-amber-900 dark:text-amber-300'
          }`}>
            {estimate?.cacheWarm ? 'WARM' : 'COLD'}
          </span>
          <span className="text-xs text-muted-foreground">
            {estimate?.cacheWarm ? 'Cache active' : `Last active ${estimate?.lastActiveRelative}`}
          </span>
        </div>

        <div className="grid grid-cols-2 gap-2 text-sm">
          <span className="text-muted-foreground">First message:</span>
          <span className="font-mono">
            ~${estimate?.firstMessageCost?.toFixed(4)}
            <span className="text-xs text-muted-foreground ml-1">
              ({estimate?.cacheWarm ? 'cached' : 'cache warming'})
            </span>
          </span>
          <span className="text-muted-foreground">Follow-ups:</span>
          <span className="font-mono">
            ~${estimate?.perMessageCost?.toFixed(4)}
            <span className="text-xs text-muted-foreground ml-1">(cached)</span>
          </span>
        </div>

        {/* Expandable cache explainer */}
        <button
          onClick={() => setShowCacheExplainer(!showCacheExplainer)}
          className="flex items-center gap-1 text-xs text-blue-600 dark:text-blue-400 hover:underline"
        >
          {showCacheExplainer ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
          What's prompt caching?
        </button>
        {showCacheExplainer && (
          <div className="text-xs text-muted-foreground bg-muted rounded p-3 space-y-1">
            <p>Anthropic caches your conversation history for 5 minutes. The first message
            warms the cache (1.25x input cost). Follow-ups read from cache (0.1x cost).</p>
            <p className="font-medium">Tip: Keep conversations flowing to stay in the cache window.</p>
          </div>
        )}
      </div>

      {/* Warning */}
      <div className="flex items-start gap-2 text-sm text-amber-600 dark:text-amber-400">
        <AlertTriangle className="h-4 w-4 mt-0.5 shrink-0" />
        <span>If this session is open in a terminal, close it first to avoid conflicts.</span>
      </div>

      {/* Model selector */}
      <div className="flex items-center gap-2">
        <label className="text-sm">Model:</label>
        <select
          value={model}
          onChange={(e) => setModel(e.target.value)}
          className="text-sm border rounded px-2 py-1"
        >
          <option value="sonnet-4">Sonnet 4</option>
          <option value="opus-4">Opus 4</option>
          <option value="haiku-3.5">Haiku 3.5</option>
        </select>
      </div>

      {/* Actions */}
      <div className="flex gap-3 justify-end">
        <button onClick={onClose} className="px-4 py-2 border rounded hover:bg-muted">
          Cancel
        </button>
        <button
          onClick={() => resumeMutation.mutate()}
          disabled={resumeMutation.isPending || estimateLoading}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
        >
          {resumeMutation.isPending ? 'Resuming...' : 'Resume in Dashboard'}
        </button>
      </div>

      {resumeMutation.isError && (
        <p className="text-sm text-red-500">
          Failed to resume: {resumeMutation.error instanceof Error ? resumeMutation.error.message : 'Unknown error'}
        </p>
      )}
    </div>
  )

  if (isMobile) {
    return (
      <Sheet open={isOpen} onOpenChange={(open) => !open && onClose()}>
        <SheetContent side="bottom" className="max-h-[80vh] overflow-y-auto">
          {content}
        </SheetContent>
      </Sheet>
    )
  }

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>Resume Session</DialogHeader>
        {content}
      </DialogContent>
    </Dialog>
  )
}
```

### Acceptance Criteria (Step 4)

| # | Criterion | Pass |
|---|-----------|------|
| 4.1 | Modal shows session info (title, project, last active) | [ ] |
| 4.2 | Cost estimate displays with warm/cold cache badge | [ ] |
| 4.3 | Cache explainer toggles open/closed | [ ] |
| 4.4 | Warning about terminal conflict is visible | [ ] |
| 4.5 | Model selector defaults to Sonnet, allows Opus/Haiku | [ ] |
| 4.6 | Cancel closes modal without side effects | [ ] |
| 4.7 | Resume button shows loading state during API call | [ ] |
| 4.8 | Error state shown if resume fails | [ ] |
| 4.9 | Mobile: renders as bottom sheet | [ ] |
| 4.10 | Desktop: renders as centered modal | [ ] |

---

## Step 5: Bidirectional WebSocket

### Protocol

```
Frontend ←──WebSocket──→ Axum ←──WebSocket──→ Sidecar ←──SDK──→ Claude Code
```

**Endpoint:** `WS /api/control/sessions/:controlId/stream`

### Message Types (JSON over WebSocket)

```typescript
// sidecar/src/types.ts

// ── Frontend → Sidecar ──

interface UserMessage {
  type: 'user_message'
  content: string
}

interface PermissionResponse {
  type: 'permission_response'
  requestId: string
  allowed: boolean
}

interface Ping {
  type: 'ping'
}

// ── Sidecar → Frontend ──

interface AssistantChunk {
  type: 'assistant_chunk'
  content: string       // partial text
  messageId: string     // for grouping chunks into a message
}

interface AssistantDone {
  type: 'assistant_done'
  messageId: string
  usage: {
    inputTokens: number
    outputTokens: number
    cacheReadTokens: number
    cacheWriteTokens: number
  }
  cost: number          // USD for this turn
  totalCost: number     // cumulative USD for this control session
}

interface ToolUseStart {
  type: 'tool_use_start'
  toolName: string
  toolInput: Record<string, unknown>
  toolUseId: string
}

interface ToolUseResult {
  type: 'tool_use_result'
  toolUseId: string
  output: string
  isError: boolean
}

interface PermissionRequest {
  type: 'permission_request'
  requestId: string
  toolName: string
  toolInput: Record<string, unknown>
  description: string   // human-readable "Claude wants to run: rm -rf node_modules"
  timeoutMs: number     // 60000
}

interface SessionStatus {
  type: 'session_status'
  status: 'active' | 'waiting_input' | 'waiting_permission' | 'completed' | 'error'
  contextUsage: number  // 0.0 - 1.0 (percentage of context window used)
  turnCount: number
}

interface ErrorMessage {
  type: 'error'
  message: string
  fatal: boolean        // if true, session is dead
}

interface Pong {
  type: 'pong'
}

type SidecarMessage =
  | AssistantChunk
  | AssistantDone
  | ToolUseStart
  | ToolUseResult
  | PermissionRequest
  | SessionStatus
  | ErrorMessage
  | Pong
```

### Axum WebSocket Proxy

```rust
// crates/server/src/routes/control.rs

pub async fn ws_proxy(
    ws: WebSocketUpgrade,
    Path(control_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_proxy(socket, control_id, state))
}

async fn handle_ws_proxy(
    frontend_ws: WebSocket,
    control_id: String,
    state: Arc<AppState>,
) {
    let socket_path = match state.sidecar.ensure_running().await {
        Ok(p) => p,
        Err(e) => {
            let _ = frontend_ws.send(Message::Text(
                serde_json::to_string(&json!({
                    "type": "error",
                    "message": format!("Sidecar unavailable: {e}"),
                    "fatal": true,
                })).unwrap()
            )).await;
            return;
        }
    };

    // Connect to sidecar WebSocket
    let sidecar_url = format!(
        "ws+unix://{}:/control/sessions/{}/stream",
        socket_path, control_id
    );
    let (sidecar_ws, _) = tokio_tungstenite::connect_async(sidecar_url)
        .await
        .expect("Failed to connect to sidecar WS");

    let (mut fe_sink, mut fe_stream) = frontend_ws.split();
    let (mut sc_sink, mut sc_stream) = sidecar_ws.split();

    // Bidirectional relay
    let fe_to_sc = async {
        while let Some(Ok(msg)) = fe_stream.next().await {
            if sc_sink.send(msg.into()).await.is_err() { break; }
        }
    };

    let sc_to_fe = async {
        while let Some(Ok(msg)) = sc_stream.next().await {
            if fe_sink.send(msg.into()).await.is_err() { break; }
        }
    };

    tokio::select! {
        _ = fe_to_sc => {},
        _ = sc_to_fe => {},
    }
}
```

### Sidecar WebSocket Handler

```typescript
// sidecar/src/ws-handler.ts
import { WebSocket } from 'ws'
import { SessionManager, ControlSession } from './session-manager.js'

export function handleWebSocket(
  ws: WebSocket,
  controlId: string,
  sessions: SessionManager,
) {
  const session = sessions.getByControlId(controlId)
  if (!session) {
    ws.send(JSON.stringify({
      type: 'error',
      message: 'Session not found',
      fatal: true,
    }))
    ws.close()
    return
  }

  // Register WS as the output channel for this session
  session.setOutputStream(ws)

  ws.on('message', async (raw) => {
    const msg = JSON.parse(raw.toString())

    switch (msg.type) {
      case 'user_message':
        await session.sendMessage(msg.content)
        break

      case 'permission_response':
        session.resolvePermission(msg.requestId, msg.allowed)
        break

      case 'ping':
        ws.send(JSON.stringify({ type: 'pong' }))
        break
    }
  })

  ws.on('close', () => {
    session.removeOutputStream()
    // Don't terminate session -- user might reconnect
  })

  // Send current status immediately
  ws.send(JSON.stringify({
    type: 'session_status',
    status: session.status,
    contextUsage: session.contextUsage,
    turnCount: session.turnCount,
  }))
}
```

### Acceptance Criteria (Step 5)

| # | Criterion | Pass |
|---|-----------|------|
| 5.1 | WebSocket connection established through Axum proxy | [ ] |
| 5.2 | User messages relay from frontend to sidecar to SDK | [ ] |
| 5.3 | Assistant chunks stream in real-time (typewriter) | [ ] |
| 5.4 | Tool use events (start, result) forwarded to frontend | [ ] |
| 5.5 | Permission requests route to frontend | [ ] |
| 5.6 | WebSocket reconnects if connection drops (frontend) | [ ] |
| 5.7 | Session survives WS disconnect (user can reconnect) | [ ] |
| 5.8 | Ping/pong keepalive every 30s | [ ] |

---

## Step 6: Dashboard Chat View

### Component: `src/components/live/DashboardChat.tsx`

Full-screen chat interface for interacting with a resumed session.

### Layout

```
┌──────────────────────────────────────────────────────────────┐
│  ← Back to Mission Control    "Fix auth middleware"    ...   │
│──────────────────────────────────────────────────────────────│
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │                 Conversation History                    │  │
│  │                 (read-only, from JSONL)                 │  │
│  │                                                        │  │
│  │  User: "Can you fix the auth middleware to..."         │  │
│  │                                                        │  │
│  │  Assistant: "I'll look at the auth middleware..."      │  │
│  │  ┌──────────────────────────────────────────┐          │  │
│  │  │ Tool: Edit file                          │          │  │
│  │  │ src/middleware/auth.ts                    │          │  │
│  │  └──────────────────────────────────────────┘          │  │
│  │                                                        │  │
│  │  ─── Dashboard session started ───                     │  │
│  │                                                        │  │
│  │  User: "Now add rate limiting"                         │  │
│  │                                                        │  │
│  │  Assistant: "I'll add rate lim|"  ← streaming cursor   │  │
│  │                                                        │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  Type a message...                              [Send] │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│──────────────────────────────────────────────────────────────│
│  Context: 34%  │  Turn 49  │  Session: $0.22  │  +$0.01 ◉  │
└──────────────────────────────────────────────────────────────┘
```

### Key Behaviors

| Feature | Behavior |
|---------|----------|
| History loading | Fetch from existing `GET /api/session/:project/:id/messages` (paginated) |
| Divider | "Dashboard session started" marker between history and new messages |
| Streaming | WebSocket chunks rendered with blinking cursor |
| Auto-scroll | Scroll to bottom on new content (unless user scrolled up) |
| Tool calls | Rendered inline with collapsible detail |
| Input | Textarea with Shift+Enter for newlines, Enter to send |
| Back button | Returns to Mission Control grid view |
| Session end | "Session completed" banner, input disabled |

### Frontend Hook: `src/hooks/use-control-session.ts`

```typescript
import { useCallback, useEffect, useRef, useState } from 'react'

interface ControlSessionState {
  status: 'connecting' | 'active' | 'waiting_input' | 'waiting_permission' | 'completed' | 'error' | 'disconnected'
  messages: ChatMessage[]       // new messages from this control session
  streamingContent: string      // partial assistant response
  contextUsage: number          // 0-1
  turnCount: number
  sessionCost: number           // cumulative USD
  lastTurnCost: number          // USD for most recent turn
  permissionRequest: PermissionRequest | null
}

export function useControlSession(controlId: string | null) {
  const [state, setState] = useState<ControlSessionState>(initialState)
  const wsRef = useRef<WebSocket | null>(null)

  useEffect(() => {
    if (!controlId) return

    const wsUrl = sseUrl(`/api/control/sessions/${controlId}/stream`)
      .replace('http', 'ws')
    const ws = new WebSocket(wsUrl)
    wsRef.current = ws

    ws.onmessage = (event) => {
      const msg = JSON.parse(event.data) as SidecarMessage

      switch (msg.type) {
        case 'assistant_chunk':
          setState(prev => ({
            ...prev,
            streamingContent: prev.streamingContent + msg.content,
            status: 'active',
          }))
          break

        case 'assistant_done':
          setState(prev => ({
            ...prev,
            messages: [...prev.messages, {
              role: 'assistant',
              content: prev.streamingContent,
              messageId: msg.messageId,
              usage: msg.usage,
            }],
            streamingContent: '',
            sessionCost: msg.totalCost,
            lastTurnCost: msg.cost,
            status: 'waiting_input',
          }))
          break

        case 'tool_use_start':
          setState(prev => ({
            ...prev,
            messages: [...prev.messages, {
              role: 'tool_use',
              toolName: msg.toolName,
              toolInput: msg.toolInput,
              toolUseId: msg.toolUseId,
            }],
          }))
          break

        case 'tool_use_result':
          setState(prev => ({
            ...prev,
            messages: [...prev.messages, {
              role: 'tool_result',
              toolUseId: msg.toolUseId,
              output: msg.output,
              isError: msg.isError,
            }],
          }))
          break

        case 'permission_request':
          setState(prev => ({
            ...prev,
            permissionRequest: msg,
            status: 'waiting_permission',
          }))
          break

        case 'session_status':
          setState(prev => ({
            ...prev,
            status: msg.status,
            contextUsage: msg.contextUsage,
            turnCount: msg.turnCount,
          }))
          break

        case 'error':
          setState(prev => ({
            ...prev,
            status: msg.fatal ? 'error' : prev.status,
          }))
          break
      }
    }

    ws.onclose = () => {
      setState(prev => ({ ...prev, status: 'disconnected' }))
      // Auto-reconnect after 2s (unless session completed)
      setTimeout(() => {
        if (wsRef.current?.readyState === WebSocket.CLOSED) {
          // Reconnect logic
        }
      }, 2000)
    }

    // Keepalive
    const pingInterval = setInterval(() => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'ping' }))
      }
    }, 30000)

    return () => {
      clearInterval(pingInterval)
      ws.close()
    }
  }, [controlId])

  const sendMessage = useCallback((content: string) => {
    if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) return
    wsRef.current.send(JSON.stringify({ type: 'user_message', content }))
    setState(prev => ({
      ...prev,
      messages: [...prev.messages, { role: 'user', content }],
      status: 'active',
    }))
  }, [])

  const respondPermission = useCallback((requestId: string, allowed: boolean) => {
    if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) return
    wsRef.current.send(JSON.stringify({ type: 'permission_response', requestId, allowed }))
    setState(prev => ({
      ...prev,
      permissionRequest: null,
      status: 'active',
    }))
  }, [])

  return { ...state, sendMessage, respondPermission }
}
```

### Acceptance Criteria (Step 6)

| # | Criterion | Pass |
|---|-----------|------|
| 6.1 | History messages load from existing paginated API | [ ] |
| 6.2 | "Dashboard session started" divider visible | [ ] |
| 6.3 | Streaming text renders with typewriter effect | [ ] |
| 6.4 | Tool use events render inline with collapsible detail | [ ] |
| 6.5 | Input box sends message on Enter, newline on Shift+Enter | [ ] |
| 6.6 | Auto-scroll to bottom on new content | [ ] |
| 6.7 | Manual scroll-up pauses auto-scroll | [ ] |
| 6.8 | Back button returns to Mission Control grid | [ ] |
| 6.9 | Mobile: full-screen chat, input fixed to bottom | [ ] |
| 6.10 | Session completed state: input disabled, banner shown | [ ] |

---

## Step 7: Permission Routing

### Flow

```
Claude Code         Sidecar              Axum/WS            Frontend
    │                  │                    │                   │
    │ canUseTool(      │                    │                   │
    │  "bash",         │                    │                   │
    │  {cmd: "rm -rf"})│                    │                   │
    │─────────────────>│                    │                   │
    │                  │  permission_request │                   │
    │                  │   {toolName, input, │                   │
    │                  │    timeout: 60s}    │                   │
    │                  │───────────────────->│                   │
    │                  │                    │  permission_request│
    │                  │                    │──────────────────->│
    │                  │                    │                   │
    │                  │                    │    ┌──────────────┤
    │                  │                    │    │ User sees    │
    │                  │                    │    │ approval     │
    │                  │                    │    │ dialog       │
    │                  │                    │    └──────────────┤
    │                  │                    │                   │
    │                  │                    │  permission_resp  │
    │                  │                    │  {allowed: true}  │
    │                  │                    │<──────────────────│
    │                  │  permission_resp   │                   │
    │                  │<──────────────────-│                   │
    │  true            │                    │                   │
    │<─────────────────│                    │                   │
    │                  │                    │                   │
    │ (executes tool)  │                    │                   │
```

### Permission Dialog Component

```tsx
// src/components/live/PermissionDialog.tsx

interface PermissionDialogProps {
  request: PermissionRequest
  onRespond: (requestId: string, allowed: boolean) => void
}

export function PermissionDialog({ request, onRespond }: PermissionDialogProps) {
  const [timeLeft, setTimeLeft] = useState(request.timeoutMs / 1000)

  // Countdown timer
  useEffect(() => {
    const timer = setInterval(() => {
      setTimeLeft(prev => {
        if (prev <= 1) {
          onRespond(request.requestId, false) // auto-deny
          return 0
        }
        return prev - 1
      })
    }, 1000)
    return () => clearInterval(timer)
  }, [request.requestId, onRespond])

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-white dark:bg-gray-900 rounded-lg shadow-xl max-w-md w-full p-6 space-y-4">
        <div className="flex items-center gap-2 text-amber-500">
          <AlertTriangle className="h-5 w-5" />
          <h3 className="font-semibold">Permission Required</h3>
        </div>

        <p className="text-sm">Claude wants to use:</p>

        <div className="bg-muted rounded-md p-3 font-mono text-sm">
          <div className="text-muted-foreground mb-1">{request.toolName}</div>
          <pre className="whitespace-pre-wrap break-all">
            {typeof request.toolInput === 'string'
              ? request.toolInput
              : JSON.stringify(request.toolInput, null, 2)}
          </pre>
        </div>

        {request.description && (
          <p className="text-sm text-muted-foreground">{request.description}</p>
        )}

        <div className="text-xs text-muted-foreground">
          Auto-deny in {timeLeft}s
        </div>

        <div className="flex gap-3 justify-end">
          <button
            onClick={() => onRespond(request.requestId, false)}
            className="px-4 py-2 border rounded hover:bg-red-50 dark:hover:bg-red-900/20 text-red-600"
          >
            Deny
          </button>
          <button
            onClick={() => onRespond(request.requestId, true)}
            className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700"
          >
            Allow
          </button>
        </div>
      </div>
    </div>
  )
}
```

### Sidecar: Agent SDK Integration

```typescript
// sidecar/src/session-manager.ts

import { query, type ToolUseEvent, type CanUseToolCallback } from '@anthropic-ai/claude-agent-sdk'

export class ControlSession {
  controlId: string
  sessionId: string
  status: string = 'active'
  contextUsage: number = 0
  turnCount: number = 0
  totalCost: number = 0
  private ws: WebSocket | null = null
  private pendingPermissions: Map<string, {
    resolve: (allowed: boolean) => void
    timer: NodeJS.Timeout
  }> = new Map()

  async sendMessage(content: string) {
    // Delegate to Agent SDK session
    // The SDK handles conversation management internally
  }

  setOutputStream(ws: WebSocket) {
    this.ws = ws
  }

  removeOutputStream() {
    this.ws = null
  }

  // Called by Agent SDK's canUseTool callback
  async requestPermission(toolName: string, toolInput: unknown): Promise<boolean> {
    if (!this.ws) {
      return false // no frontend connected, deny by default
    }

    const requestId = crypto.randomUUID()
    const timeoutMs = 60_000

    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        this.pendingPermissions.delete(requestId)
        this.emit({
          type: 'session_status',
          status: 'active',
          contextUsage: this.contextUsage,
          turnCount: this.turnCount,
        })
        resolve(false) // auto-deny on timeout
      }, timeoutMs)

      this.pendingPermissions.set(requestId, { resolve, timer })

      this.emit({
        type: 'permission_request',
        requestId,
        toolName,
        toolInput: toolInput as Record<string, unknown>,
        description: formatPermissionDescription(toolName, toolInput),
        timeoutMs,
      })
    })
  }

  resolvePermission(requestId: string, allowed: boolean) {
    const pending = this.pendingPermissions.get(requestId)
    if (pending) {
      clearTimeout(pending.timer)
      pending.resolve(allowed)
      this.pendingPermissions.delete(requestId)
    }
  }

  private emit(msg: object) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg))
    }
  }
}

function formatPermissionDescription(toolName: string, input: unknown): string {
  if (toolName === 'bash' && typeof input === 'object' && input !== null) {
    const cmd = (input as any).command ?? (input as any).cmd ?? ''
    return `Run command: ${cmd}`
  }
  if (toolName === 'edit' || toolName === 'write') {
    const file = (input as any).file_path ?? (input as any).path ?? ''
    return `${toolName === 'edit' ? 'Edit' : 'Write'} file: ${file}`
  }
  return `Use tool: ${toolName}`
}
```

### Acceptance Criteria (Step 7)

| # | Criterion | Pass |
|---|-----------|------|
| 7.1 | Permission dialog renders with tool name and input | [ ] |
| 7.2 | Allow sends `true` back through WebSocket chain | [ ] |
| 7.3 | Deny sends `false` back through WebSocket chain | [ ] |
| 7.4 | Auto-deny after 60s with visual countdown | [ ] |
| 7.5 | Multiple permission requests queue (one at a time) | [ ] |
| 7.6 | No frontend connected: auto-deny immediately | [ ] |
| 7.7 | Bash commands show the actual command in the dialog | [ ] |
| 7.8 | File edits show the file path in the dialog | [ ] |

---

## Step 8: Real-Time Cost Tracking

### Status Bar Component

Persistent bar at the bottom of the DashboardChat view.

```
┌──────────────────────────────────────────────────────────────┐
│  Context: ████████░░░░░░░░░░░░  34%  │  Turn 49  │          │
│  Session: $0.22  │  Last: +$0.01 (cached)  │  Saved: $1.87  │
└──────────────────────────────────────────────────────────────┘
```

### Cost Tracking Logic

| Event | Update |
|-------|--------|
| `assistant_done` | Add `msg.cost` to running total. Display "+$X.XX (cached)" or "+$X.XX (cache warming)" |
| `session_status` | Update context usage bar |
| Session close | Show summary: "Prompt caching saved you $X.XX" (compare cached vs uncached cost) |

### Savings Calculation

```typescript
function calculateSavings(turns: TurnCost[]): number {
  let cachedCost = 0
  let uncachedCost = 0

  for (const turn of turns) {
    cachedCost += turn.actualCost
    // What it would have cost without caching
    uncachedCost += turn.inputTokens * turn.baseInputRate / 1_000_000
      + turn.outputTokens * turn.baseOutputRate / 1_000_000
  }

  return uncachedCost - cachedCost
}
```

### Session Summary (on close/complete)

```
┌──────────────────────────────────────────────────────┐
│                                                      │
│  Session Complete                                    │
│                                                      │
│  12 turns  ·  47,832 tokens  ·  8 files edited       │
│                                                      │
│  Total cost:  $0.34                                  │
│  Without caching:  $2.21                             │
│  ─────────────────────────────                       │
│  Prompt caching saved you $1.87 (85%)                │
│                                                      │
│  [Back to Mission Control]                           │
└──────────────────────────────────────────────────────┘
```

### Acceptance Criteria (Step 8)

| # | Criterion | Pass |
|---|-----------|------|
| 8.1 | Status bar shows context usage percentage with progress bar | [ ] |
| 8.2 | Turn count updates after each assistant response | [ ] |
| 8.3 | Running cost updates after each `assistant_done` | [ ] |
| 8.4 | Last turn cost shows "(cached)" or "(cache warming)" | [ ] |
| 8.5 | Session summary shows total cost and savings | [ ] |
| 8.6 | Savings calculation compares cached vs uncached | [ ] |
| 8.7 | Epoch-zero timestamps never appear in cost display | [ ] |

---

## Step 9: Distribution Integration

### Strategy: Option (b) -- sidecar as npm dependency

The sidecar is bundled in the npx-cli package and started on-demand.

### Directory Layout in npx-cli

```
~/.cache/claude-view/
├── bin/
│   ├── vibe-recall              ← Rust binary (existing)
│   ├── dist/                    ← Frontend assets (existing)
│   └── sidecar/                 ← NEW: sidecar JS bundle
│       ├── index.js             ← Built by tsup (single file)
│       ├── node_modules/        ← Sidecar deps (agent SDK)
│       └── package.json
└── version
```

### CI Changes

Add sidecar build to the release workflow:

```yaml
# .github/workflows/release.yml (additions)

  build-sidecar:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - run: cd sidecar && npm ci && npm run build
      - uses: actions/upload-artifact@v4
        with:
          name: sidecar-dist
          path: sidecar/dist/

  build-binary:
    needs: [build-sidecar]
    # ... existing binary build
    steps:
      # After building Rust binary, include sidecar in archive
      - uses: actions/download-artifact@v4
        with:
          name: sidecar-dist
          path: staging/sidecar/
      - run: |
          # Copy sidecar into the release archive alongside binary and dist/
          cp -r staging/sidecar release-dir/sidecar
```

### npx-cli Changes

```javascript
// npx-cli/index.js additions

// After extracting binary + dist, ensure sidecar exists
const sidecarDir = path.join(binDir, 'sidecar')
if (fs.existsSync(sidecarDir)) {
  // Install sidecar dependencies if needed
  const sidecarModules = path.join(sidecarDir, 'node_modules')
  if (!fs.existsSync(sidecarModules)) {
    console.log('Installing sidecar dependencies...')
    execFileSync('npm', ['install', '--production'], {
      cwd: sidecarDir,
      stdio: 'pipe',
    })
  }
}

// Set SIDECAR_DIR so Rust knows where to find it
const env = {
  ...process.env,
  STATIC_DIR: distDir,
  SIDECAR_DIR: sidecarDir,
}
```

### On-Demand Start

The sidecar does **not** start on boot. It starts only when the user first clicks "Resume in Dashboard". This means:
- `npx claude-view` starts instantly (Rust binary only)
- Sidecar starts in ~1s when first needed
- No Node.js overhead for monitoring-only users

### Acceptance Criteria (Step 9)

| # | Criterion | Pass |
|---|-----------|------|
| 9.1 | Sidecar JS bundle included in release archive | [ ] |
| 9.2 | npx-cli sets SIDECAR_DIR env var | [ ] |
| 9.3 | Rust binary finds sidecar via SIDECAR_DIR | [ ] |
| 9.4 | Sidecar deps install on first use (production only) | [ ] |
| 9.5 | `npx claude-view` startup time unaffected (sidecar lazy) | [ ] |
| 9.6 | Release archive size increase < 5MB (sidecar is small) | [ ] |

---

## Error Handling & Edge Cases

### Sidecar Failure Modes

| Failure | Detection | Recovery |
|---------|-----------|----------|
| Sidecar crashes | Health check fails (30s poll) | Auto-restart on next `/api/control/*` call |
| Sidecar OOM | Process exits with signal | Same as crash |
| Agent SDK error | SDK throws in session | Error message via WebSocket, session marked failed |
| Network timeout to API | SDK timeout | Error message, user can retry |
| Orphan sidecar | Parent PID check (2s interval) | Self-terminate |
| Stale socket | Check on startup | Delete and recreate |
| No Node.js installed | `node` not in PATH | Error: "Node.js required for interactive mode" |

### Graceful Degradation

If the sidecar is unavailable, the rest of the app works normally:
- Monitoring (Phase A) continues
- All views (Phase B-E) work
- Only "Resume in Dashboard" is disabled
- UI shows: "Interactive mode unavailable. Install Node.js 20+ to enable."

---

## Security

| Concern | Mitigation |
|---------|------------|
| Sidecar network exposure | Unix socket (no TCP port). Filesystem permissions restrict access. |
| Agent SDK API key | Uses `ANTHROPIC_API_KEY` from user's environment (already configured for Claude Code) |
| Tool permission bypass | All dangerous tools routed to frontend for approval. No auto-allow. |
| WebSocket origin check | Validate `Origin` header matches `localhost:47892` |
| Session isolation | Each control session is an independent subprocess |
| Input sanitization | DOMPurify on all tool output displayed in chat (consistent with existing hardening) |

---

## File Summary

### New Files (Rust)

| File | Purpose |
|------|---------|
| `crates/server/src/sidecar.rs` | SidecarManager: spawn, health check, shutdown |
| `crates/server/src/routes/control.rs` | `/api/control/*` proxy routes + WebSocket handler |
| `crates/server/src/cost_estimation.rs` | `estimate_resume_cost()` function |

### New Files (Sidecar)

| File | Purpose |
|------|---------|
| `sidecar/package.json` | Dependencies and scripts |
| `sidecar/tsconfig.json` | TypeScript config |
| `sidecar/src/index.ts` | HTTP server entry, lifecycle |
| `sidecar/src/health.ts` | Health check handler |
| `sidecar/src/control.ts` | Resume, send, list, terminate handlers |
| `sidecar/src/session-manager.ts` | Per-session state machine + permission routing |
| `sidecar/src/ws-handler.ts` | WebSocket message handler |
| `sidecar/src/types.ts` | Shared IPC message types |

### New Files (Frontend)

| File | Purpose |
|------|---------|
| `src/components/live/ResumePreFlight.tsx` | Pre-flight cost/info modal |
| `src/components/live/DashboardChat.tsx` | Full chat UI for resumed sessions |
| `src/components/live/PermissionDialog.tsx` | Tool permission approval dialog |
| `src/components/live/ChatStatusBar.tsx` | Context/cost/turn status bar |
| `src/components/live/SessionSummary.tsx` | End-of-session cost summary |
| `src/hooks/use-control-session.ts` | WebSocket state management hook |
| `src/types/control.ts` | TypeScript types for control messages |

### Modified Files

| File | Change |
|------|--------|
| `crates/server/src/routes/mod.rs` | Add `control` module, nest routes |
| `crates/server/src/state.rs` | Add `SidecarManager` to `AppState` |
| `crates/server/src/main.rs` | Register sidecar manager, shutdown hook |
| `npx-cli/index.js` | Add `SIDECAR_DIR` env, sidecar dep install |
| `.github/workflows/release.yml` | Add sidecar build step |
| `src/App.tsx` (or router) | Add `/control/:controlId` route for DashboardChat |

---

## Testing Strategy

### Unit Tests

| Test | File | What |
|------|------|------|
| Cost estimation math | `crates/server/src/cost_estimation.rs` | Cache warm/cold, all model tiers, edge cases (0 tokens, epoch-zero timestamps) |
| Sidecar manager | `crates/server/src/sidecar.rs` | Spawn, health check timeout, restart, shutdown |
| WebSocket message parsing | `src/hooks/use-control-session.test.ts` | All message types correctly update state |
| Permission timeout | `src/components/live/PermissionDialog.test.tsx` | Countdown, auto-deny at 0 |
| Cost display formatting | `src/components/live/ChatStatusBar.test.tsx` | USD formatting, cached/uncached labels |

### Integration Tests

| Test | What |
|------|------|
| Resume flow | POST `/api/control/resume` with valid session ID, verify sidecar starts and returns control_id |
| WebSocket relay | Connect WS, send user_message, verify it reaches sidecar, verify assistant_chunk comes back |
| Permission round-trip | Trigger tool use, verify permission_request reaches frontend, send allow, verify tool executes |
| Sidecar crash recovery | Kill sidecar process, verify next control request restarts it |
| Cost accuracy | Resume session with known token count, verify estimate matches expected cost |

### E2E Tests

| Test | What |
|------|------|
| Full resume flow | Click "Resume in Dashboard" on session card -> pre-flight modal -> confirm -> chat view opens -> type message -> get streaming response |
| Permission flow | Resume -> Claude tries to edit file -> permission dialog appears -> click Allow -> file edit succeeds |
| Mobile resume | Same as above but at 375px viewport width (bottom sheet instead of modal) |

---

## Implementation Order

```
Step 1: Sidecar directory + bootstrap     ← 2 days
    │   (spawn, health check, shutdown)
    │
    ├── Step 2: Cost estimation endpoint  ← 0.5 day
    │   (Rust-only, no sidecar needed)
    │
    └── Step 3: Resume endpoint           ← 1.5 days
        │   (sidecar + Agent SDK integration)
        │
        ├── Step 4: Pre-flight UI         ← 1 day
        │   (modal/sheet with cost display)
        │
        └── Step 5: WebSocket relay       ← 2 days
            │   (bidirectional Axum proxy)
            │
            ├── Step 6: Dashboard Chat    ← 2 days
            │   (history + streaming + input)
            │
            ├── Step 7: Permission routing ← 1 day
            │   (dialog + timeout + relay)
            │
            └── Step 8: Cost tracking     ← 1 day
                │   (status bar + summary)
                │
                └── Step 9: Distribution  ← 1 day
                    (CI + npx-cli changes)

Total estimated effort: ~12 days
```

Steps 2 and 3 can start in parallel. Steps 6, 7, and 8 can progress in parallel after Step 5.

---

## Acceptance Criteria (Overall)

| # | Criterion | Pass |
|---|-----------|------|
| F.1 | Sidecar starts and communicates with Axum successfully | [ ] |
| F.2 | Resume loads conversation history and continues session | [ ] |
| F.3 | Pre-flight panel shows accurate cost estimate | [ ] |
| F.4 | Cache warm/cold detection is correct (based on last activity timestamp) | [ ] |
| F.5 | Chat input sends messages and receives streaming responses | [ ] |
| F.6 | Tool permission requests route to UI and back | [ ] |
| F.7 | Cost tracking updates in real-time during chat | [ ] |
| F.8 | Sidecar shuts down cleanly when Axum exits | [ ] |
| F.9 | Works via `npx claude-view` distribution | [ ] |
| F.10 | Mobile: chat view is usable on phone screens | [ ] |
| F.11 | Error handling: sidecar crash leads to graceful degradation (monitoring still works) | [ ] |
| F.12 | No security vulnerabilities (sidecar only listens on localhost/Unix socket) | [ ] |
| F.13 | Auto-deny tool permissions after 60s timeout | [ ] |
| F.14 | Session summary shows prompt caching savings | [ ] |
| F.15 | No Node.js required for monitoring-only usage | [ ] |

---

## Dependencies

| Dependency | Version | Why |
|------------|---------|-----|
| `@anthropic-ai/claude-agent-sdk` | ^0.1.0 | Core SDK for spawning/resuming Claude Code sessions |
| `hono` | ^4.0.0 | Lightweight HTTP framework for sidecar (14KB, fast) |
| `ws` | ^8.16.0 | WebSocket server for sidecar |
| `tsup` | ^8.0.0 | Bundle sidecar into single JS file for distribution |
| `hyperlocal` (Rust) | ^0.9 | HTTP-over-Unix-socket client for Axum->sidecar |
| `tokio-tungstenite` (Rust) | ^0.24 | WebSocket client for Axum->sidecar WS relay |

---

## Related Documents

- Phase A: `phase-a-monitoring.md` (depends on)
- Design: `design.md` (overall Mission Control architecture)
- Research: `PROGRESS.md` (Key Decisions Log, Research Artifacts)
- CLAUDE.md: Sidecar distribution decision, Vite dev proxy SSE bypass pattern
