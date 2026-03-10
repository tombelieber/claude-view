# Open in IDE — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add "Open in IDE" buttons that let users open projects and files in VS Code, Cursor, Windsurf, Zed, WebStorm, or IntelliJ directly from claude-view.

**Architecture:** Backend detects installed IDEs at startup, caches the list in AppState, and exposes two endpoints: `GET /api/ide/detect` and `POST /api/ide/open`. Frontend stores the user's IDE preference in localStorage and renders a split button (main click = open, dropdown = pick IDE) in three locations: Changes tab header, FileChangeHeader rows, and Kanban swimlane ProjectHeader.

**Tech Stack:** Rust (Axum), React, Radix UI Popover, react-query, localStorage

**Spec:** `docs/superpowers/specs/2026-03-11-open-in-ide-design.md`

---

## Chunk 1: Backend — IDE Detection & Launch

### Task 1: Rust types and IDE registry

**Files:**
- Create: `crates/server/src/routes/ide.rs`

- [ ] **Step 1: Create ide.rs with types and static IDE list**

```rust
// crates/server/src/routes/ide.rs
//! IDE detection and launch endpoints.

use std::sync::Arc;

use axum::{extract::State, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ============================================================================
// Wire types (Rust → TypeScript via ts-rs)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct IdeInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct IdeDetectResponse {
    pub available: Vec<IdeInfo>,
}

#[derive(Debug, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct OpenInIdeRequest {
    pub ide: String,
    pub project_path: String,
    pub file_path: Option<String>,
}

// ============================================================================
// Internal IDE definition (not exported to TS)
// ============================================================================

#[derive(Debug, Clone)]
pub struct IdeDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub command: &'static str,
}

/// All known IDEs. Detection checks which of these have their CLI available.
pub const KNOWN_IDES: &[IdeDefinition] = &[
    IdeDefinition { id: "vscode", name: "VS Code", command: "code" },
    IdeDefinition { id: "cursor", name: "Cursor", command: "cursor" },
    IdeDefinition { id: "windsurf", name: "Windsurf", command: "windsurf" },
    IdeDefinition { id: "zed", name: "Zed", command: "zed" },
    IdeDefinition { id: "webstorm", name: "WebStorm", command: "webstorm" },
    IdeDefinition { id: "intellij", name: "IntelliJ IDEA", command: "idea" },
];
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p claude-view-server`
Expected: success (no handlers yet, just types)

---

### Task 2: IDE detection function

**Files:**
- Modify: `crates/server/src/routes/ide.rs`

- [ ] **Step 1: Add detect_installed_ides function**

Append to `ide.rs`:

```rust
// ============================================================================
// Detection — runs once at server startup
// ============================================================================

/// Check which IDEs from KNOWN_IDES have their CLI command available on PATH.
pub fn detect_installed_ides() -> Vec<(IdeInfo, String)> {
    let mut found = Vec::new();
    for def in KNOWN_IDES {
        if let Ok(output) = std::process::Command::new("which")
            .arg(def.command)
            .output()
        {
            if output.status.success() {
                found.push((
                    IdeInfo {
                        id: def.id.to_string(),
                        name: def.name.to_string(),
                    },
                    def.command.to_string(),
                ));
            }
        }
    }
    found
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p claude-view-server`

---

### Task 3: Add available_ides to AppState

**Files:**
- Modify: `crates/server/src/state.rs` (add field after `teams` ~line 133, update 3 constructors)
- Modify: `crates/server/src/lib.rs` (update `create_app_full()` struct literal ~line 194)

**IMPORTANT:** There are **5 construction sites** for `AppState`:
1. `AppState::new()` in `state.rs` ~line 146 (used in tests)
2. `AppState::new_with_indexing()` in `state.rs` ~line 188 (used in tests)
3. `AppState::new_with_indexing_and_registry()` in `state.rs` ~line 229 (used in tests)
4. `create_app_with_git_sync()` struct literal in `lib.rs` ~line 115 (used in git-sync tests)
5. `create_app_full()` struct literal in `lib.rs` ~line 194 (**production path — this is the one `main.rs` calls**)

All 5 MUST be updated or the project will not compile (Rust struct literals are exhaustive).

- [ ] **Step 1: Read state.rs and lib.rs to find exact insertion points**

Read `crates/server/src/state.rs` to confirm the struct definition and constructor signatures.
Read `crates/server/src/lib.rs` lines 194–227 to see the `create_app_full()` struct literal.

- [ ] **Step 2: Add the field and type alias to state.rs**

In `state.rs`, add:
- Type alias near the top (after other type aliases): `pub type AvailableIdesHolder = Vec<(crate::routes::ide::IdeInfo, String)>;`
  - **Note:** This creates a `state → routes::ide` dependency. All other type aliases in `state.rs` reference external crates (`claude_view_core`, `claude_view_db`, etc.), never `crate::routes::*`. This compiles fine (single crate), but is an inverted dependency. Acceptable for MVP; if it bothers you, move `IdeInfo` to a shared `crate::types` module later.
