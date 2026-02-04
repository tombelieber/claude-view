---
status: pending
date: 2026-02-05
purpose: Theme 4 Phase 8 — Benchmarks Tab with Personal Progress Tracking
---

# Phase 8: Benchmarks Tab

> **Goal:** Enable users to track their AI-assisted development progress over time, compare performance across categories, measure skill adoption impact, and generate monthly summary reports.
>
> **Estimated effort:** 3-4 days
>
> **Dependencies:** Phase 5 (Insights Core — page layout, routing, time range filter)

---

## Overview

Phase 8 adds the Benchmarks tab to the `/insights` page with four major sections:

1. **Then vs Now** — Compare first month vs last month metrics
2. **Category Performance** — Table showing each category vs user's overall average
3. **Skill Adoption Impact** — Track which skills improved outcomes with learning curves
4. **Monthly Report Generator** — Generate downloadable summary reports (PDF)

This phase focuses on personal progress tracking — helping users answer "Am I getting better at AI-assisted development?"

---

## Tasks

### 8.1 GET /api/insights/benchmarks Endpoint

Create the backend endpoint that calculates all benchmark metrics.

**Location:** `crates/server/src/routes/insights.rs`

**Request:**

```
GET /api/insights/benchmarks?range=all|30d|90d|1y
```

**Response:**

```typescript
interface BenchmarksResponse {
  progress: {
    firstMonth: Metrics;
    lastMonth: Metrics;
    improvement: {
      reeditRate: number;      // % change (negative = improvement)
      editsPerFile: number;    // % change
      promptsPerTask: number;  // % change (negative = improvement)
      commitRate: number;      // % change (positive = improvement)
    };
    insight: string;           // Human-readable summary
  };

  byCategory: Array<{
    category: string;          // L1 category: 'code_work', 'support_work', 'thinking_work'
    reeditRate: number;        // 0.0-1.0
    vsAverage: number;         // Difference from user's overall average (-0.2 = 20% better)
    verdict: 'excellent' | 'good' | 'average' | 'needs_work';
    insight: string;           // Category-specific tip
  }>;

  userAverageReeditRate: number; // User's overall average re-edit rate (for centering the comparison bar)

  skillAdoption: Array<{
    skill: string;             // Skill name (e.g., 'tdd', 'commit')
    adoptedAt: string;         // ISO date of first use
    sessionCount: number;      // Total sessions using this skill
    impactOnReedit: number;    // % improvement in re-edit rate after adoption
    learningCurve: Array<{
      session: number;         // 1-indexed session number
      reeditRate: number;      // Re-edit rate for that session
    }>;
  }>;

  reportSummary: {
    month: string;             // "January 2026"
    sessionCount: number;
    linesAdded: number;
    linesRemoved: number;
    commitCount: number;
    estimatedCost: number;     // In dollars
    topWins: string[];         // Top 3 achievements
    focusAreas: string[];      // Areas needing improvement
  };
}

interface Metrics {
  reeditRate: number;          // 0.0-1.0 (files re-edited / files edited)
  editsPerFile: number;        // Average edit operations per file
  promptsPerTask: number;      // Average user prompts per session
  commitRate: number;          // % of sessions that resulted in commits
}
```

**Calculation Logic:**

```rust
// crates/db/src/queries.rs

/// Get metrics for a specific time period.
pub async fn get_period_metrics(&self, start: i64, end: i64) -> Result<PeriodMetrics, DbError> {
    let row = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as session_count,
            COALESCE(AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)), 0.0) as avg_reedit_rate,
            COALESCE(AVG(CAST(edit_count AS REAL) / NULLIF(files_edited_count, 0)), 0.0) as avg_edits_per_file,
            COALESCE(AVG(user_prompt_count), 0.0) as avg_prompts_per_task,
            COALESCE(AVG(CASE WHEN commit_count > 0 THEN 1.0 ELSE 0.0 END), 0.0) as commit_rate
        FROM sessions
        WHERE modified_at >= ?1 AND modified_at < ?2
        "#,
        start,
        end
    )
    .fetch_one(&self.pool)
    .await?;

    Ok(PeriodMetrics {
        reedit_rate: row.avg_reedit_rate.unwrap_or(0.0),
        edits_per_file: row.avg_edits_per_file.unwrap_or(0.0),
        prompts_per_task: row.avg_prompts_per_task.unwrap_or(0.0),
        commit_rate: row.commit_rate.unwrap_or(0.0),
    })
}

/// Get first month metrics (first 30 days of data).
pub async fn get_first_month_metrics(&self) -> Result<PeriodMetrics, DbError> {
    // Find earliest session timestamp
    let earliest = sqlx::query_scalar!(
        "SELECT MIN(modified_at) FROM sessions"
    )
    .fetch_one(&self.pool)
    .await?
    .unwrap_or(0);

    let thirty_days = 30 * 24 * 60 * 60; // 30 days in seconds
    self.get_period_metrics(earliest, earliest + thirty_days).await
}

/// Get last month metrics (most recent 30 days).
pub async fn get_last_month_metrics(&self) -> Result<PeriodMetrics, DbError> {
    let now = chrono::Utc::now().timestamp();
    let thirty_days = 30 * 24 * 60 * 60;
    self.get_period_metrics(now - thirty_days, now).await
}

/// Get category performance breakdown.
/// `from` and `to` are Unix timestamps bounding the query window.
pub async fn get_category_performance(&self, from: i64, to: i64) -> Result<Vec<CategoryPerformance>, DbError> {
    // First get the overall user average re-edit rate for the period
    let overall = sqlx::query_scalar!(
        r#"
        SELECT COALESCE(AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)), 0.0)
        FROM sessions
        WHERE modified_at >= ?1 AND modified_at < ?2
        "#,
        from,
        to
    )
    .fetch_one(&self.pool)
    .await?
    .unwrap_or(0.0);

    // Then get per-category metrics
    let rows = sqlx::query!(
        r#"
        SELECT
            category_l1 as category,
            COUNT(*) as session_count,
            COALESCE(AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)), 0.0) as reedit_rate
        FROM sessions
        WHERE modified_at >= ?1 AND modified_at < ?2
          AND category_l1 IS NOT NULL
        GROUP BY category_l1
        ORDER BY reedit_rate ASC
        "#,
        from,
        to
    )
    .fetch_all(&self.pool)
    .await?;

    let results = rows
        .into_iter()
        .map(|row| {
            let reedit_rate = row.reedit_rate.unwrap_or(0.0);
            let vs_average = reedit_rate - overall;
            let verdict = match vs_average {
                v if v <= -0.20 => CategoryVerdict::Excellent,
                v if v <= -0.05 => CategoryVerdict::Good,
                v if v <= 0.10 => CategoryVerdict::Average,
                _ => CategoryVerdict::NeedsWork,
            };
            CategoryPerformance {
                category: row.category.unwrap_or_default(),
                reedit_rate,
                vs_average,
                verdict,
                insight: generate_category_insight(&row.category.unwrap_or_default(), verdict),
            }
        })
        .collect();

    Ok(results)
}

/// Get skill adoption timeline with impact.
/// Returns skills sorted by impact (most beneficial first), limited to `limit` results.
pub async fn get_skill_adoption_impact(&self, limit: usize) -> Result<Vec<SkillAdoption>, DbError> {
    // Get all distinct skills and their first use date
    let skills = sqlx::query!(
        r#"
        SELECT DISTINCT skill
        FROM session_skills
        "#
    )
    .fetch_all(&self.pool)
    .await?;

    let mut results = Vec::new();

    for skill_row in skills {
        let skill_name = skill_row.skill;

        // Get adoption date and session count
        let adoption_info = sqlx::query!(
            r#"
            SELECT
                MIN(s.modified_at) as adopted_at,
                COUNT(*) as session_count
            FROM sessions s
            JOIN session_skills ss ON s.id = ss.session_id
            WHERE ss.skill = ?1
            "#,
            skill_name
        )
        .fetch_one(&self.pool)
        .await?;

        let adopted_at = adoption_info.adopted_at.unwrap_or(0);
        let session_count = adoption_info.session_count as u32;

        // Skip skills with < 3 sessions
        if session_count < 3 {
            continue;
        }

        // Calculate re-edit rate BEFORE adoption (all sessions before first use)
        let before_rate = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)), 0.0)
            FROM sessions
            WHERE modified_at < ?1
            "#,
            adopted_at
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0.0);

        // Calculate re-edit rate AFTER adoption (sessions using this skill)
        let after_rate = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(AVG(CAST(s.reedited_files_count AS REAL) / NULLIF(s.files_edited_count, 0)), 0.0)
            FROM sessions s
            JOIN session_skills ss ON s.id = ss.session_id
            WHERE ss.skill = ?1
            "#,
            skill_name
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0.0);

        // Impact as percentage change (negative = improvement)
        let impact_on_reedit = if before_rate > 0.0 {
            ((after_rate - before_rate) / before_rate * 100.0).round()
        } else {
            0.0
        };

        // Build learning curve (first 10 sessions with this skill)
        let curve_rows = sqlx::query!(
            r#"
            SELECT
                s.modified_at,
                CAST(s.reedited_files_count AS REAL) / NULLIF(s.files_edited_count, 0) as reedit_rate
            FROM sessions s
            JOIN session_skills ss ON s.id = ss.session_id
            WHERE ss.skill = ?1
            ORDER BY s.modified_at ASC
            LIMIT 10
            "#,
            skill_name
        )
        .fetch_all(&self.pool)
        .await?;

        let learning_curve: Vec<LearningCurvePoint> = curve_rows
            .into_iter()
            .enumerate()
            .map(|(i, row)| LearningCurvePoint {
                session: (i + 1) as u32,
                reedit_rate: row.reedit_rate.unwrap_or(0.0),
            })
            .collect();

        results.push(SkillAdoption {
            skill: skill_name,
            adopted_at: chrono::DateTime::from_timestamp(adopted_at, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default(),
            session_count,
            impact_on_reedit,
            learning_curve,
        });
    }

    // Sort by impact (most beneficial = most negative first)
    results.sort_by(|a, b| a.impact_on_reedit.partial_cmp(&b.impact_on_reedit).unwrap());
    results.truncate(limit);

    Ok(results)
}
```

