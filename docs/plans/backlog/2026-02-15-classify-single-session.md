---
status: approved
date: 2026-02-15
audit-score: 100
audit-rounds: 2
---

# Single-Session Classification UX Redesign

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let users classify one session at a time from the session list, building trust before committing to bulk classification.

**Architecture:** Add a synchronous `POST /api/classify/single/:session_id` endpoint that bypasses the bulk ClassifyState. On the frontend, replace empty WorkTypeBadge slots in session cards with a "Classify" button that morphs into a CategoryBadge on completion. After 3 single classifications, surface an inline "Classify All" banner with cost estimate.

**Tech Stack:** Rust/Axum (backend), React/TypeScript (frontend), React Query (cache), Tailwind CSS (styling)

---

## Task 1: Backend — Single-Session Classify Endpoint

**Files:**
- Modify: `crates/server/src/routes/classify.rs`
- Modify: `crates/db/src/queries/classification.rs`

### Step 0: Add dedicated single-session DB query

In `crates/db/src/queries/classification.rs`, add after `get_all_sessions_for_classification` (line ~267):

```rust
/// Get a single session's data for classification.
/// Returns (id, preview, skills_used) or None if not found.
pub async fn get_session_for_classification(
    &self,
    session_id: &str,
) -> DbResult<Option<(String, String, String)>> {
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT id, preview, skills_used FROM sessions WHERE id = ?1",
    )
    .bind(session_id)
    .fetch_optional(self.pool())
    .await?;
    Ok(row)
}

/// Check if a session is already classified. Returns (l1, l2, l3, confidence) if so.
pub async fn get_session_classification(
    &self,
    session_id: &str,
) -> DbResult<Option<(String, String, String, f64)>> {
    let row: Option<(String, String, String, f64)> = sqlx::query_as(
        "SELECT category_l1, category_l2, category_l3, category_confidence FROM sessions WHERE id = ?1 AND category_l1 IS NOT NULL",
    )
    .bind(session_id)
    .fetch_optional(self.pool())
    .await?;
    Ok(row)
}
```

Run: `cargo test -p claude-view-db`

Expected: Compiles, existing tests pass.

### Step 1: Add response type after `ClassifyErrorInfo` (line 135)

```rust
/// Response for POST /api/classify/single/:session_id.
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ClassifySingleResponse {
    pub session_id: String,
    pub category_l1: String,
    pub category_l2: String,
    pub category_l3: String,
    pub confidence: f64,
    /// true if result was already cached (previously classified)
    pub was_cached: bool,
}
```

### Step 2: Add handler function before `run_classification` (line 423)

```rust
/// POST /api/classify/single/:session_id — Classify a single session synchronously.
///
/// Bypasses ClassifyState entirely — no job record, no SSE.
/// Returns the classification result directly.
/// Uses dedicated O(1) DB queries — NOT the bulk session list.
async fn classify_single_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // 1. Check if already classified (O(1) query)
    if let Some((l1, l2, l3, conf)) = state.db.get_session_classification(&session_id).await? {
        return Ok((
            StatusCode::OK,
            Json(ClassifySingleResponse {
                session_id,
                category_l1: l1,
                category_l2: l2,
                category_l3: l3,
                confidence: conf,
                was_cached: true,
            }),
        ));
    }

    // 2. Fetch session data for classification (O(1) query)
    let (_, preview, skills_json) = state
        .db
        .get_session_for_classification(&session_id)
        .await?
        .ok_or_else(|| ApiError::SessionNotFound(session_id.clone()))?;

    // 3. Parse skills
    let skills: Vec<String> = serde_json::from_str(&skills_json).unwrap_or_default();

    // 4. Classify via Claude CLI
    let provider =
        claude_view_core::llm::ClaudeCliProvider::new("haiku").with_timeout(60);
    let request = ClassificationRequest {
        session_id: session_id.clone(),
        first_prompt: preview,
        files_touched: vec![],
        skills_used: skills,
    };

    let resp = provider.classify(request).await.map_err(|e| {
        ApiError::Internal(format!("Classification failed: {e}"))
    })?;

    // 5. Persist to DB
    state
        .db
        .update_session_classification(
            &session_id,
            &resp.category_l1,
            &resp.category_l2,
            &resp.category_l3,
            resp.confidence,
            "claude-cli",
        )
        .await?;

    Ok((
        StatusCode::OK,
        Json(ClassifySingleResponse {
            session_id,
            category_l1: resp.category_l1,
            category_l2: resp.category_l2,
            category_l3: resp.category_l3,
            confidence: resp.confidence,
            was_cached: false,
        }),
    ))
}
```