- New field in `AppState` struct (after `prompt_templates` ~line 139): `pub available_ides: AvailableIdesHolder,`
- Initialize as `available_ides: Vec::new()` in all 3 constructors in `state.rs`

- [ ] **Step 3: Verify it compiles (expect failure — lib.rs not yet updated)**

Run: `cargo check -p claude-view-server`
Expected: compile errors in `lib.rs` — `missing field available_ides` in both `create_app_with_git_sync()` (~line 115) and `create_app_full()` (~line 194). This confirms the field was added correctly to the struct.

---

### Task 4: Run IDE detection at startup and fix both lib.rs construction sites

**Files:**
- Modify: `crates/server/src/lib.rs` — fix both `create_app_with_git_sync()` (~line 115) and `create_app_full()` (~line 194)

There are TWO struct literals in `lib.rs` that need `available_ides`:
- `create_app_with_git_sync()` ~line 115 — test helper, just needs `available_ides: Vec::new()`
- `create_app_full()` ~line 194 — production path, runs actual IDE detection

- [ ] **Step 1: Read lib.rs to find both struct literals**

Read `crates/server/src/lib.rs` lines 114–150 (`create_app_with_git_sync`) and lines 186–227 (`create_app_full`).

- [ ] **Step 2: Fix create_app_with_git_sync() — add empty Vec**

In the struct literal at ~line 115, add after `prompt_templates`:

```rust
        available_ides: Vec::new(),
```

- [ ] **Step 3: Add IDE detection inside create_app_full(), before the struct literal**

Insert detection call just before `let state = Arc::new(state::AppState {` (~line 194):

```rust
// Detect installed IDEs (fast — just runs `which` for each)
let available_ides = crate::routes::ide::detect_installed_ides();
tracing::info!(count = available_ides.len(), "Detected installed IDEs");
for (ide, cmd) in &available_ides {
    tracing::info!(id = %ide.id, name = %ide.name, command = %cmd, "Found IDE");
}
```

Then add the field to the `create_app_full` struct literal (after `prompt_templates`):

```rust
        available_ides,
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p claude-view-server`

---

### Task 5: Implement GET /api/ide/detect endpoint

**Files:**
- Modify: `crates/server/src/routes/ide.rs`

- [ ] **Step 1: Add the detect handler**

```rust
// ============================================================================
// Handlers
// ============================================================================

/// GET /api/ide/detect — returns which IDEs are installed.
pub async fn get_detect(
    State(state): State<Arc<AppState>>,
) -> Json<IdeDetectResponse> {
    let available: Vec<IdeInfo> = state
        .available_ides
        .iter()
        .map(|(info, _cmd)| info.clone())
        .collect();
    Json(IdeDetectResponse { available })
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p claude-view-server`

---

### Task 6: Implement POST /api/open-in-ide endpoint

**Files:**
- Modify: `crates/server/src/routes/ide.rs`

- [ ] **Step 1: Add the open handler with security validation**

```rust
/// POST /api/open-in-ide — launch an IDE with a project or file.
pub async fn post_open(
    State(state): State<Arc<AppState>>,
    Json(req): Json<OpenInIdeRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // 1. Find the requested IDE in our detected list
    let (_info, command) = state
        .available_ides
        .iter()
        .find(|(info, _)| info.id == req.ide)
        .ok_or_else(|| ApiError::BadRequest(format!("IDE not found: {}", req.ide)))?;
    let command = command.clone();

    // 2. Validate projectPath
    let project_path = std::path::PathBuf::from(&req.project_path);
    if !project_path.is_absolute() {
        return Err(ApiError::BadRequest("projectPath must be absolute".into()));
    }
    let canonical_project = std::fs::canonicalize(&project_path)
        .map_err(|e| ApiError::BadRequest(format!("projectPath not found: {e}")))?;
    if !canonical_project.is_dir() {
        return Err(ApiError::BadRequest("projectPath is not a directory".into()));
    }

    // 3. Build the target path
    let target = if let Some(ref file_path) = req.file_path {
        // Validate filePath is relative and safe
        if file_path.starts_with('/') || file_path.contains("..") {
            return Err(ApiError::BadRequest(
                "filePath must be relative and cannot contain '..'".into(),
            ));
        }
        let full = canonical_project.join(file_path);
        let canonical_file = std::fs::canonicalize(&full)
            .map_err(|e| ApiError::BadRequest(format!("filePath not found: {e}")))?;
        // Ensure resolved path is within project
        if !canonical_file.starts_with(&canonical_project) {
            return Err(ApiError::BadRequest(
                "filePath resolves outside projectPath".into(),
            ));
        }
        canonical_file
    } else {
        canonical_project
    };

    // 4. Spawn the IDE (fire-and-forget, using tokio for auto-reaping)
    let target_str = target.to_string_lossy().to_string();
    let mut cmd = tokio::process::Command::new(&command);

    // VS Code forks use --goto; Zed and JetBrains take file path as positional arg
    let is_vscode_fork = matches!(req.ide.as_str(), "vscode" | "cursor" | "windsurf");
    if req.file_path.is_some() && is_vscode_fork {
        cmd.arg("--goto").arg(&target_str);
    } else {
        cmd.arg(&target_str);
    }

    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    // tokio::process::Command auto-reaps child via SIGCHLD — no zombies
    match cmd.spawn() {
        Ok(_child) => {
            tracing::info!(ide = %req.ide, target = %target_str, "Launched IDE");
            Ok(Json(serde_json::json!({})))
        }
        // Note: ApiError::Internal logs the detail but returns generic "Internal server error"
        // to the client (by design — see error.rs). The OS error is only visible in server logs.
        Err(e) => Err(ApiError::Internal(format!(
            "Failed to launch {}: {e}",
            req.ide
        ))),
    }
}
```