**Subtasks:**
- [ ] Add `PeriodMetrics` struct to `crates/core/src/types.rs`
- [ ] Add `CategoryPerformance` struct to `crates/core/src/types.rs`
- [ ] Add `SkillAdoption` struct to `crates/core/src/types.rs`
- [ ] Add `BenchmarksResponse` struct to `crates/server/src/routes/insights.rs`
- [ ] Implement `get_period_metrics()` in `crates/db/src/queries.rs`
- [ ] Implement `get_first_month_metrics()` in `crates/db/src/queries.rs`
- [ ] Implement `get_last_month_metrics()` in `crates/db/src/queries.rs`
- [ ] Implement `get_category_performance()` in `crates/db/src/queries.rs`
- [ ] Implement `get_skill_adoption_impact()` in `crates/db/src/queries.rs`
- [ ] Implement `generate_progress_insight()` helper function
- [ ] Implement `generate_category_verdict()` helper function
- [ ] Implement `generate_category_insight()` helper function
- [ ] Add `GET /api/insights/benchmarks` route handler
- [ ] Write unit tests for each query function
- [ ] Write integration tests for endpoint

**Files to create:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/server/src/routes/insights.rs` (or extend existing)

**Files to modify:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/core/src/types.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/db/src/queries.rs`
- `/Users/TBGor/dev/@vicky-ai/claude-view/crates/server/src/routes/mod.rs`

---

### 8.2 Then vs Now Comparison Component

Create React component for side-by-side first month vs last month comparison.

**Component:** `ThenVsNow.tsx`

**UI Mockup:**

```
Your Progress                                                [ All Time ]

┌──────────────────────────────────────────────────────────────────────┐
│                                                                       │
│  THEN (First Month)              NOW (Last 30 Days)                  │
│  ──────────────────              ────────────────────                │
│                                                                       │
│  Re-edit rate: 0.48              Re-edit rate: 0.22    ↓ 54%        │
│  Edits/file:   2.8               Edits/file:   1.3     ↓ 54%        │
│  Prompts/task: 12.4              Prompts/task: 6.2     ↓ 50%        │
│  Commit rate:  52%               Commit rate:  78%     ↑ 50%        │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘

💡 You've cut re-edits in half and doubled commit rate — your prompts
   are significantly more effective than when you started
```

**Props:**

```typescript
interface ThenVsNowProps {
  progress: {
    firstMonth: Metrics;
    lastMonth: Metrics;
    improvement: {
      reeditRate: number;
      editsPerFile: number;
      promptsPerTask: number;
      commitRate: number;
    };
    insight: string;
  };
  className?: string;
}
```

**Implementation Details:**

```typescript
// src/components/insights/ThenVsNow.tsx
import { cn } from '../../lib/utils'
import { TrendingUp, TrendingDown, Minus } from 'lucide-react'

export function ThenVsNow({ progress, className }: ThenVsNowProps) {
  const { firstMonth, lastMonth, improvement, insight } = progress

  const formatPercent = (value: number) => {
    const absValue = Math.abs(value)
    const sign = value > 0 ? '+' : value < 0 ? '' : ''
    return `${sign}${absValue.toFixed(0)}%`
  }

  const getImprovementIcon = (value: number, lowerIsBetter: boolean) => {
    const isImprovement = lowerIsBetter ? value < 0 : value > 0
    const isRegression = lowerIsBetter ? value > 0 : value < 0

    if (isImprovement) return <TrendingDown className="w-4 h-4 text-green-600" />
    if (isRegression) return <TrendingUp className="w-4 h-4 text-red-600" />
    return <Minus className="w-4 h-4 text-gray-400" />
  }

  const metrics = [
    { label: 'Re-edit rate', then: firstMonth.reeditRate, now: lastMonth.reeditRate, change: improvement.reeditRate, lowerIsBetter: true, format: (v: number) => v.toFixed(2) },
    { label: 'Edits/file', then: firstMonth.editsPerFile, now: lastMonth.editsPerFile, change: improvement.editsPerFile, lowerIsBetter: true, format: (v: number) => v.toFixed(1) },
    { label: 'Prompts/task', then: firstMonth.promptsPerTask, now: lastMonth.promptsPerTask, change: improvement.promptsPerTask, lowerIsBetter: true, format: (v: number) => v.toFixed(1) },
    { label: 'Commit rate', then: firstMonth.commitRate, now: lastMonth.commitRate, change: improvement.commitRate, lowerIsBetter: false, format: (v: number) => `${(v * 100).toFixed(0)}%` },
  ]

  return (
    <div className={cn('bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6', className)}>
      <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
        Your Progress
      </h3>

      <div className="grid grid-cols-3 gap-4 mb-4">
        {/* Header row */}
        <div className="text-sm font-medium text-gray-500 dark:text-gray-400">Metric</div>
        <div className="text-sm font-medium text-gray-500 dark:text-gray-400 text-center">
          THEN <span className="text-xs">(First Month)</span>
        </div>
        <div className="text-sm font-medium text-gray-500 dark:text-gray-400 text-center">
          NOW <span className="text-xs">(Last 30 Days)</span>
        </div>

        {/* Metric rows */}
        {metrics.map((m) => (
          <>
            <div className="text-sm text-gray-700 dark:text-gray-300">{m.label}</div>
            <div className="text-sm font-mono text-gray-600 dark:text-gray-400 text-center">
              {m.format(m.then)}
            </div>
            <div className="flex items-center justify-center gap-2">
              <span className="text-sm font-mono font-semibold text-gray-900 dark:text-gray-100">
                {m.format(m.now)}
              </span>
              {getImprovementIcon(m.change, m.lowerIsBetter)}
              <span className="text-xs text-gray-500 dark:text-gray-400">
                {formatPercent(m.change)}
              </span>
            </div>
          </>
        ))}
      </div>

      {/* Insight */}
      <div className="mt-4 p-3 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
        <p className="text-sm text-blue-800 dark:text-blue-200">
          <span className="mr-2">💡</span>
          {insight}
        </p>
      </div>
    </div>
  )
}
```

