# Live Monitor Rename + Rich History Sessions — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rename "Mission Control" to "Live Monitor" everywhere, then bring rich session data (cost, context gauge, cache, sub-agents) to the History detail page via a unified JSONL accumulator.

**Architecture:** Extract `SessionAccumulator` from `crates/server/src/live/manager.rs` into `crates/core/src/accumulator.rs`. Both live (streaming) and history (batch) use the same accumulator. New REST endpoint `GET /api/sessions/:id/rich` parses JSONL on demand. Frontend shares components between Live Monitor and History.

**Tech Stack:** Rust (Axum), React, TypeScript, TanStack Query, Tailwind CSS, Radix UI tabs

---

## Task 1: Rename "Mission Control" → "Live Monitor" (Frontend)

**Files:**
- Rename: `src/pages/MissionControlPage.tsx` → `src/pages/LiveMonitorPage.tsx`
- Modify: `src/router.tsx`
- Modify: `src/components/Sidebar.tsx`

**Step 1: Rename the page component file**

```bash
mv src/pages/MissionControlPage.tsx src/pages/LiveMonitorPage.tsx
```

**Step 2: Update the component name inside the file**

In `src/pages/LiveMonitorPage.tsx`:
- Replace `export function MissionControlPage()` → `export function LiveMonitorPage()`
- Replace `Mission Control` h1 heading text → `Live Monitor`

**Step 3: Update router imports and route**

In `src/router.tsx`:
- Line 7: Change import from `'./pages/MissionControlPage'` → `'./pages/LiveMonitorPage'`
- Line 7: Change `{ MissionControlPage }` → `{ LiveMonitorPage }`
- Line 49: Change `<MissionControlPage />` → `<LiveMonitorPage />`
- Line 67: Keep legacy redirect `mission-control` → `/` (still useful)

**Step 4: Update sidebar navigation label**

In `src/components/Sidebar.tsx`:
- Line 367: Change `title="Mission Control"` → `title="Live Monitor"`
- Line 439: Change `Mission Control` span text → `Live Monitor`

**Step 5: Run frontend build to verify**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/mission-control-cde && npx tsc --noEmit`
Expected: No errors

**Step 6: Commit**

```bash
git add -A && git commit -m "refactor: rename Mission Control to Live Monitor (frontend)"
```

---

## Task 2: Rename "Mission Control" → "Live Monitor" (Backend + comments)

**Files:**
- Modify: `crates/server/src/live/hook_registrar.rs` (critical: JSON status message + logs + tests)
- Modify: `crates/server/src/live/state.rs` (doc comments)
- Modify: `crates/server/src/terminal_state.rs` (doc comment)
- Modify: `crates/server/src/lib.rs` (doc comment)
- Modify: `crates/server/src/state.rs` (doc comment)
- Modify: `crates/server/src/file_tracker.rs` (doc comment)
- Modify: `crates/server/src/routes/terminal.rs` (doc comment)
- Modify: `src/store/app-store.ts` (comment)
- Modify: `src/types/generated/index.ts` (comments)
- Modify: `src/lib/format-utils.ts` (comment)

**Step 1: Update hook_registrar.rs (functional change)**

In `crates/server/src/live/hook_registrar.rs`:
- Line 54: Change `"statusMessage": "Mission Control"` → `"statusMessage": "Live Monitor"`
- Line 163: Change log `"Registered {} Mission Control hooks"` → `"Registered {} Live Monitor hooks"`
- Line 195: Change log `"Cleaned up Mission Control hooks"` → `"Cleaned up Live Monitor hooks"`
- All doc comments: Replace `Mission Control` → `Live Monitor`
- Line 219 (test): Change assertion `"Mission Control"` → `"Live Monitor"`

**Step 2: Update all Rust doc comments**

In each of these files, replace `Mission Control` → `Live Monitor` in doc comments only:
- `crates/server/src/live/state.rs` (lines 1, 113)
- `crates/server/src/terminal_state.rs` (line 1)
- `crates/server/src/lib.rs` (line 134)
- `crates/server/src/state.rs` (line 51)
- `crates/server/src/file_tracker.rs` (line 12)
- `crates/server/src/routes/terminal.rs` (line 1)

**Step 3: Update frontend comments**

- `src/store/app-store.ts` line 18: `// Live Monitor`
- `src/types/generated/index.ts` lines 164, 168: Replace `Mission Control` → `Live Monitor`
- `src/lib/format-utils.ts` line 19: `live-monitor style suffixes`