**Note on imports:** Add to the top of classify.rs (after line 14):

```rust
use axum::extract::Path;
```

This import is NOT currently in the file. The `Path` extractor is needed for `{session_id}`.

### Step 3: Add route to router (line 619)

Change the router function from:

```rust
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/classify", post(start_classification))
        .route("/classify/status", get(get_classification_status))
        .route("/classify/stream", get(stream_classification))
        .route("/classify/cancel", post(cancel_classification))
}
```

To:

```rust
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/classify", post(start_classification))
        .route("/classify/single/{session_id}", post(classify_single_session))
        .route("/classify/status", get(get_classification_status))
        .route("/classify/stream", get(stream_classification))
        .route("/classify/cancel", post(cancel_classification))
}
```

**Note:** This project uses Axum 0.8 (confirmed in workspace `Cargo.toml`), which uses `{param}` syntax (not `:param`).

### Step 4: Add tests

Add these tests inside the existing `#[cfg(test)] mod tests` block (after line 789):

```rust
#[test]
fn test_classify_single_response_serialize() {
    let resp = ClassifySingleResponse {
        session_id: "sess-123".to_string(),
        category_l1: "code_work".to_string(),
        category_l2: "feature".to_string(),
        category_l3: "new-component".to_string(),
        confidence: 0.92,
        was_cached: false,
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"sessionId\":\"sess-123\""));
    assert!(json.contains("\"categoryL1\":\"code_work\""));
    assert!(json.contains("\"wasCached\":false"));
}

#[tokio::test]
async fn test_classify_single_session_not_found() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    use claude_view_db::Database;

    let db = Database::new_in_memory().await.unwrap();
    let state = AppState::new(db);

    let app = Router::new()
        .nest("/api", router())
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/classify/single/nonexistent-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 404 because session doesn't exist
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
```

### Step 5: Run tests

Run: `cargo test -p claude-view-server -- routes::classify`

Expected: All existing tests pass + 2 new tests pass.

### Step 6: Generate TypeScript types

Run: `cargo test -p claude-view-server -- --ignored export_bindings 2>/dev/null || true`

Then verify the generated file exists:

```bash
ls src/types/generated/ClassifySingleResponse.ts
```

Expected: File contains `export type ClassifySingleResponse = { sessionId: string, categoryL1: string, ... }`

Then add the type to the barrel export in `src/types/generated/index.ts`. Find the classification section and add:

```ts
export type { ClassifySingleResponse } from './ClassifySingleResponse'
```

### Step 7: Commit

```bash
git add crates/server/src/routes/classify.rs crates/db/src/queries/classification.rs src/types/generated/ClassifySingleResponse.ts src/types/generated/index.ts
git commit -m "feat: add POST /api/classify/single/:session_id endpoint

Synchronous single-session classification that bypasses bulk ClassifyState.
Returns cached result if already classified, otherwise calls Claude Haiku."
```

---

## Task 2: Frontend — CategoryBadge Component

**Files:**
- Create: `src/lib/category-utils.ts`
- Create: `src/components/CategoryBadge.tsx`

### Step 1: Create category config utilities

Create `src/lib/category-utils.ts`:

