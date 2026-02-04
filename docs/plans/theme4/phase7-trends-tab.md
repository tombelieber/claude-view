---
status: pending
date: 2026-02-05
phase: 7
theme: 4
depends_on: [phase5-insights-core]
parallelizable_with: [phase6-categories-tab, phase8-benchmarks-tab]
---

# Phase 7: Trends Tab

> **Goal:** Add time-series visualizations to the /insights page showing efficiency trends, category evolution, and activity patterns over time.

## Overview

Phase 7 implements the Trends tab for the `/insights` page with three key visualizations:

1. **Efficiency Over Time** — Line chart showing selected metric (re-edit rate, sessions, lines, etc.) over time with trend analysis
2. **Category Evolution** — Stacked area chart showing work type distribution (Code/Support/Thinking) changes over time
3. **Activity Heatmap** — Grid showing session frequency and efficiency by day-of-week and hour-of-day

**Key Dependencies:**
- Phase 5 (Insights Core) — Page layout, routing, time range filter
- Existing `trends.rs` module — Week-over-week trend calculations (extend for longer periods)

**Not Required:**
- Phase 2 (Classification) — Category evolution requires classification data, but can show "Pending classification" state

---

## Tasks

### 7.1 GET /api/insights/trends Endpoint

**Subtasks:**

| ID | Task | Est. | Files |
|----|------|------|-------|
| 7.1.1 | Define `TrendsQuery` struct with query params | 15m | `crates/server/src/routes/insights.rs` |
| 7.1.2 | Define `TrendsResponse` struct with ts-rs export | 20m | `crates/server/src/routes/insights.rs` |
| 7.1.3 | Implement metric time-series aggregation query | 45m | `crates/db/src/insights.rs` |
| 7.1.4 | Implement category evolution aggregation query | 30m | `crates/db/src/insights.rs` |
| 7.1.5 | Implement activity heatmap aggregation query | 30m | `crates/db/src/insights.rs` |
| 7.1.6 | Generate insight text from trend data | 30m | `crates/db/src/insights.rs` |
| 7.1.7 | Wire up route handler | 15m | `crates/server/src/routes/insights.rs` |
| 7.1.8 | Add unit tests for all queries | 45m | `crates/db/src/insights.rs` |
| 7.1.9 | Add integration tests for endpoint | 30m | `crates/server/src/routes/insights.rs` |

**File Changes:**

```
crates/db/src/lib.rs           # Re-export new types
crates/db/src/insights.rs      # NEW: Insights queries module
crates/server/src/routes/mod.rs # Add insights router
crates/server/src/routes/insights.rs # NEW: Insights routes
src/types/generated/           # Auto-generated TS types
```

---

### 7.2 Efficiency Over Time Line Chart

**Subtasks:**

| ID | Task | Est. | Files |
|----|------|------|-------|
| 7.2.1 | Install Recharts library | 5m | `package.json` |
| 7.2.2 | Create `TrendsChart` component | 45m | `src/components/insights/TrendsChart.tsx` |
| 7.2.3 | Create metric selector dropdown | 20m | `src/components/insights/MetricSelector.tsx` |
| 7.2.4 | Create granularity selector (day/week/month) | 15m | `src/components/insights/GranularitySelector.tsx` |
| 7.2.5 | Implement trend line calculation | 20m | `src/lib/trendline.ts` |
| 7.2.6 | Add average reference line | 10m | `src/components/insights/TrendsChart.tsx` |
| 7.2.7 | Add insight text display | 15m | `src/components/insights/TrendsChart.tsx` |
| 7.2.8 | Add loading skeleton | 10m | `src/components/insights/TrendsChartSkeleton.tsx` |
| 7.2.9 | Add empty state | 10m | `src/components/insights/TrendsChart.tsx` |
| 7.2.10 | Write component tests | 30m | `src/components/insights/TrendsChart.test.tsx` |

**File Changes:**

```
package.json                                    # Add recharts
src/components/insights/TrendsChart.tsx         # NEW
src/components/insights/TrendsChartSkeleton.tsx # NEW
src/components/insights/MetricSelector.tsx      # NEW
src/components/insights/GranularitySelector.tsx # NEW
src/lib/trendline.ts                            # NEW: Linear regression helper
src/components/insights/TrendsChart.test.tsx    # NEW
```

---

### 7.3 Category Evolution Stacked Area Chart

**Subtasks:**

| ID | Task | Est. | Files |
|----|------|------|-------|
| 7.3.1 | Create `CategoryEvolutionChart` component | 45m | `src/components/insights/CategoryEvolutionChart.tsx` |
| 7.3.2 | Define color scheme for categories | 10m | `src/lib/categoryColors.ts` |
| 7.3.3 | Implement stacked area chart with Recharts | 30m | `src/components/insights/CategoryEvolutionChart.tsx` |
| 7.3.4 | Add legend with category breakdown | 15m | `src/components/insights/CategoryEvolutionChart.tsx` |
| 7.3.5 | Add "Classification required" placeholder | 15m | `src/components/insights/CategoryEvolutionChart.tsx` |
| 7.3.6 | Add insight text generation | 15m | `src/components/insights/CategoryEvolutionChart.tsx` |
| 7.3.7 | Write component tests | 25m | `src/components/insights/CategoryEvolutionChart.test.tsx` |

**File Changes:**

```
src/components/insights/CategoryEvolutionChart.tsx      # NEW
src/components/insights/CategoryEvolutionChart.test.tsx # NEW
src/lib/categoryColors.ts                               # NEW
```

---

### 7.4 Activity Heatmap

**Subtasks:**

| ID | Task | Est. | Files |
|----|------|------|-------|
| 7.4.1 | Create `ActivityHeatmapGrid` component | 45m | `src/components/insights/ActivityHeatmapGrid.tsx` |
| 7.4.2 | Implement 7x24 grid layout (day x hour) | 20m | `src/components/insights/ActivityHeatmapGrid.tsx` |
| 7.4.3 | Implement color intensity scale | 15m | `src/lib/heatmapColors.ts` |
| 7.4.4 | Add hover tooltip with session count + avg re-edit rate | 20m | `src/components/insights/ActivityHeatmapGrid.tsx` |
| 7.4.5 | Add click-to-filter behavior | 20m | `src/components/insights/ActivityHeatmapGrid.tsx` |
| 7.4.6 | Add insight text (peak efficiency times) | 15m | `src/components/insights/ActivityHeatmapGrid.tsx` |
| 7.4.7 | Add accessibility (aria labels, keyboard nav) | 20m | `src/components/insights/ActivityHeatmapGrid.tsx` |
| 7.4.8 | Write component tests | 25m | `src/components/insights/ActivityHeatmapGrid.test.tsx` |

**File Changes:**

```
src/components/insights/ActivityHeatmapGrid.tsx      # NEW
src/components/insights/ActivityHeatmapGrid.test.tsx # NEW
src/lib/heatmapColors.ts                             # NEW
```

---

### 7.5 Trends Tab Integration

**Subtasks:**

| ID | Task | Est. | Files |
|----|------|------|-------|
| 7.5.1 | Create `TrendsTab` container component | 20m | `src/components/insights/TrendsTab.tsx` |
| 7.5.2 | Wire up API fetch with React Query | 15m | `src/hooks/useTrendsData.ts` |
| 7.5.3 | Integrate with time range filter from Phase 5 | 15m | `src/components/insights/TrendsTab.tsx` |
| 7.5.4 | Add to Insights page tab navigation | 10m | `src/pages/InsightsPage.tsx` |
| 7.5.5 | Handle loading/error states | 15m | `src/components/insights/TrendsTab.tsx` |
| 7.5.6 | Write integration tests | 30m | `src/components/insights/TrendsTab.test.tsx` |

**File Changes:**

```
src/components/insights/TrendsTab.tsx      # NEW
src/components/insights/TrendsTab.test.tsx # NEW
src/hooks/useTrendsData.ts                 # NEW
src/pages/InsightsPage.tsx                 # Add Trends tab
```

---

## API Specification

### GET /api/insights/trends

