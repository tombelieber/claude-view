# History Detail Compact/Verbose Mode — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Unify history detail's compact/verbose mode with live monitor's RichPane rendering, and replace the inline sidebar with the live monitor's SessionDetailPanel.

**Architecture:** Create a `SessionPanelData` abstraction that both `LiveSession` and `SessionDetail+RichSessionData` can satisfy, then make `SessionDetailPanel` render from it. Add a `messagesToRichMessages()` converter so history messages feed into `RichPane`. ConversationView drops its inline sidebar and gains a toggleable `SessionDetailPanel` overlay.

**Tech Stack:** React, TypeScript, Zustand (monitor-store), react-virtuoso, RichPane

**Design doc:** `docs/plans/2026-02-20-history-detail-compact-verbose-design.md`

---

### Task 1: Create `messagesToRichMessages` converter

**Files:**
- Create: `src/lib/message-to-rich.ts`

**Step 1: Create the converter module**

```ts
// src/lib/message-to-rich.ts
import type { Message } from '../types/generated'
import type { RichMessage } from '../components/live/RichPane'

/** Strip Claude Code internal command tags from content (same logic as RichPane). */
function stripCommandTags(content: string): string {
  return content
    .replace(/<command-name>[\s\S]*?<\/command-name>/g, '')
    .replace(/<command-message>[\s\S]*?<\/command-message>/g, '')
    .replace(/<command-args>[\s\S]*?<\/command-args>/g, '')
    .replace(/<local-command-stdout>[\s\S]*?<\/local-command-stdout>/g, '')
    .replace(/<system-reminder>[\s\S]*?<\/system-reminder>/g, '')
    .trim()
}

/** Convert a timestamp string to Unix seconds, or undefined. */
function parseTimestamp(ts: string | null | undefined): number | undefined {
  if (!ts) return undefined
  const ms = Date.parse(ts)
  if (!isNaN(ms) && ms > 0) return ms / 1000
  return undefined
}

/** Try to parse a string as JSON. Returns parsed value or undefined. */
function tryParseJson(str: string): unknown | undefined {
  try {
    return JSON.parse(str)
  } catch {
    return undefined
  }
}

/**
 * Convert paginated Message[] (from JSONL parser) to RichMessage[] (for RichPane).
 *
 * Mapping:
 * - user → user
 * - assistant → thinking (if has thinking) + assistant (if has content)
 * - tool_use → tool_use (extract tool name from tool_calls[0])
 * - tool_result → tool_result
 * - system → assistant (rendered as info)
 * - progress → skipped
 * - summary → skipped
 */
export function messagesToRichMessages(messages: Message[]): RichMessage[] {
  const result: RichMessage[] = []

  for (const msg of messages) {
    const ts = parseTimestamp(msg.timestamp)

    // Emit thinking block first (if present on assistant messages)
    if (msg.thinking) {
      const thinkingContent = stripCommandTags(msg.thinking)
      if (thinkingContent) {
        result.push({ type: 'thinking', content: thinkingContent, ts })
      }
    }

    switch (msg.role) {
      case 'user': {
        const content = stripCommandTags(msg.content)
        if (content) {
          result.push({ type: 'user', content, ts })
        }
        break
      }

      case 'assistant': {
        const content = stripCommandTags(msg.content)
        if (content) {
          result.push({ type: 'assistant', content, ts })
        }
        break
      }

      case 'tool_use': {
        const toolName = msg.tool_calls?.[0]?.name ?? 'tool'
        const inputStr = msg.content || ''
        result.push({
          type: 'tool_use',
          content: '',
          name: toolName,
          input: inputStr || undefined,
          inputData: inputStr ? tryParseJson(inputStr) : undefined,
          ts,
        })
        break
      }

      case 'tool_result': {
        const content = stripCommandTags(msg.content)
        if (content) {
          result.push({ type: 'tool_result', content, ts })
        }
        break
      }

      case 'system': {
        // Render system messages as assistant info
        const content = stripCommandTags(msg.content)
        if (content) {
          result.push({ type: 'assistant', content, ts })
        }
        break
      }

      // progress, summary → skip (not useful in replay)
      default:
        break
    }
  }

  return result
}
```

