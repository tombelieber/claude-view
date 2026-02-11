---
status: pending
date: 2026-02-10
---

# Merge /system into /settings Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Merge the `/system` page's 4 unique features into the polished `/settings` page, eliminate the duplicate route, and unify navigation.

**Architecture:** Extract Index History, Health Status, CLI Status, and Danger Zone from `SystemPage.tsx` into standalone components matching the `SettingsPage.tsx` component pattern. Wire them to the existing `/api/system` sub-endpoints (clear-cache, reset) and the existing `/api/system` GET endpoint for health/history/CLI data. The polished Settings page becomes the single source of truth. Sidebar nav changes from "System" to "Settings".

**Tech Stack:** React, TypeScript, TanStack React Query, Tailwind CSS, Lucide icons, Axum (backend — no changes needed, all endpoints already exist)

---

## Context & Navigation Today

| Entry Point | Destination | Icon |
|-------------|-------------|------|
| Sidebar nav | `/system` (SystemPage) | `<Server>` |
| Header gear | `/settings` (SettingsPage) | `<Settings>` |

**Problem:** Two pages, 70% overlap, different data sources (`/api/system` vs `/api/stats/storage` + `/api/status`), user told us `/system` data is not 100% correct while `/settings` is fine-tuned.

**After merge:**

| Entry Point | Destination | Icon |
|-------------|-------------|------|
| Sidebar nav | `/settings` (SettingsPage) | `<Settings>` |
| Header gear | `/settings` (SettingsPage) | `<Settings>` (unchanged) |
| `/system` URL | Redirect → `/settings` | — |

---

## What Moves Where

| Feature from /system | Target in /settings | New Component | API Source |
|---------------------|--------------------|----|-----|
| Health Status (sessions/commits/projects/errors + status light) | Badge on "Data & Storage" section header | Inline in `SettingsPage.tsx` | `GET /api/system` → `.health` |
| Index History table (recent runs) | New section below "Data & Storage" | `IndexHistory.tsx` | `GET /api/system` → `.indexHistory` |
| Claude CLI status (install/auth detection) | New section before "About" | `CliStatus.tsx` | `GET /api/system` → `.claudeCli` |
| Clear Cache + Reset All Data | New "Danger Zone" section at bottom | `DangerZone.tsx` | `POST /api/system/clear-cache`, `POST /api/system/reset` |

**What gets deleted (duplicates — /settings already has better versions):**
- `StorageCard` (settings has donut chart)
- `PerformanceCard` (settings has inline performance stats)
- `ClassificationSection` (settings has `ClassificationStatus` with SSE + start/cancel)
- Git Re-sync button (settings has full Git Sync section with interval config)
- Export Data button (settings has format picker + scope selector)

---

## Design Rules (from UIUX Pro Max + existing patterns)

All new components MUST match the existing `SettingsPage` patterns exactly:

1. **Section wrapper:** Use `<SettingsSection icon={...} title="...">` — not the SystemPage's `<SectionCard>` (they look identical but SettingsSection is the canonical one)
2. **Info rows:** Use the existing `InfoRow` pattern from SettingsPage (label left, value right, `tabular-nums`)
3. **Buttons:** Same `cn(...)` button classes as existing Settings buttons (gray-900 bg, rounded-md, focus-visible ring)
4. **Loading states:** `<Loader2 className="w-4 h-4 animate-spin" />` + "Loading..." text
5. **Error states:** `<AlertCircle>` icon + red text + retry mechanism
6. **Dark mode:** Every element needs `dark:` variant — copy exact patterns from existing sections
7. **Spacing:** `space-y-4` between sections (matches existing SettingsPage)
8. **Typography:** `text-sm` body, `text-xs` labels, `font-medium` values, `uppercase tracking-wide` headers
9. **Accessibility:** `role="progressbar"`, `aria-label`, `aria-busy`, `focus-visible:ring-2`
10. **Touch targets:** Min 44px for buttons (`min-h-[44px]`)
11. **Destructive actions:** Red border, expandable confirmation, exact string match required
12. **No emoji icons:** SVG Lucide icons only