**Query Parameters:**

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `metric` | string | No | `reedit_rate` | Metric to chart: `reedit_rate`, `sessions`, `lines`, `cost_per_line`, `prompts` |
| `range` | string | No | `6mo` | Time range: `3mo`, `6mo`, `1yr`, `all` |
| `granularity` | string | No | `week` | Data point interval: `day`, `week`, `month` |
| `from` | i64 | No | — | Custom range start (Unix timestamp) |
| `to` | i64 | No | — | Custom range end (Unix timestamp) |

**Request Example:**

```
GET /api/insights/trends?metric=reedit_rate&range=6mo&granularity=week
```

**Response:**

```typescript
interface TrendsResponse {
  // Time-series data for selected metric
  metric: string;                                  // "reedit_rate"
  dataPoints: Array<{
    date: string;                                  // ISO date "2026-01-15"
    value: number;                                 // 0.32 (32% re-edit rate)
  }>;
  average: number;                                 // 0.24 (average over period)
  trend: number;                                   // -52 (52% improvement)
  trendDirection: 'improving' | 'worsening' | 'stable';
  insight: string;                                 // "Your re-edit rate dropped 52%..."

  // Category evolution (requires classification - null if not available)
  categoryEvolution: Array<{
    date: string;                                  // ISO date "2026-01-15"
    codeWork: number;                              // 0.65 (65% of sessions)
    supportWork: number;                           // 0.20 (20%)
    thinkingWork: number;                          // 0.15 (15%)
  }> | null;
  categoryInsight: string | null;                  // "Thinking Work increased from 8% to 15%..."
  classificationRequired: boolean;                 // true if no classification data

  // Activity heatmap (always available)
  activityHeatmap: Array<{
    dayOfWeek: number;                             // 0-6 (Monday-Sunday)
    hourOfDay: number;                             // 0-23
    sessions: number;                              // Count of sessions
    avgReeditRate: number;                         // Average re-edit rate for this slot
  }>;
  heatmapInsight: string;                          // "Tuesday-Thursday mornings..."

  // Metadata
  periodStart: string;                             // ISO date
  periodEnd: string;                               // ISO date
  totalSessions: number;                           // Sessions in period
}
```

**Response Example:**

```json
{
  "metric": "reedit_rate",
  "dataPoints": [
    { "date": "2025-09-01", "value": 0.48 },
    { "date": "2025-10-01", "value": 0.42 },
    { "date": "2025-11-01", "value": 0.35 },
    { "date": "2025-12-01", "value": 0.30 },
    { "date": "2026-01-01", "value": 0.25 },
    { "date": "2026-02-01", "value": 0.22 }
  ],
  "average": 0.34,
  "trend": -54,
  "trendDirection": "improving",
  "insight": "Your re-edit rate dropped 54% over 6 months \u2014 you're writing significantly better prompts that produce correct code first try",

  "categoryEvolution": [
    { "date": "2025-09-01", "codeWork": 0.72, "supportWork": 0.20, "thinkingWork": 0.08 },
    { "date": "2025-10-01", "codeWork": 0.70, "supportWork": 0.18, "thinkingWork": 0.12 },
    { "date": "2025-11-01", "codeWork": 0.68, "supportWork": 0.17, "thinkingWork": 0.15 },
    { "date": "2025-12-01", "codeWork": 0.65, "supportWork": 0.20, "thinkingWork": 0.15 },
    { "date": "2026-01-01", "codeWork": 0.63, "supportWork": 0.22, "thinkingWork": 0.15 },
    { "date": "2026-02-01", "codeWork": 0.62, "supportWork": 0.23, "thinkingWork": 0.15 }
  ],
  "categoryInsight": "Thinking Work increased from 8% to 15% \u2014 you're doing more planning before coding (correlates with lower re-edit rate)",
  "classificationRequired": false,

  "activityHeatmap": [
    { "dayOfWeek": 0, "hourOfDay": 9, "sessions": 45, "avgReeditRate": 0.18 },
    { "dayOfWeek": 0, "hourOfDay": 10, "sessions": 52, "avgReeditRate": 0.20 },
    { "dayOfWeek": 1, "hourOfDay": 9, "sessions": 58, "avgReeditRate": 0.15 },
    { "dayOfWeek": 4, "hourOfDay": 21, "sessions": 12, "avgReeditRate": 0.45 }
  ],
  "heatmapInsight": "Tuesday-Thursday mornings are your sweet spot \u2014 35% better efficiency than evening sessions",

  "periodStart": "2025-09-01",
  "periodEnd": "2026-02-05",
  "totalSessions": 1247
}
```

**Error Responses:**

| Status | Condition | Body |
|--------|-----------|------|
| 400 | Invalid metric | `{ "error": "Invalid metric. Must be one of: reedit_rate, sessions, lines, cost_per_line, prompts" }` |
| 400 | Invalid range | `{ "error": "Invalid range. Must be one of: 3mo, 6mo, 1yr, all" }` |
| 400 | Invalid granularity | `{ "error": "Invalid granularity. Must be one of: day, week, month" }` |
| 400 | from > to | `{ "error": "'from' timestamp must be less than 'to'" }` |

---

## UI Mockups

### 7.2 Efficiency Over Time Line Chart

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Efficiency Over Time                        [ Re-edit Rate ▼ ] [ 6 mo ▼ ] │
│                                                                             │
│   0.50 ┤●                                                                   │
│        │ ╲                                                                  │
│   0.40 ┤  ╲___●                                                             │
│        │       ╲                                                            │
│   0.30 ┤        ╲___●___●                        Your avg: 0.24             │
│        │                 ╲                    ─ ─ ─ ─ ─ ─ ─ ─ ─             │
│   0.20 ┤                  ╲___●___●___●                                     │
│        │                                                                    │
│   0.10 ┤                                                                    │
│        └────────────────────────────────────────────────────────────────    │
│          Sep    Oct    Nov    Dec    Jan    Feb                             │
│                                                                             │
│  💡 Your re-edit rate dropped 52% over 6 months — you're writing            │
│     significantly better prompts that produce correct code first try        │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Metric Selector Options:**
- Re-edit rate (default) — Lower is better
- Session count — Activity level
- Lines produced — Productivity
- Cost per line — Efficiency (requires token pricing)
- Prompts per session — Iteration count

**Granularity Options:**
- Day — For 3mo range
- Week — For 6mo range (default)
- Month — For 1yr+ range

**Interactions:**
- Hover on data point: Tooltip with exact value and date
- Click on data point: Filter sessions to that period
- Responsive: Fewer data points on mobile

---