- [ ] **Step 2: Add the router function**

```rust
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ide/detect", get(get_detect))
        .route("/ide/open", post(post_open))
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p claude-view-server`

---

### Task 7: Register IDE routes

**Files:**
- Modify: `crates/server/src/routes/mod.rs`

- [ ] **Step 1: Read routes/mod.rs to find module declarations and router nesting**

Read `crates/server/src/routes/mod.rs`.

- [ ] **Step 2: Add module declaration and route nesting**

Add `pub mod ide;` with the other module declarations (~line 41).
Add `.nest("/api", ide::router())` in the `api_routes()` function (~line 150).

- [ ] **Step 3: Build and verify server starts**

Run: `cargo build -p claude-view-server`
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add crates/server/src/routes/ide.rs crates/server/src/routes/mod.rs crates/server/src/state.rs crates/server/src/lib.rs
git commit -m "feat(ide): add IDE detection and launch API endpoints

- GET /api/ide/detect — returns installed IDEs (VS Code, Cursor, Windsurf, Zed, WebStorm, IntelliJ)
- POST /api/ide/open — launches IDE with project folder or specific file
- Security: canonicalize paths, validate containment, reject traversal
- Detection cached at startup in AppState"
```

---

### Task 7b: Add endpoint tests for ide.rs

**Files:**
- Modify: `crates/server/src/routes/ide.rs` (add `#[cfg(test)]` module at bottom)

Every route module in this codebase has a `#[cfg(test)]` block. The security validation paths in `post_open` are fully testable without spawning any IDE.

- [ ] **Step 1: Add test module to ide.rs**

Append to `crates/server/src/routes/ide.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// Build a test router with a fake "testvscode" IDE pre-populated.
    /// Uses async `new_in_memory()` (the real Database API) and seeds
    /// `available_ides` so path-validation tests actually exercise
    /// the path checks (not just "IDE not found").
    ///
    /// Pattern: `Arc::get_mut` on a freshly-created Arc (refcount=1)
    /// to mutate fields before sharing — same pattern as coaching.rs tests.
    async fn test_app() -> Router {
        let db = claude_view_db::Database::new_in_memory()
            .await
            .expect("in-memory DB");
        let mut state = crate::state::AppState::new(db);
        // Seed a fake IDE so tests can reach the path validation logic
        Arc::get_mut(&mut state).unwrap().available_ides = vec![(
            IdeInfo { id: "testvscode".into(), name: "Test VS Code".into() },
            "false".into(), // "false" is a valid binary that exits 1 — safe for testing
        )];
        Router::new()
            .route("/api/ide/detect", axum::routing::get(get_detect))
            .route("/api/ide/open", axum::routing::post(post_open))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_ide_detect_returns_seeded_list() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/ide/detect")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["available"].as_array().unwrap().len(), 1);
        assert_eq!(json["available"][0]["id"], "testvscode");
    }

    #[tokio::test]
    async fn test_open_rejects_relative_path() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/ide/open")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"ide":"testvscode","projectPath":"relative/path"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("absolute"), "Expected 'absolute' in error: {text}");
    }

    #[tokio::test]
    async fn test_open_rejects_unknown_ide() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/ide/open")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"ide":"vim","projectPath":"/tmp"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("IDE not found"), "Expected 'IDE not found' in error: {text}");
    }

    #[tokio::test]
    async fn test_open_rejects_path_traversal() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/ide/open")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"ide":"testvscode","projectPath":"/tmp","filePath":"../../etc/passwd"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
```

- [ ] **Step 2: Verify tests pass**

Run: `cargo test -p claude-view-server ide::tests`
Expected: 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/server/src/routes/ide.rs
git commit -m "test(ide): add endpoint tests for detect and open security validation"
```

---

## Chunk 2: Frontend — Hook and Component

### Task 8: Generate TypeScript types

**Files:**
- Generated: `apps/web/src/types/generated/IdeInfo.ts`
- Generated: `apps/web/src/types/generated/IdeDetectResponse.ts`
- Generated: `apps/web/src/types/generated/OpenInIdeRequest.ts`

- [ ] **Step 1: Run the codegen**

Run: `./scripts/generate-types.sh`

This is the canonical codegen entry point. It runs `cargo test --features codegen export_bindings -- --nocapture` for all four crates (core, search, db, server), post-processes imports, and formats with Biome. Do NOT use `--ignored` — the ts-rs `export_bindings` tests are NOT `#[ignore]`.