**Subtasks:**
- [ ] Create `src/components/insights/ThenVsNow.tsx`
- [ ] Add TypeScript types to `src/types/generated/` (auto-generated via ts-rs)
- [ ] Implement metric formatting helpers
- [ ] Implement improvement direction logic (lower is better vs higher is better)
- [ ] Add responsive styling for mobile
- [ ] Add dark mode support
- [ ] Add accessibility (aria-labels, screen reader support)
- [ ] Write unit tests

**Files to create:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/src/components/insights/ThenVsNow.tsx`
- `/Users/TBGor/dev/@vicky-ai/claude-view/src/components/insights/ThenVsNow.test.tsx`

---

### 8.3 Category Performance Table Component

Create React component showing performance by category with visual comparison bars.

**Component:** `CategoryPerformanceTable.tsx`

**UI Mockup:**

```
By Category Performance

  Category        Re-edit    vs Your Avg    Verdict
  ────────────────────────────────────────────────────────────────
  Feature         0.19       ████░░░░░░     ✓ Strong
  Testing         0.14       █████░░░░░     ✓ Excellent
  Refactor        0.38       ░░░░░░░███     ⚠ Needs work
  Bug Fix         0.28       ██░░░░░░░░     → Average
  Docs            0.12       █████░░░░░     ✓ Excellent
  Config          0.45       ░░░░░░░████    ⚠ Needs work
  Planning        0.08       ██████░░░░     ✓ Excellent
  Architecture    0.22       ███░░░░░░░     ✓ Good

         ◄── Better          Your avg (0.24)          Worse ──►

💡 Refactoring and Config have highest re-edit rates — try being more
   specific about desired patterns and environment constraints
```

**Props:**

```typescript
interface CategoryPerformanceTableProps {
  categories: Array<{
    category: string;
    reeditRate: number;
    vsAverage: number;
    verdict: 'excellent' | 'good' | 'average' | 'needs_work';
    insight: string;
  }>;
  userAverage: number;
  className?: string;
}
```

**Implementation Details:**

```typescript
// src/components/insights/CategoryPerformanceTable.tsx
import { cn } from '../../lib/utils'
import { Check, ArrowRight, AlertTriangle } from 'lucide-react'

const VERDICT_CONFIG = {
  excellent: { icon: Check, className: 'text-green-600', label: 'Excellent' },
  good: { icon: Check, className: 'text-green-500', label: 'Strong' },
  average: { icon: ArrowRight, className: 'text-gray-500', label: 'Average' },
  needs_work: { icon: AlertTriangle, className: 'text-amber-500', label: 'Needs work' },
}

