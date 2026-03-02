# Share Viewer Upgrade + Share UX Design

**Date:** 2026-03-02
**Status:** Done (implemented 2026-03-02)
**Scope:** `apps/share/`, `apps/web/`, `packages/shared/`, server-side share blob

## Problem

The share viewer (`apps/share/`) is an ultra-minimal read-only SPA that only shows filtered user + assistant messages. It lacks feature parity with the web app's conversation detail view:

1. **No verbose/debug mode** — hides tool_use, tool_result, system, progress, summary messages
2. **No view mode toggle** — no Chat/Debug or Rich/JSON controls
3. **No metadata panel** — no session info (cost, duration, model, files, tools used)
4. **Generic header** — "Get claude-view" link missing `target="_blank"`, no branding
5. **No share message UX** — web app copies link instantly with no preview or forward message option

## Approach: Shared Rendering Components

Extract/write rendering components in `@claude-view/shared` so both `apps/web/` and `apps/share/` consume identical UI. App-specific code (data fetching, headers) stays in each app.

**Dev workflow unchanged:** `packages/shared/` exports raw TypeScript source (`"main": "./src/index.ts"`). Vite resolves it directly — HMR works across workspace boundaries. Edit in shared, both apps hot-reload.

## 1. Expanded Share Blob Schema

Currently `ParsedSession = { messages, metadata: { totalMessages, toolCallCount } }`. Expand `SessionMetadata` to include rich data the web app already has loaded when sharing:

```typescript
// packages/shared/src/types/message/index.ts
export type SessionMetadata = {
  totalMessages: number
  toolCallCount: number
  // Expanded fields (optional for backward compat)
  sessionId?: string
  projectName?: string
  primaryModel?: string
  durationSeconds?: number
  totalInputTokens?: number
  totalOutputTokens?: number
  totalCostUsd?: number
  userPromptCount?: number
  filesRead?: string[]
  filesEdited?: string[]
  commits?: Array<{ hash: string; message: string }>
  gitBranch?: string
  toolsUsed?: Array<{ name: string; count: number }>
  sessionTitle?: string  // first user message summary (for share message)
}
```

The web app's share hook (`useCreateShare`) already has `sessionDetail` + `richData` loaded. Serialize these fields into the encrypted blob alongside messages.

## 2. New Shared Components

### a) `ViewModeToggle` (~40 lines)

Prop-driven replacement for web app's `ViewModeControls` (which depends on zustand):

```typescript
interface ViewModeToggleProps {
  verboseMode: boolean
  onToggleVerbose: () => void
  richRenderMode?: 'rich' | 'json'
  onSetRichRenderMode?: (mode: 'rich' | 'json') => void
}
```

- Chat/Debug segmented control
- Rich/JSON toggle (visible only in Debug mode)
- Same visual design as current `ViewModeControls`
- Web app: wires to zustand store. Share app: wires to `useState`.

### b) `SessionInfoPanel` (~150 lines)

Collapsible metadata sidebar showing session details:

```typescript
interface SessionInfoPanelProps {
  metadata: SessionMetadata
  className?: string
  defaultOpen?: boolean  // desktop: true, mobile: false
}
```

Sections:
- **Overview:** Duration, model, cost, token count
- **Files:** Read/edited file list (collapsible)
- **Commits:** Git commits during session
- **Tools:** Tool usage breakdown (name + count)

Gracefully hides sections when optional fields are missing (backward compat with old share blobs).

### c) Existing shared components (no changes)

- `MessageTyped` — already renders all 7 roles
- `ExpandProvider` / `ThreadHighlightProvider` — conversation state
- `buildThreadMap` / `getThreadChain` — threading
- `ErrorBoundary` — error handling

## 3. Share Viewer Changes (`apps/share/`)

### Header Redesign

Replace current flat header with a polished, branded navbar:

- **Left:** claude-view wordmark/logo + "Shared conversation" label
- **Center:** `ViewModeToggle` (Chat/Debug + Rich/JSON)
- **Right:** Panel toggle icon + "Get claude-view →" CTA button
- Sticky, backdrop-blur, dark mode via system preference
- "Get claude-view" links to `https://claudeview.ai` with `target="_blank" rel="noopener noreferrer"`