Verify the new files exist: `ls apps/web/src/types/generated/Ide*`

- [ ] **Step 2: Verify generated files exist and re-export from index**

Check `apps/web/src/types/generated/index.ts` includes the new types. If not, add:
```typescript
export type { IdeInfo } from './IdeInfo'
export type { IdeDetectResponse } from './IdeDetectResponse'
export type { OpenInIdeRequest } from './OpenInIdeRequest'
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/types/generated/
git commit -m "chore: generate TS types for IDE detection API"
```

---

### Task 9: useIdePreference hook

**Files:**
- Create: `apps/web/src/hooks/use-ide-preference.ts`

- [ ] **Step 1: Create the hook**

```typescript
import { useQuery } from '@tanstack/react-query'
import { useCallback, useMemo } from 'react'
import type { IdeDetectResponse } from '../types/generated/IdeDetectResponse'
import type { IdeInfo } from '../types/generated/IdeInfo'
import { useLocalStorage } from './use-local-storage'

const IDE_STORAGE_KEY = 'claude-view-preferred-ide'

async function fetchIdeDetect(): Promise<IdeDetectResponse> {
  const res = await fetch('/api/ide/detect')
  if (!res.ok) throw new Error(`IDE detect failed: ${res.status}`)
  return res.json()
}

async function postOpenInIde(ide: string, projectPath: string, filePath?: string): Promise<void> {
  const res = await fetch('/api/ide/open', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ ide, projectPath, filePath }),
  })
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: `HTTP ${res.status}` }))
    throw new Error(body.error || `Failed to open IDE: ${res.status}`)
  }
}

export function useIdePreference() {
  const { data } = useQuery({
    queryKey: ['ide-detect'],
    queryFn: fetchIdeDetect,
    staleTime: Number.POSITIVE_INFINITY,
    retry: false,
  })

  const availableIdes = data?.available ?? []
  const [storedIdeId, setStoredIdeId] = useLocalStorage<string | null>(IDE_STORAGE_KEY, null)

  const preferredIde = useMemo<IdeInfo | null>(() => {
    if (availableIdes.length === 0) return null
    if (storedIdeId) {
      const found = availableIdes.find((ide) => ide.id === storedIdeId)
      if (found) return found
    }
    return availableIdes[0]
  }, [availableIdes, storedIdeId])

  const setPreferredIde = useCallback(
    (id: string) => {
      setStoredIdeId(id)
    },
    [setStoredIdeId],
  )

  const openProject = useCallback(
    async (projectPath: string) => {
      if (!preferredIde) return
      await postOpenInIde(preferredIde.id, projectPath)
    },
    [preferredIde],
  )

  const openFile = useCallback(
    async (projectPath: string, filePath: string) => {
      if (!preferredIde) return
      await postOpenInIde(preferredIde.id, projectPath, filePath)
    },
    [preferredIde],
  )

  // Escape hatch: open with an explicit IDE id, bypassing the preferredIde closure.
  // Used by OpenInIdeButton dropdown to avoid stale-closure bug when the user
  // picks a different IDE (setPreferredIde triggers re-render AFTER the await).
  const openWithIde = useCallback(
    async (ideId: string, projectPath: string, filePath?: string) => {
      await postOpenInIde(ideId, projectPath, filePath)
    },
    [],
  )

  return { availableIdes, preferredIde, setPreferredIde, openProject, openFile, openWithIde }
}
```

- [ ] **Step 2: Create useLocalStorage hook (MANDATORY — this file does NOT exist)**

Create `apps/web/src/hooks/use-local-storage.ts`. This file does NOT exist in the codebase — you MUST create it before `use-ide-preference.ts` will compile:

```typescript
import { useCallback, useState } from 'react'

export function useLocalStorage<T>(key: string, initialValue: T): [T, (value: T) => void] {
  const [stored, setStored] = useState<T>(() => {
    try {
      const item = localStorage.getItem(key)
      return item ? JSON.parse(item) : initialValue
    } catch {
      return initialValue
    }
  })

  const setValue = useCallback(
    (value: T) => {
      setStored(value)
      try {
        localStorage.setItem(key, JSON.stringify(value))
      } catch {
        // localStorage full or unavailable — ignore
      }
    },
    [key],
  )

  return [stored, setValue]
}
```

- [ ] **Step 3: Verify types resolve**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -20`

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/hooks/use-ide-preference.ts apps/web/src/hooks/use-local-storage.ts
git commit -m "feat(ide): add useIdePreference hook with localStorage persistence"
```

---

### Task 10: OpenInIdeButton component

**Files:**
- Create: `apps/web/src/components/live/OpenInIdeButton.tsx`

- [ ] **Step 1: Create the component**

