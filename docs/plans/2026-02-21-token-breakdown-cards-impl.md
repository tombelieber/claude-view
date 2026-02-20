# Token Breakdown Cards Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the misleading "Tokens Used" card on the analytics page with an honest hero card + stacked bar + 4 detail cards showing all token categories (output, cache read, cache write, fresh input).

**Architecture:** Add `cache_read_tokens` and `cache_creation_tokens` to the `AIGenerationStats` Rust struct and SQL query. Create a new `TokenBreakdown` React component with a `StackedBar` sub-component. Update `formatTokens` to handle billions.

**Tech Stack:** Rust (sqlx, serde, ts-rs), React, Tailwind CSS, Lucide icons

---

### Task 1: Add cache token fields to Rust AIGenerationStats struct

**Files:**
- Modify: `crates/db/src/queries/types.rs:138-151`

**Step 1: Add two new fields to `AIGenerationStats`**

In `crates/db/src/queries/types.rs`, add `cache_read_tokens` and `cache_creation_tokens` after `total_output_tokens`:

```rust
pub struct AIGenerationStats {
    #[ts(type = "number")]
    pub lines_added: i64,
    #[ts(type = "number")]
    pub lines_removed: i64,
    #[ts(type = "number")]
    pub files_created: i64,
    #[ts(type = "number")]
    pub total_input_tokens: i64,
    #[ts(type = "number")]
    pub total_output_tokens: i64,
    #[ts(type = "number")]
    pub cache_read_tokens: i64,
    #[ts(type = "number")]
    pub cache_creation_tokens: i64,
    pub tokens_by_model: Vec<TokensByModel>,
    pub tokens_by_project: Vec<TokensByProject>,
}
```

**Step 2: Verify it compiles**

Run: `cargo check -p claude-view-db`
Expected: Compilation error in `ai_generation.rs` (missing fields in struct constructor) — that's correct, Task 2 fixes it.

**Step 3: Commit**

```bash
git add crates/db/src/queries/types.rs
git commit -m "feat: add cache token fields to AIGenerationStats struct"
```

---

### Task 2: Update SQL query to fetch cache tokens

**Files:**
- Modify: `crates/db/src/queries/ai_generation.rs:23-42` (aggregate query)
- Modify: `crates/db/src/queries/ai_generation.rs:136-144` (struct construction)

**Step 1: Update the aggregate SQL query**

Change the first query to also select cache tokens. Replace the `query_as` tuple type from `(i64, i64, i64)` to `(i64, i64, i64, i64, i64)`:

```rust
let (files_created, total_input_tokens, total_output_tokens, cache_read_tokens, cache_creation_tokens): (i64, i64, i64, i64, i64) =
    sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUM(files_edited_count), 0),
            COALESCE(SUM(total_input_tokens), 0),
            COALESCE(SUM(total_output_tokens), 0),
            COALESCE(SUM(cache_read_tokens), 0),
            COALESCE(SUM(cache_creation_tokens), 0)
        FROM valid_sessions
        WHERE last_message_at >= ?1
          AND last_message_at <= ?2
          AND (?3 IS NULL OR project_id = ?3)
          AND (?4 IS NULL OR git_branch = ?4)
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(project)
    .bind(branch)
    .fetch_one(self.pool())
    .await?;
```

**Step 2: Update the Ok() return to include the new fields**

```rust
Ok(AIGenerationStats {
    lines_added: 0,
    lines_removed: 0,
    files_created,
    total_input_tokens,
    total_output_tokens,
    cache_read_tokens,
    cache_creation_tokens,
    tokens_by_model,
    tokens_by_project,
})
```

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-db`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/db/src/queries/ai_generation.rs
git commit -m "feat: include cache_read and cache_creation tokens in AI generation stats query"
```

---

### Task 3: Regenerate TypeScript types

**Files:**
- Auto-generated: `src/types/generated/AIGenerationStats.ts`

**Step 1: Run ts-rs export**

Run: `cargo test -p claude-view-db ts_export -- --ignored 2>/dev/null || cargo test -p claude-view-db export 2>/dev/null || cargo test -p claude-view-server export 2>/dev/null`

If ts-rs tests don't exist or don't trigger, manually update the generated file.

**Step 2: Verify the generated type**

Check that `src/types/generated/AIGenerationStats.ts` now includes `cacheReadTokens: number` and `cacheCreationTokens: number`:

```typescript
export type AIGenerationStats = {
  linesAdded: number,
  linesRemoved: number,
  filesCreated: number,
  totalInputTokens: number,
  totalOutputTokens: number,
  cacheReadTokens: number,
  cacheCreationTokens: number,
  tokensByModel: Array<TokensByModel>,
  tokensByProject: Array<TokensByProject>,
};
```

If auto-generation doesn't work, manually edit the file to match.

**Step 3: Commit**

```bash
git add src/types/generated/AIGenerationStats.ts
git commit -m "chore: regenerate TS types with cache token fields"
```

---

