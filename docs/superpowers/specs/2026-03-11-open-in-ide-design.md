# Open in IDE — Design Spec

## Problem

Users viewing session file changes or monitoring live sessions want to jump directly into their IDE at the relevant project or file. Currently there's no way to do this from claude-view.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Mechanism | Backend API (Rust spawns CLI) | No browser permission dialogs; can detect installed IDEs |
| IDE preference | `localStorage` | Simple, frontend-only, survives page reloads |
| IDE detection | `which <command>` at server startup, cached in `AppState` | IDEs don't change mid-session |
| Worktree behavior | Use `projectPath` as-is | Session already has the correct CWD |
| Platform | macOS only (MVP) | Per project roadmap |

## Supported IDEs

Static list. Each entry has an id, display name, CLI command, and file-open syntax.

| ID | Name | Command | File open syntax |
|----|------|---------|-----------------|
| `vscode` | VS Code | `code` | `--goto file:line` |
| `cursor` | Cursor | `cursor` | `--goto file:line` |
| `windsurf` | Windsurf | `windsurf` | `--goto file:line` |
| `zed` | Zed | `zed` | `file:line` (positional) |
| `webstorm` | WebStorm | `webstorm` | `--line N file` |
| `intellij` | IntelliJ IDEA | `idea` | `--line N file` |

VS Code forks (Cursor, Windsurf) share the same `--goto` syntax. Adding a new IDE = one struct entry.

**MVP note:** Line number targeting is out of scope. The "File open syntax" column documents the CLI format for future use, but the MVP opens files without line numbers — just `code --goto /path/to/file` (no `:line` suffix).

## Rust Types (source of truth)

All API types live in `crates/server/src/routes/ide.rs` with `#[derive(Serialize, Deserialize, TS)]` and generate into `apps/web/src/types/generated/`.

```rust
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
```

Frontend imports these from `types/generated/IdeInfo`, `types/generated/IdeDetectResponse`, `types/generated/OpenInIdeRequest`.

## Backend

### New file: `crates/server/src/routes/ide.rs`

**`GET /api/ide/detect`**

Returns which IDEs are installed on the user's machine.

```json
{
  "available": [
    { "id": "vscode", "name": "VS Code" },
    { "id": "cursor", "name": "Cursor" }
  ]
}
```

Detection runs once at startup: iterate the static IDE list, run `which <command>` for each, cache results in `AppState.available_ides: Vec<IdeInfo>`. The endpoint just returns the cached list.

**`POST /api/open-in-ide`**

Opens a project or file in the specified IDE.

Request body:
```json
{
  "ide": "cursor",
  "projectPath": "/Users/TBGor/dev/claude-view",
  "filePath": "src/components/StatusDot.tsx"
}
```

- `ide` (required) — IDE id from the detect response
- `projectPath` (required) — Absolute path to project directory
- `filePath` (optional) — Relative path within project to open a specific file

Behavior:
- Project only: `Command::new("cursor").arg(projectPath).spawn()`
- With file: `Command::new("cursor").arg("--goto").arg(format!("{projectPath}/{filePath}")).spawn()`
- Fire-and-forget (detached process). Returns `200 {}` immediately.
- JetBrains syntax differs: `Command::new("idea").arg(format!("{projectPath}/{filePath}")).spawn()`
- Zed syntax: `Command::new("zed").arg(format!("{projectPath}/{filePath}")).spawn()`

**Security validation:**
1. `projectPath` must be an absolute path, must exist on disk, must be a directory
2. `filePath` must not contain `..` or start with `/` (relative only)
3. Both paths are canonicalized via `std::fs::canonicalize()` — the resolved file path must be a child of the resolved project path (prevents symlink traversal)
4. `ide` must match a detected (installed) IDE id

Returns `400 Bad Request` with `{ "error": "<message>" }` for validation failures. Returns `404` with `{ "error": "IDE not found: <id>" }` if IDE id not in available list.

**Spawn semantics:** Fire-and-forget. `Command::new(cmd).arg(path).spawn()` returns `Ok(Child)` if the OS accepted the spawn — the 200 confirms "spawn succeeded", not "IDE window opened". If `spawn()` itself fails (e.g. command not found at runtime despite detection at startup), return `500 { "error": "Failed to launch <ide>: <msg>" }`.

### AppState addition

```rust
pub struct AppState {
    // ... existing fields
    pub available_ides: Vec<IdeInfo>,
}
```

`IdeInfo` is a simple struct: `{ id, name, command, file_syntax }`. Populated during server startup before binding the port.