---

## Tasks

### Task 1: Create `useSystemStatus` hook (thin wrapper for health/history/CLI)

The existing `useSystem()` hook fetches everything from `/api/system`. We need a focused hook that the Settings page can use for just the 3 new data types.

**Files:**
- Create: `src/hooks/use-system-status.ts`

**Step 1: Write the hook**

```typescript
import { useQuery } from '@tanstack/react-query'
import type {
  SystemResponse,
  HealthInfo,
  IndexRunInfo,
  ClaudeCliStatus,
} from '../types/generated'

async function fetchSystemStatus(): Promise<SystemResponse> {
  const response = await fetch('/api/system')
  if (!response.ok) {
    throw new Error(`Failed to fetch system status: ${await response.text()}`)
  }
  return response.json()
}

/**
 * Hook for system health, index history, and CLI status.
 * Used by Settings page sections that need data from /api/system.
 * Polls every 30 seconds.
 */
export function useSystemStatus() {
  const query = useQuery({
    queryKey: ['system-status'],
    queryFn: fetchSystemStatus,
    staleTime: 10_000,
    refetchInterval: 30_000,
  })

  return {
    health: query.data?.health,
    indexHistory: query.data?.indexHistory,
    claudeCli: query.data?.claudeCli,
    isLoading: query.isLoading,
    error: query.error,
  }
}
```

**Step 2: Verify TypeScript compiles**

Run: `cd frontend && npx tsc --noEmit --pretty 2>&1 | head -20` (or from repo root: `npx tsc --noEmit` if tsconfig is at root)

Actually, since this is a Bun project, run:
```bash
bunx tsc --noEmit --pretty 2>&1 | head -20
```
Expected: No errors related to use-system-status.ts

**Step 3: Commit**

```bash
git add src/hooks/use-system-status.ts
git commit -m "feat(settings): add useSystemStatus hook for health/history/CLI data"
```

---

### Task 2: Create `IndexHistory` component

Extract the index history table from SystemPage, adapt to use `SettingsSection` wrapper and existing InfoRow patterns.

**Files:**
- Create: `src/components/IndexHistory.tsx`

**Step 1: Write the component**