```ts
/**
 * AI classification category utilities.
 *
 * Categories come from Claude Haiku classification (L1/L2/L3).
 * Separate from WorkType (rule-based, Theme 3).
 */

export interface CategoryConfig {
  label: string
  bgColor: string
  textColor: string
  borderColor: string
  icon: string // lucide icon name
}

/**
 * L2 category display config.
 * L2 is the most useful granularity for badges.
 */
export const CATEGORY_L2_CONFIG: Record<string, CategoryConfig> = {
  // code_work children
  feature: {
    label: 'Feature',
    bgColor: 'bg-blue-50 dark:bg-blue-950/30',
    textColor: 'text-blue-700 dark:text-blue-400',
    borderColor: 'border-blue-200 dark:border-blue-800',
    icon: 'Plus',
  },
  bugfix: {
    label: 'Bug Fix',
    bgColor: 'bg-red-50 dark:bg-red-950/30',
    textColor: 'text-red-700 dark:text-red-400',
    borderColor: 'border-red-200 dark:border-red-800',
    icon: 'Bug',
  },
  refactor: {
    label: 'Refactor',
    bgColor: 'bg-orange-50 dark:bg-orange-950/30',
    textColor: 'text-orange-700 dark:text-orange-400',
    borderColor: 'border-orange-200 dark:border-orange-800',
    icon: 'RefreshCw',
  },
  testing: {
    label: 'Testing',
    bgColor: 'bg-green-50 dark:bg-green-950/30',
    textColor: 'text-green-700 dark:text-green-400',
    borderColor: 'border-green-200 dark:border-green-800',
    icon: 'FlaskConical',
  },
  // support_work children
  docs: {
    label: 'Docs',
    bgColor: 'bg-cyan-50 dark:bg-cyan-950/30',
    textColor: 'text-cyan-700 dark:text-cyan-400',
    borderColor: 'border-cyan-200 dark:border-cyan-800',
    icon: 'FileText',
  },
  config: {
    label: 'Config',
    bgColor: 'bg-gray-50 dark:bg-gray-800',
    textColor: 'text-gray-600 dark:text-gray-400',
    borderColor: 'border-gray-200 dark:border-gray-700',
    icon: 'Settings',
  },
  ops: {
    label: 'Ops',
    bgColor: 'bg-indigo-50 dark:bg-indigo-950/30',
    textColor: 'text-indigo-700 dark:text-indigo-400',
    borderColor: 'border-indigo-200 dark:border-indigo-800',
    icon: 'Server',
  },
  // thinking_work children
  planning: {
    label: 'Planning',
    bgColor: 'bg-purple-50 dark:bg-purple-950/30',
    textColor: 'text-purple-700 dark:text-purple-400',
    borderColor: 'border-purple-200 dark:border-purple-800',
    icon: 'ClipboardList',
  },
  explanation: {
    label: 'Learning',
    bgColor: 'bg-amber-50 dark:bg-amber-950/30',
    textColor: 'text-amber-700 dark:text-amber-400',
    borderColor: 'border-amber-200 dark:border-amber-800',
    icon: 'Lightbulb',
  },
  architecture: {
    label: 'Architecture',
    bgColor: 'bg-violet-50 dark:bg-violet-950/30',
    textColor: 'text-violet-700 dark:text-violet-400',
    borderColor: 'border-violet-200 dark:border-violet-800',
    icon: 'Blocks',
  },
}

const DEFAULT_CONFIG: CategoryConfig = {
  label: 'Other',
  bgColor: 'bg-gray-50 dark:bg-gray-800',
  textColor: 'text-gray-600 dark:text-gray-400',
  borderColor: 'border-gray-200 dark:border-gray-700',
  icon: 'Tag',
}

export function getCategoryConfig(l2: string | null | undefined): CategoryConfig {
  if (!l2) return DEFAULT_CONFIG
  return CATEGORY_L2_CONFIG[l2] || DEFAULT_CONFIG
}
```