## Frontend

### `useIdePreference()` hook

New file: `apps/web/src/hooks/use-ide-preference.ts`

- Calls `GET /api/ide/detect` once (react-query, `staleTime: Infinity`)
- Reads `localStorage.getItem('claude-view-preferred-ide')` for saved preference
- If no preference saved, defaults to first available IDE
- Exposes:
  - `availableIdes: IdeInfo[]` (from generated type)
  - `preferredIde: IdeInfo | null` (null if no IDEs detected)
  - `setPreferredIde(id: string): void` (saves to localStorage)
  - `openProject(projectPath: string): Promise<void>` (throws on API error)
  - `openFile(projectPath: string, filePath: string): Promise<void>` (throws on API error)

Both `openProject` and `openFile` call `POST /api/open-in-ide` with the current `preferredIde.id`. On non-2xx response, they throw with the server error message. The `OpenInIdeButton` component catches errors and shows a brief toast/console warning — no complex error UI needed since failures are rare (IDE was detected at startup).

### `OpenInIdeButton` component

New file: `apps/web/src/components/live/OpenInIdeButton.tsx`

Split button with dropdown:
- **Main area**: Click opens with preferred IDE. Shows IDE icon + name (or just icon when compact).
- **Chevron**: Opens Radix Popover dropdown listing all available IDEs. Clicking one changes the preference and triggers the open action.
- Uses existing Radix Popover pattern from `NotificationSoundPopover.tsx`.
- Existing `MenuItem` pattern from `PluginActionMenu.tsx` for dropdown items.

Props:
```typescript
interface OpenInIdeButtonProps {
  projectPath: string
  filePath?: string        // If set, opens specific file
  compact?: boolean        // Icon-only mode (for FileChangeHeader rows)
}
```

When `availableIdes` is empty (no IDEs detected), the button is hidden entirely — no disabled state, no "install an IDE" message. Just absent.

### Placement

**1. Changes tab header** (`ChangesTab.tsx`)

After the `+N / -N` stats, right-aligned. `mode="project"`. Needs `projectPath` passed down — the `ChangesTab` currently receives `fileHistory` and `sessionId`. Will also need `projectPath` from the parent `SessionDetailPanel`.

**2. FileChangeHeader rows** (`FileChangeHeader.tsx`)

Small icon button after the stats/NEW badge, before the expand area. `compact={true}`. Opens the specific file: `filePath={file.filePath}`. Needs `projectPath` passed from parent.

**3. Swimlane ProjectHeader** (`KanbanSwimLaneHeader.tsx`)

After the session count badge, before the cost display. `mode="project"`. The `ProjectHeader` currently receives `projectName` (display name). Will need to also receive `projectPath` — the actual filesystem path from the first session in the group.

## Data Flow

```
User clicks "Open in Cursor" on FileChangeHeader
  → OpenInIdeButton.onClick()
    → useIdePreference().openFile(projectPath, "src/components/StatusDot.tsx")
      → POST /api/open-in-ide { ide: "cursor", projectPath: "...", filePath: "..." }
        → Rust: Command::new("cursor").arg("--goto").arg(full_path).spawn()
          → Cursor opens with file focused
```

```
User clicks "Open in VS Code" on ProjectHeader swimlane
  → OpenInIdeButton.onClick()
    → useIdePreference().openProject("/Users/TBGor/dev/claude-view")
      → POST /api/open-in-ide { ide: "vscode", projectPath: "..." }
        → Rust: Command::new("code").arg(projectPath).spawn()
          → VS Code opens project folder
```

## Prop threading

### ChangesTab + FileChangeHeader

`SessionDetailPanel` already has `data.projectPath`. Thread it through:
- `SessionDetailPanel` → `ChangesTab` (new prop: `projectPath`)
- `ChangesTab` → `FileChangeHeader` (new prop: `projectPath`)

### KanbanSwimLaneHeader ProjectHeader

`KanbanView` groups sessions by project. The grouping key is `projectDisplayName`. Need to also carry `projectPath` from the first session in each group into the `ProjectHeader` props. Use the first session's `projectPath` (the actual CWD). This is the filesystem path the session is working in — for worktrees it's the worktree path, for main it's the repo root. This matches decision C (use projectPath as-is).

## Out of Scope

- Line number targeting from diff hunks (future enhancement)
- Windows/Linux IDE detection
- Server-side config persistence
- Custom IDE path configuration
- Terminal opening (e.g. iTerm, Warp)