```tsx
import { useState } from 'react'
import {
  History,
  CheckCircle2,
  XCircle,
  Loader2,
} from 'lucide-react'
import { cn } from '../lib/utils'
import type { IndexRunInfo } from '../types/generated'
import { formatRelativeTimestamp, formatDuration } from '../hooks/use-system'

interface IndexHistoryProps {
  history?: IndexRunInfo[]
  isLoading: boolean
}

/**
 * Index history table for the Settings page.
 * Shows recent index runs with time, type badge, session count, duration, and status.
 */
export function IndexHistory({ history, isLoading }: IndexHistoryProps) {
  const [showAll, setShowAll] = useState(false)

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-gray-400 py-4">
        <Loader2 className="w-4 h-4 animate-spin" />
        <span className="text-sm">Loading index history...</span>
      </div>
    )
  }

  if (!history || history.length === 0) {
    return (
      <p className="text-sm text-gray-500 dark:text-gray-400 py-2">
        No index runs recorded yet.
      </p>
    )
  }

  const displayed = showAll ? history : history.slice(0, 5)

  return (
    <div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-gray-500 dark:text-gray-400 border-b border-gray-100 dark:border-gray-800">
              <th className="pb-2 font-medium">Time</th>
              <th className="pb-2 font-medium">Type</th>
              <th className="pb-2 font-medium text-right">Sessions</th>
              <th className="pb-2 font-medium text-right">Duration</th>
              <th className="pb-2 font-medium text-right">Status</th>
            </tr>
          </thead>
          <tbody>
            {displayed.map((run, i) => (
              <tr
                key={`${run.timestamp}-${i}`}
                className="border-b border-gray-50 dark:border-gray-800/50 last:border-0"
              >
                <td className="py-1.5 text-gray-700 dark:text-gray-300">
                  {formatRelativeTimestamp(run.timestamp)}
                </td>
                <td className="py-1.5">
                  <span
                    className={cn(
                      'inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium',
                      run.type === 'full'
                        ? 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300'
                        : run.type === 'incremental'
                          ? 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400'
                          : 'bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300'
                    )}
                  >
                    {run.type}
                  </span>
                </td>
                <td className="py-1.5 text-right tabular-nums text-gray-700 dark:text-gray-300">
                  {run.sessionsCount != null ? run.sessionsCount.toLocaleString() : '--'}
                </td>
                <td className="py-1.5 text-right tabular-nums text-gray-700 dark:text-gray-300">
                  {run.durationMs != null ? formatDuration(run.durationMs) : '--'}
                </td>
                <td className="py-1.5 text-right">
                  {run.status === 'completed' ? (
                    <CheckCircle2 className="w-4 h-4 text-green-500 inline" />
                  ) : run.status === 'failed' ? (
                    <span className="inline-flex items-center gap-1">
                      <XCircle className="w-4 h-4 text-red-500" />
                      {run.errorMessage && (
                        <span
                          className="text-xs text-red-500 max-w-[120px] truncate"
                          title={run.errorMessage}
                        >
                          {run.errorMessage}
                        </span>
                      )}
                    </span>
                  ) : (
                    <Loader2 className="w-4 h-4 text-blue-500 animate-spin inline" />
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {history.length > 5 && (
        <button
          type="button"
          onClick={() => setShowAll(!showAll)}
          className="mt-3 text-sm text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
        >
          {showAll ? 'Show Less' : `Show All (${history.length})`}
        </button>
      )}
    </div>
  )
}
```

**Step 2: Verify TypeScript compiles**

```bash
bunx tsc --noEmit --pretty 2>&1 | head -20
```
Expected: No errors

**Step 3: Commit**

```bash
git add src/components/IndexHistory.tsx
git commit -m "feat(settings): add IndexHistory component"
```

---

### Task 3: Create `CliStatus` component

Extract CLI status display from SystemPage into a standalone component.

**Files:**
- Create: `src/components/CliStatus.tsx`

**Step 1: Write the component**

```tsx
import {
  CheckCircle2,
  XCircle,
  AlertCircle,
  Loader2,
} from 'lucide-react'
import type { ClaudeCliStatus } from '../types/generated'

interface CliStatusProps {
  cli?: ClaudeCliStatus
  isLoading: boolean
}

/**
 * Claude CLI installation and authentication status for the Settings page.
 */
export function CliStatus({ cli, isLoading }: CliStatusProps) {
  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-gray-400 py-4">
        <Loader2 className="w-4 h-4 animate-spin" />
        <span className="text-sm">Detecting Claude CLI...</span>
      </div>
    )
  }

  if (!cli || !cli.path) {
    return (
      <div className="flex items-start gap-3">
        <XCircle className="w-5 h-5 text-red-500 flex-shrink-0 mt-0.5" />
        <div>
          <p className="text-sm font-medium text-gray-900 dark:text-gray-100 mb-2">
            Not installed
          </p>
          <p className="text-sm text-gray-500 dark:text-gray-400 mb-3">
            Install Claude CLI to enable AI classification:
          </p>
          <div className="bg-gray-50 dark:bg-gray-800 rounded-md p-3 text-sm font-mono text-gray-700 dark:text-gray-300 space-y-1">
            <p>npm install -g @anthropic-ai/claude-code</p>
            <p className="text-gray-400"># or</p>
            <p>brew install claude</p>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <CheckCircle2 className="w-4 h-4 text-green-500" />
        <span className="text-sm text-gray-700 dark:text-gray-300">
          Installed:{' '}
          <code className="font-mono text-xs bg-gray-100 dark:bg-gray-800 px-1.5 py-0.5 rounded">
            {cli.path}
          </code>
        </span>
      </div>
      {cli.version && (
        <div className="flex items-center gap-2 ml-6">
          <span className="text-sm text-gray-500 dark:text-gray-400">
            Version: {cli.version}
          </span>
        </div>
      )}
      <div className="flex items-center gap-2">
        {cli.authenticated ? (
          <>
            <CheckCircle2 className="w-4 h-4 text-green-500" />
            <span className="text-sm text-gray-700 dark:text-gray-300">
              Authenticated
              {cli.subscriptionType && cli.subscriptionType !== 'unknown' && (
                <span className="text-gray-500 dark:text-gray-400">
                  {' '}
                  ({cli.subscriptionType.charAt(0).toUpperCase() + cli.subscriptionType.slice(1)}{' '}
                  subscription)
                </span>
              )}
            </span>
          </>
        ) : (
          <>
            <AlertCircle className="w-4 h-4 text-amber-500" />
            <span className="text-sm text-gray-700 dark:text-gray-300">Not authenticated</span>
            <span className="text-xs text-gray-400 ml-1">
              Run:{' '}
              <code className="font-mono bg-gray-100 dark:bg-gray-800 px-1 rounded">
                claude auth login
              </code>
            </span>
          </>
        )}
      </div>
    </div>
  )
}
```