### Step 2: Create CategoryBadge component

Create `src/components/CategoryBadge.tsx`:

```tsx
import {
  Plus, Bug, RefreshCw, FlaskConical,
  FileText, Settings, Server,
  ClipboardList, Lightbulb, Blocks, Tag,
} from 'lucide-react'
import { cn } from '../lib/utils'
import { getCategoryConfig } from '../lib/category-utils'

const ICON_MAP: Record<string, React.ComponentType<{ className?: string }>> = {
  Plus, Bug, RefreshCw, FlaskConical,
  FileText, Settings, Server,
  ClipboardList, Lightbulb, Blocks, Tag,
}

interface CategoryBadgeProps {
  l1?: string | null
  l2?: string | null
  l3?: string | null
  className?: string
}

/**
 * Renders an AI classification category badge from L1/L2/L3 fields.
 * Displays the L2 category (most useful granularity) with icon.
 */
export function CategoryBadge({ l2, className }: CategoryBadgeProps) {
  if (!l2) return null

  const config = getCategoryConfig(l2)
  const Icon = ICON_MAP[config.icon] || Tag

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 px-1.5 py-0.5 text-xs font-medium rounded border',
        config.bgColor,
        config.textColor,
        config.borderColor,
        className,
      )}
      title={`AI classified: ${config.label}`}
    >
      <Icon className="w-3 h-3" />
      <span>{config.label}</span>
    </span>
  )
}
```

### Step 3: Commit

```bash
git add src/lib/category-utils.ts src/components/CategoryBadge.tsx
git commit -m "feat: add CategoryBadge component for AI classification display

10 L2 categories with distinct colors/icons.
Separate from WorkTypeBadge (rule-based Theme 3)."
```

---

## Task 3: Frontend — useClassifySingle Hook

**Files:**
- Create: `src/hooks/use-classify-single.ts`

### Step 1: Create the hook

Create `src/hooks/use-classify-single.ts`:

```ts
import { useState, useCallback } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import type { ClassifySingleResponse } from '../types/generated/ClassifySingleResponse'

export interface UseClassifySingleResult {
  /** ID of the session currently being classified, or null */
  classifyingId: string | null
  /** Classify a single session. Returns the result or null on error. */
  classifySession: (sessionId: string) => Promise<ClassifySingleResponse | null>
  /** Last error message, or null */
  error: string | null
}

/**
 * Hook for classifying a single session via POST /api/classify/single/:id.
 *
 * Lightweight — no SSE, no job tracking. Just request→response.
 * Optimistically updates the React Query cache so the badge appears instantly.
 */
export function useClassifySingle(): UseClassifySingleResult {
  const queryClient = useQueryClient()
  const [classifyingId, setClassifyingId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const classifySession = useCallback(
    async (sessionId: string): Promise<ClassifySingleResponse | null> => {
      setClassifyingId(sessionId)
      setError(null)

      try {
        const res = await fetch(`/api/classify/single/${encodeURIComponent(sessionId)}`, {
          method: 'POST',
        })

        if (!res.ok) {
          const errData = await res.json().catch(() => ({ error: 'Unknown error' }))
          const msg = errData.details || errData.error || `Failed: ${res.status}`
          setError(msg)
          return null
        }

        const data: ClassifySingleResponse = await res.json()

        // Optimistically update session in React Query cache.
        // The server already persisted the result, so this IS the truth —
        // no need for invalidateQueries (which would cause a redundant refetch + flicker).
        queryClient.setQueriesData<{ sessions: Array<Record<string, unknown>> }>(
          { queryKey: ['project-sessions'] },
          (old) => {
            if (!old?.sessions) return old
            return {
              ...old,
              sessions: old.sessions.map((s) =>
                s.id === sessionId
                  ? {
                      ...s,
                      categoryL1: data.categoryL1,
                      categoryL2: data.categoryL2,
                      categoryL3: data.categoryL3,
                      categoryConfidence: data.confidence,
                      categorySource: 'claude-cli',
                      classifiedAt: new Date().toISOString(),
                    }
                  : s,
              ),
            }
          },
        )

        // Track classify count and notify banner via CustomEvent (instant, same-tab)
        const countKey = 'classify-single-count'
        const prev = parseInt(localStorage.getItem(countKey) || '0', 10)
        const newCount = prev + 1
        localStorage.setItem(countKey, String(newCount))
        window.dispatchEvent(new CustomEvent('classify-single-done', { detail: newCount }))

        return data
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Classification failed'
        setError(msg)
        return null
      } finally {
        setClassifyingId(null)
      }
    },
    [queryClient],
  )

  return { classifyingId, classifySession, error }
}
```