**Step 4: Run Rust tests**

Run: `cargo test -p vibe-recall-server`
Expected: All tests pass (hook_registrar test updated in step 1)

**Step 5: Commit**

```bash
git add -A && git commit -m "refactor: rename Mission Control to Live Monitor (backend + comments)"
```

---

## Task 3: Rename Smart/Full → Compact/Verbose

**Files:**
- Modify: `src/components/ConversationView.tsx`

**Step 1: Update button labels**

In `src/components/ConversationView.tsx`:
- Line ~400: Change `Smart` label → `Compact`
- Line ~413: Change `Full` label → `Verbose`

Internal state already uses `'compact' | 'full'` — no state rename needed since the internal values don't leak to UI. The label change is sufficient.

**Step 2: Visual verify in browser**

Open `/sessions/:any-session-id`, confirm buttons now read "Compact" and "Verbose".

**Step 3: Commit**

```bash
git add src/components/ConversationView.tsx && git commit -m "refactor: rename Smart/Full to Compact/Verbose for consistency"
```

---

## Task 4: Extract SessionAccumulator into crates/core

**Files:**
- Create: `crates/core/src/accumulator.rs`
- Modify: `crates/core/src/lib.rs`
- Modify: `crates/server/src/live/manager.rs`

**Step 1: Write the failing test**

Create `crates/core/src/accumulator.rs` with a test that exercises batch mode:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accumulate_empty() {
        let acc = SessionAccumulator::new();
        let data = acc.finish();
        assert_eq!(data.tokens.total_tokens, 0);
        assert_eq!(data.turn_count, 0);
        assert!(data.cost.total_usd == 0.0);
    }
}
```

**Step 2: Move struct + accumulation logic from manager.rs**

Extract from `crates/server/src/live/manager.rs` (lines 30-86 struct, token accumulation from `process_jsonl_update`):

Create `crates/core/src/accumulator.rs`:
```rust
//! Unified session accumulator for both live (streaming) and history (batch) JSONL parsing.

use crate::live_parser::{LiveLine, LineType};
use crate::pricing::{TokenUsage, CostBreakdown, CacheStatus, ModelPricing, calculate_cost, default_pricing};
use crate::subagent::{SubAgentInfo, SubAgentStatus};
use crate::progress::ProgressItem;

pub struct SessionAccumulator {
    pub tokens: TokenUsage,
    pub context_window_tokens: u64,
    pub model: Option<String>,
    pub user_turn_count: u32,
    pub first_user_message: Option<String>,
    pub last_user_message: Option<String>,
    pub git_branch: Option<String>,
    pub started_at: Option<i64>,
    pub sub_agents: Vec<SubAgentInfo>,
    pub todo_items: Vec<ProgressItem>,
    pub task_items: Vec<ProgressItem>,
    pub last_cache_hit_at: Option<i64>,
}

/// Rich session data — output of accumulation. Same shape for live and history.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RichSessionData {
    pub tokens: TokenUsage,
    pub cost: CostBreakdown,
    pub cache_status: CacheStatus,
    pub sub_agents: Vec<SubAgentInfo>,
    pub progress_items: Vec<ProgressItem>,
    pub context_window_tokens: u64,
    pub model: Option<String>,
    pub git_branch: Option<String>,
    pub turn_count: u32,
    pub first_user_message: Option<String>,
    pub last_user_message: Option<String>,
    pub last_cache_hit_at: Option<i64>,
}

impl SessionAccumulator {
    pub fn new() -> Self { /* ... zero-init all fields ... */ }

    /// Process a single parsed JSONL line (streaming or batch).
    pub fn process_line(&mut self, line: &LiveLine) {
        // Token accumulation (from manager.rs lines 578-601)
        // Context window tracking (lines 613-622)
        // Sub-agent spawn/completion/progress (lines 666-758)
        // Todo/task items (lines 760-844)
        // Cache hit tracking
        // Model/branch/user message tracking
    }