**Step 2: Verify TypeScript compiles**

```bash
bunx tsc --noEmit --pretty 2>&1 | head -20
```

**Step 3: Commit**

```bash
git add src/components/CliStatus.tsx
git commit -m "feat(settings): add CliStatus component"
```

---

### Task 4: Create `DangerZone` component

Extract destructive actions (clear cache + reset all data) from SystemPage into a standalone component with the GitHub-style danger zone pattern.

**Files:**
- Create: `src/components/DangerZone.tsx`

**Step 1: Write the component**

```tsx
import { useState, useCallback } from 'react'
import {
  AlertTriangle,
  Trash2,
  CheckCircle2,
  XCircle,
  Loader2,
} from 'lucide-react'
import { useClearCache, useReset, formatBytes } from '../hooks/use-system'
import { cn } from '../lib/utils'

/**
 * Danger Zone section for destructive settings actions.
 * Includes Clear Cache and Reset All Data with confirmation.
 */
export function DangerZone() {
  const clearCache = useClearCache()
  const reset = useReset()

  const [showResetConfirm, setShowResetConfirm] = useState(false)
  const [resetInput, setResetInput] = useState('')
  const [toast, setToast] = useState<{ type: 'success' | 'error'; message: string } | null>(null)

  const showToast = useCallback((type: 'success' | 'error', message: string) => {
    setToast({ type, message })
    setTimeout(() => setToast(null), 3000)
  }, [])

  const handleClearCache = async () => {
    try {
      const result = await clearCache.mutateAsync()
      showToast('success', `Cache cleared (${formatBytes(result.clearedBytes)})`)
    } catch (e) {
      showToast('error', `Clear cache failed: ${e instanceof Error ? e.message : 'Unknown error'}`)
    }
  }

  const handleReset = async () => {
    if (resetInput !== 'RESET_ALL_DATA') return
    try {
      await reset.mutateAsync('RESET_ALL_DATA')
      showToast('success', 'All data has been reset')
      setShowResetConfirm(false)
      setResetInput('')
    } catch (e) {
      showToast('error', `Reset failed: ${e instanceof Error ? e.message : 'Unknown error'}`)
    }
  }

  return (
    <div>
      {/* Toast notification */}
      {toast && (
        <div
          className={cn(
            'flex items-center gap-2 px-3 py-2 rounded-md text-sm mb-4',
            toast.type === 'success'
              ? 'bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-300'
              : 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300'
          )}
        >
          {toast.type === 'success' ? (
            <CheckCircle2 className="w-4 h-4 flex-shrink-0" />
          ) : (
            <XCircle className="w-4 h-4 flex-shrink-0" />
          )}
          <span>{toast.message}</span>
        </div>
      )}

      <div className="space-y-4">
        {/* Clear Cache */}
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">Clear Cache</p>
            <p className="text-xs text-gray-500 dark:text-gray-400">
              Remove cached search index data. Will be rebuilt on next index.
            </p>
          </div>
          <button
            type="button"
            onClick={handleClearCache}
            disabled={clearCache.isPending}
            className={cn(
              'inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md cursor-pointer',
              'transition-colors duration-150',
              'border border-gray-200 dark:border-gray-700',
              'text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2'
            )}
          >
            {clearCache.isPending ? (
              <Loader2 className="w-4 h-4 animate-spin" />
            ) : (
              <Trash2 className="w-4 h-4" />
            )}
            Clear Cache
          </button>
        </div>

        {/* Reset All Data */}
        <div className="border-t border-gray-100 dark:border-gray-800 pt-4">
          {!showResetConfirm ? (
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm font-medium text-red-700 dark:text-red-400">Reset All Data</p>
                <p className="text-xs text-gray-500 dark:text-gray-400">
                  Permanently delete all session metadata, indexes, and classification data.
                </p>
              </div>
              <button
                type="button"
                onClick={() => setShowResetConfirm(true)}
                className="inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-red-600 dark:text-red-400 border border-red-200 dark:border-red-800 rounded-md hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors cursor-pointer"
              >
                <AlertTriangle className="w-4 h-4" />
                Reset...
              </button>
            </div>
          ) : (
            <div className="bg-red-50 dark:bg-red-900/10 border border-red-200 dark:border-red-800 rounded-lg p-4">
              <div className="flex items-start gap-3 mb-3">
                <AlertTriangle className="w-5 h-5 text-red-500 flex-shrink-0 mt-0.5" />
                <div>
                  <p className="text-sm font-medium text-red-800 dark:text-red-200 mb-1">
                    This action cannot be undone.
                  </p>
                  <p className="text-sm text-red-600 dark:text-red-300">
                    This will permanently delete all session metadata, indexes, commit correlations,
                    and classification data. Your original JSONL files will NOT be deleted.
                  </p>
                </div>
              </div>
              <div className="mb-3">
                <label
                  htmlFor="reset-confirm"
                  className="text-sm text-red-700 dark:text-red-300 mb-1 block"
                >
                  Type <code className="font-mono bg-red-100 dark:bg-red-900/30 px-1 rounded">RESET_ALL_DATA</code> to confirm:
                </label>
                <input
                  id="reset-confirm"
                  type="text"
                  value={resetInput}
                  onChange={(e) => setResetInput(e.target.value)}
                  className="w-full text-sm border border-red-200 dark:border-red-700 rounded px-3 py-1.5 bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100 focus:ring-2 focus:ring-red-400 focus:outline-none"
                  placeholder="RESET_ALL_DATA"
                />
              </div>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => {
                    setShowResetConfirm(false)
                    setResetInput('')
                  }}
                  className="px-3 py-1.5 text-sm font-medium text-gray-700 dark:text-gray-300 border border-gray-200 dark:border-gray-700 rounded-md hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors cursor-pointer"
                >
                  Cancel
                </button>
                <button
                  type="button"
                  onClick={handleReset}
                  disabled={resetInput !== 'RESET_ALL_DATA' || reset.isPending}
                  className="inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-white bg-red-600 rounded-md hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors cursor-pointer"
                >
                  {reset.isPending ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <AlertTriangle className="w-4 h-4" />
                  )}
                  Reset All
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
```