**Step 2: Verify TypeScript compiles**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/mission-control-cde && npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: No errors related to `message-to-rich.ts`

**Step 3: Commit**

```bash
git add src/lib/message-to-rich.ts
git commit -m "feat: add messagesToRichMessages converter for history→RichPane"
```

---

### Task 2: Create `SessionPanelData` type and adapters

**Files:**
- Create: `src/components/live/session-panel-data.ts`

**Step 1: Create the type + adapter module**

```ts
// src/components/live/session-panel-data.ts
import type { LiveSession } from './use-live-sessions'
import type { AgentState } from './types'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'
import type { SessionDetail } from '../../types/generated/SessionDetail'
import type { SessionInfo } from '../../types/generated'
import type { RichSessionData } from '../../types/generated/RichSessionData'
import type { RichMessage } from './RichPane'

/**
 * Unified data shape that SessionDetailPanel can render from.
 * Both LiveSession and historical SessionDetail+RichSessionData map to this.
 */
export interface SessionPanelData {
  // Identity
  id: string
  project: string
  projectDisplayName: string
  projectPath: string
  gitBranch: string | null

  // Status
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

  // Live-only fields (optional)
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

  // Terminal messages source
  // - For live: undefined (uses WebSocket via useLiveSessionMessages)
  // - For history: pre-converted RichMessage[] from messagesToRichMessages
  terminalMessages?: RichMessage[]
}

/** Adapt a LiveSession into SessionPanelData (thin wrapper, mostly passthrough). */
export function liveSessionToPanelData(session: LiveSession): SessionPanelData {
  return {
    id: session.id,
    project: session.project,
    projectDisplayName: session.projectDisplayName,
    projectPath: session.projectPath,
    gitBranch: session.gitBranch,
    status: session.status,
    model: session.model,
    turnCount: session.turnCount,
    tokens: session.tokens,
    contextWindowTokens: session.contextWindowTokens,
    cost: session.cost,
    cacheStatus: session.cacheStatus,
    subAgents: session.subAgents,
    startedAt: session.startedAt,
    lastActivityAt: session.lastActivityAt,
    lastUserMessage: session.lastUserMessage,
    lastCacheHitAt: session.lastCacheHitAt,
    agentState: session.agentState,
    pid: session.pid,
    currentActivity: session.currentActivity,
  }
}

/** Adapt history data (SessionDetail + RichSessionData) into SessionPanelData. */
export function historyToPanelData(
  sessionDetail: SessionDetail,
  richData: RichSessionData | undefined,
  sessionInfo: SessionInfo | undefined,
  terminalMessages: RichMessage[],
): SessionPanelData {
  const tokens = richData?.tokens ?? {
    inputTokens: sessionDetail.totalInputTokens ?? 0,
    outputTokens: sessionDetail.totalOutputTokens ?? 0,
    cacheReadTokens: sessionDetail.totalCacheReadTokens ?? 0,
    cacheCreationTokens: sessionDetail.totalCacheCreationTokens ?? 0,
    totalTokens: (sessionDetail.totalInputTokens ?? 0) + (sessionDetail.totalOutputTokens ?? 0),
  }

  const cost = richData?.cost ?? {
    totalUsd: 0,
    inputCostUsd: 0,
    outputCostUsd: 0,
    cacheReadCostUsd: 0,
    cacheCreationCostUsd: 0,
    cacheSavingsUsd: 0,
    isEstimated: true,
  }

  return {
    id: sessionDetail.id,
    project: sessionDetail.project,
    projectDisplayName: sessionDetail.project, // history doesn't have displayName
    projectPath: sessionDetail.projectPath,
    gitBranch: richData?.gitBranch ?? sessionDetail.gitBranch ?? null,
    status: 'done',
    model: richData?.model ?? sessionDetail.primaryModel ?? null,
    turnCount: richData?.turnCount ?? sessionDetail.turnCount,
    tokens,
    contextWindowTokens: richData?.contextWindowTokens ?? 0,
    cost,
    cacheStatus: richData?.cacheStatus ?? 'unknown',
    subAgents: richData?.subAgents,
    lastUserMessage: richData?.lastUserMessage ?? undefined,
    historyExtras: {
      sessionDetail,
      sessionInfo,
    },
    terminalMessages,
  }
}
```