### 7.3 Category Evolution Stacked Area Chart

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Category Evolution                                             [ 6 mo ▼ ] │
│                                                                             │
│  100%┤░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   │
│      │████████████████████████████████████████████████████████████████░░░   │
│   75%┤████████████████████████████████████████████████████████████░░░░░░░   │
│      │████████████████████████████████████████████████████████░░░░░░░░░░░   │
│   50%┤██████████████████████████████████████████████████░░░░░░░░░░░░░░░░░   │
│      │████████████████████████████████████████████░░░░░░░░░░░░░░░░░░░░░░░   │
│   25%┤██████████████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   │
│      │██████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   │
│    0%└────────────────────────────────────────────────────────────────────   │
│        Sep    Oct    Nov    Dec    Jan    Feb                               │
│                                                                             │
│       ██ Code Work (62%)   ░░ Support Work (23%)   ▒▒ Thinking Work (15%)   │
│                                                                             │
│  💡 Thinking Work increased from 8% to 15% — you're doing more              │
│     planning before coding (correlates with lower re-edit rate)             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Category Colors:**
- Code Work: Blue (#3B82F6)
- Support Work: Amber (#F59E0B)
- Thinking Work: Purple (#8B5CF6)

**Classification Required State:**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Category Evolution                                             [ 6 mo ▼ ] │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                                                                       │  │
│  │                    📊 Classification Required                         │  │
│  │                                                                       │  │
│  │    Category breakdown requires session classification.                │  │
│  │    Go to System → Classify Sessions to enable this chart.            │  │
│  │                                                                       │  │
│  │                    [ Classify Sessions → ]                            │  │
│  │                                                                       │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Interactions:**
- Hover on area: Tooltip with percentage and session count for that category
- Click on category in legend: Toggle visibility
- Click on area: Filter to sessions of that category type

---

### 7.4 Activity Heatmap

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Activity Heatmap                                                           │
│                                                                             │
│           Mon   Tue   Wed   Thu   Fri   Sat   Sun                           │
│                                                                             │
│    6 AM    ░     ░     ░     ░     ░     ░     ░                             │
│    9 AM    ██    ███   ██    ███   ██    ░     ░      ← Peak efficiency     │
│   12 PM    ██    ██    ███   ██    ██    ░     ░                             │
│    3 PM    ██    ██    ██    ██    █     ░     ░                             │
│    6 PM    █     █     █     █     ░     ░     ░                             │
│    9 PM    █     ░     █     ░     ░     ░     ░      ← Higher re-edit      │
│   12 AM    ░     ░     ░     ░     ░     ░     ░                             │
│                                                                             │
│    ░ Low (0-5)   █ Medium (6-15)   ██ High (16-30)   ███ Peak (30+)         │
│                                                                             │
│  💡 Tuesday-Thursday mornings are your sweet spot — 35% better              │
│     efficiency than evening sessions                                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Color Scale:**
- Low: Gray (#374151)
- Medium: Light Blue (#60A5FA)
- High: Blue (#3B82F6)
- Peak: Dark Blue (#1E40AF)

**Tooltip Content (on hover):**

```
┌─────────────────────────┐
│  Tuesday, 9 AM - 10 AM  │
│  58 sessions            │
│  15% avg re-edit rate   │
│  Click to filter        │
└─────────────────────────┘
```

**Interactions:**
- Hover: Show tooltip with session count and avg re-edit rate
- Click: Navigate to search with time filter (e.g., `day:tuesday hour:9`)
- Keyboard: Arrow keys navigate cells, Enter to select

---

## React Components

### TrendsChart Component

```tsx
// src/components/insights/TrendsChart.tsx

import { useMemo } from 'react';
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ReferenceLine,
  ResponsiveContainer,
} from 'recharts';
import { TrendingDown, TrendingUp, Minus } from 'lucide-react';
import { cn } from '@/lib/utils';

interface DataPoint {
  date: string;
  value: number;
}

interface TrendsChartProps {
  data: DataPoint[];
  metric: string;
  average: number;
  trend: number;
  trendDirection: 'improving' | 'worsening' | 'stable';
  insight: string;
  isLowerBetter?: boolean;
}

export function TrendsChart({
  data,
  metric,
  average,
  trend,
  trendDirection,
  insight,
  isLowerBetter = false,
}: TrendsChartProps) {
  const trendLine = useMemo(() => calculateTrendLine(data), [data]);

  const formatValue = (value: number) => {
    if (metric === 'reedit_rate') return `${(value * 100).toFixed(0)}%`;
    if (metric === 'cost_per_line') return `$${value.toFixed(3)}`;
    return value.toLocaleString();
  };

  const TrendIcon = trendDirection === 'stable'
    ? Minus
    : (trendDirection === 'improving') === isLowerBetter
      ? TrendingDown
      : TrendingUp;

  const trendColor = trendDirection === 'stable'
    ? 'text-gray-500'
    : trendDirection === 'improving'
      ? 'text-green-500'
      : 'text-red-500';

  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg p-6">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold">Efficiency Over Time</h3>
        <div className="flex items-center gap-2">
          <TrendIcon className={cn('h-5 w-5', trendColor)} />
          <span className={cn('text-sm font-medium', trendColor)}>
            {Math.abs(trend)}%
          </span>
        </div>
      </div>

      <ResponsiveContainer width="100%" height={300}>
        <LineChart data={data} margin={{ top: 5, right: 30, left: 20, bottom: 5 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="#374151" opacity={0.3} />
          <XAxis
            dataKey="date"
            tickFormatter={(date) => new Date(date).toLocaleDateString('en-US', { month: 'short' })}
            stroke="#9CA3AF"
          />
          <YAxis
            tickFormatter={formatValue}
            stroke="#9CA3AF"
            domain={['dataMin - 0.05', 'dataMax + 0.05']}
          />
          <Tooltip
            formatter={(value: number) => [formatValue(value), metric.replace('_', ' ')]}
            labelFormatter={(date) => new Date(date).toLocaleDateString('en-US', {
              month: 'long',
              year: 'numeric',
            })}
            contentStyle={{
              backgroundColor: '#1F2937',
              border: 'none',
              borderRadius: '8px',
            }}
          />
          <ReferenceLine
            y={average}
            stroke="#9CA3AF"
            strokeDasharray="5 5"
            label={{
              value: `Avg: ${formatValue(average)}`,
              position: 'right',
              fill: '#9CA3AF',
              fontSize: 12,
            }}
          />
          <Line
            type="monotone"
            dataKey="value"
            stroke="#3B82F6"
            strokeWidth={2}
            dot={{ fill: '#3B82F6', strokeWidth: 2, r: 4 }}
            activeDot={{ r: 6, stroke: '#3B82F6', strokeWidth: 2 }}
          />
          {/* Trend line */}
          <Line
            data={trendLine}
            type="linear"
            dataKey="value"
            stroke="#F59E0B"
            strokeWidth={1}
            strokeDasharray="5 5"
            dot={false}
          />
        </LineChart>
      </ResponsiveContainer>

      <p className="mt-4 text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
        <span className="text-lg">💡</span>
        <span>{insight}</span>
      </p>
    </div>
  );
}

function calculateTrendLine(data: DataPoint[]): DataPoint[] {
  if (data.length < 2) return data;

  const n = data.length;
  const sumX = data.reduce((sum, _, i) => sum + i, 0);
  const sumY = data.reduce((sum, d) => sum + d.value, 0);
  const sumXY = data.reduce((sum, d, i) => sum + i * d.value, 0);
  const sumXX = data.reduce((sum, _, i) => sum + i * i, 0);

  const slope = (n * sumXY - sumX * sumY) / (n * sumXX - sumX * sumX);
  const intercept = (sumY - slope * sumX) / n;

  return [
    { date: data[0].date, value: intercept },
    { date: data[n - 1].date, value: intercept + slope * (n - 1) },
  ];
}
```

### CategoryEvolutionChart Component

```tsx
// src/components/insights/CategoryEvolutionChart.tsx

import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts';
import { Link } from 'react-router-dom';
import { Layers } from 'lucide-react';

interface CategoryDataPoint {
  date: string;
  codeWork: number;
  supportWork: number;
  thinkingWork: number;
}

interface CategoryEvolutionChartProps {
  data: CategoryDataPoint[] | null;
  insight: string | null;
  classificationRequired: boolean;
}

const CATEGORY_COLORS = {
  codeWork: '#3B82F6',      // Blue
  supportWork: '#F59E0B',   // Amber
  thinkingWork: '#8B5CF6',  // Purple
};

const CATEGORY_LABELS = {
  codeWork: 'Code Work',
  supportWork: 'Support Work',
  thinkingWork: 'Thinking Work',
};

export function CategoryEvolutionChart({
  data,
  insight,
  classificationRequired,
}: CategoryEvolutionChartProps) {
  if (classificationRequired || !data) {
    return (
      <div className="bg-white dark:bg-gray-900 rounded-lg p-6">
        <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
          <Layers className="h-5 w-5" />
          Category Evolution
        </h3>
        <div className="flex flex-col items-center justify-center py-12 px-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
          <div className="text-4xl mb-4">📊</div>
          <h4 className="text-lg font-medium mb-2">Classification Required</h4>
          <p className="text-sm text-gray-600 dark:text-gray-400 text-center mb-4">
            Category breakdown requires session classification.
            <br />
            Go to System → Classify Sessions to enable this chart.
          </p>
          <Link
            to="/system?tab=classification"
            className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
          >
            Classify Sessions →
          </Link>
        </div>
      </div>
    );
  }

  // Calculate latest percentages for legend
  const latest = data[data.length - 1];
  const legendPayload = [
    { value: `Code Work (${(latest.codeWork * 100).toFixed(0)}%)`, color: CATEGORY_COLORS.codeWork },
    { value: `Support Work (${(latest.supportWork * 100).toFixed(0)}%)`, color: CATEGORY_COLORS.supportWork },
    { value: `Thinking Work (${(latest.thinkingWork * 100).toFixed(0)}%)`, color: CATEGORY_COLORS.thinkingWork },
  ];

  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg p-6">
      <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
        <Layers className="h-5 w-5" />
        Category Evolution
      </h3>

      <ResponsiveContainer width="100%" height={300}>
        <AreaChart data={data} margin={{ top: 10, right: 30, left: 0, bottom: 0 }}>
          <CartesianGrid strokeDasharray="3 3" stroke="#374151" opacity={0.3} />
          <XAxis
            dataKey="date"
            tickFormatter={(date) => new Date(date).toLocaleDateString('en-US', { month: 'short' })}
            stroke="#9CA3AF"
          />
          <YAxis
            tickFormatter={(value) => `${(value * 100).toFixed(0)}%`}
            stroke="#9CA3AF"
            domain={[0, 1]}
          />
          <Tooltip
            formatter={(value: number, name: string) => [
              `${(value * 100).toFixed(1)}%`,
              CATEGORY_LABELS[name as keyof typeof CATEGORY_LABELS] || name,
            ]}
            labelFormatter={(date) => new Date(date).toLocaleDateString('en-US', {
              month: 'long',
              year: 'numeric',
            })}
            contentStyle={{
              backgroundColor: '#1F2937',
              border: 'none',
              borderRadius: '8px',
            }}
          />
          <Legend
            payload={legendPayload.map((item) => ({
              value: item.value,
              type: 'rect',
              color: item.color,
            }))}
          />
          <Area
            type="monotone"
            dataKey="thinkingWork"
            stackId="1"
            stroke={CATEGORY_COLORS.thinkingWork}
            fill={CATEGORY_COLORS.thinkingWork}
            fillOpacity={0.8}
          />
          <Area
            type="monotone"
            dataKey="supportWork"
            stackId="1"
            stroke={CATEGORY_COLORS.supportWork}
            fill={CATEGORY_COLORS.supportWork}
            fillOpacity={0.8}
          />
          <Area
            type="monotone"
            dataKey="codeWork"
            stackId="1"
            stroke={CATEGORY_COLORS.codeWork}
            fill={CATEGORY_COLORS.codeWork}
            fillOpacity={0.8}
          />
        </AreaChart>
      </ResponsiveContainer>

      {insight && (
        <p className="mt-4 text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
          <span className="text-lg">💡</span>
          <span>{insight}</span>
        </p>
      )}
    </div>
  );
}
```

### ActivityHeatmapGrid Component

```tsx
// src/components/insights/ActivityHeatmapGrid.tsx

import { useMemo } from 'react';
import * as Tooltip from '@radix-ui/react-tooltip';
import { useNavigate } from 'react-router-dom';
import { cn } from '@/lib/utils';

interface HeatmapCell {
  dayOfWeek: number;
  hourOfDay: number;
  sessions: number;
  avgReeditRate: number;
}

interface ActivityHeatmapGridProps {
  data: HeatmapCell[];
  insight: string;
}

const DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
const HOURS = [6, 9, 12, 15, 18, 21, 0];
const HOUR_LABELS = ['6 AM', '9 AM', '12 PM', '3 PM', '6 PM', '9 PM', '12 AM'];

function getIntensityClass(sessions: number, max: number): string {
  if (sessions === 0) return 'bg-gray-200 dark:bg-gray-700';
  const ratio = sessions / max;
  if (ratio < 0.25) return 'bg-blue-200 dark:bg-blue-900';
  if (ratio < 0.5) return 'bg-blue-400 dark:bg-blue-700';
  if (ratio < 0.75) return 'bg-blue-500 dark:bg-blue-600';
  return 'bg-blue-700 dark:bg-blue-500';
}

export function ActivityHeatmapGrid({ data, insight }: ActivityHeatmapGridProps) {
  const navigate = useNavigate();

  const { grid, maxSessions } = useMemo(() => {
    const grid: Map<string, HeatmapCell> = new Map();
    let maxSessions = 0;

    for (const cell of data) {
      const key = `${cell.dayOfWeek}-${cell.hourOfDay}`;
      grid.set(key, cell);
      if (cell.sessions > maxSessions) maxSessions = cell.sessions;
    }

    return { grid, maxSessions };
  }, [data]);

  const getCell = (day: number, hour: number): HeatmapCell | undefined => {
    return grid.get(`${day}-${hour}`);
  };

  const handleCellClick = (day: number, hour: number) => {
    const dayName = DAYS[day].toLowerCase();
    navigate(`/history?day=${dayName}&hour=${hour}`);
  };

  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg p-6">
      <h3 className="text-lg font-semibold mb-4">Activity Heatmap</h3>

      <div className="overflow-x-auto">
        <table className="w-full" role="grid" aria-label="Activity heatmap by day and hour">
          <thead>
            <tr>
              <th className="w-16" />
              {DAYS.map((day) => (
                <th
                  key={day}
                  className="text-xs font-medium text-gray-500 dark:text-gray-400 pb-2 text-center"
                >
                  {day}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {HOURS.map((hour, hourIdx) => (
              <tr key={hour}>
                <td className="text-xs font-medium text-gray-500 dark:text-gray-400 pr-2 text-right">
                  {HOUR_LABELS[hourIdx]}
                </td>
                {DAYS.map((_, dayIdx) => {
                  const cell = getCell(dayIdx, hour);
                  const sessions = cell?.sessions ?? 0;
                  const reeditRate = cell?.avgReeditRate ?? 0;

                  return (
                    <td key={dayIdx} className="p-0.5">
                      <Tooltip.Provider delayDuration={0}>
                        <Tooltip.Root>
                          <Tooltip.Trigger asChild>
                            <button
                              onClick={() => handleCellClick(dayIdx, hour)}
                              className={cn(
                                'w-8 h-8 rounded-sm cursor-pointer transition-all',
                                'hover:ring-2 hover:ring-blue-400 hover:ring-offset-1',
                                'focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-1',
                                getIntensityClass(sessions, maxSessions)
                              )}
                              aria-label={`${DAYS[dayIdx]} ${HOUR_LABELS[hourIdx]}: ${sessions} sessions, ${(reeditRate * 100).toFixed(0)}% re-edit rate`}
                            />
                          </Tooltip.Trigger>
                          <Tooltip.Portal>
                            <Tooltip.Content
                              className="bg-gray-900 text-white px-3 py-2 rounded-lg shadow-lg text-sm z-50"
                              sideOffset={5}
                            >
                              <div className="font-medium">
                                {DAYS[dayIdx]}, {HOUR_LABELS[hourIdx]} - {HOUR_LABELS[hourIdx + 1] || '6 AM'}
                              </div>
                              <div>{sessions} session{sessions !== 1 ? 's' : ''}</div>
                              <div>{(reeditRate * 100).toFixed(0)}% avg re-edit rate</div>
                              <div className="text-gray-400 text-xs mt-1">Click to filter</div>
                              <Tooltip.Arrow className="fill-gray-900" />
                            </Tooltip.Content>
                          </Tooltip.Portal>
                        </Tooltip.Root>
                      </Tooltip.Provider>
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Legend */}
      <div className="flex items-center gap-4 mt-4 text-xs text-gray-500 dark:text-gray-400">
        <div className="flex items-center gap-1">
          <div className="w-3 h-3 rounded-sm bg-gray-200 dark:bg-gray-700" />
          <span>Low (0-5)</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="w-3 h-3 rounded-sm bg-blue-200 dark:bg-blue-900" />
          <span>Medium</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="w-3 h-3 rounded-sm bg-blue-500 dark:bg-blue-600" />
          <span>High</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="w-3 h-3 rounded-sm bg-blue-700 dark:bg-blue-500" />
          <span>Peak</span>
        </div>
      </div>

      <p className="mt-4 text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
        <span className="text-lg">💡</span>
        <span>{insight}</span>
      </p>
    </div>
  );
}
```

---

## Chart Library

**Recommended: Recharts**

| Criteria | Recharts | Chart.js | D3 |
|----------|----------|----------|-----|
| React integration | Native | Wrapper | Manual |
| Bundle size | 47KB gzip | 62KB gzip | 44KB gzip |
| Line charts | Built-in | Built-in | Manual |
| Stacked area | Built-in | Built-in | Manual |
| Tooltip customization | Easy | Medium | Full control |
| Responsive | Built-in | Built-in | Manual |
| Learning curve | Low | Low | High |
| TypeScript support | Excellent | Good | Good |

**Installation:**

```bash
pnpm add recharts
```

**Why Recharts:**
1. Native React components — no wrapper needed
2. Excellent TypeScript support with `@types/recharts`
3. Built-in responsive container
4. Customizable tooltips with React components
5. Matches existing dashboard charts (if any)
6. Active maintenance (50k+ GitHub stars)

---

## Interactivity Specification

### Hover Behaviors

| Chart | Element | Tooltip Content | Delay |
|-------|---------|-----------------|-------|
| Efficiency Line | Data point | Value, date, metric name | 0ms |
| Category Area | Area segment | Percentage, count, category name | 0ms |
| Activity Heatmap | Cell | Day+hour, session count, avg re-edit | 0ms |

### Click Behaviors

| Chart | Element | Action |
|-------|---------|--------|
| Efficiency Line | Data point | Navigate to `/history?from={date}&to={date+period}` |
| Category Area | Area segment | Navigate to `/history?category={category}&from={date}` |
| Activity Heatmap | Cell | Navigate to `/history?day={day}&hour={hour}` |
| Category Legend | Legend item | Toggle category visibility |

### Keyboard Navigation

| Key | Heatmap Action |
|-----|----------------|
| Arrow keys | Move focus between cells |
| Enter | Activate click handler |
| Tab | Move to next interactive element |
| Escape | Close tooltip |

### Zoom Behaviors (Future)

For future enhancement:
- Mouse wheel zoom on line chart
- Drag to select date range
- Pinch-to-zoom on mobile

---

## State Management

### URL Parameters

| Param | Type | Default | Persisted |
|-------|------|---------|-----------|
| `metric` | string | `reedit_rate` | localStorage |
| `range` | string | `6mo` | localStorage |
| `granularity` | string | `week` | localStorage |

### React Query Keys

```typescript
// src/hooks/useTrendsData.ts

import { useQuery } from '@tanstack/react-query';

interface TrendsParams {
  metric: string;
  range: string;
  granularity: string;
  from?: number;
  to?: number;
}

export function useTrendsData(params: TrendsParams) {
  return useQuery({
    queryKey: ['insights', 'trends', params],
    queryFn: () => fetchTrends(params),
    staleTime: 5 * 60 * 1000,  // 5 minutes
    gcTime: 30 * 60 * 1000,    // 30 minutes cache
  });
}

async function fetchTrends(params: TrendsParams): Promise<TrendsResponse> {
  const searchParams = new URLSearchParams();
  searchParams.set('metric', params.metric);
  searchParams.set('range', params.range);
  searchParams.set('granularity', params.granularity);
  if (params.from) searchParams.set('from', params.from.toString());
  if (params.to) searchParams.set('to', params.to.toString());

  const response = await fetch(`/api/insights/trends?${searchParams}`);
  if (!response.ok) {
    throw new Error('Failed to fetch trends data');
  }
  return response.json();
}
```

### Local State

```typescript
// src/components/insights/TrendsTab.tsx

interface TrendsTabState {
  metric: 'reedit_rate' | 'sessions' | 'lines' | 'cost_per_line' | 'prompts';
  granularity: 'day' | 'week' | 'month';
  // range comes from shared InsightsPage context (Phase 5)
}
```

---

## Backend Implementation

### Database Queries

```rust
// crates/db/src/insights.rs

use crate::{Database, DbResult};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use serde::Serialize;
use ts_rs::TS;

/// Time-series data point for metric trends.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct MetricDataPoint {
    pub date: String,  // ISO date
    pub value: f64,
}

/// Category evolution data point.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CategoryDataPoint {
    pub date: String,
    pub code_work: f64,
    pub support_work: f64,
    pub thinking_work: f64,
}

/// Activity heatmap cell.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct HeatmapCell {
    pub day_of_week: u8,    // 0-6 (Monday-Sunday)
    pub hour_of_day: u8,    // 0-23
    pub sessions: i64,
    pub avg_reedit_rate: f64,
}

/// Full trends response.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct TrendsResponse {
    pub metric: String,
    pub data_points: Vec<MetricDataPoint>,
    pub average: f64,
    pub trend: f64,
    pub trend_direction: String,  // "improving" | "worsening" | "stable"
    pub insight: String,

    pub category_evolution: Option<Vec<CategoryDataPoint>>,
    pub category_insight: Option<String>,
    pub classification_required: bool,

    pub activity_heatmap: Vec<HeatmapCell>,
    pub heatmap_insight: String,

    pub period_start: String,
    pub period_end: String,
    pub total_sessions: i64,
}

impl Database {
    /// Get time-series data for a metric.
    pub async fn get_metric_timeseries(
        &self,
        metric: &str,
        from: i64,
        to: i64,
        granularity: &str,
    ) -> DbResult<Vec<MetricDataPoint>> {
        let group_by = match granularity {
            "day" => "date(last_message_at, 'unixepoch')",
            "week" => "strftime('%Y-%W', last_message_at, 'unixepoch')",
            "month" => "strftime('%Y-%m', last_message_at, 'unixepoch')",
            _ => "strftime('%Y-%W', last_message_at, 'unixepoch')",
        };

        let value_expr = match metric {
            "reedit_rate" => "CAST(SUM(reedited_files_count) AS REAL) / NULLIF(SUM(files_edited_count), 0)",
            "sessions" => "COUNT(*)",
            "lines" => "SUM(files_edited_count * 50)",  // Estimate 50 lines per file
            "cost_per_line" => "CAST(SUM(total_input_tokens + total_output_tokens) AS REAL) / NULLIF(SUM(files_edited_count * 50), 0) * 0.00001",
            "prompts" => "CAST(SUM(user_prompt_count) AS REAL) / COUNT(*)",
            _ => "CAST(SUM(reedited_files_count) AS REAL) / NULLIF(SUM(files_edited_count), 0)",
        };

        let sql = format!(
            r#"
            SELECT
                {group_by} as period,
                COALESCE({value_expr}, 0) as value
            FROM sessions
            WHERE is_sidechain = 0
              AND last_message_at >= ?1
              AND last_message_at <= ?2
            GROUP BY period
            ORDER BY period
            "#,
            group_by = group_by,
            value_expr = value_expr
        );

        let rows: Vec<(String, f64)> = sqlx::query_as(&sql)
            .bind(from)
            .bind(to)
            .fetch_all(self.pool())
            .await?;

        Ok(rows
            .into_iter()
            .map(|(date, value)| MetricDataPoint { date, value })
            .collect())
    }

    /// Get category evolution data (requires classification).
    pub async fn get_category_evolution(
        &self,
        from: i64,
        to: i64,
        granularity: &str,
    ) -> DbResult<Option<Vec<CategoryDataPoint>>> {
        // Check if any sessions are classified
        let (classified_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sessions WHERE category_l1 IS NOT NULL"
        )
        .fetch_one(self.pool())
        .await?;

        if classified_count == 0 {
            return Ok(None);
        }

        let group_by = match granularity {
            "day" => "date(last_message_at, 'unixepoch')",
            "week" => "strftime('%Y-%W', last_message_at, 'unixepoch')",
            "month" => "strftime('%Y-%m', last_message_at, 'unixepoch')",
            _ => "strftime('%Y-%W', last_message_at, 'unixepoch')",
        };

        let sql = format!(
            r#"
            SELECT
                {group_by} as period,
                CAST(SUM(CASE WHEN category_l1 = 'code' THEN 1 ELSE 0 END) AS REAL) / COUNT(*) as code_work,
                CAST(SUM(CASE WHEN category_l1 = 'support' THEN 1 ELSE 0 END) AS REAL) / COUNT(*) as support_work,
                CAST(SUM(CASE WHEN category_l1 = 'thinking' THEN 1 ELSE 0 END) AS REAL) / COUNT(*) as thinking_work
            FROM sessions
            WHERE is_sidechain = 0
              AND last_message_at >= ?1
              AND last_message_at <= ?2
              AND category_l1 IS NOT NULL
            GROUP BY period
            ORDER BY period
            "#,
            group_by = group_by
        );

        let rows: Vec<(String, f64, f64, f64)> = sqlx::query_as(&sql)
            .bind(from)
            .bind(to)
            .fetch_all(self.pool())
            .await?;

        Ok(Some(
            rows.into_iter()
                .map(|(date, code, support, thinking)| CategoryDataPoint {
                    date,
                    code_work: code,
                    support_work: support,
                    thinking_work: thinking,
                })
                .collect(),
        ))
    }

    /// Get activity heatmap data.
    pub async fn get_activity_heatmap(
        &self,
        from: i64,
        to: i64,
    ) -> DbResult<Vec<HeatmapCell>> {
        let rows: Vec<(i64, i64, i64, f64)> = sqlx::query_as(
            r#"
            SELECT
                CAST(strftime('%w', last_message_at, 'unixepoch') AS INTEGER) as dow,
                CAST(strftime('%H', last_message_at, 'unixepoch') AS INTEGER) as hour,
                COUNT(*) as sessions,
                COALESCE(
                    CAST(SUM(reedited_files_count) AS REAL) / NULLIF(SUM(files_edited_count), 0),
                    0
                ) as avg_reedit
            FROM sessions
            WHERE is_sidechain = 0
              AND last_message_at >= ?1
              AND last_message_at <= ?2
            GROUP BY dow, hour
            ORDER BY dow, hour
            "#
        )
        .bind(from)
        .bind(to)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(dow, hour, sessions, avg_reedit)| {
                // Convert SQLite's dow (0=Sunday) to our format (0=Monday)
                let adjusted_dow = if dow == 0 { 6 } else { dow - 1 };
                HeatmapCell {
                    day_of_week: adjusted_dow as u8,
                    hour_of_day: hour as u8,
                    sessions,
                    avg_reedit_rate: avg_reedit,
                }
            })
            .collect())
    }
}
```

### Route Handler

```rust
// crates/server/src/routes/insights.rs

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use chrono::{Duration, Utc};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use vibe_recall_db::insights::TrendsResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendsQuery {
    #[serde(default = "default_metric")]
    pub metric: String,
    #[serde(default = "default_range")]
    pub range: String,
    #[serde(default = "default_granularity")]
    pub granularity: String,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

fn default_metric() -> String { "reedit_rate".to_string() }
fn default_range() -> String { "6mo".to_string() }
fn default_granularity() -> String { "week".to_string() }

const VALID_METRICS: &[&str] = &["reedit_rate", "sessions", "lines", "cost_per_line", "prompts"];
const VALID_RANGES: &[&str] = &["3mo", "6mo", "1yr", "all"];
const VALID_GRANULARITIES: &[&str] = &["day", "week", "month"];

pub async fn get_trends(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TrendsQuery>,
) -> ApiResult<Json<TrendsResponse>> {
    // Validate inputs
    if !VALID_METRICS.contains(&query.metric.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid metric. Must be one of: {}",
            VALID_METRICS.join(", ")
        )));
    }
    if !VALID_RANGES.contains(&query.range.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid range. Must be one of: {}",
            VALID_RANGES.join(", ")
        )));
    }
    if !VALID_GRANULARITIES.contains(&query.granularity.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid granularity. Must be one of: {}",
            VALID_GRANULARITIES.join(", ")
        )));
    }

    // Calculate time bounds
    let now = Utc::now().timestamp();
    let (from, to) = match (query.from, query.to) {
        (Some(f), Some(t)) if f > t => {
            return Err(ApiError::BadRequest(
                "'from' timestamp must be less than 'to'".to_string(),
            ));
        }
        (Some(f), Some(t)) => (f, t),
        _ => {
            let duration = match query.range.as_str() {
                "3mo" => Duration::days(90),
                "6mo" => Duration::days(180),
                "1yr" => Duration::days(365),
                "all" => Duration::days(365 * 10), // 10 years
                _ => Duration::days(180),
            };
            (now - duration.num_seconds(), now)
        }
    };

    // Fetch data
    let data_points = state
        .db
        .get_metric_timeseries(&query.metric, from, to, &query.granularity)
        .await?;

    let category_evolution = state
        .db
        .get_category_evolution(from, to, &query.granularity)
        .await?;

    let activity_heatmap = state.db.get_activity_heatmap(from, to).await?;

    // Calculate statistics
    let (average, trend, trend_direction) = calculate_trend_stats(&data_points, &query.metric);
    let insight = generate_metric_insight(&query.metric, trend, &query.range);
    let category_insight = category_evolution.as_ref().map(|data| {
        generate_category_insight(data)
    });
    let heatmap_insight = generate_heatmap_insight(&activity_heatmap);

    // Get total sessions
    let (total_sessions,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sessions WHERE is_sidechain = 0 AND last_message_at >= ?1 AND last_message_at <= ?2"
    )
    .bind(from)
    .bind(to)
    .fetch_one(state.db.pool())
    .await?;

    let classification_required = category_evolution.is_none();

    Ok(Json(TrendsResponse {
        metric: query.metric,
        data_points,
        average,
        trend,
        trend_direction,
        insight,
        category_evolution,
        category_insight,
        classification_required,
        activity_heatmap,
        heatmap_insight,
        period_start: chrono::DateTime::from_timestamp(from, 0)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
        period_end: chrono::DateTime::from_timestamp(to, 0)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
        total_sessions,
    }))
}

fn calculate_trend_stats(
    data: &[vibe_recall_db::insights::MetricDataPoint],
    metric: &str,
) -> (f64, f64, String) {
    if data.is_empty() {
        return (0.0, 0.0, "stable".to_string());
    }

    let average = data.iter().map(|d| d.value).sum::<f64>() / data.len() as f64;

    if data.len() < 2 {
        return (average, 0.0, "stable".to_string());
    }

    let first = data.first().unwrap().value;
    let last = data.last().unwrap().value;

    let trend = if first == 0.0 {
        0.0
    } else {
        ((last - first) / first) * 100.0
    };

    // For reedit_rate and cost_per_line, lower is better
    let is_lower_better = metric == "reedit_rate" || metric == "cost_per_line";
    let direction = if trend.abs() < 5.0 {
        "stable"
    } else if (trend < 0.0) == is_lower_better {
        "improving"
    } else {
        "worsening"
    };

    (average, trend, direction.to_string())
}

fn generate_metric_insight(metric: &str, trend: f64, range: &str) -> String {
    let range_text = match range {
        "3mo" => "3 months",
        "6mo" => "6 months",
        "1yr" => "1 year",
        "all" => "all time",
        _ => "the selected period",
    };

    match metric {
        "reedit_rate" if trend < -20.0 => {
            format!(
                "Your re-edit rate dropped {:.0}% over {} — you're writing significantly better prompts that produce correct code first try",
                trend.abs(),
                range_text
            )
        }
        "reedit_rate" if trend > 20.0 => {
            format!(
                "Your re-edit rate increased {:.0}% over {} — consider being more specific in your prompts",
                trend,
                range_text
            )
        }
        "sessions" if trend > 50.0 => {
            format!(
                "Your session count grew {:.0}% over {} — you're using AI assistance more frequently",
                trend,
                range_text
            )
        }
        "prompts" if trend < -20.0 => {
            format!(
                "Your prompts per session dropped {:.0}% over {} — you're getting results faster",
                trend.abs(),
                range_text
            )
        }
        _ => format!(
            "Your {} changed by {:.0}% over {}",
            metric.replace('_', " "),
            trend,
            range_text
        ),
    }
}

fn generate_category_insight(data: &[vibe_recall_db::insights::CategoryDataPoint]) -> String {
    if data.len() < 2 {
        return "Not enough data to determine category trends".to_string();
    }

    let first = &data[0];
    let last = &data[data.len() - 1];

    let thinking_change = ((last.thinking_work - first.thinking_work) * 100.0).round() as i32;

    if thinking_change > 5 {
        format!(
            "Thinking Work increased from {:.0}% to {:.0}% — you're doing more planning before coding (correlates with lower re-edit rate)",
            first.thinking_work * 100.0,
            last.thinking_work * 100.0
        )
    } else if thinking_change < -5 {
        format!(
            "Thinking Work decreased from {:.0}% to {:.0}% — consider more upfront planning to reduce re-edits",
            first.thinking_work * 100.0,
            last.thinking_work * 100.0
        )
    } else {
        format!(
            "Work distribution is stable: {:.0}% Code, {:.0}% Support, {:.0}% Thinking",
            last.code_work * 100.0,
            last.support_work * 100.0,
            last.thinking_work * 100.0
        )
    }
}

fn generate_heatmap_insight(data: &[vibe_recall_db::insights::HeatmapCell]) -> String {
    if data.is_empty() {
        return "Not enough activity data to determine patterns".to_string();
    }

    // Find peak efficiency (lowest re-edit rate with significant sessions)
    let min_sessions = 5;
    let best_slots: Vec<_> = data
        .iter()
        .filter(|c| c.sessions >= min_sessions)
        .collect();

    if best_slots.is_empty() {
        return "Build more history to see your peak productivity times".to_string();
    }

    let overall_avg_reedit = best_slots.iter().map(|c| c.avg_reedit_rate).sum::<f64>()
        / best_slots.len() as f64;

    let best = best_slots
        .iter()
        .min_by(|a, b| a.avg_reedit_rate.partial_cmp(&b.avg_reedit_rate).unwrap())
        .unwrap();

    let worst = best_slots
        .iter()
        .max_by(|a, b| a.avg_reedit_rate.partial_cmp(&b.avg_reedit_rate).unwrap())
        .unwrap();

    let days = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
    let efficiency_diff = ((worst.avg_reedit_rate - best.avg_reedit_rate) / worst.avg_reedit_rate * 100.0).round() as i32;

    if efficiency_diff > 20 {
        format!(
            "{} {}:00 is your sweet spot — {:.0}% better efficiency than {} sessions",
            days[best.day_of_week as usize],
            best.hour_of_day,
            efficiency_diff,
            if worst.hour_of_day >= 18 { "evening" } else { "other" }
        )
    } else {
        format!(
            "Your productivity is consistent across the week (±{:.0}% variation)",
            efficiency_diff
        )
    }
}

/// Create the insights routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/insights/trends", get(get_trends))
}
```

---

## Testing Strategy

### Unit Tests (Rust)

```rust
// crates/db/src/insights.rs (tests module)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    #[tokio::test]
    async fn test_get_metric_timeseries_empty_db() {
        let db = Database::new_in_memory().await.unwrap();
        let now = chrono::Utc::now().timestamp();
        let data = db
            .get_metric_timeseries("reedit_rate", now - 86400 * 30, now, "week")
            .await
            .unwrap();
        assert!(data.is_empty());
    }

    #[tokio::test]
    async fn test_get_metric_timeseries_with_data() {
        let db = Database::new_in_memory().await.unwrap();
        // Insert test sessions spanning multiple weeks
        // ... (insert sessions)
        let now = chrono::Utc::now().timestamp();
        let data = db
            .get_metric_timeseries("sessions", now - 86400 * 30, now, "week")
            .await
            .unwrap();
        assert!(!data.is_empty());
    }

    #[tokio::test]
    async fn test_get_category_evolution_no_classification() {
        let db = Database::new_in_memory().await.unwrap();
        let now = chrono::Utc::now().timestamp();
        let data = db
            .get_category_evolution(now - 86400 * 30, now, "week")
            .await
            .unwrap();
        assert!(data.is_none());
    }

    #[tokio::test]
    async fn test_get_activity_heatmap() {
        let db = Database::new_in_memory().await.unwrap();
        // Insert test sessions at various times
        // ...
        let now = chrono::Utc::now().timestamp();
        let data = db.get_activity_heatmap(now - 86400 * 30, now).await.unwrap();
        // Verify structure
        for cell in &data {
            assert!(cell.day_of_week < 7);
            assert!(cell.hour_of_day < 24);
            assert!(cell.sessions >= 0);
            assert!(cell.avg_reedit_rate >= 0.0);
        }
    }

    #[test]
    fn test_calculate_trend_stats_improving() {
        let data = vec![
            MetricDataPoint { date: "2026-01".to_string(), value: 0.5 },
            MetricDataPoint { date: "2026-02".to_string(), value: 0.3 },
        ];
        let (avg, trend, direction) = calculate_trend_stats(&data, "reedit_rate");
        assert!((avg - 0.4).abs() < 0.01);
        assert!(trend < 0.0);  // Decreased
        assert_eq!(direction, "improving");  // Lower is better for reedit_rate
    }

    #[test]
    fn test_calculate_trend_stats_stable() {
        let data = vec![
            MetricDataPoint { date: "2026-01".to_string(), value: 0.3 },
            MetricDataPoint { date: "2026-02".to_string(), value: 0.31 },
        ];
        let (_, _, direction) = calculate_trend_stats(&data, "sessions");
        assert_eq!(direction, "stable");
    }
}
```

### Integration Tests (Rust)

```rust
// crates/server/src/routes/insights.rs (tests module)

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::{Request, StatusCode}};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_get_trends_default_params() {
        let db = vibe_recall_db::Database::new_in_memory().await.unwrap();
        let app = crate::create_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/insights/trends")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["metric"], "reedit_rate");
        assert!(json["dataPoints"].is_array());
        assert!(json["activityHeatmap"].is_array());
    }

    #[tokio::test]
    async fn test_get_trends_invalid_metric() {
        let db = vibe_recall_db::Database::new_in_memory().await.unwrap();
        let app = crate::create_app(db);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/insights/trends?metric=invalid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_get_trends_custom_range() {
        let db = vibe_recall_db::Database::new_in_memory().await.unwrap();
        let app = crate::create_app(db);

        let now = chrono::Utc::now().timestamp();
        let from = now - 86400 * 30;

        let response = app
            .oneshot(
                Request::builder()
                    .uri(&format!("/api/insights/trends?from={}&to={}", from, now))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
```

### Component Tests (React)

```tsx
// src/components/insights/TrendsChart.test.tsx

import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { TrendsChart } from './TrendsChart';

const mockData = [
  { date: '2026-01-01', value: 0.45 },
  { date: '2026-02-01', value: 0.30 },
];

describe('TrendsChart', () => {
  it('renders chart with data', () => {
    render(
      <TrendsChart
        data={mockData}
        metric="reedit_rate"
        average={0.375}
        trend={-33}
        trendDirection="improving"
        insight="Your re-edit rate dropped 33%"
      />
    );

    expect(screen.getByText('Efficiency Over Time')).toBeInTheDocument();
    expect(screen.getByText('33%')).toBeInTheDocument();
    expect(screen.getByText(/Your re-edit rate dropped 33%/)).toBeInTheDocument();
  });

  it('shows improving trend with correct icon', () => {
    render(
      <TrendsChart
        data={mockData}
        metric="reedit_rate"
        average={0.375}
        trend={-33}
        trendDirection="improving"
        insight="Test"
        isLowerBetter
      />
    );

    // TrendingDown icon should be used for improving when lower is better
    expect(screen.getByText('33%').parentElement).toHaveClass('text-green-500');
  });

  it('handles empty data', () => {
    render(
      <TrendsChart
        data={[]}
        metric="reedit_rate"
        average={0}
        trend={0}
        trendDirection="stable"
        insight="No data available"
      />
    );

    expect(screen.getByText('Efficiency Over Time')).toBeInTheDocument();
  });
});
```

```tsx
// src/components/insights/ActivityHeatmapGrid.test.tsx

import { render, screen, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { ActivityHeatmapGrid } from './ActivityHeatmapGrid';

const mockData = [
  { dayOfWeek: 0, hourOfDay: 9, sessions: 45, avgReeditRate: 0.18 },
  { dayOfWeek: 1, hourOfDay: 9, sessions: 58, avgReeditRate: 0.15 },
];

describe('ActivityHeatmapGrid', () => {
  it('renders grid with correct structure', () => {
    render(
      <MemoryRouter>
        <ActivityHeatmapGrid data={mockData} insight="Test insight" />
      </MemoryRouter>
    );

    expect(screen.getByText('Activity Heatmap')).toBeInTheDocument();
    expect(screen.getByText('Mon')).toBeInTheDocument();
    expect(screen.getByText('9 AM')).toBeInTheDocument();
  });

  it('shows tooltip on hover', async () => {
    render(
      <MemoryRouter>
        <ActivityHeatmapGrid data={mockData} insight="Test insight" />
      </MemoryRouter>
    );

    const cell = screen.getByRole('button', { name: /Monday 9 AM/i });
    fireEvent.mouseEnter(cell);

    expect(await screen.findByText('45 sessions')).toBeInTheDocument();
    expect(await screen.findByText('18% avg re-edit rate')).toBeInTheDocument();
  });

  it('navigates on click', () => {
    const navigateMock = vi.fn();
    vi.mock('react-router-dom', async () => ({
      ...(await vi.importActual('react-router-dom')),
      useNavigate: () => navigateMock,
    }));

    render(
      <MemoryRouter>
        <ActivityHeatmapGrid data={mockData} insight="Test" />
      </MemoryRouter>
    );

    const cell = screen.getByRole('button', { name: /Monday 9 AM/i });
    fireEvent.click(cell);

    expect(navigateMock).toHaveBeenCalledWith('/history?day=mon&hour=9');
  });
});
```

### E2E Tests

```typescript
// e2e/trends.spec.ts

import { test, expect } from '@playwright/test';

test.describe('Trends Tab', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/insights?tab=trends');
  });

  test('displays all three charts', async ({ page }) => {
    await expect(page.getByText('Efficiency Over Time')).toBeVisible();
    await expect(page.getByText('Category Evolution')).toBeVisible();
    await expect(page.getByText('Activity Heatmap')).toBeVisible();
  });

  test('metric selector changes chart', async ({ page }) => {
    await page.getByRole('combobox', { name: /metric/i }).click();
    await page.getByRole('option', { name: /Sessions/i }).click();

    await expect(page.getByText('Sessions')).toBeVisible();
  });

  test('heatmap cell shows tooltip on hover', async ({ page }) => {
    const cell = page.locator('[aria-label*="Tuesday 9 AM"]');
    await cell.hover();

    await expect(page.getByText(/sessions/)).toBeVisible();
    await expect(page.getByText(/avg re-edit rate/)).toBeVisible();
  });

  test('heatmap cell click navigates to history', async ({ page }) => {
    const cell = page.locator('[aria-label*="Tuesday 9 AM"]');
    await cell.click();

    await expect(page).toHaveURL(/\/history\?day=tue&hour=9/);
  });

  test('classification required state shows CTA', async ({ page }) => {
    // Assuming no classification data
    await expect(page.getByText('Classification Required')).toBeVisible();
    await expect(page.getByRole('link', { name: /Classify Sessions/i })).toBeVisible();
  });
});
```

---

## Acceptance Criteria

### AC-7.1: API Endpoint

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 7.1.1 | `GET /api/insights/trends` (no params) | Returns 200 with default metric=reedit_rate, range=6mo, granularity=week | [ ] |
| 7.1.2 | `GET /api/insights/trends?metric=sessions` | Returns session count data points | [ ] |
| 7.1.3 | `GET /api/insights/trends?metric=invalid` | Returns 400 Bad Request | [ ] |
| 7.1.4 | `GET /api/insights/trends?range=3mo` | Returns 3 months of data | [ ] |
| 7.1.5 | `GET /api/insights/trends?granularity=day` | Returns daily data points | [ ] |
| 7.1.6 | `GET /api/insights/trends?from=X&to=Y` | Returns data within custom range | [ ] |
| 7.1.7 | `from > to` | Returns 400 Bad Request | [ ] |
| 7.1.8 | No classification data | `categoryEvolution: null`, `classificationRequired: true` | [ ] |
| 7.1.9 | Response includes all fields | `dataPoints`, `average`, `trend`, `insight`, `activityHeatmap` | [ ] |

### AC-7.2: Efficiency Line Chart

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 7.2.1 | Chart renders | Line chart with data points visible | [ ] |
| 7.2.2 | Hover on data point | Tooltip shows value and date | [ ] |
| 7.2.3 | Average reference line | Dashed line at average value | [ ] |
| 7.2.4 | Trend indicator | Arrow up/down with percentage | [ ] |
| 7.2.5 | Insight text | Displayed below chart | [ ] |
| 7.2.6 | Metric selector | Changes chart data on selection | [ ] |
| 7.2.7 | Granularity selector | Changes data point frequency | [ ] |
| 7.2.8 | Empty data | Shows "No data for this period" | [ ] |
| 7.2.9 | Responsive | Chart resizes on window change | [ ] |

### AC-7.3: Category Evolution Chart

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 7.3.1 | Chart renders (with classification) | Stacked area chart visible | [ ] |
| 7.3.2 | Legend shows percentages | "Code Work (62%)", etc. | [ ] |
| 7.3.3 | Hover on area | Tooltip shows category and percentage | [ ] |
| 7.3.4 | Classification required state | Shows CTA to classify sessions | [ ] |
| 7.3.5 | Click CTA | Navigates to /system?tab=classification | [ ] |
| 7.3.6 | Insight text | Displayed below chart | [ ] |

### AC-7.4: Activity Heatmap

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 7.4.1 | Grid renders | 7x7 grid (days x hours) visible | [ ] |
| 7.4.2 | Color intensity | Darker = more sessions | [ ] |
| 7.4.3 | Hover tooltip | Shows day, hour, sessions, re-edit rate | [ ] |
| 7.4.4 | Click cell | Navigates to /history with filters | [ ] |
| 7.4.5 | Legend | Shows Low/Medium/High/Peak scale | [ ] |
| 7.4.6 | Insight text | Shows peak efficiency times | [ ] |
| 7.4.7 | Keyboard navigation | Arrow keys move focus, Enter activates | [ ] |
| 7.4.8 | Screen reader | Cells have descriptive aria-labels | [ ] |

### AC-7.5: Integration

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 7.5.1 | Tab navigation | Trends tab accessible from /insights | [ ] |
| 7.5.2 | Time range filter | Shared with other tabs | [ ] |
| 7.5.3 | Loading state | Skeleton shown while fetching | [ ] |
| 7.5.4 | Error state | Error message with retry button | [ ] |
| 7.5.5 | Data refresh | Updates when time range changes | [ ] |

### AC-7.6: Performance

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 7.6.1 | API response time | < 500ms for 1 year of data | [ ] |
| 7.6.2 | Chart render time | < 100ms after data received | [ ] |
| 7.6.3 | Tooltip latency | < 16ms (no jank) | [ ] |
| 7.6.4 | Memory usage | < 50MB for charts | [ ] |

---

## Dependencies

### Required Before This Phase

- **Phase 5 (Insights Core)** — Page layout, routing, time range context

### Optional (Graceful Degradation)

- **Phase 2 (Classification)** — Category evolution shows "Classification Required" without it

### External Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| recharts | ^2.12.x | Charts |
| @radix-ui/react-tooltip | ^1.0.x | Heatmap tooltips |

---

## Estimated Effort

| Task | Effort |
|------|--------|
| 7.1 Backend endpoint | 4h |
| 7.2 Efficiency chart | 3h |
| 7.3 Category evolution chart | 2.5h |
| 7.4 Activity heatmap | 3h |
| 7.5 Tab integration | 1.5h |
| Testing | 2h |
| **Total** | **16h** |

---

## Rollout Plan

1. **Backend first** — Deploy API endpoint, verify with curl
2. **Charts one by one** — Efficiency → Heatmap → Category Evolution
3. **Feature flag** — `VITE_FEATURE_TRENDS_TAB=true`
4. **Monitor** — Watch for slow queries, memory spikes
5. **Iterate** — Adjust insights text based on user feedback

---

## Future Enhancements

- **Zoom/pan** on line chart
- **Drill-down** on category areas
- **Export** charts as PNG
- **Compare periods** overlay
- **Anomaly detection** highlighting unusual patterns