**Step 2: Verify TypeScript compiles**

```bash
bunx tsc --noEmit --pretty 2>&1 | head -20
```

**Step 3: Commit**

```bash
git add src/components/DangerZone.tsx
git commit -m "feat(settings): add DangerZone component with clear cache and reset"
```

---

### Task 5: Integrate new components into SettingsPage

Add the 4 new sections to SettingsPage in the correct order, using the `useSystemStatus` hook.

**Files:**
- Modify: `src/components/SettingsPage.tsx`

**Step 1: Add imports and hook call**

Add to top of file after existing imports:
```tsx
import { History, Terminal, AlertTriangle, HeartPulse, CheckCircle2, AlertCircle as AlertCircleIcon, XCircle as XCircleIcon } from 'lucide-react'
import { useSystemStatus } from '../hooks/use-system-status'
import { IndexHistory } from './IndexHistory'
import { CliStatus } from './CliStatus'
import { DangerZone } from './DangerZone'
```

Add inside `SettingsPage()` component, after existing hooks:
```tsx
const { health, indexHistory, claudeCli, isLoading: systemLoading } = useSystemStatus()
```

**Step 2: Add Health badge to Data & Storage section header**

Modify the Storage section to show a health status indicator in the section header. Change the `SettingsSection` for storage from:
```tsx
<SettingsSection icon={<HardDrive className="w-4 h-4" />} title="Data & Storage">
```
to use a custom header that includes the health badge. The cleanest approach: add an optional `badge` prop to `SettingsSection`:

```tsx
interface SettingsSectionProps {
  icon: React.ReactNode
  title: string
  badge?: React.ReactNode
  children: React.ReactNode
}

function SettingsSection({ icon, title, badge, children }: SettingsSectionProps) {
  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
      <div className="flex items-center gap-2 px-4 py-3 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
        <span className="text-gray-500 dark:text-gray-400">{icon}</span>
        <h2 className="text-sm font-semibold text-gray-700 dark:text-gray-300 uppercase tracking-wide">
          {title}
        </h2>
        {badge && <span className="ml-auto">{badge}</span>}
      </div>
      <div className="p-4">{children}</div>
    </div>
  )
}
```

Then render the health badge:
```tsx
const healthBadge = health ? (
  <span className={cn(
    'inline-flex items-center gap-1 text-xs font-medium',
    health.status === 'healthy' ? 'text-green-600 dark:text-green-400' :
    health.status === 'warning' ? 'text-amber-600 dark:text-amber-400' :
    'text-red-600 dark:text-red-400'
  )}>
    {health.status === 'healthy' ? <CheckCircle2 className="w-3.5 h-3.5" /> :
     health.status === 'warning' ? <AlertCircleIcon className="w-3.5 h-3.5" /> :
     <XCircleIcon className="w-3.5 h-3.5" />}
    {health.status === 'healthy' ? 'Healthy' :
     health.status === 'warning' ? `${health.errorsCount} error${health.errorsCount !== 1 ? 's' : ''}` :
     'Needs attention'}
  </span>
) : null

// In JSX:
<SettingsSection
  icon={<HardDrive className="w-4 h-4" />}
  title="Data & Storage"
  badge={healthBadge}
>
  <StorageOverview />
</SettingsSection>
```

**Step 3: Add Index History section after Data & Storage**

```tsx
{/* INDEX HISTORY */}
<SettingsSection icon={<History className="w-4 h-4" />} title="Index History">
  <IndexHistory history={indexHistory} isLoading={systemLoading} />
</SettingsSection>
```

**Step 4: Add CLI Status section before About**

```tsx
{/* CLAUDE CLI */}
<SettingsSection icon={<Terminal className="w-4 h-4" />} title="Claude CLI">
  <CliStatus cli={claudeCli} isLoading={systemLoading} />
</SettingsSection>
```

**Step 5: Add Danger Zone section at very bottom (after About)**

```tsx
{/* DANGER ZONE */}
<SettingsSection icon={<AlertTriangle className="w-4 h-4 text-red-500" />} title="Danger Zone">
  <DangerZone />
</SettingsSection>
```

**Final section order in SettingsPage:**
1. Data & Storage (with health badge) — existing
2. Index History — **NEW**
3. Classification — existing
4. Classification Provider — existing (conditional)
5. Git Sync — existing
6. Export Data — existing
7. Claude CLI — **NEW**
8. About — existing
9. Danger Zone — **NEW**