```typescript
import * as Popover from '@radix-ui/react-popover'
import { ChevronDown, ExternalLink } from 'lucide-react'
import { useState } from 'react'
import { useIdePreference } from '../../hooks/use-ide-preference'
import { cn } from '../../lib/utils'

interface OpenInIdeButtonProps {
  projectPath: string
  filePath?: string
  compact?: boolean
}

export function OpenInIdeButton({ projectPath, filePath, compact }: OpenInIdeButtonProps) {
  const { availableIdes, preferredIde, setPreferredIde, openWithIde } =
    useIdePreference()
  const [open, setOpen] = useState(false)

  if (!preferredIde || availableIdes.length === 0) return null

  const handleOpen = async (ideId?: string) => {
    const targetId = ideId ?? preferredIde.id
    if (ideId) setPreferredIde(ideId)
    try {
      // Use openWithIde(targetId, ...) to avoid stale-closure bug.
      // openProject/openFile closures capture preferredIde from the last render,
      // so clicking a dropdown item would open the PREVIOUS IDE, not the clicked one.
      await openWithIde(targetId, projectPath, filePath)
    } catch (e) {
      console.warn('Failed to open IDE:', e)
    }
    setOpen(false)
  }

  const label = compact
    ? preferredIde.name
    : `Open in ${preferredIde.name}`

  return (
    <div className="inline-flex items-center" onClick={(e) => e.stopPropagation()}>
      {/* Main button */}
      <button
        type="button"
        onClick={() => handleOpen()}
        title={label}
        className={cn(
          'inline-flex items-center gap-1 transition-colors cursor-pointer',
          'text-gray-500 dark:text-gray-400 hover:text-indigo-600 dark:hover:text-indigo-400',
          compact
            ? 'p-0.5 rounded'
            : 'px-1.5 py-0.5 rounded text-[10px] font-medium',
        )}
      >
        <ExternalLink className={compact ? 'w-3 h-3' : 'w-3.5 h-3.5'} />
        {!compact && <span>{preferredIde.name}</span>}
      </button>

      {/* Dropdown chevron — only if multiple IDEs */}
      {availableIdes.length > 1 && (
        <Popover.Root open={open} onOpenChange={setOpen}>
          <Popover.Trigger asChild>
            <button
              type="button"
              className={cn(
                'p-0.5 rounded transition-colors cursor-pointer',
                'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300',
              )}
            >
              <ChevronDown className="w-3 h-3" />
            </button>
          </Popover.Trigger>
          <Popover.Portal>
            <Popover.Content
              align="end"
              sideOffset={4}
              className="z-50 w-40 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg py-1"
            >
              {availableIdes.map((ide) => (
                <button
                  key={ide.id}
                  type="button"
                  onClick={() => handleOpen(ide.id)}
                  className={cn(
                    'w-full flex items-center gap-2 px-3 py-1.5 text-xs transition-colors cursor-pointer',
                    ide.id === preferredIde.id
                      ? 'text-indigo-600 dark:text-indigo-400 bg-indigo-50 dark:bg-indigo-900/20'
                      : 'text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800',
                  )}
                >
                  <ExternalLink className="w-3.5 h-3.5" />
                  {ide.name}
                </button>
              ))}
            </Popover.Content>
          </Popover.Portal>
        </Popover.Root>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Verify types resolve**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -20`

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/live/OpenInIdeButton.tsx
git commit -m "feat(ide): add OpenInIdeButton split button component with IDE dropdown"
```

---

## Chunk 3: Wire Up — All Three Placement Locations

### Task 11: Wire into ChangesTab + FileChangeHeader

**Files:**
- Modify: `apps/web/src/components/live/ChangesTab.tsx`
- Modify: `apps/web/src/components/live/FileChangeHeader.tsx`
- Modify: `apps/web/src/components/live/SessionDetailPanel.tsx` (~line 842)

- [ ] **Step 1: Add projectPath prop to ChangesTab**

In `ChangesTab.tsx`, update the interface and destructure:

```typescript
interface ChangesTabProps {
  fileHistory: FileHistoryResponse
  sessionId: string
  projectPath: string  // NEW
}