**Step 2: Verify TypeScript compiles**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/mission-control-cde && npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: No errors related to `session-panel-data.ts`

**Step 3: Commit**

```bash
git add src/components/live/session-panel-data.ts
git commit -m "feat: add SessionPanelData type and live/history adapters"
```

---

### Task 3: Modify SessionDetailPanel to accept SessionPanelData

**Files:**
- Modify: `src/components/live/SessionDetailPanel.tsx`

This is the largest task. The panel needs to:
1. Accept either `session: LiveSession` or `panelData: SessionPanelData`
2. Internally resolve to a unified `data: SessionPanelData`
3. Use WebSocket messages for live, `data.terminalMessages` for history
4. Append history-only sections to the overview tab

**Step 1: Update imports and props**

Add these imports at the top of `SessionDetailPanel.tsx`:

```ts
import type { SessionPanelData } from './session-panel-data'
import { liveSessionToPanelData } from './session-panel-data'
import { SessionMetricsBar } from '../SessionMetricsBar'
import { FilesTouchedPanel, buildFilesTouched } from '../FilesTouchedPanel'
import { CommitsPanel } from '../CommitsPanel'
```

Change props interface:

```ts
interface SessionDetailPanelProps {
  /** Live session (existing callers) */
  session?: LiveSession
  /** Unified panel data (new — for history detail) */
  panelData?: SessionPanelData
  onClose: () => void
}
```

**Step 2: Resolve data source at top of component**

At the start of the component function, resolve to unified data:

```ts
export function SessionDetailPanel({ session, panelData: panelDataProp, onClose }: SessionDetailPanelProps) {
  // Resolve to unified data shape
  const data: SessionPanelData = panelDataProp ?? liveSessionToPanelData(session!)
  const isLive = !panelDataProp
  const hasSubAgents = data.subAgents && data.subAgents.length > 0
```

**Step 3: Update WebSocket / messages source**

Change the messages hook to be conditional:

```ts
  // Live mode: WebSocket messages; History mode: pre-loaded messages
  const { messages: liveMessages, bufferDone: liveBufferDone } = useLiveSessionMessages(
    data.id,
    isLive, // only connect WebSocket for live sessions
  )
  const richMessages = isLive ? liveMessages : (data.terminalMessages ?? [])
  const bufferDone = isLive ? liveBufferDone : true // history messages are always fully loaded
```

**Step 4: Replace all `session.xxx` references with `data.xxx`**

Throughout the component, replace:
- `session.id` → `data.id`
- `session.project` → `data.project`
- `session.projectDisplayName` → `data.projectDisplayName`
- `session.projectPath` → `data.projectPath`
- `session.gitBranch` → `data.gitBranch`
- `session.status` → `data.status`
- `session.model` → `data.model`
- `session.turnCount` → `data.turnCount`
- `session.tokens` → `data.tokens`
- `session.cost` → `data.cost`
- `session.subAgents` → `data.subAgents`
- `session.startedAt` → `data.startedAt`
- `session.lastActivityAt` → `data.lastActivityAt ?? 0`
- `session.lastUserMessage` → `data.lastUserMessage`
- `session.lastCacheHitAt` → `data.lastCacheHitAt`
- `session.cacheStatus` → `data.cacheStatus`
- `session.agentState` → `data.agentState`
- `session.contextWindowTokens` → `data.contextWindowTokens`

For `sessionTotalCost(session)` → compute inline: `data.cost.totalUsd + (data.subAgents?.reduce((s, a) => s + (a.costUsd ?? 0), 0) ?? 0)`

For `ContextGauge` that uses `session.agentState.group` → `data.agentState?.group ?? 'needs_you'`

**Step 5: Update terminal tab**

Replace the terminal tab's content from:
```tsx
<RichPane messages={richMessages} isVisible={true} verboseMode={verboseMode} bufferDone={bufferDone} />
```
(Already uses `richMessages` and `bufferDone` from Step 3 — no change needed here.)