### Task 4: Update formatTokens to handle billions

**Files:**
- Modify: `src/hooks/use-ai-generation.ts:69-78`

**Step 1: Add billion tier to formatTokens**

```typescript
export function formatTokens(tokens: number | null | undefined): string {
  if (tokens === null || tokens === undefined) return '--'
  if (tokens >= 1_000_000_000) {
    return `${(tokens / 1_000_000_000).toFixed(1)}B`
  }
  if (tokens >= 1_000_000) {
    return `${(tokens / 1_000_000).toFixed(1)}M`
  }
  if (tokens >= 1_000) {
    return `${(tokens / 1_000).toFixed(0)}k`
  }
  return tokens.toString()
}
```

**Step 2: Commit**

```bash
git add src/hooks/use-ai-generation.ts
git commit -m "feat: add billion-tier formatting to formatTokens"
```

---

### Task 5: Create StackedBar component

**Files:**
- Create: `src/components/ui/StackedBar.tsx`

**Step 1: Create the component**

```tsx
import { cn } from '../../lib/utils'

export interface StackedBarSegment {
  /** Segment label (shown in legend) */
  label: string
  /** Raw value */
  value: number
  /** Tailwind bg color class for light mode */
  color: string
  /** Tailwind bg color class for dark mode */
  darkColor: string
}

interface StackedBarProps {
  segments: StackedBarSegment[]
  className?: string
}

/**
 * Horizontal stacked bar showing proportional segments.
 * Each segment width = percentage of total. Segments < 1% get min-width for visibility.
 */
export function StackedBar({ segments, className }: StackedBarProps) {
  const total = segments.reduce((sum, s) => sum + s.value, 0)
  if (total === 0) return null

  return (
    <div className={cn('space-y-2', className)}>
      {/* Bar */}
      <div className="flex h-3 w-full rounded-full overflow-hidden bg-gray-100 dark:bg-gray-800">
        {segments.map((seg) => {
          const pct = (seg.value / total) * 100
          if (pct === 0) return null
          return (
            <div
              key={seg.label}
              className={cn(seg.color, seg.darkColor, 'transition-all duration-300')}
              style={{ width: `${Math.max(pct, 0.5)}%` }}
              title={`${seg.label}: ${pct.toFixed(1)}%`}
            />
          )
        })}
      </div>

      {/* Legend */}
      <div className="flex flex-wrap gap-x-4 gap-y-1">
        {segments.map((seg) => {
          const pct = (seg.value / total) * 100
          if (pct === 0) return null
          return (
            <div key={seg.label} className="flex items-center gap-1.5 text-xs text-gray-500 dark:text-gray-400">
              <span className={cn('w-2 h-2 rounded-full', seg.color, seg.darkColor)} />
              <span>{seg.label}</span>
              <span className="tabular-nums font-medium">{pct.toFixed(1)}%</span>
            </div>
          )
        })}
      </div>
    </div>
  )
}
```

**Step 2: Export from ui/index**

Check `src/components/ui/index.ts` (or wherever ui components are exported) and add:

```typescript
export { StackedBar } from './StackedBar'
export type { StackedBarSegment } from './StackedBar'
```

**Step 3: Commit**

```bash
git add src/components/ui/StackedBar.tsx src/components/ui/index.ts
git commit -m "feat: add StackedBar component for proportional visualization"
```

---

### Task 6: Create TokenBreakdown component

**Files:**
- Create: `src/components/TokenBreakdown.tsx`

**Step 1: Create the component**

```tsx
import { Zap } from 'lucide-react'
import { MetricCard, StackedBar } from './ui'
import type { StackedBarSegment } from './ui/StackedBar'
import { formatTokens } from '../hooks/use-ai-generation'

interface TokenBreakdownProps {
  totalInputTokens: number
  totalOutputTokens: number
  cacheReadTokens: number
  cacheCreationTokens: number
}

const SEGMENTS: Array<{ key: keyof TokenBreakdownProps; label: string; cardLabel: string; color: string; darkColor: string }> = [
  { key: 'cacheReadTokens', label: 'Cache Read', cardLabel: 'Cache Read', color: 'bg-emerald-500', darkColor: 'dark:bg-emerald-400' },
  { key: 'cacheCreationTokens', label: 'Cache Write', cardLabel: 'Cache Write', color: 'bg-amber-500', darkColor: 'dark:bg-amber-400' },
  { key: 'totalOutputTokens', label: 'Output', cardLabel: 'Output', color: 'bg-blue-600', darkColor: 'dark:bg-blue-400' },
  { key: 'totalInputTokens', label: 'Fresh Input', cardLabel: 'Fresh Input', color: 'bg-gray-400', darkColor: 'dark:bg-gray-500' },
]

export function TokenBreakdown(props: TokenBreakdownProps) {
  const grandTotal =
    props.totalInputTokens +
    props.totalOutputTokens +
    props.cacheReadTokens +
    props.cacheCreationTokens

  if (grandTotal === 0) return null

  const segments: StackedBarSegment[] = SEGMENTS.map((s) => ({
    label: s.label,
    value: props[s.key],
    color: s.color,
    darkColor: s.darkColor,
  }))

  return (
    <div className="space-y-3">
      {/* Hero card with stacked bar */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-4 sm:p-6">
        <p className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-2 flex items-center gap-1.5">
          <Zap className="w-3.5 h-3.5" />
          Total Tokens Processed
        </p>
        <p className="text-3xl sm:text-4xl font-semibold text-blue-800 dark:text-blue-300 tabular-nums mb-4">
          {formatTokens(grandTotal)}
        </p>
        <StackedBar segments={segments} />
      </div>

      {/* Detail cards */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        {SEGMENTS.map((s) => {
          const value = props[s.key]
          const pct = grandTotal > 0 ? ((value / grandTotal) * 100).toFixed(1) : '0.0'
          return (
            <MetricCard
              key={s.key}
              label={s.cardLabel}
              value={formatTokens(value)}
              subValue={`${pct}% of total`}
            />
          )
        })}
      </div>
    </div>
  )
}
```