export function ChangesTab({ fileHistory, sessionId, projectPath }: ChangesTabProps) {
```

Add `import { OpenInIdeButton } from './OpenInIdeButton'` alongside the existing imports at the top.

Then insert `<OpenInIdeButton>` in the summary header div, after the `−N` stats span and before the closing `</div>` (~line 28). The surrounding code looks like:

```typescript
        {summary.totalRemoved > 0 && (
          <span className="font-mono text-red-500 dark:text-red-400">−{summary.totalRemoved}</span>
        )}
        {/* INSERT HERE — after stats, before </div> */}
        <OpenInIdeButton projectPath={projectPath} />
      </div>
```

- [ ] **Step 2: Add projectPath prop to FileChangeHeader**

In `FileChangeHeader.tsx`, update the interface:

```typescript
interface FileChangeHeaderProps {
  file: FileChange
  sessionId: string
  projectPath: string  // NEW
}
```

Add a compact `OpenInIdeButton` in the header row `<div>` (**NOT** inside the inner expand `<button>`).

The header structure in `FileChangeHeader.tsx` is:
- `<div>` (flex row, ~line 87) — outer container
  - `<button>` (expand/collapse, lines 95–106) — chevron + icon + filename. **Closes at line 106.**
  - Version pills (lines 108–133) — siblings of the button, inside the div
  - NEW badge (lines 135–140)
  - Stats spans (lines 142–152) — `+N` / `−N`
  - **INSERT `<OpenInIdeButton>` HERE** — after stats (~line 152), before `</div>` (~line 153)
- `</div>` (line 153)

```typescript
import { OpenInIdeButton } from './OpenInIdeButton'

// Insert AFTER the −N stats span (~line 152), BEFORE the closing </div> (~line 153).
// This is OUTSIDE the expand <button> (which closes at line 106) — no nested buttons.
<OpenInIdeButton projectPath={projectPath} filePath={file.filePath} compact />
```

- [ ] **Step 3: Thread projectPath through ChangesTab → FileChangeHeader**

In `ChangesTab.tsx`, pass `projectPath` to each `FileChangeHeader`:

```typescript
{files.map((file) => (
  <FileChangeHeader key={file.fileHash} file={file} sessionId={sessionId} projectPath={projectPath} />
))}
```

- [ ] **Step 4: Pass projectPath from SessionDetailPanel**

In `SessionDetailPanel.tsx` (~line 842), add the prop:

```typescript
<ChangesTab fileHistory={fileHistory!} sessionId={data.id} projectPath={data.projectPath} />
```

- [ ] **Step 5: Verify types resolve**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -20`

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/components/live/ChangesTab.tsx apps/web/src/components/live/FileChangeHeader.tsx apps/web/src/components/live/SessionDetailPanel.tsx
git commit -m "feat(ide): wire OpenInIdeButton into Changes tab and file headers"
```

---

### Task 12: Wire into Kanban swimlane ProjectHeader

**Files:**
- Modify: `apps/web/src/components/live/use-kanban-grouping.ts` (add `projectPath` to `ProjectGroup`)
- Modify: `apps/web/src/components/live/KanbanSwimLaneHeader.tsx` (add prop + button)
- Modify: `apps/web/src/components/live/KanbanView.tsx` (pass `projectPath` through)

- [ ] **Step 1: Add projectPath to ProjectGroup interface**

In `use-kanban-grouping.ts`, update the interface:

```typescript
export interface ProjectGroup {
  projectName: string
  projectPath: string  // NEW — filesystem path from first session
  branches: BranchGroup[]
  totalSessionCount: number
  totalCostUsd: number
  maxActivityAt: number
}
```

In the `groupSessionsByProjectBranch` function, at the `groups.push({` call (~line 100), add:

```typescript
projectPath: branches[0]?.sessions[0]?.projectPath ?? '',
```

The full updated `groups.push` block becomes:

```typescript
    groups.push({
      projectName,
      projectPath: branches[0]?.sessions[0]?.projectPath ?? '',  // NEW
      branches,
      totalSessionCount,
      totalCostUsd,
      maxActivityAt: projectMaxActivity,
    })
```

Note: There is no pre-existing `firstSession` variable in scope at this point. `branches[0].sessions[0]` is the first session from the first branch in the group. `LiveSession.projectPath` is a required `string` (never undefined), so the `?? ''` is just defensive.

- [ ] **Step 2: Refactor ProjectHeader and add projectPath prop**

In `KanbanSwimLaneHeader.tsx`:

**IMPORTANT:** The current `ProjectHeader` root element is a `<button>`. Inserting `<OpenInIdeButton>` (which contains `<button>` elements) inside a `<button>` produces **invalid nested interactive HTML**. Browsers will auto-correct by ejecting the nested buttons, breaking layout and accessibility.

**Fix:** Add `import { OpenInIdeButton } from './OpenInIdeButton'` to the existing imports at the top of `KanbanSwimLaneHeader.tsx`. Then replace ONLY the `ProjectHeader` function (lines 13–46). Leave `BranchHeader` (lines 55–83) and all existing imports (`ChevronDown`, `ChevronRight`, `FolderOpen`, `GitBranch`, `formatCostUsd`, `cn`) unchanged.

Replace the `ProjectHeader` function with:

```typescript
// Add this import alongside the existing ones at the top of the file:
// import { OpenInIdeButton } from './OpenInIdeButton'

interface ProjectHeaderProps {
  projectName: string
  projectPath: string  // NEW
  totalCostUsd: number
  sessionCount: number
  isCollapsed: boolean
  onToggle: () => void
}

export function ProjectHeader({
  projectName,
  projectPath,  // NEW
  totalCostUsd,
  sessionCount,
  isCollapsed,
  onToggle,
}: ProjectHeaderProps) {
  const Chevron = isCollapsed ? ChevronRight : ChevronDown

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onToggle}
      onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onToggle() } }}
      className={cn(
        'w-full flex items-center gap-2 py-2 px-3 cursor-pointer',
        'bg-gray-100/60 dark:bg-gray-800/40',
        'hover:bg-gray-100 dark:hover:bg-gray-800/60',
        'transition-colors',
      )}
    >
      <Chevron className="w-4 h-4 text-gray-400 dark:text-gray-500 shrink-0" />
      <FolderOpen className="w-4 h-4 text-amber-500 dark:text-amber-400 shrink-0" />
      <span className="text-sm font-semibold text-gray-700 dark:text-gray-300 truncate">
        {projectName}
      </span>
      <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums">
        ({sessionCount})
      </span>
      <OpenInIdeButton projectPath={projectPath} />
      <span className="ml-auto text-xs font-mono text-gray-500 dark:text-gray-400 tabular-nums shrink-0">
        {formatCostUsd(totalCostUsd)}
      </span>
    </div>
  )
}
```

This replaces ONLY the `ProjectHeader` function (lines 13–46). **Do NOT delete `BranchHeader` (lines 55–83) or any other code in this file.** The `<div role="button">` with `tabIndex={0}` and `onKeyDown` preserves keyboard accessibility. The `OpenInIdeButton`'s `e.stopPropagation()` wrapper prevents IDE clicks from toggling collapse.

- [ ] **Step 3: Pass projectPath in KanbanView (TWO changes)**

In `KanbanView.tsx`, make TWO changes:

**Change A:** Update the `<ProjectHeader>` JSX call (~line 605):

```typescript
<ProjectHeader
  projectName={project.projectName}
  projectPath={project.projectPath}  // NEW
  totalCostUsd={project.totalCostUsd}
  sessionCount={project.totalSessionCount}
  isCollapsed={projCollapsed}
  onToggle={() => toggleCollapse(projKey)}