    /// Finalize and produce RichSessionData with cost calculation.
    pub fn finish(&self) -> RichSessionData {
        let pricing_table = default_pricing();
        let pricing = self.model.as_deref()
            .and_then(|m| crate::pricing::lookup_pricing(m, &pricing_table));
        let cost = calculate_cost(&self.tokens, pricing);
        let cache_status = derive_cache_status(self.last_cache_hit_at);
        // Build and return RichSessionData
    }

    /// Convenience: parse an entire JSONL file and return RichSessionData.
    pub fn from_file(path: &std::path::Path) -> std::io::Result<RichSessionData> {
        let data = std::fs::read(path)?;
        let (lines, _) = crate::live_parser::parse_tail(&data, 0);
        let mut acc = Self::new();
        for line in &lines {
            acc.process_line(line);
        }
        Ok(acc.finish())
    }
}
```

The exact token accumulation, sub-agent tracking, and progress handling logic is moved verbatim from `manager.rs` `process_jsonl_update()` (lines 578-844).

**Step 3: Register module in lib.rs**

In `crates/core/src/lib.rs`, add after line 1:
```rust
pub mod accumulator;
```

**Step 4: Update manager.rs to delegate to shared accumulator**

In `crates/server/src/live/manager.rs`:
- Replace the `SessionAccumulator` struct with `use vibe_recall_core::accumulator::SessionAccumulator`
- Remove duplicated accumulation logic from `process_jsonl_update()`
- Call `self.accumulator.process_line(&line)` for each parsed line
- Keep orchestration logic (file watching, SSE broadcast, process detection) in manager

**Step 5: Run tests**

Run: `cargo test -p vibe-recall-core && cargo test -p vibe-recall-server`
Expected: All pass

**Step 6: Commit**

```bash
git add -A && git commit -m "refactor: extract SessionAccumulator to crates/core for unified live+history parsing"
```

---

## Task 5: Add GET /api/sessions/:id/rich endpoint

**Files:**
- Modify: `crates/server/src/routes/sessions.rs`

**Step 1: Write the handler**

Add to `crates/server/src/routes/sessions.rs`:

```rust
use vibe_recall_core::accumulator::{SessionAccumulator, RichSessionData};

/// GET /api/sessions/:id/rich — Parse JSONL on demand and return rich session data.
pub async fn get_session_rich(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<RichSessionData>> {
    // 1. Look up session in DB to get file_path
    let pool = state.db_pool();
    let row = sqlx::query_scalar::<_, String>(
        "SELECT file_path FROM sessions WHERE id = ?1"
    )
    .bind(&session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(format!("DB error: {e}")))?
    .ok_or_else(|| ApiError::not_found("Session not found"))?;

    let path = std::path::PathBuf::from(&row);
    if !path.exists() {
        return Err(ApiError::not_found("JSONL file not found on disk"));
    }

    // 2. Parse JSONL through shared accumulator
    let rich_data = tokio::task::spawn_blocking(move || {
        SessionAccumulator::from_file(&path)
    })
    .await
    .map_err(|e| ApiError::internal(format!("Parse error: {e}")))?
    .map_err(|e| ApiError::internal(format!("IO error: {e}")))?;

    Ok(Json(rich_data))
}
```

**Step 2: Register the route**

In the `router()` function (line 442), add after line 448:
```rust
.route("/sessions/{id}/rich", get(get_session_rich))
```

**Step 3: Run tests**

Run: `cargo test -p vibe-recall-server`
Expected: Pass. Also manually test: `curl http://localhost:47892/api/sessions/<any-id>/rich | jq .`

**Step 4: Commit**

```bash
git add crates/server/src/routes/sessions.rs && git commit -m "feat: add GET /api/sessions/:id/rich endpoint for JSONL-derived rich data"
```

---

## Task 6: Add TypeScript types + React hook for rich session data

**Files:**
- Create: `src/types/generated/RichSessionData.ts`
- Create: `src/hooks/use-rich-session-data.ts`
- Modify: `src/types/generated/index.ts`

**Step 1: Create TypeScript type**

Create `src/types/generated/RichSessionData.ts`:
```typescript
import type { SubAgentInfo } from './SubAgentInfo'
import type { ProgressItem } from './ProgressItem'

export interface RichSessionData {
  tokens: {
    inputTokens: number
    outputTokens: number
    cacheReadTokens: number
    cacheCreationTokens: number
    cacheCreation5mTokens: number
    cacheCreation1hrTokens: number
    totalTokens: number
  }
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
  subAgents: SubAgentInfo[]
  progressItems: ProgressItem[]
  contextWindowTokens: number
  model: string | null
  gitBranch: string | null
  turnCount: number
  firstUserMessage: string | null
  lastUserMessage: string | null
  lastCacheHitAt: number | null
}
```

**Step 2: Create React hook**

Create `src/hooks/use-rich-session-data.ts`:
```typescript
import { useQuery } from '@tanstack/react-query'
import type { RichSessionData } from '../types/generated/RichSessionData'

async function fetchRichSessionData(sessionId: string): Promise<RichSessionData> {
  const response = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/rich`)
  if (!response.ok) {
    throw new Error(`Failed to fetch rich session data: ${response.status}`)
  }
  return response.json()
}

