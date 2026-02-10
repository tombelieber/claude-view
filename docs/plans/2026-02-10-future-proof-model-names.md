---
status: done
date: 2026-02-10
---

# Future-Proof Model Name System — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Eliminate all hardcoded model lists so the dashboard automatically handles any new Claude model (4.6, 5.0, etc.) without code changes.

**Architecture:** Replace the 3 hardcoded model lists (formatter, filter, pricing fallback) with a single data-driven approach. Build a generic `formatModelName()` regex parser that handles any Claude model ID pattern, move it to shared utils, derive filter options from loaded session data (same pattern as branches), and add prefix-match in the backend for legacy URL compat.

**Tech Stack:** TypeScript (Vitest), Rust (Axum, sqlx), React

---

## Bug Summary

Three interlocking problems:

| Layer | File | Bug |
|-------|------|-----|
| **Frontend formatter** | `src/components/AIGenerationStats.tsx:156-194` | `formatModelName()` has a hardcoded lookup table missing `claude-opus-4-6`. The regex fallback only joins `3 5` → `3.5` and `4 5` → `4.5` — misses `4 6` → `4.6`, `4 1` → `4.1`, and all future versions. |
| **Frontend filter** | `src/components/FilterPopover.tsx:67` | Model options hardcoded to `['claude-opus-4', 'claude-sonnet-4', 'claude-haiku-4']`. Users can't see or select Opus 4.5, Opus 4.6, Sonnet 4.5, Haiku 4.5, etc. |
| **Backend filter** | `crates/server/src/routes/sessions.rs:260-271` | Model filter uses exact match (`models.contains(&m.as_str())`). Filter value `claude-opus-4` never matches `primary_model` value `claude-opus-4-6`. |

**Real-world model IDs found in JSONL data:**

| Model ID | Format | Notes |
|----------|--------|-------|
| `claude-opus-4-6` | Modern, no date | Current default (Opus 4.6) |
| `claude-opus-4-5-20251101` | Modern, with date | Previous Opus |
| `claude-sonnet-4-5-20250929` | Modern, with date | |
| `claude-haiku-4-5-20251001` | Modern, with date | |
| `claude-sonnet-4-20250514` | Modern, major only | |
| `claude-haiku-4-20250514` | Modern, major only | |
| `haiku`, `opus`, `sonnet` | Bare alias | Shorthand references |

---

## Task 1: Move `formatModelName()` to shared utils + rewrite with generic regex

**Files:**
- Create: `src/lib/format-model.ts` (new, single-purpose module)
- Modify: `src/components/AIGenerationStats.tsx` (remove exported function, import from new location)
- Modify: `src/components/AIGenerationStats.test.tsx` (update import, add new tests)

**Why:** The formatter belongs in a shared utility, not inside a component file. Both `AIGenerationStats` and `FilterPopover` (Task 2) need it. Also: the current regex has a **critical greedy-match bug** — Pattern A's `(\d+)` for minor version greedily captures the entire date suffix (e.g., `claude-opus-4-20250514` → minor=`20250514` → "Claude Opus 4.20250514"). Fix: constrain minor to `(\d{1,2})`.

**Step 1: Write failing tests for new model IDs**

Add these test cases to `src/components/AIGenerationStats.test.tsx` inside the `describe('formatModelName')` block. Update the import to use the new path:

```typescript
// At top of file, change:
// import { AIGenerationStats, formatModelName } from './AIGenerationStats'
// To:
import { AIGenerationStats } from './AIGenerationStats'
import { formatModelName } from '../lib/format-model'
```

Add inside the `'unknown model IDs (regex fallback)'` describe block:

```typescript
it('should handle claude-opus-4-6 (no date suffix)', () => {
  expect(formatModelName('claude-opus-4-6')).toBe('Claude Opus 4.6')
})

it('should handle claude-opus-4-1-20250805 (with date suffix)', () => {
  expect(formatModelName('claude-opus-4-1-20250805')).toBe('Claude Opus 4.1')
})

it('should handle claude-sonnet-4-5-20250929 (with date suffix)', () => {
  expect(formatModelName('claude-sonnet-4-5-20250929')).toBe('Claude Sonnet 4.5')
})

it('should handle claude-haiku-4-5-20251001 (with date suffix)', () => {
  expect(formatModelName('claude-haiku-4-5-20251001')).toBe('Claude Haiku 4.5')
})

it('should handle claude-opus-4-20250514 (major only, with date)', () => {
  expect(formatModelName('claude-opus-4-20250514')).toBe('Claude Opus 4')
})

it('should handle claude-haiku-4-20250514 (major only, with date)', () => {
  expect(formatModelName('claude-haiku-4-20250514')).toBe('Claude Haiku 4')
})

it('should handle hypothetical claude-sonnet-5-0-20270101', () => {
  expect(formatModelName('claude-sonnet-5-0-20270101')).toBe('Claude Sonnet 5.0')
})

it('should handle hypothetical claude-opus-5-20270601 (major only)', () => {
  expect(formatModelName('claude-opus-5-20270601')).toBe('Claude Opus 5')
})
```