/>
```

**Change B:** Update the `merged.push()` at ~line 576 (the "closed-only projects" synthesis block) to include the new required field. This is a `ProjectGroup` literal that currently has NO `projectPath` — `tsc` will fail without this:

```typescript
      merged.push({
        projectName: projectKey,
        projectPath: '',  // NEW — no active sessions to derive a path from
        branches,
        totalSessionCount: 0,
        totalCostUsd: 0,
        maxActivityAt: 0,
      })
```

- [ ] **Step 4: Verify types resolve**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -20`

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/components/live/use-kanban-grouping.ts apps/web/src/components/live/KanbanSwimLaneHeader.tsx apps/web/src/components/live/KanbanView.tsx
git commit -m "feat(ide): wire OpenInIdeButton into Kanban swimlane project headers"
```

---

## Chunk 4: Build, Test, Verify

### Task 13: Full build and smoke test

**Files:** None (verification only)

- [ ] **Step 1: Build frontend (Rust was already verified in Task 7)**

Run: `bun run build`
Expected: All Turbo tasks successful, no errors. Note: `bun run build` runs `turbo build` which covers JS/TS apps only. The Rust binary was compiled and verified in Task 7 Step 3.

- [ ] **Step 2: Run Rust tests for server crate (includes new ide::tests)**

Run: `cargo test -p claude-view-server`
Expected: All tests pass, including the 4 new `ide::tests` from Task 7b.

- [ ] **Step 3: Run frontend typecheck**

Run: `cd apps/web && bunx tsc --noEmit`
Expected: No type errors.

- [ ] **Step 4: Run frontend tests**

Run: `cd apps/web && bunx vitest run`
Expected: All existing tests pass.

- [ ] **Step 5: Manual smoke test**

1. Start the server: `bun run dev:server` (in one terminal)
2. Check IDE detection: `curl -s http://localhost:47892/api/ide/detect | python3 -m json.tool`
   - Expected: JSON with your installed IDEs listed
3. Test opening a project: `curl -s -X POST http://localhost:47892/api/ide/open -H 'Content-Type: application/json' -d '{"ide":"vscode","projectPath":"/Users/TBGor/dev/@vicky-ai/claude-view"}'`
   - Expected: VS Code opens the claude-view project
4. Test opening a file: `curl -s -X POST http://localhost:47892/api/ide/open -H 'Content-Type: application/json' -d '{"ide":"vscode","projectPath":"/Users/TBGor/dev/@vicky-ai/claude-view","filePath":"apps/web/src/App.tsx"}'`
   - Expected: VS Code opens App.tsx

- [ ] **Step 6: Test security validation**

```bash
# Should fail — relative projectPath
curl -s -X POST http://localhost:47892/api/ide/open -H 'Content-Type: application/json' -d '{"ide":"vscode","projectPath":"relative/path"}'

# Should fail — path traversal in filePath
curl -s -X POST http://localhost:47892/api/ide/open -H 'Content-Type: application/json' -d '{"ide":"vscode","projectPath":"/Users/TBGor/dev/@vicky-ai/claude-view","filePath":"../../etc/passwd"}'

# Should fail — unknown IDE
curl -s -X POST http://localhost:47892/api/ide/open -H 'Content-Type: application/json' -d '{"ide":"vim","projectPath":"/Users/TBGor/dev/@vicky-ai/claude-view"}'
```