export function useRichSessionData(sessionId: string | null) {
  return useQuery({
    queryKey: ['session-rich', sessionId],
    queryFn: () => {
      if (!sessionId) throw new Error('sessionId is required')
      return fetchRichSessionData(sessionId)
    },
    enabled: !!sessionId,
    staleTime: 60_000, // JSONL doesn't change for completed sessions
  })
}
```

**Step 3: Export from index**

In `src/types/generated/index.ts`, add:
```typescript
// Live Monitor: Rich Session Data (unified accumulator output)
export type { RichSessionData } from './RichSessionData'
```

**Step 4: TypeScript check**

Run: `npx tsc --noEmit`
Expected: No errors

**Step 5: Commit**

```bash
git add -A && git commit -m "feat: add RichSessionData type and useRichSessionData hook"
```

---

## Task 7: Redesign History detail sidebar with tabs (Overview, Sub-Agents, Cost)

**Files:**
- Modify: `src/components/ConversationView.tsx`
- Create: `src/components/HistoryOverviewTab.tsx`
- Create: `src/components/HistoryCostTab.tsx`

**Step 1: Create HistoryOverviewTab component**

Create `src/components/HistoryOverviewTab.tsx`:
```typescript
import type { SessionDetail } from '../types/generated'
import type { RichSessionData } from '../types/generated/RichSessionData'
import { ContextGauge } from './live/ContextGauge'
import { SessionMetricsBar } from './SessionMetricsBar'
import { FilesTouchedPanel } from './FilesTouchedPanel'
import { CommitsPanel } from './CommitsPanel'

interface HistoryOverviewTabProps {
  sessionDetail: SessionDetail
  richData: RichSessionData | undefined
  isLoadingRich: boolean
}

export function HistoryOverviewTab({ sessionDetail, richData, isLoadingRich }: HistoryOverviewTabProps) {
  // Render:
  // 1. Rich data section (context gauge, cost summary, model, cache info) — if richData loaded
  // 2. Existing SessionMetricsBar (vertical variant)
  // 3. Existing FilesTouchedPanel
  // 4. Existing CommitsPanel
}
```

This component composes the existing sidebar sections (SessionMetricsBar, FilesTouchedPanel, CommitsPanel) plus new rich data (ContextGauge, cost summary pill) into a single scrollable tab.

**Step 2: Create HistoryCostTab component**

Create `src/components/HistoryCostTab.tsx`:
```typescript
import type { RichSessionData } from '../types/generated/RichSessionData'
import { CostBreakdown } from './live/CostBreakdown'

interface HistoryCostTabProps {
  richData: RichSessionData
}

export function HistoryCostTab({ richData }: HistoryCostTabProps) {
  // Reuse CostBreakdown from Live Monitor with richData.cost and richData.tokens
}
```

**Step 3: Modify ConversationView sidebar to use Radix Tabs**

In `src/components/ConversationView.tsx`, replace the right sidebar section (line ~586) with a Radix `Tabs.Root`:

```tsx
import * as Tabs from '@radix-ui/react-tabs'
import { useRichSessionData } from '../hooks/use-rich-session-data'
import { HistoryOverviewTab } from './HistoryOverviewTab'
import { HistoryCostTab } from './HistoryCostTab'
import { SwimLanes } from './live/SwimLanes'