Add inside the `'edge cases'` describe block:

```typescript
it('should capitalize bare alias "opus"', () => {
  expect(formatModelName('opus')).toBe('Opus')
})

it('should capitalize bare alias "sonnet"', () => {
  expect(formatModelName('sonnet')).toBe('Sonnet')
})

it('should capitalize bare alias "haiku"', () => {
  expect(formatModelName('haiku')).toBe('Haiku')
})
```

**Step 2: Run tests to verify they fail**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/dashboard-analytics && bun test src/components/AIGenerationStats.test.tsx`

Expected: FAIL — import path doesn't exist yet, and new test cases would fail with old implementation.

**Step 3: Create `src/lib/format-model.ts` with generic parser**

```typescript
/**
 * Format a model ID into a human-readable name.
 *
 * Handles all patterns generically — no hardcoded model map:
 *   "claude-opus-4-6"             → "Claude Opus 4.6"
 *   "claude-opus-4-5-20251101"    → "Claude Opus 4.5"
 *   "claude-sonnet-4-20250514"    → "Claude Sonnet 4"
 *   "claude-3-5-sonnet-20241022"  → "Claude 3.5 Sonnet"
 *   "opus"                        → "Opus"  (bare alias)
 *   "gpt-4-turbo"                 → "gpt-4-turbo" (non-Claude passthrough)
 *
 * New models (5.0, 6.1, etc.) are handled automatically.
 */
export function formatModelName(modelId: string): string {
  if (!modelId) return modelId

  // Bare aliases: "opus", "sonnet", "haiku" → capitalize
  if (!modelId.includes('-')) {
    return modelId.charAt(0).toUpperCase() + modelId.slice(1)
  }

  // Non-Claude models pass through unchanged
  if (!modelId.startsWith('claude-')) return modelId

  // Need at least "claude-X-Y" (3 parts) to parse
  if (modelId.split('-').length < 3) return modelId

  // Pattern A: Modern — claude-{family}-{major}[-{minor}][-{date}]
  //   claude-opus-4-6, claude-sonnet-4-5-20250929, claude-opus-4-20250514
  //
  // IMPORTANT: minor uses (\d{1,2}), NOT (\d+). With (\d+), the greedy match
  // captures the 8-digit date suffix as a minor version number.
  // With (\d{1,2}), the regex engine backtracks correctly:
  //   "claude-opus-4-20250514" → minor skipped, date=20250514 → "Claude Opus 4"
  const modernMatch = modelId.match(
    /^claude-([a-z]+)-(\d+)(?:-(\d{1,2}))?(?:-(\d{8}))?$/
  )
  if (modernMatch) {
    const [, family, major, minor] = modernMatch
    const familyName = family.charAt(0).toUpperCase() + family.slice(1)
    const version = minor !== undefined ? `${major}.${minor}` : major
    return `Claude ${familyName} ${version}`
  }

  // Pattern B: Legacy — claude-{major}[-{minor}]-{family}[-{date}]
  //   claude-3-5-sonnet-20241022, claude-3-opus-20240229
  const legacyMatch = modelId.match(
    /^claude-(\d+)(?:-(\d{1,2}))?-([a-z]+)(?:-(\d{8}))?$/
  )
  if (legacyMatch) {
    const [, major, minor, family] = legacyMatch
    const familyName = family.charAt(0).toUpperCase() + family.slice(1)
    const version = minor !== undefined ? `${major}.${minor}` : major
    return `Claude ${version} ${familyName}`
  }

  // Pattern C: Unknown Claude format — strip date suffix, capitalize parts
  //   Handles multi-word families like "claude-3-super-fast-20260101"
  const parts = modelId.split('-')
  if (parts[parts.length - 1]?.match(/^\d{8}$/)) {
    parts.pop()
  }
  return parts
    .map((p, i) => (i === 0 ? 'Claude' : p.charAt(0).toUpperCase() + p.slice(1)))
    .join(' ')
}
```

**Step 4: Update `AIGenerationStats.tsx` — remove function, import from new location**

In `src/components/AIGenerationStats.tsx`:

1. Add import at top:
```typescript
import { formatModelName } from '../lib/format-model'
```

2. Delete the entire exported `formatModelName` function (lines 152-194). The only external consumer was the test file, and Step 1 already updated that import to point at `../lib/format-model`. No re-export needed — adding one would be dead code per CLAUDE.md ("Avoid backwards-compatibility hacks... re-exporting types... If you are certain that something is unused, you can delete it completely").

**Step 5: Run all formatModelName tests**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/dashboard-analytics && bun test src/components/AIGenerationStats.test.tsx`