**Step 2: Commit**

```bash
git add src/components/TokenBreakdown.tsx
git commit -m "feat: add TokenBreakdown hero card with stacked bar and detail cards"
```

---

### Task 7: Wire TokenBreakdown into AIGenerationStats

**Files:**
- Modify: `src/components/AIGenerationStats.tsx:92-98` (replace Tokens Used card)

**Step 1: Add import**

Add at top of `AIGenerationStats.tsx`:

```typescript
import { TokenBreakdown } from './TokenBreakdown'
```

**Step 2: Replace the "Tokens Used" MetricCard**

Replace lines 92-98 (the Tokens Used MetricCard block) with the TokenBreakdown. The component should be placed **after** the metric cards grid, as a full-width section. Restructure the layout:

1. Remove the "Tokens Used" `<MetricCard>` from the 3-column grid (lines 92-98)
2. Add `<TokenBreakdown>` as a new section right after the metric cards grid, before the "Token Usage Breakdowns" section

The relevant section of the return should become:

```tsx
<div className="space-y-4 sm:space-y-6">
  {/* Metric Cards Row — only Files Edited (and Lines Generated when available) */}
  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 sm:gap-4">
    {hasLineData && (
      <MetricCard
        label="Lines Generated"
        value={formatLineCount(stats.linesAdded)}
        subValue={stats.linesRemoved > 0 ? `${formatLineCount(-stats.linesRemoved, false)} removed` : undefined}
        footer={netLines !== stats.linesAdded ? `net: ${formatLineCount(netLines)}` : undefined}
      />
    )}
    <MetricCard
      label="Files Edited"
      value={stats.filesCreated.toLocaleString()}
      subValue="modified by AI"
    />
  </div>

  {/* Token Breakdown — hero card + stacked bar + detail cards */}
  {hasTokenData && (
    <TokenBreakdown
      totalInputTokens={stats.totalInputTokens}
      totalOutputTokens={stats.totalOutputTokens}
      cacheReadTokens={stats.cacheReadTokens}
      cacheCreationTokens={stats.cacheCreationTokens}
    />
  )}

  {/* Token Usage Breakdowns (by model / by project) — unchanged */}
  ...
```

**Step 3: Update `hasTokenData` check to include cache tokens**

Replace line 62:

```typescript
const hasTokenData = stats.totalInputTokens > 0 || stats.totalOutputTokens > 0 ||
  stats.cacheReadTokens > 0 || stats.cacheCreationTokens > 0
```

**Step 4: Update `totalModelTokens` and `totalProjectTokens` calculations**

These use `inputTokens + outputTokens` for the progress bars — keep them as-is since the by-model/by-project breakdowns don't include cache tokens in the API response. No change needed here.

**Step 5: Verify in browser**

Run: `bun run dev` and open the analytics page.
Expected: Hero card shows total tokens in billions with stacked bar. 4 detail cards below show each category.

**Step 6: Commit**

```bash
git add src/components/AIGenerationStats.tsx
git commit -m "feat: wire TokenBreakdown into analytics page, replacing Tokens Used card"
```

---

### Task 8: Visual polish and dark mode verification

**Files:**
- Possibly tweak: `src/components/TokenBreakdown.tsx`, `src/components/ui/StackedBar.tsx`

**Step 1: Test dark mode**

Toggle dark mode and verify:
- Hero card text has sufficient contrast
- Stacked bar segments are distinguishable
- Legend dots match bar colors
- Detail cards look correct

**Step 2: Test responsive**

Resize browser to 375px (mobile), 768px (tablet), 1024px+ (desktop).
- Mobile: 2-col detail cards, stacked bar readable
- Tablet: 2x2 detail cards
- Desktop: 4-col detail cards

**Step 3: Final commit**

```bash
git add -A
git commit -m "fix: dark mode and responsive tweaks for token breakdown"
```

Only commit if changes were needed.