// Inside component:
const { data: richData, isLoading: isLoadingRich } = useRichSessionData(sessionId)
const hasSubAgents = (richData?.subAgents?.length ?? 0) > 0

// Replace sidebar JSX:
<div className="w-[300px] flex-shrink-0 hidden lg:flex flex-col border-l border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
  <Tabs.Root defaultValue="overview" className="flex flex-col h-full">
    <Tabs.List className="flex border-b border-gray-200 dark:border-gray-700 px-2 pt-2 gap-1 flex-shrink-0">
      <Tabs.Trigger value="overview" className="px-2.5 py-1.5 text-xs ...">Overview</Tabs.Trigger>
      {hasSubAgents && (
        <Tabs.Trigger value="sub-agents" className="px-2.5 py-1.5 text-xs ...">Sub-Agents</Tabs.Trigger>
      )}
      <Tabs.Trigger value="cost" className="px-2.5 py-1.5 text-xs ...">Cost</Tabs.Trigger>
    </Tabs.List>
    <Tabs.Content value="overview" className="flex-1 overflow-y-auto p-4 space-y-4">
      <HistoryOverviewTab sessionDetail={sessionDetail} richData={richData} isLoadingRich={isLoadingRich} />
    </Tabs.Content>
    {hasSubAgents && (
      <Tabs.Content value="sub-agents" className="flex-1 overflow-y-auto p-4">
        <SwimLanes subAgents={richData!.subAgents} />
      </Tabs.Content>
    )}
    <Tabs.Content value="cost" className="flex-1 overflow-y-auto p-4">
      {richData && <HistoryCostTab richData={richData} />}
    </Tabs.Content>
  </Tabs.Root>
</div>
```

**Step 4: Verify shared components accept history data**

Check that `ContextGauge`, `CostBreakdown`, and `SwimLanes` props are compatible with `RichSessionData` fields. They should be — they take the same token/cost/subagent shapes. If `ContextGauge` requires `AgentStateGroup`, pass `'Delivered'` for completed sessions.

**Step 5: TypeScript check + visual verify**

Run: `npx tsc --noEmit`
Then open `/sessions/:any-id` in browser and verify:
- Overview tab shows enriched metrics + existing stats/files/commits
- Cost tab shows detailed breakdown
- Sub-Agents tab appears only if session had sub-agents
- Continue + Export buttons untouched

**Step 6: Commit**

```bash
git add -A && git commit -m "feat: redesign history sidebar with tabbed Overview/Sub-Agents/Cost panels"
```

---

## Task 8: End-to-end verification

**Step 1: Run full Rust test suite**

Run: `cargo test -p vibe-recall-core -p vibe-recall-server`
Expected: All pass

**Step 2: Run frontend type check**

Run: `npx tsc --noEmit`
Expected: No errors

**Step 3: Manual browser verification**

1. Open `/` — page title reads "Live Monitor", nav reads "Live Monitor"
2. Open a live session — all Mission Control features work as before
3. Open `/sessions` — history list loads
4. Open `/sessions/:id` — sidebar has Overview/Cost tabs (+ Sub-Agents if applicable)
5. Overview tab: context gauge, cost pill, cache info at top, then metrics, files, commits
6. Cost tab: detailed breakdown with savings
7. Compact/Verbose toggle works on conversation
8. Continue + Export buttons work

**Step 4: Commit any fixes, then final commit if needed**

---

## Task Summary

| # | Task | Type | Estimated effort |
|---|------|------|-----------------|
| 1 | Rename Mission Control → Live Monitor (frontend) | Rename | Small |
| 2 | Rename Mission Control → Live Monitor (backend + comments) | Rename | Small |
| 3 | Rename Smart/Full → Compact/Verbose | Rename | Trivial |
| 4 | Extract SessionAccumulator into crates/core | Refactor | Medium |
| 5 | Add GET /api/sessions/:id/rich endpoint | Backend | Small |
| 6 | Add TypeScript types + React hook | Frontend | Small |
| 7 | Redesign History sidebar with tabs | Frontend | Medium |
| 8 | End-to-end verification | Testing | Small |

**Dependencies:** Tasks 1-3 are independent (can parallelize). Task 5 depends on Task 4. Tasks 6-7 depend on Task 5. Task 8 depends on all.