### Step 2: Commit

```bash
git add src/hooks/use-classify-single.ts
git commit -m "feat: add useClassifySingle hook for single-session classification

Optimistic cache update + localStorage counter for banner trigger."
```

---

## Task 4: Frontend — SessionCard & CompactSessionTable Integration

**Files:**
- Modify: `src/components/SessionCard.tsx` (lines 326-332)
- Modify: `src/components/CompactSessionTable.tsx` (lines 81-237)

### Step 1: Update SessionCard footer

In `src/components/SessionCard.tsx`, add imports at the top (after line 5):

```tsx
import { CategoryBadge } from './CategoryBadge'
```

Then replace lines 329-332:

```tsx
          {/* Work type badge (Theme 3) */}
          {workType && (
            <WorkTypeBadge workType={workType} />
          )}
```

With:

```tsx
          {/* AI category badge (from classification) */}
          {session.categoryL2 ? (
            <CategoryBadge l1={session.categoryL1} l2={session.categoryL2} l3={session.categoryL3} />
          ) : workType ? (
            <WorkTypeBadge workType={workType} />
          ) : null}
```

**Why this order:** AI classification (L2) takes priority over rule-based WorkType. If neither exists, nothing renders — the ClassifyButton is added separately (Task 5's banner handles the "classify" CTA, not individual card buttons, to avoid cluttering every card).

**Decision update:** After further consideration, we should NOT put a Classify button on every single session card. That would add visual noise to hundreds of cards. Instead:
- CategoryBadge shows when classified
- WorkTypeBadge shows as fallback (rule-based)
- The "Classify All" banner (Task 5) is the primary CTA
- Users can also classify individual sessions from the **session detail** header (future enhancement)

### Step 2: Update CompactSessionTable

In `src/components/CompactSessionTable.tsx`, add import at the top:

```tsx
import { CategoryBadge } from './CategoryBadge'
```

Add a new column after the `preview` column definition (after line 139's closing `]),`, before the `userPromptCount` accessor at line 140):

```tsx
    columnHelper.display({
      id: 'category',
      header: 'Type',
      size: 90,
      cell: ({ row }) => {
        const s = row.original
        return (
          <Link to={sessionUrl(s)} className="block">
            {s.categoryL2 ? (
              <CategoryBadge l2={s.categoryL2} />
            ) : (
              <span className="text-[11px] text-gray-300 dark:text-gray-600">--</span>
            )}
          </Link>
        )
      },
    }),
```

### Step 3: Run dev server and verify visually

Run: `bun run dev`

Open browser → Sessions list → verify:
1. Classified sessions show colored CategoryBadge in card footer and table column
2. Unclassified sessions show `--` in table, nothing extra in card footer
3. No visual regressions

### Step 4: Commit

```bash
git add src/components/SessionCard.tsx src/components/CompactSessionTable.tsx
git commit -m "feat: show CategoryBadge in session cards and compact table

AI classification badges in card footer (priority over WorkType) and
new Type column in compact table view."
```

---

## Task 5: Frontend — Classify All Banner + Single Classify Action

**Files:**
- Create: `src/components/ClassifyBanner.tsx`
- Modify: `src/components/HistoryView.tsx`

### Step 1: Create the ClassifyBanner component

Create `src/components/ClassifyBanner.tsx`:

```tsx
import { useState, useEffect } from 'react'
import { Sparkles, X, Loader2 } from 'lucide-react'
import { useClassification } from '../hooks/use-classification'

const CLASSIFY_COUNT_KEY = 'classify-single-count'
const BANNER_DISMISSED_KEY = 'classify-banner-dismissed'
const SHOW_AFTER_COUNT = 3

interface ClassifyBannerProps {
  unclassifiedCount: number
  estimatedCostCents: number
}

/**
 * Inline banner that appears after the user has classified 3+ sessions individually.
 * Prompts them to classify all remaining sessions with a clear cost estimate.
 */
export function ClassifyBanner({ unclassifiedCount, estimatedCostCents }: ClassifyBannerProps) {
  const [dismissed, setDismissed] = useState(() =>
    localStorage.getItem(BANNER_DISMISSED_KEY) === 'true'
  )
  const [singleCount, setSingleCount] = useState(() =>
    parseInt(localStorage.getItem(CLASSIFY_COUNT_KEY) || '0', 10)
  )
  const { startClassification, isLoading } = useClassification()
  const [isStarting, setIsStarting] = useState(false)

  // Listen for classify count changes via CustomEvent (instant, same-tab)
  // and StorageEvent (cross-tab fallback)
  useEffect(() => {
    const handleCustom = (e: Event) => {
      const count = (e as CustomEvent<number>).detail
      setSingleCount(count)
    }
    const handleStorage = (e: StorageEvent) => {
      if (e.key === CLASSIFY_COUNT_KEY && e.newValue) {
        setSingleCount(parseInt(e.newValue, 10))
      }
    }
    window.addEventListener('classify-single-done', handleCustom)
    window.addEventListener('storage', handleStorage)
    return () => {
      window.removeEventListener('classify-single-done', handleCustom)
      window.removeEventListener('storage', handleStorage)
    }
  }, [])

  // Don't show if: dismissed, not enough single classifies, no unclassified sessions
  if (dismissed || singleCount < SHOW_AFTER_COUNT || unclassifiedCount === 0) {
    return null
  }

  const costDisplay = estimatedCostCents < 1
    ? '<$0.01'
    : `~$${(estimatedCostCents / 100).toFixed(2)}`

  const handleClassifyAll = async () => {
    setIsStarting(true)
    await startClassification('unclassified')
    setIsStarting(false)
  }

  const handleDismiss = () => {
    setDismissed(true)
    localStorage.setItem(BANNER_DISMISSED_KEY, 'true')
  }

  return (
    <div className="flex items-center justify-between gap-3 px-4 py-2.5 bg-blue-50 dark:bg-blue-950/30 border border-blue-200 dark:border-blue-800 rounded-lg text-sm">
      <div className="flex items-center gap-2 text-blue-700 dark:text-blue-300">
        <Sparkles className="w-4 h-4 flex-shrink-0" />
        <span>
          <strong>{unclassifiedCount}</strong> sessions unclassified.
          Classify all ({costDisplay}, ~{Math.ceil(unclassifiedCount * 0.4)}s)
        </span>
      </div>
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={handleClassifyAll}
          disabled={isStarting || isLoading}
          className="px-3 py-1 text-xs font-medium text-white bg-blue-600 hover:bg-blue-700 disabled:opacity-50 rounded-md transition-colors"
        >
          {isStarting ? <Loader2 className="w-3 h-3 animate-spin" /> : 'Classify All'}
        </button>
        <button
          type="button"
          onClick={handleDismiss}
          className="p-0.5 text-blue-400 hover:text-blue-600 dark:text-blue-500 dark:hover:text-blue-300"
          aria-label="Dismiss"
        >
          <X className="w-4 h-4" />
        </button>
      </div>
    </div>
  )
}
```

### Step 2: Wire banner into HistoryView

In `src/components/HistoryView.tsx`, add import:

```tsx
import { ClassifyBanner } from './ClassifyBanner'
```

Add a lightweight status fetch (NOT `useClassification` — that adds SSE + polling overhead). Use a simple React Query:

```tsx
const { data: classifyStatus } = useQuery({
  queryKey: ['classify-status'],
  queryFn: async () => {
    const res = await fetch('/api/classify/status')
    if (!res.ok) return null
    return res.json() as Promise<ClassifyStatusResponse>
  },
  staleTime: 30_000, // refresh every 30s, not 3s like useClassification
})
```

Add the import for the type:

```tsx
import type { ClassifyStatusResponse } from '../types/generated'
```

Then insert the banner at line 460 (between the grouping safeguard warning and the session list `<div className="mt-5">`):

```tsx
        {/* Classify All banner — appears after 3 single classifications */}
        {classifyStatus && classifyStatus.unclassifiedSessions > 0 && (
          <div className="mb-4">
            <ClassifyBanner
              unclassifiedCount={classifyStatus.unclassifiedSessions}
              estimatedCostCents={Math.ceil(classifyStatus.unclassifiedSessions * 0.8)}
            />
          </div>
        )}

        {/* Session List or Table */}
        <div className="mt-5">
```

**Why not `useClassification`?** That hook auto-connects SSE and polls every 3 seconds when running. We only need the unclassified count, not real-time progress. A simple `useQuery` with 30s staleTime is sufficient and avoids duplicate SSE connections.

### Step 3: Commit

```bash
git add src/components/ClassifyBanner.tsx src/components/HistoryView.tsx
git commit -m "feat: add Classify All banner after 3 single classifications

Shows unclassified count + cost estimate. Dismissible. Triggers
existing bulk classification flow."
```

---

## Task 6: Frontend — Settings Page Cleanup

**Files:**
- Modify: `src/components/ClassificationStatus.tsx`

### Step 1: Remove Classify/Cancel buttons, keep stats

In `src/components/ClassificationStatus.tsx`, find the Classify/Cancel button section and replace with a read-only message:

Find this pattern (approximately):
```tsx
{isIdle && unclassifiedSessions > 0 ? (
  <button onClick={handleStartClassify} ...>Classify</button>
) : isRunning ? (
  <button onClick={handleCancel} ...>Cancel</button>
) : null}
```

Replace with:
```tsx
{unclassifiedSessions > 0 && (
  <p className="text-sm text-gray-500 dark:text-gray-400">
    Classify sessions from the Sessions list.
  </p>
)}
```

Keep everything else: progress bar, last run info, provider info, error display.

### Step 2: Commit

```bash
git add src/components/ClassificationStatus.tsx
git commit -m "refactor: make Settings classification section read-only

Remove Classify/Cancel buttons. Classification actions now live
in the session list (single classify + Classify All banner)."
```

---

## Task 7: Fix React Query Invalidation in Bulk Classification

**Files:**
- Modify: `src/hooks/use-classification.ts` (lines 123-124)

### Step 1: Fix query keys

In `src/hooks/use-classification.ts`, change lines 123-124 from:

```ts
queryClient.invalidateQueries({ queryKey: ['sessions'] })
queryClient.invalidateQueries({ queryKey: ['stats'] })
```

To:

```ts
queryClient.invalidateQueries({ queryKey: ['project-sessions'] })
queryClient.invalidateQueries({ queryKey: ['stats'] })
queryClient.invalidateQueries({ queryKey: ['facet-badges'] })
```

### Step 2: Run dev server and verify

After a bulk classify completes:
1. Session list should refresh showing new CategoryBadges
2. Facet badges should refresh
3. No stale data

### Step 3: Commit

```bash
git add src/hooks/use-classification.ts
git commit -m "fix: correct React Query invalidation keys after bulk classification

Invalidate 'project-sessions' (actual key) instead of 'sessions' (unused).
Also invalidate 'facet-badges' so quality dots refresh."
```

---

## Verification Checklist

After all tasks are complete, verify end-to-end:

- [ ] `cargo test -p claude-view-server -- routes::classify` — all tests pass
- [ ] `bun run dev` — frontend compiles with no TypeScript errors
- [ ] Open Sessions list → classified sessions show CategoryBadge
- [ ] CompactSessionTable shows "Type" column with badges
- [ ] Settings page shows stats but NO Classify button
- [ ] After classifying 3+ sessions via banner or API, the "Classify All" banner appears
- [ ] Banner shows unclassified count and cost estimate
- [ ] "Classify All" triggers bulk flow with progress modal
- [ ] Banner dismiss persists across page refreshes

## Rollback

If things go wrong:
- All changes are in separate commits — `git revert` any individual commit
- The backend endpoint is additive (new route) — removing it doesn't break existing flows
- The frontend components are new files — deleting them reverts to original behavior
- The only modifications to existing files are in SessionCard footer (3 lines) and CompactSessionTable (1 column) — easy to revert

---

## Changelog of Fixes Applied (Audit → Final Plan)

### Round 1 (Initial audit)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | WorkTypeBadge takes `workType` (contribution metrics), not `categoryL1/L2/L3` | Blocker | Created separate CategoryBadge component using L2 categories |
| 2 | No empty slot in SessionCard footer | Blocker | Changed to conditional: `categoryL2 ? CategoryBadge : workType ? WorkTypeBadge : null` |
| 3 | CompactSessionTable has no Work Type column | Blocker | Added new `category` display column after Preview |
| 4 | ClassifyState blocks concurrent single+bulk | Blocker | Single endpoint skips ClassifyState entirely |
| 5 | No `classifySingleSession` in hook | Warning | Created separate `useClassifySingle` hook |
| 6 | Wrong React Query invalidation keys | Warning | Fixed to `['project-sessions']` + `['facet-badges']` |
| 7 | Progress modal unsuitable for single-session | Warning | Single-session uses inline fetch, no modal |
| 8 | Removing Classify button kills bulk re-classify | Warning | Banner provides bulk classify CTA; settings becomes stats-only |
| 9 | `Path` import missing from classify.rs | Blocker | Added `use axum::extract::Path;` to imports |
| 10 | Axum route syntax version-dependent | Minor | Confirmed Axum 0.8, using `{param}` syntax |

### Round 2 (Adversarial review — score 62/100 → fixes applied)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 11 | O(n) fetch of ALL sessions for single-session lookup (200K rows) | Warning | Added `get_session_for_classification(id)` and `get_session_classification(id)` — O(1) queries |
| 12 | Optimistic update immediately invalidated (causes flicker) | Warning | Removed `invalidateQueries` after `setQueriesData` — optimistic update IS truth |
| 13 | `StorageEvent` doesn't fire in same tab; 2s polling is laggy | Warning | Replaced with `CustomEvent('classify-single-done')` for instant same-tab communication |
| 14 | CompactSessionTable insertion point says "after line 138" | Warning | Fixed to "after line 139's closing `]),`" |
| 15 | HistoryView banner insertion location vague | Warning | Specified exact line 460, between grouping warning and session list div |
| 16 | `useClassification` adds SSE + 3s polling to HistoryView | Warning | Replaced with lightweight `useQuery` (30s staleTime, no SSE) |
| 17 | Barrel export not updated for `ClassifySingleResponse` | Minor | Added `export type { ClassifySingleResponse }` to `index.ts` |
| 18 | Cost estimate is rough (0.8 * count cents) | Minor | Acceptable for MVP — noted as enhancement opportunity |