Expected: ALL tests pass — every existing test AND every new test.

**Regression check — trace every existing test case through new code:**

| Input | Pattern | Result | Matches old? |
|-------|---------|--------|------|
| `claude-opus-4-5-20251101` | A | Claude Opus 4.5 | ✓ |
| `claude-opus-4-20250514` | A | Claude Opus 4 | ✓ |
| `claude-sonnet-4-20250514` | A | Claude Sonnet 4 | ✓ |
| `claude-3-5-sonnet-20241022` | B | Claude 3.5 Sonnet | ✓ |
| `claude-3-5-haiku-20241022` | B | Claude 3.5 Haiku | ✓ |
| `claude-3-opus-20240229` | B | Claude 3 Opus | ✓ |
| `claude-3-haiku-20240307` | B | Claude 3 Haiku | ✓ |
| `claude-3-5-opus-20260101` | B | Claude 3.5 Opus | ✓ |
| `claude-3-turbo` | B | Claude 3 Turbo | ✓ |
| `claude-4-5-haiku-20260601` | B | Claude 4.5 Haiku | ✓ |
| `claude-3-mega-20260101` | B | Claude 3 Mega | ✓ |
| `claude-3-super-fast-20260101` | C | Claude 3 Super Fast | ✓ |
| `""` | guard | `""` | ✓ |
| `gpt-4-turbo` | guard | `gpt-4-turbo` | ✓ |
| `unknown` | bare alias | `Unknown` | **CHANGED** |
| `claude-opus` | guard (<3) | `claude-opus` | ✓ |
| `claude` | bare alias | `Claude` | **CHANGED** |

**Note:** Two existing tests need updating due to the bare alias path (`!modelId.includes('-')` → capitalize). Single-word strings without a dash are now capitalized:

```typescript
// Old:
it('should return short non-claude string as-is', () => {
  expect(formatModelName('unknown')).toBe('unknown')
})
// New:
it('should capitalize short non-claude single-word string', () => {
  expect(formatModelName('unknown')).toBe('Unknown')
})

// Old:
it('should handle model ID that is just "claude"', () => {
  expect(formatModelName('claude')).toBe('claude')
})
// New:
it('should capitalize bare single-word "claude"', () => {
  expect(formatModelName('claude')).toBe('Claude')
})
```

**Step 6: Commit**

```bash
git add src/lib/format-model.ts src/components/AIGenerationStats.tsx src/components/AIGenerationStats.test.tsx
git commit -m "fix(dashboard): replace hardcoded model name formatter with generic regex parser

Moved formatModelName() to src/lib/format-model.ts (shared utility).
Handles any claude-{family}-{major}-{minor} pattern (4.6, 4.1, 5.0, etc.)
without needing manual updates for each new model release.

Critical fix: minor version regex uses (\d{1,2}) not (\d+) to prevent
greedy match from capturing 8-digit date suffixes as version numbers."
```

---

## Task 2: Make model filter data-driven (frontend)

**Files:**
- Modify: `src/components/FilterPopover.tsx`
- Modify: `src/components/FilterPopover.test.tsx`
- Modify: `src/components/SessionToolbar.tsx`

**Why:** The filter popover hardcodes 3 model families. Users can't filter by actual models in their data.

**Step 1: Write failing test — FilterPopover accepts `models` prop**

Add to `src/components/FilterPopover.test.tsx`:

```typescript
import { formatModelName } from '../lib/format-model';

it('renders model checkboxes from models prop instead of hardcoded list', () => {
  const onChange = vi.fn();
  const onClear = vi.fn();
  const models = ['claude-opus-4-6', 'claude-sonnet-4-5-20250929', 'claude-haiku-4-5-20251001'];

  render(
    <FilterPopover
      filters={DEFAULT_FILTERS}
      onChange={onChange}
      onClear={onClear}
      activeCount={0}
      branches={TEST_BRANCHES}
      models={models}
    />
  );

  const trigger = screen.getByRole('button', { name: /filters/i });
  fireEvent.click(trigger);

  // Should show formatted model names from data-driven list
  expect(screen.getByText('Claude Opus 4.6')).toBeInTheDocument();
  expect(screen.getByText('Claude Sonnet 4.5')).toBeInTheDocument();
  expect(screen.getByText('Claude Haiku 4.5')).toBeInTheDocument();
});

it('hides model section when models prop is empty', () => {
  const onChange = vi.fn();
  const onClear = vi.fn();

  render(
    <FilterPopover
      filters={DEFAULT_FILTERS}
      onChange={onChange}
      onClear={onClear}
      activeCount={0}
      branches={TEST_BRANCHES}
      models={[]}
    />
  );

  const trigger = screen.getByRole('button', { name: /filters/i });
  fireEvent.click(trigger);

  // Model section label should not appear
  expect(screen.queryByText('Model')).not.toBeInTheDocument();
});
```

