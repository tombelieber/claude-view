# Open in IDE — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add "Open in IDE" buttons that let users open projects and files in VS Code, Cursor, Windsurf, Zed, WebStorm, or IntelliJ directly from claude-view.

**Architecture:** Backend detects installed IDEs at startup, caches the list in AppState, and exposes two endpoints: detection and launch. Frontend stores the user's IDE preference in localStorage and renders a split button (main click = open, dropdown = pick IDE) in three locations: Changes tab header, FileChangeHeader rows, and Kanban swimlane ProjectHeader.

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

use std::path::Path;
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
- Modify: `crates/server/src/state.rs` (add field after `teams` ~line 133)
- Modify: `crates/server/src/state.rs` (update all constructors)

- [ ] **Step 1: Read state.rs to find exact insertion points**

Read `crates/server/src/state.rs` to confirm the struct definition and constructor signatures.

- [ ] **Step 2: Add the field and type alias**

In `state.rs`, add:
- Type alias near the top (after other type aliases): `pub type AvailableIdesHolder = Vec<(crate::routes::ide::IdeInfo, String)>;`
- New field in `AppState` struct: `pub available_ides: AvailableIdesHolder,`
- Initialize as `available_ides: Vec::new()` in all constructors

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p claude-view-server`

---

### Task 4: Run IDE detection at startup

**Files:**
- Modify: `crates/server/src/main.rs` — add detection call during startup

- [ ] **Step 1: Read main.rs to find the startup sequence**

Read `crates/server/src/main.rs` around where `AppState` is constructed (lines 310–340).

- [ ] **Step 2: Add IDE detection before AppState construction**

After the existing state construction, run detection and store results:

```rust
// Detect installed IDEs (fast — just runs `which` for each)
let available_ides = crate::routes::ide::detect_installed_ides();
tracing::info!(count = available_ides.len(), "Detected installed IDEs");
for (ide, cmd) in &available_ides {
    tracing::info!(id = %ide.id, name = %ide.name, command = %cmd, "Found IDE");
}
```

Set `available_ides` on the AppState instance.

- [ ] **Step 3: Verify it compiles**

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

    // 4. Spawn the IDE (fire-and-forget)
    let target_str = target.to_string_lossy().to_string();
    let mut cmd = std::process::Command::new(&command);

    if req.file_path.is_some() {
        // For file opening, use --goto for VS Code forks
        cmd.arg("--goto").arg(&target_str);
    } else {
        cmd.arg(&target_str);
    }

    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    match cmd.spawn() {
        Ok(_child) => {
            tracing::info!(ide = %req.ide, target = %target_str, "Launched IDE");
            Ok(Json(serde_json::json!({})))
        }
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
git add crates/server/src/routes/ide.rs crates/server/src/routes/mod.rs crates/server/src/state.rs crates/server/src/main.rs
git commit -m "feat(ide): add IDE detection and launch API endpoints

- GET /api/ide/detect — returns installed IDEs (VS Code, Cursor, Windsurf, Zed, WebStorm, IntelliJ)
- POST /api/ide/open — launches IDE with project folder or specific file
- Security: canonicalize paths, validate containment, reject traversal
- Detection cached at startup in AppState"
```

---

## Chunk 2: Frontend — Hook and Component

### Task 8: Generate TypeScript types

**Files:**
- Generated: `apps/web/src/types/generated/IdeInfo.ts`
- Generated: `apps/web/src/types/generated/IdeDetectResponse.ts`
- Generated: `apps/web/src/types/generated/OpenInIdeRequest.ts`

- [ ] **Step 1: Run the codegen**

Run: `cargo test -p claude-view-server --features codegen export_bindings -- --ignored 2>/dev/null; ls apps/web/src/types/generated/Ide*`

If the project uses a different codegen command, check `package.json` for a `generate:types` script.

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

  return { availableIdes, preferredIde, setPreferredIde, openProject, openFile }
}
```

- [ ] **Step 2: Verify useLocalStorage hook exists**

Check if `apps/web/src/hooks/use-local-storage.ts` exists. If not, create a minimal one:

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
  const { availableIdes, preferredIde, setPreferredIde, openProject, openFile } =
    useIdePreference()
  const [open, setOpen] = useState(false)

  if (!preferredIde || availableIdes.length === 0) return null

  const handleOpen = async (ideId?: string) => {
    const id = ideId ?? preferredIde.id
    if (ideId) setPreferredIde(id)
    try {
      if (filePath) {
        await openFile(projectPath, filePath)
      } else {
        await openProject(projectPath)
      }
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

Add `OpenInIdeButton` import and place it in the header after the stats:

```typescript
import { OpenInIdeButton } from './OpenInIdeButton'

// In the header div, after the +/- stats spans:
<OpenInIdeButton projectPath={projectPath} />
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

Add a compact `OpenInIdeButton` in the header row, after the stats/NEW badge:

```typescript
import { OpenInIdeButton } from './OpenInIdeButton'

// After the stats spans, before the closing of the header button area:
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

In the `groupSessionsByProjectBranch` function, where `ProjectGroup` objects are constructed, add `projectPath` from the first session in each group. Find where the project groups are built and add:

```typescript
projectPath: firstSession.projectPath,
```

The first session in each project group is available from `branchMap` values — use the first session from the first branch.

- [ ] **Step 2: Add projectPath prop to ProjectHeader**

In `KanbanSwimLaneHeader.tsx`, update `ProjectHeaderProps`:

```typescript
interface ProjectHeaderProps {
  projectName: string
  projectPath: string  // NEW
  totalCostUsd: number
  sessionCount: number
  isCollapsed: boolean
  onToggle: () => void
}
```

Add the `OpenInIdeButton` inside the header, after the session count badge and before cost:

```typescript
import { OpenInIdeButton } from './OpenInIdeButton'

// Inside ProjectHeader render, after session count, before cost:
<OpenInIdeButton projectPath={projectPath} />
```

- [ ] **Step 3: Pass projectPath in KanbanView**

In `KanbanView.tsx`, update the `<ProjectHeader>` call (~line 605):

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

- [ ] **Step 1: Build everything**

Run: `bun run build`
Expected: All tasks successful, no errors.

- [ ] **Step 2: Run Rust tests for server crate**

Run: `cargo test -p claude-view-server`
Expected: All existing tests pass (no new tests needed for fire-and-forget spawn logic).

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