export function CategoryPerformanceTable({ categories, userAverage, className }: CategoryPerformanceTableProps) {
  // Sort by re-edit rate (best first)
  const sorted = [...categories].sort((a, b) => a.reeditRate - b.reeditRate)

  // Calculate bar position (centered at user average)
  const maxDelta = Math.max(...categories.map(c => Math.abs(c.vsAverage)))
  const scale = maxDelta > 0 ? 100 / (maxDelta * 2) : 1

  const getBarStyle = (vsAverage: number) => {
    const width = Math.abs(vsAverage) * scale
    const isBetter = vsAverage < 0
    return {
      width: `${Math.min(width, 50)}%`,
      marginLeft: isBetter ? `${50 - width}%` : '50%',
      backgroundColor: isBetter ? '#22c55e' : '#f59e0b',
    }
  }

  return (
    <div className={cn('bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6', className)}>
      <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
        By Category Performance
      </h3>

      <table className="w-full" role="table">
        <thead>
          <tr className="text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
            <th className="pb-3">Category</th>
            <th className="pb-3 text-right">Re-edit</th>
            <th className="pb-3 px-4">vs Your Avg</th>
            <th className="pb-3">Verdict</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
          {sorted.map((cat) => {
            const config = VERDICT_CONFIG[cat.verdict]
            const Icon = config.icon

            return (
              <tr key={cat.category} className="group">
                <td className="py-2 text-sm font-medium text-gray-700 dark:text-gray-300 capitalize">
                  {cat.category.replace('_', ' ')}
                </td>
                <td className="py-2 text-sm font-mono text-right text-gray-600 dark:text-gray-400">
                  {cat.reeditRate.toFixed(2)}
                </td>
                <td className="py-2 px-4">
                  <div className="relative h-4 bg-gray-100 dark:bg-gray-800 rounded overflow-hidden">
                    {/* Center line */}
                    <div className="absolute left-1/2 top-0 bottom-0 w-px bg-gray-300 dark:bg-gray-600" />
                    {/* Bar */}
                    <div
                      className="absolute top-0 bottom-0 rounded"
                      style={getBarStyle(cat.vsAverage)}
                    />
                  </div>
                </td>
                <td className="py-2">
                  <div className={cn('flex items-center gap-1 text-sm', config.className)}>
                    <Icon className="w-4 h-4" />
                    <span>{config.label}</span>
                  </div>
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>

      {/* Legend */}
      <div className="mt-4 flex justify-center text-xs text-gray-500 dark:text-gray-400">
        <span>◄ Better</span>
        <span className="mx-4 font-mono">Your avg ({userAverage.toFixed(2)})</span>
        <span>Worse ►</span>
      </div>

      {/* Insight for worst categories */}
      {sorted.filter(c => c.verdict === 'needs_work').length > 0 && (
        <div className="mt-4 p-3 bg-amber-50 dark:bg-amber-900/20 rounded-lg">
          <p className="text-sm text-amber-800 dark:text-amber-200">
            <span className="mr-2">💡</span>
            {sorted.find(c => c.verdict === 'needs_work')?.insight}
          </p>
        </div>
      )}
    </div>
  )
}
```

**Subtasks:**
- [ ] Create `src/components/insights/CategoryPerformanceTable.tsx`
- [ ] Implement horizontal bar chart visualization
- [ ] Implement verdict badge with icon
- [ ] Add sorting (best performance first)
- [ ] Add tooltip on hover showing detailed stats
- [ ] Add responsive styling for mobile
- [ ] Add dark mode support
- [ ] Add accessibility (table roles, aria-labels)
- [ ] Write unit tests

**Files to create:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/src/components/insights/CategoryPerformanceTable.tsx`
- `/Users/TBGor/dev/@vicky-ai/claude-view/src/components/insights/CategoryPerformanceTable.test.tsx`

---

### 8.4 Skill Adoption Impact Component

Create React component showing skill adoption timeline with learning curves.

**Component:** `SkillAdoptionImpact.tsx`

**UI Mockup:**

```
Skill Adoption Impact

  Skill            Adopted    Sessions   Impact on Re-edit
  ────────────────────────────────────────────────────────────────
  tdd              Oct 15     47         -48% ████████████░░░░
  brainstorming    Nov 02     23         -35% █████████░░░░░░░
  debugging        Nov 18     31         -28% ███████░░░░░░░░░
  commit           Sep 01     156        -12% ████░░░░░░░░░░░░

  ──────────────────────────────────────────────────────────────────

  Learning Curve (TDD skill example):

  0.4 ┤●
      │ ●
  0.3 ┤  ●
      │   ●  ●
  0.2 ┤      ●  ●  ●  ●  ●  ●
      │                       ●  ●  ●  ●
  0.1 ┤                                   ●  ●  ●
      └────────────────────────────────────────────────────────────
       1   2   3   4   5   6   7   8   9  10  11  12  (sessions)

💡 TDD took ~5 sessions to show benefits — stick with new skills,
   improvement comes with practice
```

**Props:**

```typescript
interface SkillAdoptionImpactProps {
  skills: Array<{
    skill: string;
    adoptedAt: string;
    sessionCount: number;
    impactOnReedit: number;
    learningCurve: Array<{ session: number; reeditRate: number }>;
  }>;
  className?: string;
}
```

**Implementation Details:**

```typescript
// src/components/insights/SkillAdoptionImpact.tsx
import { useState } from 'react'
import { cn } from '../../lib/utils'
import { format, parseISO } from 'date-fns'

export function SkillAdoptionImpact({ skills, className }: SkillAdoptionImpactProps) {
  const [selectedSkill, setSelectedSkill] = useState<string | null>(
    skills.length > 0 ? skills[0].skill : null
  )

  // Sort by impact (most beneficial first)
  const sorted = [...skills].sort((a, b) => a.impactOnReedit - b.impactOnReedit)

  const selectedData = skills.find(s => s.skill === selectedSkill)

  const maxImpact = Math.max(...skills.map(s => Math.abs(s.impactOnReedit)))
  const barScale = maxImpact > 0 ? 100 / maxImpact : 1

  return (
    <div className={cn('bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6', className)}>
      <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-4">
        Skill Adoption Impact
      </h3>

      {/* Skills table */}
      <table className="w-full mb-6" role="table">
        <thead>
          <tr className="text-left text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
            <th className="pb-3">Skill</th>
            <th className="pb-3">Adopted</th>
            <th className="pb-3 text-right">Sessions</th>
            <th className="pb-3 px-4">Impact on Re-edit</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
          {sorted.map((skill) => (
            <tr
              key={skill.skill}
              className={cn(
                'cursor-pointer transition-colors',
                selectedSkill === skill.skill
                  ? 'bg-blue-50 dark:bg-blue-900/20'
                  : 'hover:bg-gray-50 dark:hover:bg-gray-800'
              )}
              onClick={() => setSelectedSkill(skill.skill)}
            >
              <td className="py-2 text-sm font-medium text-gray-700 dark:text-gray-300">
                {skill.skill}
              </td>
              <td className="py-2 text-sm text-gray-500 dark:text-gray-400">
                {format(parseISO(skill.adoptedAt), 'MMM d')}
              </td>
              <td className="py-2 text-sm font-mono text-right text-gray-600 dark:text-gray-400">
                {skill.sessionCount}
              </td>
              <td className="py-2 px-4">
                <div className="flex items-center gap-2">
                  <span className={cn(
                    'text-sm font-mono',
                    skill.impactOnReedit < 0 ? 'text-green-600' : 'text-amber-600'
                  )}>
                    {skill.impactOnReedit > 0 ? '+' : ''}{skill.impactOnReedit}%
                  </span>
                  <div className="flex-1 h-2 bg-gray-100 dark:bg-gray-800 rounded overflow-hidden">
                    <div
                      className={cn(
                        'h-full rounded',
                        skill.impactOnReedit < 0 ? 'bg-green-500' : 'bg-amber-500'
                      )}
                      style={{ width: `${Math.abs(skill.impactOnReedit) * barScale}%` }}
                    />
                  </div>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {/* Learning curve chart */}
      {selectedData && selectedData.learningCurve.length > 0 && (
        <div className="border-t border-gray-200 dark:border-gray-700 pt-4">
          <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
            Learning Curve ({selectedData.skill})
          </h4>
          <LearningCurveChart data={selectedData.learningCurve} />
        </div>
      )}

      {/* Insight */}
      {selectedData && (
        <div className="mt-4 p-3 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
          <p className="text-sm text-blue-800 dark:text-blue-200">
            <span className="mr-2">💡</span>
            {generateSkillInsight(selectedData)}
          </p>
        </div>
      )}
    </div>
  )
}

function LearningCurveChart({ data }: { data: Array<{ session: number; reeditRate: number }> }) {
  const maxRate = Math.max(...data.map(d => d.reeditRate))
  const minRate = Math.min(...data.map(d => d.reeditRate))
  const range = maxRate - minRate || 0.1

  const chartHeight = 120
  const chartWidth = '100%'

  const points = data.map((d, i) => ({
    x: (i / (data.length - 1)) * 100,
    y: ((maxRate - d.reeditRate) / range) * (chartHeight - 20) + 10,
  }))

  const pathD = points
    .map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x}% ${p.y}`)
    .join(' ')

  return (
    <div className="relative" style={{ height: chartHeight }}>
      <svg width={chartWidth} height={chartHeight} className="overflow-visible">
        {/* Grid lines */}
        <line x1="0" y1="10" x2="100%" y2="10" stroke="currentColor" className="text-gray-200 dark:text-gray-700" />
        <line x1="0" y1={chartHeight / 2} x2="100%" y2={chartHeight / 2} stroke="currentColor" className="text-gray-200 dark:text-gray-700" strokeDasharray="4" />
        <line x1="0" y1={chartHeight - 10} x2="100%" y2={chartHeight - 10} stroke="currentColor" className="text-gray-200 dark:text-gray-700" />

        {/* Line */}
        <path d={pathD} fill="none" stroke="currentColor" className="text-blue-500" strokeWidth="2" />

        {/* Points */}
        {points.map((p, i) => (
          <circle
            key={i}
            cx={`${p.x}%`}
            cy={p.y}
            r="4"
            fill="currentColor"
            className="text-blue-500"
          />
        ))}
      </svg>

      {/* Y-axis labels */}
      <div className="absolute left-0 top-0 text-xs text-gray-500 dark:text-gray-400 font-mono">
        {maxRate.toFixed(2)}
      </div>
      <div className="absolute left-0 bottom-0 text-xs text-gray-500 dark:text-gray-400 font-mono">
        {minRate.toFixed(2)}
      </div>

      {/* X-axis label */}
      <div className="absolute right-0 bottom-0 text-xs text-gray-500 dark:text-gray-400">
        (sessions)
      </div>
    </div>
  )
}

function generateSkillInsight(skill: SkillAdoption): string {
  const { learningCurve, impactOnReedit } = skill

  // Find inflection point (where rate stabilizes)
  const inflectionPoint = learningCurve.findIndex((d, i, arr) => {
    if (i < 2 || i >= arr.length - 1) return false
    const prevDelta = arr[i - 1].reeditRate - arr[i - 2].reeditRate
    const currDelta = d.reeditRate - arr[i - 1].reeditRate
    return Math.abs(currDelta) < Math.abs(prevDelta) * 0.5
  })

  if (inflectionPoint > 0) {
    return `${skill.skill} took ~${inflectionPoint} sessions to show benefits — stick with new skills, improvement comes with practice`
  }

  if (impactOnReedit < -30) {
    return `${skill.skill} has dramatically improved your workflow — consider using it more consistently`
  }

  return `${skill.skill} is contributing to your improvement — keep using it to build mastery`
}
```

**Subtasks:**
- [ ] Create `src/components/insights/SkillAdoptionImpact.tsx`
- [ ] Implement skills table with impact bars
- [ ] Implement learning curve SVG chart
- [ ] Implement skill selection state
- [ ] Implement insight generation logic
- [ ] Add date formatting with date-fns
- [ ] Add responsive styling for mobile
- [ ] Add dark mode support
- [ ] Add accessibility (interactive table rows)
- [ ] Write unit tests

**Files to create:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/src/components/insights/SkillAdoptionImpact.tsx`
- `/Users/TBGor/dev/@vicky-ai/claude-view/src/components/insights/SkillAdoptionImpact.test.tsx`

---

### 8.5 Monthly Report Generator

Create React component and backend endpoint for generating downloadable monthly reports.

**UI Mockup:**

```
Monthly Report                                        [ Generate PDF ]

┌──────────────────────────────────────────────────────────────────────┐
│                                                                       │
│  January 2026 Summary                                                │
│                                                                       │
│  Sessions: 127    Lines: +12,847    Commits: 34    Cost: $18.42     │
│                                                                       │
│  Top 3 Wins:                                                         │
│  ✓ Re-edit rate hit all-time low (0.19)                             │
│  ✓ Planning sessions up 40%                                          │
│  ✓ Testing efficiency improved 23%                                   │
│                                                                       │
│  Focus Areas:                                                        │
│  → Refactor prompts still need work                                  │
│  → Evening sessions underperforming                                  │
│                                                                       │
│                                   [ View Full Report ] [ Download ]  │
└──────────────────────────────────────────────────────────────────────┘
```

**Component:** `MonthlyReportGenerator.tsx`

**Props:**

```typescript
interface MonthlyReportGeneratorProps {
  reportSummary: {
    month: string;
    sessionCount: number;
    linesAdded: number;
    linesRemoved: number;
    commitCount: number;
    estimatedCost: number;
    topWins: string[];
    focusAreas: string[];
  };
  className?: string;
}
```

**PDF Generation Strategy:**

We will use **react-pdf** for PDF generation. This library:
- Renders React components to PDF
- Supports custom styling
- Works client-side (no server-side rendering needed)
- Produces small, clean PDFs

**Alternative considered:** jspdf — More manual, less React-native, but smaller bundle. We choose react-pdf for cleaner component model.

**Implementation Details:**

```typescript
// src/components/insights/MonthlyReportGenerator.tsx
import { useState } from 'react'
import { cn } from '../../lib/utils'
import { FileText, Download, Eye, X } from 'lucide-react'

export function MonthlyReportGenerator({ reportSummary, className }: MonthlyReportGeneratorProps) {
  const [isGenerating, setIsGenerating] = useState(false)
  const [showPreview, setShowPreview] = useState(false)

  const handleDownload = async () => {
    setIsGenerating(true)
    try {
      // Dynamic import to avoid bundling react-pdf when not needed
      const { generateReportPdf } = await import('./reportPdfGenerator')
      const blob = await generateReportPdf(reportSummary)

      // Trigger download
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `claude-code-report-${reportSummary.month.toLowerCase().replace(' ', '-')}.pdf`
      a.click()
      URL.revokeObjectURL(url)
    } catch (error) {
      console.error('Failed to generate PDF:', error)
    } finally {
      setIsGenerating(false)
    }
  }

  return (
    <div className={cn('bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6', className)}>
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          Monthly Report
        </h3>
        <button
          onClick={handleDownload}
          disabled={isGenerating}
          className="flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <Download className="w-4 h-4" />
          {isGenerating ? 'Generating...' : 'Generate PDF'}
        </button>
      </div>

      {/* Summary card */}
      <div className="bg-gray-50 dark:bg-gray-800 rounded-lg p-4 mb-4">
        <h4 className="text-base font-semibold text-gray-900 dark:text-gray-100 mb-3">
          {reportSummary.month} Summary
        </h4>

        <div className="grid grid-cols-4 gap-4 mb-4">
          <div className="text-center">
            <p className="text-2xl font-semibold text-blue-600 dark:text-blue-400 font-mono">
              {reportSummary.sessionCount}
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400">Sessions</p>
          </div>
          <div className="text-center">
            <p className="text-2xl font-semibold text-green-600 dark:text-green-400 font-mono">
              +{reportSummary.linesAdded.toLocaleString()}
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400">Lines</p>
          </div>
          <div className="text-center">
            <p className="text-2xl font-semibold text-purple-600 dark:text-purple-400 font-mono">
              {reportSummary.commitCount}
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400">Commits</p>
          </div>
          <div className="text-center">
            <p className="text-2xl font-semibold text-gray-600 dark:text-gray-400 font-mono">
              ${reportSummary.estimatedCost.toFixed(2)}
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400">Est. Cost</p>
          </div>
        </div>

        {/* Top wins */}
        <div className="mb-3">
          <h5 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Top 3 Wins:
          </h5>
          <ul className="space-y-1">
            {reportSummary.topWins.map((win, i) => (
              <li key={i} className="flex items-start gap-2 text-sm text-gray-600 dark:text-gray-400">
                <span className="text-green-500">✓</span>
                {win}
              </li>
            ))}
          </ul>
        </div>

        {/* Focus areas */}
        <div>
          <h5 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Focus Areas:
          </h5>
          <ul className="space-y-1">
            {reportSummary.focusAreas.map((area, i) => (
              <li key={i} className="flex items-start gap-2 text-sm text-gray-600 dark:text-gray-400">
                <span className="text-gray-400">→</span>
                {area}
              </li>
            ))}
          </ul>
        </div>
      </div>

      {/* Action buttons */}
      <div className="flex justify-end gap-2">
        <button
          onClick={() => setShowPreview(true)}
          className="flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-lg"
        >
          <Eye className="w-4 h-4" />
          View Full Report
        </button>
        <button
          onClick={handleDownload}
          disabled={isGenerating}
          className="flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-lg disabled:opacity-50"
        >
          <FileText className="w-4 h-4" />
          Download
        </button>
      </div>

      {/* Preview modal (simplified) */}
      {showPreview && (
        <ReportPreviewModal
          reportSummary={reportSummary}
          onClose={() => setShowPreview(false)}
        />
      )}
    </div>
  )
}

/**
 * ReportPreviewModal - Full-screen modal for previewing the monthly report before download.
 */
interface ReportPreviewModalProps {
  reportSummary: {
    month: string;
    sessionCount: number;
    linesAdded: number;
    linesRemoved: number;
    commitCount: number;
    estimatedCost: number;
    topWins: string[];
    focusAreas: string[];
  };
  onClose: () => void;
}

function ReportPreviewModal({ reportSummary, onClose }: ReportPreviewModalProps) {
  const [isGenerating, setIsGenerating] = useState(false)

  const handleDownload = async () => {
    setIsGenerating(true)
    try {
      const { generateReportPdf } = await import('./reportPdfGenerator')
      const blob = await generateReportPdf(reportSummary)
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `claude-code-report-${reportSummary.month.toLowerCase().replace(' ', '-')}.pdf`
      a.click()
      URL.revokeObjectURL(url)
    } catch (error) {
      console.error('Failed to generate PDF:', error)
    } finally {
      setIsGenerating(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-white dark:bg-gray-900 rounded-xl shadow-2xl max-w-2xl w-full mx-4 max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Report Preview
          </h2>
          <button
            onClick={onClose}
            className="p-1 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
            aria-label="Close preview"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Report content */}
        <div className="p-6">
          <h3 className="text-xl font-bold text-gray-900 dark:text-gray-100 mb-2">
            Claude Code Monthly Report
          </h3>
          <p className="text-gray-500 dark:text-gray-400 mb-6">{reportSummary.month}</p>

          {/* Stats grid */}
          <div className="grid grid-cols-4 gap-4 mb-6">
            <div className="text-center">
              <p className="text-2xl font-semibold text-blue-600 dark:text-blue-400 font-mono">
                {reportSummary.sessionCount}
              </p>
              <p className="text-xs text-gray-500 dark:text-gray-400">Sessions</p>
            </div>
            <div className="text-center">
              <p className="text-2xl font-semibold text-green-600 dark:text-green-400 font-mono">
                +{reportSummary.linesAdded.toLocaleString()}
              </p>
              <p className="text-xs text-gray-500 dark:text-gray-400">Lines</p>
            </div>
            <div className="text-center">
              <p className="text-2xl font-semibold text-purple-600 dark:text-purple-400 font-mono">
                {reportSummary.commitCount}
              </p>
              <p className="text-xs text-gray-500 dark:text-gray-400">Commits</p>
            </div>
            <div className="text-center">
              <p className="text-2xl font-semibold text-gray-600 dark:text-gray-400 font-mono">
                ${reportSummary.estimatedCost.toFixed(2)}
              </p>
              <p className="text-xs text-gray-500 dark:text-gray-400">Est. Cost</p>
            </div>
          </div>

          {/* Top wins */}
          <div className="mb-4">
            <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Top Achievements
            </h4>
            <ul className="space-y-1">
              {reportSummary.topWins.map((win, i) => (
                <li key={i} className="flex items-start gap-2 text-sm text-gray-600 dark:text-gray-400">
                  <span className="text-green-500">✓</span>
                  {win}
                </li>
              ))}
            </ul>
          </div>

          {/* Focus areas */}
          <div>
            <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              Focus Areas
            </h4>
            <ul className="space-y-1">
              {reportSummary.focusAreas.map((area, i) => (
                <li key={i} className="flex items-start gap-2 text-sm text-gray-600 dark:text-gray-400">
                  <span className="text-gray-400">→</span>
                  {area}
                </li>
              ))}
            </ul>
          </div>
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-2 p-4 border-t border-gray-200 dark:border-gray-700">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-lg"
          >
            Close
          </button>
          <button
            onClick={handleDownload}
            disabled={isGenerating}
            className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Download className="w-4 h-4" />
            {isGenerating ? 'Generating...' : 'Download PDF'}
          </button>
        </div>
      </div>
    </div>
  )
}
```

**PDF Generator Module:**

```typescript
// src/components/insights/reportPdfGenerator.tsx
import { pdf, Document, Page, Text, View, StyleSheet } from '@react-pdf/renderer'

const styles = StyleSheet.create({
  page: {
    padding: 40,
    fontFamily: 'Helvetica',
  },
  title: {
    fontSize: 24,
    marginBottom: 20,
    fontWeight: 'bold',
  },
  section: {
    marginBottom: 15,
  },
  sectionTitle: {
    fontSize: 14,
    fontWeight: 'bold',
    marginBottom: 8,
    color: '#374151',
  },
  statsRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    marginBottom: 10,
  },
  stat: {
    alignItems: 'center',
  },
  statValue: {
    fontSize: 20,
    fontWeight: 'bold',
    color: '#2563eb',
  },
  statLabel: {
    fontSize: 10,
    color: '#6b7280',
  },
  listItem: {
    fontSize: 11,
    marginBottom: 4,
    color: '#374151',
  },
  footer: {
    position: 'absolute',
    bottom: 30,
    left: 40,
    right: 40,
    fontSize: 9,
    color: '#9ca3af',
    textAlign: 'center',
  },
})

interface ReportData {
  month: string
  sessionCount: number
  linesAdded: number
  linesRemoved: number
  commitCount: number
  estimatedCost: number
  topWins: string[]
  focusAreas: string[]
}

function ReportDocument({ data }: { data: ReportData }) {
  return (
    <Document>
      <Page size="A4" style={styles.page}>
        <Text style={styles.title}>Claude Code Monthly Report</Text>
        <Text style={{ fontSize: 16, marginBottom: 20, color: '#6b7280' }}>
          {data.month}
        </Text>

        {/* Stats */}
        <View style={styles.statsRow}>
          <View style={styles.stat}>
            <Text style={styles.statValue}>{data.sessionCount}</Text>
            <Text style={styles.statLabel}>Sessions</Text>
          </View>
          <View style={styles.stat}>
            <Text style={[styles.statValue, { color: '#22c55e' }]}>+{data.linesAdded.toLocaleString()}</Text>
            <Text style={styles.statLabel}>Lines Added</Text>
          </View>
          <View style={styles.stat}>
            <Text style={[styles.statValue, { color: '#a855f7' }]}>{data.commitCount}</Text>
            <Text style={styles.statLabel}>Commits</Text>
          </View>
          <View style={styles.stat}>
            <Text style={[styles.statValue, { color: '#64748b' }]}>${data.estimatedCost.toFixed(2)}</Text>
            <Text style={styles.statLabel}>Est. Cost</Text>
          </View>
        </View>

        {/* Top Wins */}
        <View style={styles.section}>
          <Text style={styles.sectionTitle}>Top Achievements</Text>
          {data.topWins.map((win, i) => (
            <Text key={i} style={styles.listItem}>✓ {win}</Text>
          ))}
        </View>

        {/* Focus Areas */}
        <View style={styles.section}>
          <Text style={styles.sectionTitle}>Focus Areas</Text>
          {data.focusAreas.map((area, i) => (
            <Text key={i} style={styles.listItem}>→ {area}</Text>
          ))}
        </View>

        {/* Footer */}
        <Text style={styles.footer}>
          Generated by vibe-recall • {new Date().toLocaleDateString()}
        </Text>
      </Page>
    </Document>
  )
}

export async function generateReportPdf(data: ReportData): Promise<Blob> {
  const doc = <ReportDocument data={data} />
  const blob = await pdf(doc).toBlob()
  return blob
}
```

**Backend Endpoint for Report Data:**

```
GET /api/insights/report?month=2026-01
```

Returns the full report data including:
- All benchmark metrics
- Trend charts data
- Category breakdown
- Skill adoption data

**Subtasks:**
- [ ] Add `@react-pdf/renderer` dependency to package.json
- [ ] Create `src/components/insights/MonthlyReportGenerator.tsx`
- [ ] Create `src/components/insights/reportPdfGenerator.tsx`
- [ ] Create `src/components/insights/ReportPreviewModal.tsx`
- [ ] Add `GET /api/insights/report` endpoint (optional, for full report data)
- [ ] Implement report data aggregation in backend
- [ ] Implement top wins generation logic
- [ ] Implement focus areas generation logic
- [ ] Add loading state during PDF generation
- [ ] Add error handling for PDF generation
- [ ] Add responsive styling
- [ ] Add dark mode support
- [ ] Write unit tests

**Files to create:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/src/components/insights/MonthlyReportGenerator.tsx`
- `/Users/TBGor/dev/@vicky-ai/claude-view/src/components/insights/reportPdfGenerator.tsx`
- `/Users/TBGor/dev/@vicky-ai/claude-view/src/components/insights/ReportPreviewModal.tsx`
- `/Users/TBGor/dev/@vicky-ai/claude-view/src/components/insights/MonthlyReportGenerator.test.tsx`

**Files to modify:**
- `/Users/TBGor/dev/@vicky-ai/claude-view/package.json` (add @react-pdf/renderer)

---

## API Specification

### GET /api/insights/benchmarks

**Request:**

```
GET /api/insights/benchmarks?range=all|30d|90d|1y
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `range` | string | `all` | Time range filter: `all`, `30d`, `90d`, `1y` |

**Range Parameter Semantics:**

The `range` parameter defines the **window of sessions** used for all benchmark calculations:

| Range | Data Window | "First Month" | "Last Month" |
|-------|-------------|---------------|--------------|
| `all` | All sessions | First 30 days of user's history | Most recent 30 days |
| `1y` | Last 365 days | First 30 days within that year | Most recent 30 days |
| `90d` | Last 90 days | Days 61-90 (earliest 30 in range) | Days 1-30 (most recent) |
| `30d` | Last 30 days | N/A (not enough data) | Entire 30-day window |

**Edge cases:**
- If `range=30d`, the "Then vs Now" section returns null for `firstMonth` and frontend shows "Not enough data for comparison"
- If the data window contains fewer than 30 days total, `firstMonth` and `lastMonth` may overlap or be identical
- `byCategory`, `skillAdoption`, and `reportSummary` always use the full range window regardless of first/last month logic

**Response (200 OK):**

```json
{
  "progress": {
    "firstMonth": {
      "reeditRate": 0.48,
      "editsPerFile": 2.8,
      "promptsPerTask": 12.4,
      "commitRate": 0.52
    },
    "lastMonth": {
      "reeditRate": 0.22,
      "editsPerFile": 1.3,
      "promptsPerTask": 6.2,
      "commitRate": 0.78
    },
    "improvement": {
      "reeditRate": -54.2,
      "editsPerFile": -53.6,
      "promptsPerTask": -50.0,
      "commitRate": 50.0
    },
    "insight": "You've cut re-edits in half and doubled commit rate — your prompts are significantly more effective than when you started"
  },
  "byCategory": [
    {
      "category": "code_work",
      "reeditRate": 0.19,
      "vsAverage": -0.05,
      "verdict": "good",
      "insight": "Feature development is your strongest area"
    },
    {
      "category": "support_work",
      "reeditRate": 0.38,
      "vsAverage": 0.14,
      "verdict": "needs_work",
      "insight": "Try being more specific about desired patterns and environment constraints"
    }
  ],
  "userAverageReeditRate": 0.24,
  "skillAdoption": [
    {
      "skill": "tdd",
      "adoptedAt": "2025-10-15T00:00:00Z",
      "sessionCount": 47,
      "impactOnReedit": -48,
      "learningCurve": [
        { "session": 1, "reeditRate": 0.42 },
        { "session": 2, "reeditRate": 0.38 },
        { "session": 3, "reeditRate": 0.31 },
        { "session": 4, "reeditRate": 0.28 },
        { "session": 5, "reeditRate": 0.22 }
      ]
    }
  ],
  "reportSummary": {
    "month": "January 2026",
    "sessionCount": 127,
    "linesAdded": 12847,
    "linesRemoved": 3421,
    "commitCount": 34,
    "estimatedCost": 18.42,
    "topWins": [
      "Re-edit rate hit all-time low (0.19)",
      "Planning sessions up 40%",
      "Testing efficiency improved 23%"
    ],
    "focusAreas": [
      "Refactor prompts still need work",
      "Evening sessions underperforming"
    ]
  }
}
```

**Error Responses:**

| Status | Body | Cause |
|--------|------|-------|
| 400 | `{ "error": "Invalid range parameter" }` | Invalid range value |
| 500 | `{ "error": "Database error" }` | Internal error |

---

## Rust Types

### Core Types (crates/core/src/types.rs)

```rust
/// Metrics for a time period.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct PeriodMetrics {
    /// Re-edit rate: files re-edited / files edited (0.0-1.0)
    pub reedit_rate: f64,
    /// Average edit operations per file
    pub edits_per_file: f64,
    /// Average user prompts per session
    pub prompts_per_task: f64,
    /// Percentage of sessions with commits (0.0-1.0)
    pub commit_rate: f64,
}

/// Improvement percentages between two periods.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ImprovementMetrics {
    /// Re-edit rate change (negative = improvement)
    pub reedit_rate: f64,
    /// Edits per file change (negative = improvement)
    pub edits_per_file: f64,
    /// Prompts per task change (negative = improvement)
    pub prompts_per_task: f64,
    /// Commit rate change (positive = improvement)
    pub commit_rate: f64,
}

/// Progress comparison between first and last month.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ProgressComparison {
    pub first_month: PeriodMetrics,
    pub last_month: PeriodMetrics,
    pub improvement: ImprovementMetrics,
    pub insight: String,
}

/// Verdict for category performance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "snake_case")]
pub enum CategoryVerdict {
    Excellent,
    Good,
    Average,
    NeedsWork,
}

/// Performance metrics for a single category.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CategoryPerformance {
    pub category: String,
    pub reedit_rate: f64,
    /// Difference from user's overall average (negative = better)
    pub vs_average: f64,
    pub verdict: CategoryVerdict,
    pub insight: String,
}

/// Learning curve data point.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct LearningCurvePoint {
    pub session: u32,
    pub reedit_rate: f64,
}

/// Skill adoption with impact metrics.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SkillAdoption {
    pub skill: String,
    pub adopted_at: String,
    pub session_count: u32,
    /// Percentage improvement in re-edit rate after adoption
    pub impact_on_reedit: f64,
    pub learning_curve: Vec<LearningCurvePoint>,
}

/// Monthly report summary.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ReportSummary {
    pub month: String,
    pub session_count: u32,
    pub lines_added: i64,
    pub lines_removed: i64,
    pub commit_count: u32,
    pub estimated_cost: f64,
    pub top_wins: Vec<String>,
    pub focus_areas: Vec<String>,
}

/// Full benchmarks response.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct BenchmarksResponse {
    pub progress: ProgressComparison,
    pub by_category: Vec<CategoryPerformance>,
    /// User's overall average re-edit rate (for CategoryPerformanceTable to display center line)
    pub user_average_reedit_rate: f64,
    pub skill_adoption: Vec<SkillAdoption>,
    pub report_summary: ReportSummary,
}
```

---

## React Components

### Component Hierarchy

```
BenchmarksTab
├── ThenVsNow
│   └── MetricRow (internal)
├── CategoryPerformanceTable
│   └── PerformanceBar (internal)
├── SkillAdoptionImpact
│   ├── SkillsTable (internal)
│   └── LearningCurveChart (internal)
└── MonthlyReportGenerator
    ├── ReportSummaryCard (internal)
    └── ReportPreviewModal
```

### Component Files

| Component | File | Purpose |
|-----------|------|---------|
| `BenchmarksTab` | `src/pages/InsightsPage/BenchmarksTab.tsx` | Main tab container, data fetching |
| `ThenVsNow` | `src/components/insights/ThenVsNow.tsx` | First vs last month comparison |
| `CategoryPerformanceTable` | `src/components/insights/CategoryPerformanceTable.tsx` | Category breakdown with bars |
| `SkillAdoptionImpact` | `src/components/insights/SkillAdoptionImpact.tsx` | Skill timeline with learning curves |
| `MonthlyReportGenerator` | `src/components/insights/MonthlyReportGenerator.tsx` | Report preview and PDF download |
| `ReportPreviewModal` | `src/components/insights/ReportPreviewModal.tsx` | Full-screen report preview |
| `reportPdfGenerator` | `src/components/insights/reportPdfGenerator.tsx` | PDF generation with react-pdf |

---

## PDF Generation

### Library Choice: @react-pdf/renderer

**Why react-pdf over alternatives:**

| Library | Bundle Size | React-Native | Pros | Cons |
|---------|-------------|--------------|------|------|
| `@react-pdf/renderer` | ~400KB | Yes | React components, styled-components-like API | Larger bundle |
| `jspdf` | ~300KB | No | Smaller, mature | Manual layout, no React |
| `pdfmake` | ~500KB | No | Good tables | Complex API |
| Browser print | 0KB | — | Zero deps | Limited styling, requires modal |

**Decision:** Use `@react-pdf/renderer` for its React-native component model. The bundle size increase is acceptable given the improved DX and consistent styling.

### PDF Content Structure

```
┌─────────────────────────────────────────┐
│  Claude Code Monthly Report             │
│  January 2026                           │
│                                         │
│  ┌─────┬─────┬─────┬─────┐             │
│  │ 127 │+12K │  34 │$18  │             │
│  │sess │lines│comm │cost │             │
│  └─────┴─────┴─────┴─────┘             │
│                                         │
│  Top Achievements                       │
│  ✓ Re-edit rate hit all-time low       │
│  ✓ Planning sessions up 40%            │
│  ✓ Testing efficiency improved 23%     │
│                                         │
│  Focus Areas                            │
│  → Refactor prompts still need work    │
│  → Evening sessions underperforming    │
│                                         │
│  ─────────────────────────────────────  │
│                                         │
│  Performance by Category                │
│  [Chart: horizontal bars]               │
│                                         │
│  Skill Adoption Impact                  │
│  [Table: skills with impact %]          │
│                                         │
│  ─────────────────────────────────────  │
│  Generated by vibe-recall • 2026-02-05  │
└─────────────────────────────────────────┘
```

### Lazy Loading

PDF generation is lazy-loaded to avoid bundling react-pdf in the main chunk:

```typescript
const handleDownload = async () => {
  // Only load react-pdf when user clicks download
  const { generateReportPdf } = await import('./reportPdfGenerator')
  const blob = await generateReportPdf(data)
  // ...
}
```

---

## State Management

### React Query Hook

```typescript
// src/hooks/use-benchmarks.ts
import { useQuery } from '@tanstack/react-query'
import type { BenchmarksResponse } from '../types/generated'

interface UseBenchmarksOptions {
  range?: 'all' | '30d' | '90d' | '1y'
}

async function fetchBenchmarks(range: string): Promise<BenchmarksResponse> {
  const response = await fetch(`/api/insights/benchmarks?range=${range}`)
  if (!response.ok) {
    const errorText = await response.text()
    throw new Error(`Failed to fetch benchmarks: ${errorText}`)
  }
  return response.json()
}

export function useBenchmarks({ range = 'all' }: UseBenchmarksOptions = {}) {
  return useQuery({
    queryKey: ['benchmarks', range],
    queryFn: () => fetchBenchmarks(range),
    staleTime: 60_000, // Cache for 1 minute (benchmarks change slowly)
  })
}
```

### Local State

| State | Location | Purpose |
|-------|----------|---------|
| `selectedSkill` | `SkillAdoptionImpact` | Track which skill's learning curve to display |
| `showPreview` | `MonthlyReportGenerator` | Toggle report preview modal |
| `isGenerating` | `MonthlyReportGenerator` | PDF generation loading state |

---

## Testing Strategy

### Unit Tests

| Test | Location | Scope |
|------|----------|-------|
| `get_period_metrics` | `crates/db/src/queries.rs` | Verify metric calculation |
| `get_first_month_metrics` | `crates/db/src/queries.rs` | Verify date range selection |
| `get_category_performance` | `crates/db/src/queries.rs` | Verify category aggregation |
| `get_skill_adoption_impact` | `crates/db/src/queries.rs` | Verify skill timeline and impact |
| `generate_progress_insight` | `crates/server/src/routes/insights.rs` | Verify insight text generation |
| `ThenVsNow rendering` | `ThenVsNow.test.tsx` | Component renders correctly |
| `CategoryPerformanceTable` | `CategoryPerformanceTable.test.tsx` | Table and bars render correctly |
| `SkillAdoptionImpact` | `SkillAdoptionImpact.test.tsx` | Interactive state works |
| `MonthlyReportGenerator` | `MonthlyReportGenerator.test.tsx` | PDF generation triggers |

### Integration Tests

| Test | Location | Scope |
|------|----------|-------|
| `GET /api/insights/benchmarks` | `crates/server/src/routes/insights.rs` | Full endpoint with test data |
| Benchmarks tab rendering | `BenchmarksTab.test.tsx` | Full tab with mocked API |

### Test Commands

```bash
# Backend tests
cargo test -p db -- benchmarks
cargo test -p server -- insights

# Frontend tests
npm test -- --testPathPattern="Benchmarks|ThenVsNow|CategoryPerformance|SkillAdoption|MonthlyReport"
```

### Test Fixtures

```rust
// crates/db/src/test_fixtures.rs

/// Create test sessions spanning 3 months for benchmark testing.
pub fn create_benchmark_test_data(db: &Database) -> Result<(), DbError> {
    let now = chrono::Utc::now().timestamp();
    let day = 86400;

    // First month sessions (90 days ago)
    for i in 0..10 {
        db.insert_session(&SessionInfo {
            id: format!("first-month-{}", i),
            modified_at: now - (90 * day) + (i as i64 * day),
            // High re-edit rate, low commit rate
            reedited_files_count: 5,
            files_edited_count: 10,
            commit_count: if i % 2 == 0 { 1 } else { 0 },
            user_prompt_count: 15,
            ..Default::default()
        }, "test-project", "Test Project").await?;
    }

    // Last month sessions (within 30 days)
    for i in 0..10 {
        db.insert_session(&SessionInfo {
            id: format!("last-month-{}", i),
            modified_at: now - (15 * day) + (i as i64 * day),
            // Low re-edit rate, high commit rate
            reedited_files_count: 2,
            files_edited_count: 10,
            commit_count: 2,
            user_prompt_count: 6,
            skills_used: vec!["tdd".to_string()],
            ..Default::default()
        }, "test-project", "Test Project").await?;
    }

    Ok(())
}
```

---

## Acceptance Criteria

### Task 8.1: GET /api/insights/benchmarks Endpoint

- [ ] Endpoint returns 200 OK with valid JSON
- [ ] `progress.firstMonth` contains metrics from first 30 days of data
- [ ] `progress.lastMonth` contains metrics from most recent 30 days
- [ ] `progress.improvement` correctly calculates percentage changes
- [ ] `progress.insight` is a non-empty, human-readable string
- [ ] `byCategory` includes all L1 categories with sessions
- [ ] `byCategory[].vsAverage` is relative to user's overall average
- [ ] `byCategory[].verdict` follows threshold rules (excellent < -20%, needs_work > +20%)
- [ ] `skillAdoption` sorted by impact (most beneficial first)
- [ ] `skillAdoption[].learningCurve` contains first 10 sessions with skill
- [ ] `reportSummary` aggregates current month data
- [ ] Range parameter filters data correctly

### Task 8.2: ThenVsNow Component

- [ ] Displays first month vs last month side-by-side
- [ ] Shows improvement arrows (up/down) correctly based on metric type
- [ ] Shows percentage change for each metric
- [ ] Shows insight message at bottom
- [ ] Handles missing data gracefully (e.g., no first month data)
- [ ] Responsive on mobile
- [ ] Dark mode supported
- [ ] Screen reader accessible (aria-labels)

### Task 8.3: CategoryPerformanceTable Component

- [ ] Displays all categories in table format
- [ ] Shows horizontal bars relative to user average
- [ ] Bars centered at user average, extend left (better) or right (worse)
- [ ] Verdict badges show correct icon and color
- [ ] Shows insight for worst-performing category
- [ ] Sorted by re-edit rate (best first)
- [ ] Responsive on mobile
- [ ] Dark mode supported

### Task 8.4: SkillAdoptionImpact Component

- [ ] Displays skills table with adoption date, count, impact
- [ ] Click on row selects skill and shows learning curve
- [ ] Learning curve chart renders correctly
- [ ] Chart shows re-edit rate over first N sessions
- [ ] Insight generated based on learning curve shape
- [ ] Empty state when no skills
- [ ] Responsive on mobile
- [ ] Dark mode supported

### Task 8.5: MonthlyReportGenerator Component

- [ ] Displays summary card with key metrics
- [ ] Shows top 3 wins and focus areas
- [ ] "Generate PDF" button triggers download
- [ ] PDF contains all summary data
- [ ] PDF styled consistently with app
- [ ] "View Full Report" shows preview modal
- [ ] Loading state during PDF generation
- [ ] Error handling for PDF generation failure
- [ ] Responsive on mobile
- [ ] Dark mode supported

### Overall

- [ ] All 577+ existing backend tests pass
- [ ] All 578+ existing frontend tests pass
- [ ] New tests pass: benchmark queries, components
- [ ] No TypeScript compilation errors
- [ ] `cargo clippy` passes with no warnings
- [ ] TypeScript types auto-generated via ts-rs
- [ ] Tab integrates with Phase 5 InsightsPage layout
- [ ] Time range filter works across all sections

---

## Dependencies

### Required Before Starting

- **Phase 5: Insights Core** — Page layout, routing, time range filter
  - `/insights` page must exist
  - Tab navigation must be implemented
  - Time range filter context must be available

### Does Not Require

- Phase 2 (Classification) — Benchmarks can work without classification (category breakdown shows "uncategorized")
- Phase 6 (Categories Tab) — Independent
- Phase 7 (Trends Tab) — Independent

### Dependency Graph

```
Phase 5 (Insights Core)
     ↓
Phase 8 (Benchmarks Tab)  ←── Can run in parallel with Phase 6, 7
```

---

## File Changes Summary

### Files to Create

| File | Purpose |
|------|---------|
| `crates/server/src/routes/insights.rs` | Benchmarks endpoint (extend or create) |
| `src/hooks/use-benchmarks.ts` | React Query hook |
| `src/components/insights/ThenVsNow.tsx` | Progress comparison component |
| `src/components/insights/ThenVsNow.test.tsx` | Tests |
| `src/components/insights/CategoryPerformanceTable.tsx` | Category table component |
| `src/components/insights/CategoryPerformanceTable.test.tsx` | Tests |
| `src/components/insights/SkillAdoptionImpact.tsx` | Skill timeline component |
| `src/components/insights/SkillAdoptionImpact.test.tsx` | Tests |
| `src/components/insights/MonthlyReportGenerator.tsx` | Report generator component |
| `src/components/insights/MonthlyReportGenerator.test.tsx` | Tests |
| `src/components/insights/reportPdfGenerator.tsx` | PDF generation module |
| `src/components/insights/ReportPreviewModal.tsx` | Report preview modal |
| `src/pages/InsightsPage/BenchmarksTab.tsx` | Tab container |

### Files to Modify

| File | Change |
|------|--------|
| `crates/core/src/types.rs` | Add benchmark types |
| `crates/db/src/queries.rs` | Add benchmark query functions |
| `crates/server/src/routes/mod.rs` | Add insights router |
| `src/types/generated/index.ts` | Export new types (auto-generated) |
| `src/pages/InsightsPage/index.tsx` | Add BenchmarksTab |
| `package.json` | Add @react-pdf/renderer dependency |

---

## Notes

- **Calculation precision:** All percentages stored as decimals (0.0-1.0) in backend, formatted as percentages in frontend
- **Empty state handling:** If user has < 30 days of data, show "Not enough data yet" for Then vs Now
- **Skill adoption threshold:** Only show skills used in 3+ sessions to filter out one-off uses
- **Learning curve points:** Cap at 10-15 data points for clean visualization
- **PDF generation is async:** Use loading spinner, handle errors gracefully
- **react-pdf bundle:** Lazy-load to avoid main bundle bloat
- **Mobile responsiveness:** Table components should scroll horizontally on small screens