**Step 2: Run tests to verify they fail**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/dashboard-analytics && bun test src/components/FilterPopover.test.tsx`

Expected: FAIL — `FilterPopover` doesn't accept `models` prop yet.

**Step 3: Update FilterPopover**

In `src/components/FilterPopover.tsx`:

1. Add import:
```typescript
import { formatModelName } from '../lib/format-model';
```

2. Add `models` to interface:
```typescript
interface FilterPopoverProps {
  filters: SessionFilters;
  onChange: (filters: SessionFilters) => void;
  onClear: () => void;
  activeCount: number;
  /** Available branch names derived from loaded sessions */
  branches: string[];
  /** Available model IDs from indexed session data (data-driven) */
  models?: string[];
}
```

3. Update component signature:
```typescript
export function FilterPopover({ filters, onChange, onClear, activeCount, branches = [], models = [] }: FilterPopoverProps) {
```

4. Replace hardcoded `modelOptions` (line 67):
```typescript
const modelOptions = models;
```

5. Update model label rendering (line 235-237):
```tsx
<span className="ml-2 text-xs text-gray-700 dark:text-gray-300">
  {formatModelName(model)}
</span>
```

6. Wrap Model filter section in conditional (lines 213-241):
```tsx
{modelOptions.length > 0 && (
  <div>
    <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">
      Model
    </label>
    <div className="flex flex-wrap gap-2">
      {modelOptions.map((model) => (
        // ... existing checkbox JSX unchanged ...
      ))}
    </div>
  </div>
)}
```

**Step 4: Update SessionToolbar to accept and pass `models`**

In `src/components/SessionToolbar.tsx`:

1. Add to interface:
```typescript
interface SessionToolbarProps {
  filters: SessionFilters;
  onFiltersChange: (filters: SessionFilters) => void;
  onClearFilters: () => void;
  groupByDisabled?: boolean;
  branches?: string[];
  /** Available model IDs from indexed session data */
  models?: string[];
}
```

2. Destructure in component and pass to FilterPopover:
```tsx
<FilterPopover
  filters={filters}
  onChange={onFiltersChange}
  onClear={onClearFilters}
  activeCount={activeFilterCount}
  branches={branches}
  models={models}
/>
```

**Step 5: Run tests**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/dashboard-analytics && bun test src/components/FilterPopover.test.tsx`

Expected: ALL tests pass. Existing tests that don't pass `models` prop get default `[]`, so the model section is hidden — existing tests don't assert on model checkboxes, so no regressions.

**Step 6: Commit**

```bash
git add src/components/FilterPopover.tsx src/components/FilterPopover.test.tsx src/components/SessionToolbar.tsx
git commit -m "feat(filters): make model filter data-driven via models prop

Replaces hardcoded ['claude-opus-4', ...] with prop from parent.
Displays formatted model names using shared formatModelName().
Hides model section when no models available."
```

---

## Task 3: Wire model list from sessions to FilterPopover

**Files:**
- Modify: `src/components/HistoryView.tsx` (line ~103, near `availableBranches`)
- Modify: `src/components/ProjectView.tsx` (line ~99, near `availableBranches`)

**Why:** Two components render `<SessionToolbar>` — both need the `models` prop wired. Follow the identical pattern already used for `availableBranches`.

**Step 1: Add `availableModels` to HistoryView**

In `src/components/HistoryView.tsx`, after the `availableBranches` useMemo (line ~108), add:

```typescript
// Extract unique model IDs from sessions for the filter popover
// (same pattern as availableBranches above)
const availableModels = useMemo(() => {
  const set = new Set<string>()
  for (const s of allSessions) {
    if (s.primaryModel) set.add(s.primaryModel)
  }
  return [...set].sort()
}, [allSessions])
```

Then pass to SessionToolbar (line ~394):

```tsx
<SessionToolbar
  filters={filters}
  onFiltersChange={setFilters}
  onClearFilters={() => setFilters(DEFAULT_FILTERS)}
  groupByDisabled={tooManyToGroup}
  branches={availableBranches}
  models={availableModels}
/>
```

**Step 2: Add `availableModels` to ProjectView**

In `src/components/ProjectView.tsx`, after the `availableBranches` useMemo (line ~104), add:

```typescript
const availableModels = useMemo(() => {
  const set = new Set<string>()
  for (const s of page?.sessions ?? []) {
    if (s.primaryModel) set.add(s.primaryModel)
  }
  return [...set].sort()
}, [page?.sessions])
```

Then pass to SessionToolbar (line ~185):

```tsx
<SessionToolbar
  filters={filters}
  onFiltersChange={setFilters}
  onClearFilters={() => setFilters(DEFAULT_FILTERS)}
  groupByDisabled={tooManyToGroup}
  branches={availableBranches}
  models={availableModels}
/>
```

**Step 3: Verify in browser**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/dashboard-analytics && bun run preview`

Open the dashboard, click Filters. The Model section should list actual models from your sessions (e.g., "Claude Opus 4.6", "Claude Sonnet 4.5"). Navigate to a project view and verify the same.

**Step 4: Commit**

```bash
git add src/components/HistoryView.tsx src/components/ProjectView.tsx
git commit -m "feat(filters): derive model options from loaded session data

Both HistoryView and ProjectView now extract unique primaryModel values
from sessions and pass them to the filter popover. Same pattern as branches.
New models appear automatically after re-indexing."
```

---

## Task 4: Fix backend model filter — prefix match for legacy URLs

**Files:**
- Modify: `crates/server/src/routes/sessions.rs:259-271`

**Why:** Now that the frontend sends full model IDs (`claude-opus-4-6`), exact match works for new usage. But legacy bookmarked URLs may contain family-level prefixes (`?models=claude-opus-4`). Add prefix match as fallback so those URLs don't silently break.

**Step 1: Update filter logic (zero-alloc version)**

In `crates/server/src/routes/sessions.rs`, replace lines 259-271:

```rust
// Filter by models (comma-separated, supports exact + prefix match)
// Exact: models=claude-opus-4-6 matches "claude-opus-4-6"
// Prefix: models=claude-opus-4 matches "claude-opus-4-6" (legacy URL compat)
// The dash check prevents "claude-opus-4" from matching "claude-opus-40-*".
if let Some(models_str) = &query.models {
    let models: Vec<&str> = models_str.split(',').map(|s| s.trim()).collect();
    all_sessions = all_sessions
        .into_iter()
        .filter(|s| {
            s.primary_model
                .as_ref()
                .map(|m| {
                    models.iter().any(|&filter| {
                        m == filter
                            || (m.starts_with(filter)
                                && m.as_bytes().get(filter.len()) == Some(&b'-'))
                    })
                })
                .unwrap_or(false)
        })
        .collect();
}
```

**Step 2: Run Rust tests**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/dashboard-analytics && cargo test -p vibe-recall-server -- routes::sessions`

Expected: All existing tests pass. Note: the model filter test is currently skipped (commented out) due to `insert_session()` not persisting `primary_model`. This is a pre-existing issue — don't block on it.

**Step 3: Commit**

```bash
git add crates/server/src/routes/sessions.rs
git commit -m "fix(api): model filter supports prefix match for legacy URL compat

'models=claude-opus-4' now matches 'claude-opus-4-6' via prefix + dash
boundary check. Uses zero-alloc byte comparison. Exact match still works."
```

---

## Task 5: Verify pricing table (no code change expected)

**Files:**
- Verify: `crates/db/src/pricing.rs:79`

**Why:** Pricing drives cost-per-line calculations. Confirm `claude-opus-4-6` entry exists and values are correct.

**Step 1: Verify**

`crates/db/src/pricing.rs:79` already has:
```rust
"claude-opus-4-6".into(),
ModelPricing {
    input_cost_per_token: 5e-6,   // $5/M
    output_cost_per_token: 25e-6, // $25/M
    ...
}
```

This is correct. No changes needed. Mark task as done.

---

## Task 6: Rename ModelComparison's local formatter

**Files:**
- Modify: `src/components/contributions/ModelComparison.tsx`

**Why:** This file has a local `formatModelName()` (line 231) that returns just the family name (`"Opus"`, `"Sonnet"`) — completely different from the exported `formatModelName()`. Rename to `formatModelFamily()` to prevent name collision and confusion.

**Step 1: Rename function and all call sites**

In `src/components/contributions/ModelComparison.tsx`:

1. Rename the function (line 231):
```typescript
function formatModelFamily(model: string): string {
```

2. Update all 4 call sites:
- Line 33: `displayName: formatModelFamily(m.model),`
- Line 174: `{formatModelFamily(model.model)}`
- Line 271: `formatModelFamily(best.model)` and `formatModelFamily(worst.model)`

**Step 2: Run tests**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/dashboard-analytics && bun test src/components/contributions/`

Expected: All pass (no tests reference the function by name).

**Step 3: Commit**

```bash
git add src/components/contributions/ModelComparison.tsx
git commit -m "refactor: rename ModelComparison local formatter to formatModelFamily

Prevents name collision with the shared formatModelName() in format-model.ts.
This function intentionally returns just the family name (Opus, Sonnet, etc.)
for space-constrained chart labels."
```

---

## Task 7: End-to-end verification

**Step 1: Run full test suite**

```bash
cd /Users/user/dev/@myorg/claude-view/.worktrees/dashboard-analytics
bun test
cargo test -p vibe-recall-server
```

Expected: All tests pass.

**Step 2: Build and run**

```bash
bun run preview
```

**Step 3: Verify in browser**

| Check | Expected |
|-------|----------|
| Token Usage by Model | Shows "Claude Opus 4.6", not "Claude Opus 4 6" or raw ID |
| Token Usage by Model | Shows "Claude Sonnet 4.5", not "Claude Sonnet 4 5" |
| Filter → Model section | Lists actual models from sessions (Claude Opus 4.6, Claude Sonnet 4.5, etc.) |
| Select model filter | Sessions list filters correctly (only shows sessions with that model) |
| Clear model filter | All sessions return |
| Model Comparison chart | Shows "Opus", "Sonnet", "Haiku" labels (short form, unchanged) |
| Cost per line column | Shows dollar values (not `--`) for Opus 4.6 sessions |
| Legacy URL `?models=claude-opus-4` | Still filters correctly (prefix match) |
| Bare alias `opus` in data | Displays as "Opus" (capitalized), not "opus" |

---

## Design Decisions & Rationale

### Why `(\d{1,2})` not `(\d+)` for minor version?

With `(\d+)`, regex greedily matches:
```
"claude-opus-4-20250514"
                ^^^^^^^^ captured as minor version!
→ "Claude Opus 4.20250514"  ← WRONG
```

With `(\d{1,2})`, regex backtracks correctly:
```
"claude-opus-4-20250514"
                        ← (\d{1,2}) tries "20" → remaining "250514" can't be end → backtracks
                        ← (\d{1,2}) tries "2" → remaining "0250514" can't be end → backtracks
                        ← (\d{1,2}) skipped entirely
              ^^^^^^^^ captured by (\d{8}) as date
→ "Claude Opus 4"  ← CORRECT
```

### Why move to `src/lib/format-model.ts`?

Both `AIGenerationStats.tsx` and `FilterPopover.tsx` need the formatter. Importing a utility from a sibling component (`FilterPopover` → `AIGenerationStats`) is an architectural smell — components shouldn't import utilities from each other. `src/lib/` is the existing home for shared formatters (`format-utils.ts` is already there).

### Why derive models from sessions, not GET /api/models?

- Zero additional API calls (session data already loaded)
- Shows only models relevant to the current view
- If a model exists in DB but has no sessions in current filter, showing it would return 0 results — confusing UX
- Identical pattern to how branches already work

### Why prefix match in backend filter?

Users may have bookmarked `?models=claude-opus-4` (the old hardcoded values). Breaking those URLs silently is a regression. Prefix match with dash boundary (`starts_with("claude-opus-4") && next_byte == '-'`) is safe — `claude-opus-4` can never accidentally match `claude-opus-40-*`.

### Why not normalize model IDs in the database?

Storing raw model IDs from JSONL is correct — it's the source of truth. Normalization is a display concern that belongs in the frontend. If Anthropic changes their ID format, only the frontend parser needs updating.