**Step 6: Verify TypeScript compiles and visually inspect**

```bash
bunx tsc --noEmit --pretty 2>&1 | head -20
```

Then open `http://localhost:5173/settings` and verify:
- Health badge appears on Data & Storage header
- Index History table renders below storage
- CLI Status shows install/auth info
- Danger Zone at bottom with Clear Cache and Reset

**Step 7: Commit**

```bash
git add src/components/SettingsPage.tsx
git commit -m "feat(settings): integrate health badge, index history, CLI status, and danger zone"
```

---

### Task 6: Update sidebar navigation — System → Settings

Replace the "System" nav item with "Settings", change the icon from `<Server>` to `<Settings>`.

**Files:**
- Modify: `src/components/Sidebar.tsx`

**Step 1: Replace import**

Change:
```tsx
import { ..., Server, Lightbulb } from 'lucide-react'
```
to:
```tsx
import { ..., Settings, Lightbulb } from 'lucide-react'
```

(Remove `Server` from imports, add `Settings` — but check if `Settings` might conflict with another import. If so, import as `SettingsIcon`.)

**Step 2: Replace nav link**

Change the System link (lines ~411-422) from:
```tsx
<Link
  to={`/system${paramString ? `?${paramString}` : ""}`}
  className={cn(
    '...',
    location.pathname === '/system' ? '...' : '...'
  )}
>
  <Server className="w-4 h-4" />
  <span className="font-medium">System</span>
</Link>
```

to:
```tsx
<Link
  to={`/settings${paramString ? `?${paramString}` : ""}`}
  className={cn(
    '...',
    location.pathname === '/settings' ? '...' : '...'
  )}
>
  <Settings className="w-4 h-4" />
  <span className="font-medium">Settings</span>
</Link>
```

**Step 3: Verify navigation works**

Open `http://localhost:5173` and verify:
- Sidebar shows "Settings" with gear icon (not "System" with server icon)
- Clicking "Settings" navigates to `/settings`
- Active state highlights correctly on `/settings`
- Header gear icon still works (both point to same page)

**Step 4: Commit**

```bash
git add src/components/Sidebar.tsx
git commit -m "feat(nav): replace System nav item with Settings in sidebar"
```

---

### Task 7: Add /system → /settings redirect

Keep backward compatibility for anyone who bookmarked `/system`.

**Files:**
- Modify: `src/router.tsx`

**Step 1: Replace SystemPage route with redirect**

Change:
```tsx
import { SystemPage } from './components/SystemPage'
// ...
{ path: 'system', element: <SystemPage /> },
```

to:
```tsx
// Remove: import { SystemPage } from './components/SystemPage'
// ...
{ path: 'system', element: <Navigate to="/settings" replace /> },
```

(`Navigate` is already imported from `react-router-dom` at top of file.)

**Step 2: Verify redirect works**

Open `http://localhost:5173/system` — should redirect to `/settings`.
Open `http://localhost:5173/settings` — should render Settings page.

**Step 3: Commit**

```bash
git add src/router.tsx
git commit -m "feat(routing): redirect /system to /settings"
```

---

### Task 8: Clean up — remove SystemPage and unused code

Remove the now-unused SystemPage component and its dedicated hook.

**Files:**
- Delete: `src/components/SystemPage.tsx`
- Delete: `src/hooks/use-system.ts` — **WAIT, check if other files import from it**

Before deleting `use-system.ts`, check what imports it:
```bash
grep -r "use-system" src/ --include="*.ts" --include="*.tsx"
```

The new components (`DangerZone.tsx`) import `useClearCache`, `useReset`, `formatBytes` from `use-system`. And `IndexHistory.tsx` imports `formatRelativeTimestamp`, `formatDuration` from `use-system`. So we need to **keep `use-system.ts`** but can delete the `useSystem` main hook (since `use-system-status.ts` replaces it) and the SystemPage-specific type re-exports.