### Layout

Two-column on desktop (conversation + collapsible panel), single-column on mobile:

```
Desktop (≥1024px):
┌─────────────────────────────────────────────────┐
│ Header (sticky, backdrop-blur)                  │
├────────────────────────────┬────────────────────┤
│ Conversation (scrollable)  │ SessionInfoPanel   │
│                            │ (collapsible)      │
├────────────────────────────┴────────────────────┤
│ Footer: message count + hidden count            │
└─────────────────────────────────────────────────┘

Mobile (<1024px):
Panel hidden by default, toggle via header button.
```

### Verbose Mode

- **Chat mode (default):** Filter to user + assistant (current behavior)
- **Debug mode:** Show ALL messages (tool_use, tool_result, system, progress, summary)
- `MessageTyped` already renders all roles — verbose mode just removes the filter
- Footer updates: "142 messages" vs "142 messages · 87 system messages hidden"

### Dependencies Added

- `lucide-react` — already a dependency of `@claude-view/shared` (tree-shaken)
- No new direct dependencies in `apps/share/package.json`

## 4. ChatGPT-Style Share Modal (`apps/web/`)

Replace the current "click → instant clipboard copy" with a modal dialog:

### Trigger
Share button in ConversationView header opens the modal (instead of immediately copying).

### Modal Content
- Share link in a copyable input field with "Copy Link" button
- Divider: "or share with context"
- Pre-formatted message preview:
  ```
  Check out my Claude session about "{sessionTitle}":
  {shareUrl}

  Shared via claude-view
  ```
- "Copy Message" button
- Close button (X) + ESC to dismiss

### Session Title Generation
Auto-generate from the first user message:
- Truncate to ~60 characters at word boundary
- Fallback to project name if first message is very long or unclear

### Implementation
- Use Radix UI `Dialog` (already in web app dependencies) for accessible modal
- Share creation happens on modal open (not on copy click) — show loading state
- Both copy buttons show brief "Copied!" feedback

## 5. Server-Side Changes

### Share Blob Serialization (`apps/web/`)

In the share creation hook, include expanded metadata when building the blob:

```typescript
const parsedSession: ParsedSession = {
  messages: session.messages,
  metadata: {
    totalMessages: session.messages.length,
    toolCallCount: sessionDetail.toolCallCount,
    sessionId,
    projectName,
    primaryModel: sessionDetail.primaryModel,
    durationSeconds: sessionDetail.durationSeconds,
    totalInputTokens: sessionDetail.totalInputTokens,
    totalOutputTokens: sessionDetail.totalOutputTokens,
    totalCostUsd: richData?.costBreakdown?.totalUsd,
    userPromptCount: sessionDetail.userPromptCount,
    filesRead: sessionDetail.filesRead,
    filesEdited: sessionDetail.filesEdited,
    commits: sessionDetail.commits,
    gitBranch: sessionDetail.gitBranch,
    toolsUsed: computeToolsUsed(session.messages),
    sessionTitle: generateSessionTitle(session.messages, projectName),
  },
}
```

No Rust server changes needed — the blob is encrypted client-side before upload.

## 6. What's NOT Included (Intentional)

- **RichPane (1060 lines)** — Too coupled to live session state (hook events, real-time progress, category filtering). Not appropriate for static share viewer.
- **Chat input / Resume** — Share viewer is read-only by design.
- **Export (HTML/PDF/MD)** — Could be a follow-up. Not needed for MVP.
- **Sub-agent timeline** — Requires live data not in the blob.
- **Storybook** — YAGNI at current team size.

## Component Ownership Summary

| Component | Location | Used By |
|-----------|----------|---------|
| `ViewModeToggle` | `packages/shared/` | web + share |
| `SessionInfoPanel` | `packages/shared/` | web + share |
| `MessageTyped` | `packages/shared/` | web + share (already) |
| `ShareModal` | `apps/web/` | web only |
| `ShareHeader` | `apps/share/` | share only |
| `ConversationView` wrapper | `apps/web/` | web only |
| `App` wrapper | `apps/share/` | share only |