All three should return 400 with error messages.

- [ ] **Step 7: Final commit if any fixes needed**

```bash
git add -A
git commit -m "fix(ide): address issues found during smoke testing"
```

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `--goto` flag sent to ALL IDEs — breaks Zed & JetBrains file-open | Blocker | Task 6: Gate `--goto` on `matches!(req.ide.as_str(), "vscode" \| "cursor" \| "windsurf")`; other IDEs get positional arg only |
| 2 | `create_app_full()` and `create_app_with_git_sync()` in `lib.rs` — plan missed both struct literals | Blocker | Tasks 3 & 4: Explicitly list all 5 construction sites; Task 4 fixes both `lib.rs` sites |
| 3 | Codegen command `--ignored` runs zero tests (ts-rs tests are NOT `#[ignore]`) | Blocker | Task 8: Replaced with `./scripts/generate-types.sh` (canonical entry point, uses `--nocapture`) |
| 4 | `ProjectHeader` root is `<button>` — nesting `OpenInIdeButton` creates invalid HTML | Blocker | Task 12 Step 2: Full rewrite from `<button>` to `<div role="button">` with keyboard accessibility |
| 5 | `std::process::Command` spawn creates zombie processes | Warning | Task 6: Switched to `tokio::process::Command` which auto-reaps via SIGCHLD |
| 6 | Task 4 pointed to `main.rs` but AppState constructed in `lib.rs` | Warning | Task 4: Rewritten to target `lib.rs:create_app_full()` directly |
| 7 | Plan says "no new tests needed" — 6 security paths are testable | Warning | Added Task 7b with 4 endpoint tests (detect, reject relative path, reject unknown IDE, reject traversal) |
| 8 | `projectPath` population says `firstSession.projectPath` but no such variable in scope | Warning | Task 12 Step 1: Replaced with `branches[0]?.sessions[0]?.projectPath ?? ''` with full code block |
| 9 | `ApiError::Internal` hides detail from client — plan implies client sees it | Minor | Task 6: Added comment explaining server-only logging behavior |
| 10 | Architecture header said `POST /api/open-in-ide` but code uses `/api/ide/open` | Minor | Fixed architecture description to show actual endpoint paths |
| 11 | `bun run build` is frontend-only but Task 13 implied "build everything" | Minor | Task 13 Step 1: Added clarification that Rust was verified in Task 7 |
| 12 | Task 7 commit staged `main.rs` but changes are in `lib.rs` | Minor | Fixed `git add` to list `lib.rs` instead of `main.rs` |
| 13 | `Database::in_memory()` doesn't exist — real API is async `new_in_memory()` | Blocker | Task 7b: Fixed to `async fn test_app()` with `Database::new_in_memory().await` |
| 14 | 5th AppState construction site `create_app_with_git_sync()` in `lib.rs:115` missed | Blocker | Tasks 3 & 4: Added as 5th site, Task 4 Step 2 adds `available_ides: Vec::new()` there |
| 15 | `test_open_rejects_relative_path` hits "IDE not found" before path check (empty `available_ides`) | Blocker | Task 7b: Tests now seed a fake `"testvscode"` IDE via `Arc::get_mut` pattern (matches coaching.rs) |
| 16 | Stale closure in `handleOpen` — dropdown opens wrong (prior) IDE | Blocker | Task 9: Added `openWithIde(ideId, ...)` escape hatch; Task 10: `handleOpen` uses it |
| 17 | Unused `use std::path::Path` import | Warning | Task 1: Removed dead import |
| 18 | Ambiguous `FileChangeHeader` insertion point — could nest inside `<button>` | Warning | Task 11 Step 2: Added full structural diagram showing insert between line 152 and `</div>` at 153 |
| 19 | Task 12 Step 2 code block looks like full-file rewrite — would delete `BranchHeader` | Blocker | Added explicit "replace ONLY ProjectHeader (lines 13–46), leave BranchHeader (lines 55–83) unchanged" |
| 20 | Unused `openProject`/`openFile` in destructure — Biome lint rejects unused vars | Blocker | Task 10: Removed from destructure, only `openWithIde` is used |
| 21 | ChangesTab insertion point lacks surrounding-line context — ambiguous placement | Warning | Task 11 Step 1: Added explicit code snippet with `totalRemoved` span → insert → `</div>` |
| 22 | `use-local-storage.ts` doesn't exist but creation was conditional ("if not, create") — import is unconditional | Blocker | Task 9 Step 2: Changed to MANDATORY creation with explicit "this file does NOT exist" warning |
| 23 | `merged.push()` at KanbanView.tsx:576 missing required `projectPath` field — tsc compile error | Blocker | Task 12 Step 3: Added "Change B" updating `merged.push()` with `projectPath: ''` |
| 24 | `AvailableIdesHolder` type alias creates inverted `state → routes::ide` dependency | Warning | Task 3 Step 2: Added note acknowledging the inversion; acceptable for MVP |