Actually, since the formatters and mutation hooks in `use-system.ts` are still needed, the cleanest approach is:
- Keep `use-system.ts` as-is (the `useSystem` hook just won't be called anymore — dead code is fine for now)
- Delete only `SystemPage.tsx`

**Step 1: Delete SystemPage**

```bash
git rm src/components/SystemPage.tsx
```

**Step 2: Remove SystemPage import from router**

Verify the router no longer imports `SystemPage` (should already be done in Task 7).

**Step 3: Verify the app compiles and runs**

```bash
bunx tsc --noEmit --pretty 2>&1 | head -20
```

**Step 4: Commit**

```bash
git add -A
git commit -m "chore: remove SystemPage (merged into Settings)"
```

---

### Task 9: Widen SettingsPage max-width

The existing SettingsPage uses `max-w-2xl` (672px). With the new Index History table (5 columns), this is too narrow. The SystemPage used `max-w-4xl` (896px). Bump to `max-w-3xl` (768px) as a compromise — wide enough for the table, not so wide that single-column sections look sparse.

**Files:**
- Modify: `src/components/SettingsPage.tsx`

**Step 1: Change max-width**

```tsx
// Change:
<div className="max-w-2xl mx-auto px-6 py-6">
// To:
<div className="max-w-3xl mx-auto px-6 py-6">
```

**Step 2: Visual check**

Open `/settings` — table should have room, other sections shouldn't look too wide.

**Step 3: Commit**

```bash
git add src/components/SettingsPage.tsx
git commit -m "style(settings): widen max-width for index history table"
```

---

### Task 10: Final verification and visual QA

**Step 1: Full TypeScript check**

```bash
bunx tsc --noEmit --pretty
```
Expected: 0 errors

**Step 2: Run relevant tests**

```bash
cargo test -p vibe-recall-server -- routes::system
```
Expected: All system route tests pass (backend unchanged)

**Step 3: Visual QA checklist**

Open `http://localhost:5173/settings` and verify:

- [ ] Health badge shows on Data & Storage header (green/amber/red)
- [ ] Donut chart still renders correctly
- [ ] Stats grid still renders correctly
- [ ] Index History table shows runs with type badges, session counts, durations, status icons
- [ ] "Show All" button appears when >5 runs
- [ ] Classification section unchanged (start/cancel/progress)
- [ ] Git Sync section unchanged (interval picker + manual sync)
- [ ] Export section unchanged (format + scope radio)
- [ ] Claude CLI section shows install path, version, auth status
- [ ] About section unchanged (version + keyboard shortcuts)
- [ ] Danger Zone: Clear Cache button works, shows toast
- [ ] Danger Zone: Reset requires typing RESET_ALL_DATA
- [ ] Danger Zone: Cancel closes confirmation
- [ ] Dark mode: all new sections have proper dark variants
- [ ] `/system` redirects to `/settings`
- [ ] Sidebar "Settings" link highlights correctly
- [ ] Header gear icon still navigates to `/settings`
- [ ] No console errors
- [ ] No layout shift on load (loading states render at correct height)

**Step 4: Final commit if any fixes needed**

---

## Summary

| Task | Description | Files | Est. Lines |
|------|-------------|-------|------------|
| 1 | useSystemStatus hook | 1 new | ~30 |
| 2 | IndexHistory component | 1 new | ~100 |
| 3 | CliStatus component | 1 new | ~80 |
| 4 | DangerZone component | 1 new | ~160 |
| 5 | Integrate into SettingsPage | 1 modify | ~40 added |
| 6 | Sidebar nav update | 1 modify | ~5 changed |
| 7 | Router redirect | 1 modify | ~3 changed |
| 8 | Delete SystemPage | 1 delete | -809 |
| 9 | Widen max-width | 1 modify | ~1 changed |
| 10 | Final verification | 0 | 0 |

**Net result:** -400 lines (809 deleted, ~410 added in cleaner componentized form). One page, one navigation entry, zero data duplication.
