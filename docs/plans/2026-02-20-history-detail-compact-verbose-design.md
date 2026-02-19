# History Detail Compact/Verbose Mode — Design

**Date:** 2026-02-20
**Status:** Approved

## Goal

Unify the history detail page's compact/verbose mode with the live monitor's terminal rendering, and reuse the live monitor's `SessionDetailPanel` (portal overlay, resizable, 5-tab interface) as the sidebar for history detail.

## Current State

### History Detail (ConversationView)
- Compact/verbose toggle filters messages (hides tool_use, tool_result, system, progress, summary in compact)
- Uses `MessageTyped` component for rendering (7-type editorial styling)
- Inline 300px sidebar with 3 Radix tabs: Overview, Sub-Agents, Cost
- Separate components: `HistoryOverviewTab`, `HistoryCostTab`

### Live Monitor (SessionDetailPanel)
- Compact/verbose toggle filters `RichPane` messages
- Uses `RichPane` for terminal rendering (color-coded borders, collapsible tools, Shiki highlighting)
- Resizable portal overlay with 5 tabs: Overview, Terminal, Log, Sub-Agents, Cost
- Richer overview: cost card, session info card, cache countdown, context gauge, sub-agent pills, timeline mini, last prompt

## Design

### 1. Data Adapter — `SessionPanelData`

Create a unified interface that both `LiveSession` and historical data can satisfy:

```ts
// src/components/live/session-panel-data.ts

interface SessionPanelData {
  // Identity
  id: string
  project: string
  projectDisplayName: string
  projectPath: string
  gitBranch: string | null

  // Status (history = always 'done')
  status: 'working' | 'paused' | 'done'

  // Metrics
  model: string | null
  turnCount: number
  tokens: {
    inputTokens: number
    outputTokens: number
    cacheReadTokens: number
    cacheCreationTokens: number
    totalTokens: number
  }
  contextWindowTokens: number
  cost: {
    totalUsd: number
    inputCostUsd: number
    outputCostUsd: number
    cacheReadCostUsd: number
    cacheCreationCostUsd: number
    cacheSavingsUsd: number
    isEstimated: boolean
  }
  cacheStatus: 'warm' | 'cold' | 'unknown'

  // Sub-agents
  subAgents?: SubAgentInfo[]

  // Live-only (optional)
  startedAt?: number | null
  lastActivityAt?: number
  lastUserMessage?: string
  lastCacheHitAt?: number | null
  agentState?: AgentState
  pid?: number | null
  currentActivity?: string

  // History-only extensions (optional)
  historyExtras?: {
    sessionDetail: SessionDetail
    sessionInfo?: SessionInfo
  }

  // Terminal messages (pre-converted for history, WebSocket for live)
  terminalMessages?: RichMessage[]
}
```

A `toSessionPanelData()` function maps `SessionDetail + RichSessionData + SessionInfo` into this shape. `LiveSession` already satisfies most fields natively.

### 2. Message Conversion — `messagesToRichMessages`

```ts
// src/lib/message-to-rich.ts

function messagesToRichMessages(messages: Message[]): RichMessage[]
```

Maps the 7-type `Message[]` from paginated JSONL parsing to `RichMessage[]` for RichPane:

| Message.role | RichMessage.type | Notes |
|---|---|---|
| `user` | `user` | Direct map |
| `assistant` | `assistant` | Skip if empty content (tool-only turns) |
| `assistant` (with thinking) | `thinking` + `assistant` | Emit thinking block first |
| `tool_use` | `tool_use` | Extract tool name from `tool_calls[0]`, parse input as JSON |
| `tool_result` | `tool_result` | Direct content map |
| `system` | `assistant` | Render as info |
| `progress` | Skip | Not useful in replay |
| `summary` | Skip | Not useful in replay |

### 3. SessionDetailPanel Modifications

**Props become polymorphic:**
```ts
interface SessionDetailPanelProps {
  // Option A: Live session (existing)
  session?: LiveSession
  // Option B: Unified data (new)
  panelData?: SessionPanelData
  onClose: () => void
}
```

Internally resolves: `const data = panelData ?? liveSessionToData(session!)`

**Tab changes:**
- All 5 tabs render regardless of mode (Overview, Terminal, Log, Sub-Agents, Cost)
- Terminal tab: uses WebSocket messages for live, `panelData.terminalMessages` for history
- Log tab: same ActionLogTab with appropriate message source

**Overview tab — merged (all sections):**

From live monitor:
1. Cost card (2-col grid, clickable → Cost tab)
2. Session info card (status, model, turns, tokens)
3. Cache countdown bar (if cache data available)
4. Context gauge
5. Sub-agent pills (if sub-agents exist)
6. Mini timeline (if sub-agents exist)
7. Last prompt

From history (appended when `historyExtras` present):
8. Session Metrics Bar (prompts, tokens, files read/edited, re-edit %, commits)
9. Files Touched Panel (read vs edited file lists)
10. Commits Panel (linked commits with tier)

**Status handling for history:**
- Status always 'Done' (gray)
- Cache countdown hidden (no live timer)
- Last prompt sourced from `richData.lastUserMessage`

### 4. ConversationView Changes

**Layout transformation:**
```
BEFORE:
┌─────────────────────────┬─────────────┐
│  Messages (Virtuoso)    │ Sidebar     │  300px inline
│  using MessageTyped     │ (3 tabs)    │
└─────────────────────────┴─────────────┘

AFTER:
┌────────────────────────────────────┐  ┌──────────────────┐
│  Messages (full width)             │  │ SessionDetailPanel│
│  Compact: MessageTyped             │  │ (portal overlay)  │
│  Verbose: RichPane                 │  │ (resizable)       │
│                                    │  │ (5 tabs)          │
└────────────────────────────────────┘  └──────────────────┘
```

**Header changes:**
- Compact/verbose toggle stays in header (controls main content area)
- Add panel toggle button (sidebar icon) to open/close the detail panel
- Continue + Export buttons remain

**Main content area:**
- Compact mode: existing `MessageTyped` + Virtuoso (filtered, chat-only)
- Verbose mode: `RichPane` (full-width, all messages rendered like terminal)

**Removed:**
- Inline 300px sidebar
- `HistoryOverviewTab` component (merged into SessionDetailPanel overview)
- `HistoryCostTab` component (replaced by `CostBreakdown`)

### 5. Files to Create/Modify/Delete

| File | Action | Purpose |
|------|--------|---------|
| `src/lib/message-to-rich.ts` | Create | `Message[] → RichMessage[]` converter |
| `src/components/live/session-panel-data.ts` | Create | `SessionPanelData` type + adapters |
| `src/components/live/SessionDetailPanel.tsx` | Modify | Accept `SessionPanelData`, add history overview extras |
| `src/components/ConversationView.tsx` | Modify | Remove inline sidebar, add panel toggle, RichPane for verbose |
| `src/components/HistoryOverviewTab.tsx` | Delete | Merged into panel |
| `src/components/HistoryCostTab.tsx` | Delete | Replaced by CostBreakdown |

### 6. Graceful Degradation

| Feature | Live | History |
|---------|------|---------|
| Status indicator | Running/Paused/Done | Always "Done" |
| Cache countdown | Live timer | Hidden (no timer) |
| Terminal messages | WebSocket real-time | Pre-loaded from JSONL |
| Files Touched | Not available | Shown from sessionDetail |
| Commits | Not available | Shown from sessionDetail |
| Session Metrics Bar | Not shown | Shown from sessionInfo |
| Sub-agents | Live activity | Static from richData |