**Step 6: Update log tab**

Same — already uses `richMessages` and `bufferDone`.

**Step 7: Update sub-agents tab**

For the `SubAgentDrillDown` component which takes `sessionId`, pass `data.id`. For live-only features like `session.status === 'working'` in SwimLanes `sessionActive` prop, use `data.status === 'working'`.

**Step 8: Add history-only overview sections**

At the bottom of the overview tab (`activeTab === 'overview'`), after the "Last Prompt" section, add:

```tsx
            {/* ---- History-only: Session Metrics ---- */}
            {data.historyExtras?.sessionInfo && data.historyExtras.sessionInfo.userPromptCount > 0 && (
              <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
                <SessionMetricsBar
                  prompts={data.historyExtras.sessionInfo.userPromptCount}
                  tokens={
                    data.historyExtras.sessionInfo.totalInputTokens != null && data.historyExtras.sessionInfo.totalOutputTokens != null
                      ? BigInt(data.historyExtras.sessionInfo.totalInputTokens) + BigInt(data.historyExtras.sessionInfo.totalOutputTokens)
                      : null
                  }
                  filesRead={data.historyExtras.sessionInfo.filesReadCount}
                  filesEdited={data.historyExtras.sessionInfo.filesEditedCount}
                  reeditRate={
                    data.historyExtras.sessionInfo.filesEditedCount > 0
                      ? data.historyExtras.sessionInfo.reeditedFilesCount / data.historyExtras.sessionInfo.filesEditedCount
                      : null
                  }
                  commits={data.historyExtras.sessionInfo.commitCount}
                  variant="vertical"
                />
              </div>
            )}

            {/* ---- History-only: Files Touched ---- */}
            {data.historyExtras?.sessionDetail && (
              <FilesTouchedPanel
                files={buildFilesTouched(
                  data.historyExtras.sessionDetail.filesRead ?? [],
                  data.historyExtras.sessionDetail.filesEdited ?? []
                )}
              />
            )}

            {/* ---- History-only: Linked Commits ---- */}
            {data.historyExtras?.sessionDetail && (
              <CommitsPanel commits={data.historyExtras.sessionDetail.commits ?? []} />
            )}
```

**Step 9: Cache countdown — graceful degradation**

Wrap the cache countdown section with a check for live-only data:

```tsx
{isLive && (data.lastCacheHitAt || data.cacheStatus !== 'unknown') && (
  // existing cache countdown card
)}
```

**Step 10: Verify TypeScript compiles**

Run: `npx tsc --noEmit --pretty 2>&1 | head -30`
Expected: No errors

**Step 11: Commit**

```bash
git add src/components/live/SessionDetailPanel.tsx
git commit -m "feat: make SessionDetailPanel accept SessionPanelData for history mode"
```

---

### Task 4: Modify ConversationView — remove inline sidebar, add panel toggle and RichPane

**Files:**
- Modify: `src/components/ConversationView.tsx`

**Step 1: Add new imports**

```ts
import { SessionDetailPanel } from './live/SessionDetailPanel'
import { RichPane } from './live/RichPane'
import { messagesToRichMessages } from '../lib/message-to-rich'
import { historyToPanelData } from './live/session-panel-data'
import { PanelRight } from 'lucide-react' // sidebar toggle icon
```

Remove old imports that are no longer needed:
```ts
// REMOVE these:
import * as Tabs from '@radix-ui/react-tabs'
import { HistoryOverviewTab } from './HistoryOverviewTab'
import { HistoryCostTab } from './HistoryCostTab'
import { SwimLanes } from './live/SwimLanes'
import { SessionMetricsBar } from './SessionMetricsBar'
import { FilesTouchedPanel, buildFilesTouched } from './FilesTouchedPanel'
import { CommitsPanel } from './CommitsPanel'
```

Also remove the `TAB_TRIGGER_CLASS` constant.

**Step 2: Add panel state and RichMessage conversion**

Inside the `ConversationView` component, add:

```ts
const [panelOpen, setPanelOpen] = useState(false)

// Convert messages to RichMessage[] for verbose mode + terminal tab
const richMessages = useMemo(
  () => allMessages.length > 0 ? messagesToRichMessages(allMessages) : [],
  [allMessages]
)

// Build panel data for SessionDetailPanel
const panelData = useMemo(() => {
  if (!sessionDetail) return undefined
  return historyToPanelData(sessionDetail, richData ?? undefined, sessionInfo, richMessages)
}, [sessionDetail, richData, sessionInfo, richMessages])
```

**Step 3: Add panel toggle button in header**

In the header bar, add a panel toggle button alongside the existing buttons:

```tsx
{/* Panel toggle */}
<button
  onClick={() => setPanelOpen(!panelOpen)}
  aria-pressed={panelOpen}
  className={cn(
    'p-1.5 rounded-md transition-colors',
    panelOpen
      ? 'bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400'
      : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'
  )}
  title="Toggle detail panel"
>
  <PanelRight className="w-4 h-4" />
</button>
```

Place this at the right end of the header, before Continue/Export buttons.

**Step 4: Replace main content area — compact uses MessageTyped, verbose uses RichPane**

Replace the message list section. The Virtuoso-based MessageTyped rendering stays for compact mode. For verbose mode, use RichPane:

```tsx
{/* Left: Conversation messages */}
<div className="flex-1 min-w-0">
  {viewMode === 'compact' ? (
    <ThreadHighlightProvider>
    <ExpandProvider>
      <Virtuoso
        data={filteredMessages}
        startReached={handleStartReached}
        initialTopMostItemIndex={Math.max(0, filteredMessages.length - 1)}
        followOutput="smooth"
        itemContent={(index, message) => {
          const thread = message.uuid ? threadMap.get(message.uuid) : undefined
          return (
            <div className="max-w-4xl mx-auto px-6 pb-4">
              <ErrorBoundary key={message.uuid || index}>
                <MessageTyped
                  message={message}
                  messageIndex={index}
                  messageType={message.role}
                  metadata={message.metadata}
                  parentUuid={thread?.parentUuid}
                  indent={thread?.indent ?? 0}
                  isChildMessage={thread?.isChild ?? false}
                  onGetThreadChain={getThreadChainForUuid}
                />
              </ErrorBoundary>
            </div>
          )
        }}
        components={{
          Header: () => (
            isFetchingPreviousPage ? (
              <div className="max-w-4xl mx-auto px-6 py-4 text-center text-sm text-gray-400 dark:text-gray-500">
                Loading older messages...
              </div>
            ) : hasPreviousPage ? (
              <div className="h-6" />
            ) : filteredMessages.length > 0 ? (
              <div className="max-w-4xl mx-auto px-6 py-4 text-center text-sm text-gray-400 dark:text-gray-500">
                Beginning of conversation
              </div>
            ) : (
              <div className="h-6" />
            )
          ),
          Footer: () => (
            filteredMessages.length > 0 ? (
              <div className="max-w-4xl mx-auto px-6 py-6 text-center text-sm text-gray-400 dark:text-gray-500">
                {totalMessages} messages
                {hiddenCount > 0 && (
                  <> &bull; {hiddenCount} hidden in compact view</>
                )}
                {sessionInfo && sessionInfo.toolCallCount > 0 && (
                  <> &bull; {sessionInfo.toolCallCount} tool calls</>
                )}
              </div>
            ) : null
          )
        }}
        increaseViewportBy={{ top: 400, bottom: 400 }}
        className="h-full overflow-auto"
      />
    </ExpandProvider>
    </ThreadHighlightProvider>
  ) : (
    <RichPane
      messages={richMessages}
      isVisible={true}
      verboseMode={true}
      bufferDone={true}
    />
  )}
</div>
```

**Step 5: Remove inline sidebar, add SessionDetailPanel**

Remove the entire `{/* Right: Tabbed Sidebar */}` section (the 300px div with Radix Tabs).

After the closing `</div>` of the main layout, add the panel:

```tsx
{/* Detail panel overlay */}
{panelOpen && panelData && (
  <SessionDetailPanel
    panelData={panelData}
    onClose={() => setPanelOpen(false)}
  />
)}
```

**Step 6: Update file-gone state sidebar**

The file-gone state also has an inline sidebar with SessionMetricsBar, FilesTouched, and Commits. Replace that sidebar with a panel toggle button + SessionDetailPanel too (or keep the simple inline sidebar for the degraded state — simpler to keep it since there's no rich data).

Decision: Keep the file-gone sidebar as-is — it only needs basic metrics and doesn't warrant the full panel.

But we still need to import `SessionMetricsBar`, `FilesTouchedPanel`, `buildFilesTouched`, and `CommitsPanel` for the file-gone case. So only remove the main-view sidebar, keep these imports.

Actually — cleaner to just keep imports for the file-gone case. Update the "remove imports" list:
- Remove: `@radix-ui/react-tabs`, `HistoryOverviewTab`, `HistoryCostTab`, `SwimLanes`
- Keep: `SessionMetricsBar`, `FilesTouchedPanel`, `buildFilesTouched`, `CommitsPanel` (used in file-gone view)

Also remove the `TAB_TRIGGER_CLASS` constant.

**Step 7: Verify TypeScript compiles**

Run: `npx tsc --noEmit --pretty 2>&1 | head -30`
Expected: No errors

**Step 8: Manual verification**

Run: `cd /Users/TBGor/dev/@vicky-ai/claude-view/.worktrees/mission-control-cde && bun run dev`

1. Navigate to a session detail page
2. Verify compact mode shows chat-only (MessageTyped rendering)
3. Toggle to verbose mode — verify RichPane renders with color-coded borders, collapsible tools
4. Click panel toggle button — verify SessionDetailPanel slides in from right
5. Check Overview tab has all sections (cost card, session info, context gauge, sub-agents, files touched, commits)
6. Check Terminal tab shows same RichPane content
7. Check Cost tab shows CostBreakdown

**Step 9: Commit**

```bash
git add src/components/ConversationView.tsx
git commit -m "feat: replace inline sidebar with SessionDetailPanel, add RichPane for verbose mode"
```

---

### Task 5: Delete obsolete components

**Files:**
- Delete: `src/components/HistoryOverviewTab.tsx`
- Delete: `src/components/HistoryCostTab.tsx`

**Step 1: Verify no remaining imports**

Run: `rg "HistoryOverviewTab|HistoryCostTab" src/` — should return zero results after Task 4.

**Step 2: Delete files**

```bash
rm src/components/HistoryOverviewTab.tsx src/components/HistoryCostTab.tsx
```

**Step 3: Verify TypeScript compiles**

Run: `npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: No errors

**Step 4: Commit**

```bash
git add -u src/components/HistoryOverviewTab.tsx src/components/HistoryCostTab.tsx
git commit -m "chore: remove HistoryOverviewTab and HistoryCostTab (merged into SessionDetailPanel)"
```

---

### Task 6: End-to-end verification

**Step 1: Run build**

Run: `bun run build`
Expected: Clean build, no errors

**Step 2: Run dev server and manually verify**

Run: `bun run dev`

Checklist:
- [ ] History list → click session → detail page loads
- [ ] Compact mode: MessageTyped rendering, chat-only, hidden count shown
- [ ] Verbose mode: RichPane rendering, all messages with tool calls, thinking blocks
- [ ] Panel toggle: SessionDetailPanel slides in from right
- [ ] Panel Overview tab: cost card, session info, context gauge, sub-agents (if any), session metrics bar, files touched, commits
- [ ] Panel Terminal tab: RichPane with verbose toggle
- [ ] Panel Log tab: ActionLogTab works
- [ ] Panel Sub-Agents tab: SwimLanes + Timeline (if sub-agents exist)
- [ ] Panel Cost tab: CostBreakdown with full detail
- [ ] Panel resize: drag left edge to resize
- [ ] Panel close: X button or ESC
- [ ] Live Monitor: SessionDetailPanel still works for live sessions (regression check)
- [ ] File-gone state: still shows inline sidebar with basic metrics

**Step 3: Final commit (if any fixes needed)**

```bash
git add -A
git commit -m "fix: address e2e verification issues"
```
