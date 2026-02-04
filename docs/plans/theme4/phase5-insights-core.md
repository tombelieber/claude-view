---
status: pending
date: 2026-02-05
purpose: Phase 5 Implementation Plan — Insights Page Core
depends_on: phase4-pattern-engine.md
---

# Phase 5: Insights Page Core

> **Goal:** Create the `/insights` page with hero insight, quick stats, and patterns tab. This is the primary user-facing surface for pattern discovery.

## Overview

Phase 5 builds the core Insights page that displays patterns computed by the Phase 4 pattern engine. The page follows a progressive disclosure model: hero insight at the top, quick stats row, then tabbed content starting with the Patterns tab.

### Dependencies

- **Phase 4 (Pattern Engine)**: Provides `GET /api/insights` endpoint with computed patterns and impact scores

### Design System

| Element | Specification |
|---------|---------------|
| Style | Data-Dense Dashboard |
| Primary Color | Blue data (#1E40AF, #3B82F6) |
| Accent Color | Amber highlights (#F59E0B) |
| Typography | Fira Code (monospace), Fira Sans (body) |
| Transitions | 150-300ms for filter animations |
| Interactions | Hover tooltips, smooth state changes |

---

## Tasks

### 5.1 Page Layout & Routing

**Files to modify:**
- `src/router.tsx` — Add `/insights` route
- `src/components/Sidebar.tsx` — Add Insights nav link
- `src/components/InsightsPage.tsx` — New file (main page component)

**Subtasks:**

#### 5.1.1 Add Route
```typescript
// src/router.tsx
import { InsightsPage } from './components/InsightsPage'

// Add to children array (between history and settings):
{ path: 'insights', element: <InsightsPage /> },
```

#### 5.1.2 Update Sidebar Navigation

Add Insights link between History and Projects:

```typescript
// src/components/Sidebar.tsx
import { Lightbulb } from 'lucide-react'

// In nav links section (after History link):
<Link
  to="/insights"
  className={cn(
    'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors',
    location.pathname === '/insights'
      ? 'bg-blue-500 text-white'
      : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
  )}
>
  <Lightbulb className="w-4 h-4" />
  <span className="font-medium">Insights</span>
</Link>
```

#### 5.1.3 Create Page Shell

```typescript
// src/components/InsightsPage.tsx
export function InsightsPage() {
  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-5xl mx-auto px-6 py-6">
        {/* Header with time range filter */}
        {/* Hero insight */}
        {/* Quick stats row */}
        {/* Tab bar */}
        {/* Tab content */}
      </div>
    </div>
  )
}
```

---

### 5.2 Hero Insight Component

**Files to create:**
- `src/components/insights/HeroInsight.tsx`

**Purpose:** Display the #1 highest-impact insight prominently at the top of the page.

#### Component Props

```typescript
interface HeroInsightProps {
  insight: {
    id: string
    title: string           // e.g., "TDD sessions have 52% lower re-edit rate"
    description: string     // Plain-English explanation
    impactScore: number     // 0-1, determines "high impact" status
    category: string        // e.g., "skill_usage", "time_of_day"
    metric: {
      value: number
      comparison: number
      unit: string          // e.g., "re-edit rate", "edits/file"
      improvement: number   // percentage improvement
    }
    sampleSize: number      // sessions analyzed
  } | null
  isLoading: boolean
  onViewDetails?: () => void
}
```

#### Visual Design

```
┌─ YOUR #1 INSIGHT ────────────────────────────────────────────────────────┐
│                                                                           │
│  ⚡ Sessions using TDD skill have 52% lower re-edit rate                 │
│                                                                           │
│  You used TDD in 12 sessions with 0.18 re-edit rate vs 0.38 without.     │
│  Structured workflows produce better first-attempt code.                  │
│                                                                           │
│  Based on 247 sessions                               [ View Details → ]   │
└───────────────────────────────────────────────────────────────────────────┘
```

#### Implementation

```typescript
// src/components/insights/HeroInsight.tsx
import { Zap, ArrowRight } from 'lucide-react'
import { cn } from '../../lib/utils'

export function HeroInsight({ insight, isLoading, onViewDetails }: HeroInsightProps) {
  if (isLoading) {
    return <HeroInsightSkeleton />
  }

  if (!insight) {
    return <HeroInsightEmpty />
  }

  return (
    <div className="bg-gradient-to-r from-blue-50 to-blue-100 dark:from-blue-900/20 dark:to-blue-800/20 rounded-xl border border-blue-200 dark:border-blue-800 p-6">
      <div className="flex items-center gap-2 mb-3">
        <Zap className="w-4 h-4 text-amber-500" />
        <span className="text-xs font-semibold text-blue-600 dark:text-blue-400 uppercase tracking-wider">
          Your #1 Insight
        </span>
      </div>

      <h2 className="text-xl font-semibold text-gray-900 dark:text-gray-100 mb-2">
        {insight.title}
      </h2>

      <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">
        {insight.description}
      </p>

      <div className="flex items-center justify-between">
        <span className="text-xs text-gray-500 dark:text-gray-500">
          Based on {insight.sampleSize} sessions
        </span>

        {onViewDetails && (
          <button
            onClick={onViewDetails}
            className="inline-flex items-center gap-1 text-sm font-medium text-blue-600 dark:text-blue-400 hover:text-blue-700 dark:hover:text-blue-300 transition-colors"
          >
            View Details
            <ArrowRight className="w-4 h-4" />
          </button>
        )}
      </div>
    </div>
  )
}
```

#### States

| State | Behavior |
|-------|----------|
| Loading | Pulsing skeleton with gradient background |
| Empty | "Not enough data yet" message with illustration |
| Populated | Full insight with title, description, sample size |

---

### 5.3 Quick Stats Cards

**Files to create:**
- `src/components/insights/QuickStatsRow.tsx`
- `src/components/insights/QuickStatCard.tsx`

**Purpose:** Three cards showing high-level metrics at a glance.

#### Card Types

| Card | Metrics | Visual |
|------|---------|--------|
| Session Breakdown | Total, with commits, exploration | Session count stats |
| Efficiency | Edits/file, re-edit rate, trend | Single number + trend arrow |
| Best Time | Best day + time slot | Text with improvement % |

#### Visual Design

```
┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐
│  SESSIONS           │  │  EFFICIENCY         │  │  PEAK TIME          │
│                     │  │                     │  │                     │
│     247             │  │     1.4             │  │  Tuesday            │
│   sessions          │  │   edits/file        │  │  9-11am             │
│                     │  │                     │  │                     │
│  189 committed      │  │  ↓ 23% improving    │  │  43% more           │
│  58 exploration     │  │  0.18 re-edit rate  │  │  efficient          │
│                     │  │                     │  │                     │
│  ~32 min avg        │  │                     │  │                     │
└─────────────────────┘  └─────────────────────┘  └─────────────────────┘
```

#### Component Props

Note: These props are mapped from Phase 4 API response (see `mapApiToUi` in use-insights hook).

```typescript
interface QuickStatsRowProps {
  workBreakdown: {
    total: number           // total sessions in period
    withCommits: number     // sessions that resulted in commits
    exploration: number     // sessions without commits (exploration/lookup)
    avgMinutes: number      // average session duration in minutes
  } | null
  efficiency: {
    editsPerFile: number    // avgEditVelocity from API
    trend: number           // trendPct from API (percentage change)
    reeditRate: number      // avgReeditRate from API
    trendDirection: 'improving' | 'stable' | 'declining'
  } | null
  patterns: {
    bestTime: string        // timeSlot from API (e.g., "9-11am")
    bestDay: string         // dayOfWeek from API (e.g., "Tuesday")
    improvementPct: number  // how much better vs worst time
  } | null
  isLoading: boolean
}
```

#### Implementation Pattern

```typescript
// src/components/insights/QuickStatCard.tsx
interface QuickStatCardProps {
  title: string
  icon: React.ReactNode
  children: React.ReactNode
  isLoading?: boolean
}

export function QuickStatCard({ title, icon, children, isLoading }: QuickStatCardProps) {
  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-5">
      <div className="flex items-center gap-2 mb-4">
        <span className="text-gray-400">{icon}</span>
        <h3 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">
          {title}
        </h3>
      </div>
      {isLoading ? <QuickStatSkeleton /> : children}
    </div>
  )
}
```

---

### 5.4 Patterns Tab

**Files to create:**
- `src/components/insights/PatternsTabs.tsx`
- `src/components/insights/PatternsTab.tsx`
- `src/components/insights/PatternCard.tsx`

**Purpose:** Feed-style display of all patterns, grouped by impact level.

#### Impact Grouping

| Group | Score Range | Icon | Color |
|-------|-------------|------|-------|
| HIGH IMPACT | > 0.7 | `ArrowUp` | Blue (#1E40AF) |
| MEDIUM IMPACT | 0.4 - 0.7 | `ArrowRight` | Gray (#6B7280) |
| OBSERVATIONS | < 0.4 | `Eye` | Light gray (#9CA3AF) |

#### Pattern Card Props

```typescript
interface PatternCardProps {
  pattern: {
    id: string
    category: string        // e.g., "skill_usage", "time_of_day"
    title: string
    insight: string         // Plain English "so what?"
    impactScore: number
    metric: {
      label: string
      value: number
      comparison: number
      unit: string
      improvement: number   // percentage
    }
    sampleSize: number
    confidence: 'high' | 'medium' | 'low'
  }
  onClick?: () => void
}
```

#### Visual Design

```
┌──────────────────────────────────────────────────────────────────────────┐
│  ⬆ HIGH IMPACT                                                           │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │  Skill Usage                                      Impact: ████████░ │  │
│  │                                                                    │  │
│  │  TDD sessions: 0.18 re-edit rate vs 0.38 without (+52% better)    │  │
│  │                                                                    │  │
│  │  ████████████████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░  │  │
│  │  TDD (0.18)                            No TDD (0.38)              │  │
│  │                                                                    │  │
│  │  Based on 247 sessions • High confidence                          │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                          │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │  Time of Day                                      Impact: ███████░░ │  │
│  │                                                                    │  │
│  │  Morning (9-11am): 1.2 edits/file vs Evening (10pm+): 2.1         │  │
│  │  You're 43% more efficient in the morning.                        │  │
│  │                                                                    │  │
│  │  ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │  │
│  │  Morning (1.2)                              Evening (2.1)         │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

#### Tab Bar Component

```typescript
// src/components/insights/PatternsTabs.tsx
type TabId = 'patterns' | 'trends' | 'categories' | 'benchmarks'

interface PatternsTabsProps {
  activeTab: TabId
  onTabChange: (tab: TabId) => void
  disabledTabs?: TabId[]  // For tabs not yet implemented
}

export function PatternsTabs({ activeTab, onTabChange, disabledTabs = [] }: PatternsTabsProps) {
  const tabs: { id: TabId; label: string }[] = [
    { id: 'patterns', label: 'Patterns' },
    { id: 'trends', label: 'Trends' },
    { id: 'categories', label: 'Categories' },
    { id: 'benchmarks', label: 'Benchmarks' },
  ]

  return (
    <div className="flex items-center gap-1 border-b border-gray-200 dark:border-gray-700">
      {tabs.map(tab => (
        <button
          key={tab.id}
          onClick={() => onTabChange(tab.id)}
          disabled={disabledTabs.includes(tab.id)}
          className={cn(
            'px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-px',
            activeTab === tab.id
              ? 'text-blue-600 dark:text-blue-400 border-blue-600 dark:border-blue-400'
              : 'text-gray-500 dark:text-gray-400 border-transparent hover:text-gray-700 dark:hover:text-gray-300',
            disabledTabs.includes(tab.id) && 'opacity-50 cursor-not-allowed'
          )}
        >
          {tab.label}
        </button>
      ))}
    </div>
  )
}
```

---

### 5.5 Time Range Filter

**Files to create:**
- `src/components/insights/TimeRangeFilter.tsx`
- `src/hooks/use-insights.ts`

**Purpose:** Allow users to filter insights by time range.

#### Filter Options

| Option | Value | Description |
|--------|-------|-------------|
| This Week | `7d` | Current calendar week |
| This Month | `30d` | Default selection |
| Last 90 Days | `90d` | Quarter view |
| All Time | `all` | No time filter |
| Custom | `custom` | Date picker (stretch goal) |

#### Component Props

```typescript
type TimeRange = '7d' | '30d' | '90d' | 'all' | 'custom'

interface TimeRangeFilterProps {
  value: TimeRange
  onChange: (range: TimeRange) => void
  customRange?: { start: Date; end: Date }
  onCustomRangeChange?: (range: { start: Date; end: Date }) => void
}
```

#### Implementation

```typescript
// src/components/insights/TimeRangeFilter.tsx
import { Calendar } from 'lucide-react'
import { cn } from '../../lib/utils'

const TIME_RANGE_OPTIONS: { value: TimeRange; label: string }[] = [
  { value: '7d', label: 'This Week' },
  { value: '30d', label: 'This Month' },
  { value: '90d', label: 'Last 90 Days' },
  { value: 'all', label: 'All Time' },
]

export function TimeRangeFilter({ value, onChange }: TimeRangeFilterProps) {
  return (
    <div className="inline-flex items-center gap-1 p-1 bg-gray-100 dark:bg-gray-800 rounded-lg">
      {TIME_RANGE_OPTIONS.map(option => (
        <button
          key={option.value}
          onClick={() => onChange(option.value)}
          className={cn(
            'px-3 py-1.5 text-sm font-medium rounded-md transition-all duration-150',
            value === option.value
              ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
              : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'
          )}
        >
          {option.label}
        </button>
      ))}
    </div>
  )
}
```

---

## UI Mockups

### Full Page — Populated State

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│  ⚡ Insights                                        [ This Week | *Month* | ... ]│
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│  ┌─ YOUR #1 INSIGHT ────────────────────────────────────────────────────────┐  │
│  │  Sessions using TDD skill have 52% lower re-edit rate                   │  │
│  │                                                                          │  │
│  │  You used TDD in 12 sessions with 0.18 re-edit rate vs 0.38 without.    │  │
│  │  Structured workflows produce better first-attempt code.                 │  │
│  │                                                                          │  │
│  │  Based on 247 sessions                              [ View Details → ]   │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                                                                 │
│  ┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐     │
│  │  SESSIONS           │  │  EFFICIENCY         │  │  PEAK TIME          │     │
│  │                     │  │                     │  │                     │     │
│  │     247             │  │     1.4             │  │  Tuesday            │     │
│  │   sessions          │  │   edits/file        │  │  9-11am             │     │
│  │                     │  │                     │  │                     │     │
│  │  189 committed      │  │  ↓ 23% improving    │  │  43% more           │     │
│  │  58 exploration     │  │  0.18 re-edit rate  │  │  efficient          │     │
│  │                     │  │                     │  │                     │     │
│  │  ~32 min avg        │  │                     │  │                     │     │
│  │                     │  │                     │  │                     │     │
│  └─────────────────────┘  └─────────────────────┘  └─────────────────────┘     │
│                                                                                 │
│  [*Patterns*]  [ Trends ]  [ Categories ]  [ Benchmarks ]                       │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│  ⬆ HIGH IMPACT (3)                                                             │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │  Skill Usage — TDD sessions 52% better                                  │  │
│  │  ████████████████████████████████████░░░░░░░░░░░░  0.18 vs 0.38        │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │  Time of Day — Morning 43% more efficient                               │  │
│  │  ████████████████████████████░░░░░░░░░░░░░░░░░░░  1.2 vs 2.1 edits/file│  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                                                                 │
│  ➡ MEDIUM IMPACT (5)                                                           │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │  Session Duration — 15-45 min optimal                                   │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                                                                 │
│  👁 OBSERVATIONS (12)                                                          │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │  Model Selection — Opus used for 78% of architecture sessions           │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### Loading State

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│  ⚡ Insights                                        [ This Week | *Month* | ... ]│
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ← pulsing     │  │
│  │  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░                                    │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
│                                                                                 │
│  ┌────────────────────┐  ┌────────────────────┐  ┌────────────────────┐        │
│  │  ░░░░░░░░░░        │  │  ░░░░░░░░░░        │  │  ░░░░░░░░░░        │        │
│  │  ░░░░░░░░░░░░░░    │  │  ░░░░░░░░░░░░░░    │  │  ░░░░░░░░░░░░░░    │        │
│  └────────────────────┘  └────────────────────┘  └────────────────────┘        │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### Empty State (Not Enough Data)

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│  ⚡ Insights                                        [ This Week | *Month* | ... ]│
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│                          ┌──────────────────────┐                               │
│                          │       📊             │                               │
│                          │                      │                               │
│                          │  Not Enough Data     │                               │
│                          │                      │                               │
│                          │  We need at least    │                               │
│                          │  20 sessions to      │                               │
│                          │  detect patterns.    │                               │
│                          │                      │                               │
│                          │  You have 7 sessions │                               │
│                          │  indexed.            │                               │
│                          │                      │                               │
│                          │  [View Sessions →]   │                               │
│                          └──────────────────────┘                               │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### Error State

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│  ⚡ Insights                                        [ This Week | *Month* | ... ]│
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│                          ┌──────────────────────┐                               │
│                          │       ⚠️             │                               │
│                          │                      │                               │
│                          │  Unable to Load      │                               │
│                          │  Insights            │                               │
│                          │                      │                               │
│                          │  Something went      │                               │
│                          │  wrong while         │                               │
│                          │  analyzing your      │                               │
│                          │  sessions.           │                               │
│                          │                      │                               │
│                          │  [ Try Again ]       │                               │
│                          └──────────────────────┘                               │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## React Components

### Component Hierarchy

```
InsightsPage
├── InsightsHeader
│   ├── Title + Icon
│   └── TimeRangeFilter
├── HeroInsight
│   ├── HeroInsightSkeleton (loading)
│   └── HeroInsightEmpty (empty)
├── QuickStatsRow
│   ├── SessionsCard
│   │   └── Session count stats
│   ├── EfficiencyCard
│   │   └── TrendIndicator + re-edit rate
│   └── PeakTimeCard
├── PatternsTabs
│   ├── Tab: Patterns (active)
│   ├── Tab: Trends (disabled, Phase 7)
│   ├── Tab: Categories (disabled, Phase 6)
│   └── Tab: Benchmarks (disabled, Phase 8)
└── PatternsTab (tab content)
    ├── PatternGroup (HIGH IMPACT)
    │   └── PatternCard[]
    ├── PatternGroup (MEDIUM IMPACT)
    │   └── PatternCard[]
    └── PatternGroup (OBSERVATIONS)
        └── PatternCard[]
```

### File Structure

```
src/components/
├── InsightsPage.tsx           # Main page container
└── insights/
    ├── index.ts               # Barrel export
    ├── HeroInsight.tsx        # Hero insight component
    ├── QuickStatsRow.tsx      # Stats row container
    ├── QuickStatCard.tsx      # Reusable stat card
    ├── SessionsCard.tsx       # Session counts breakdown
    ├── EfficiencyCard.tsx     # Edits/file with trend + re-edit rate
    ├── PeakTimeCard.tsx       # Best day/time patterns
    ├── PatternsTabs.tsx       # Tab bar
    ├── PatternsTab.tsx        # Patterns tab content
    ├── PatternCard.tsx        # Individual pattern card
    ├── PatternGroup.tsx       # Impact-grouped patterns
    ├── TimeRangeFilter.tsx    # Segmented control
    └── InsightsSkeleton.tsx   # Loading states
```

---

## API Integration

### Hook: use-insights

**File:** `src/hooks/use-insights.ts`

#### TimeRange to Unix Timestamp Conversion

Phase 4 API expects `from`/`to` unix timestamps. This helper converts TimeRange strings:

```typescript
// src/hooks/use-insights.ts

/**
 * Convert TimeRange to unix timestamps (seconds) for API
 */
function timeRangeToTimestamps(
  timeRange: TimeRange,
  customRange?: { start: Date; end: Date }
): { from: number; to: number } {
  const now = Math.floor(Date.now() / 1000)  // Unix timestamp in seconds

  switch (timeRange) {
    case '7d':
      return { from: now - 7 * 86400, to: now }
    case '30d':
      return { from: now - 30 * 86400, to: now }
    case '90d':
      return { from: now - 90 * 86400, to: now }
    case 'all':
      return { from: 0, to: now }
    case 'custom':
      if (!customRange) {
        return { from: now - 30 * 86400, to: now }  // fallback to 30d
      }
      return {
        from: Math.floor(customRange.start.getTime() / 1000),
        to: Math.floor(customRange.end.getTime() / 1000),
      }
  }
}
```

#### API Response Mapping

The Phase 4 API returns a different structure. Map it to the UI types:

```typescript
// Phase 4 API response structure (from phase4-pattern-engine.md)
interface Phase4Response {
  topInsight: GeneratedInsight | null
  overview: {
    workBreakdown: {
      totalSessions: number
      withCommits: number
      exploration: number
      avgSessionMinutes: number
    }
    efficiency: {
      avgReeditRate: number
      avgEditVelocity: number
      trend: 'improving' | 'stable' | 'declining'
      trendPct: number
    }
    bestTime: {
      dayOfWeek: string
      timeSlot: string
      improvementPct: number
    }
  }
  patterns: {
    high: GeneratedInsight[]
    medium: GeneratedInsight[]
    observations: GeneratedInsight[]
  }
  classificationStatus: {
    classified: number
    total: number
    pendingClassification: number
    classificationPct: number
  }
  meta: {
    computedAt: number
    timeRangeStart: number
    timeRangeEnd: number
    patternsEvaluated: number
    patternsReturned: number
  }
}

// UI types for components
interface InsightsData {
  heroInsight: HeroInsightData | null
  quickStats: {
    workBreakdown: WorkBreakdownData | null
    efficiency: EfficiencyData | null
    patterns: PatternStatsData | null
  }
  patterns: GeneratedInsight[]
  meta: {
    totalSessions: number
    filteredSessions: number
    minSessionsRequired: number
    hasEnoughData: boolean
  }
}

/**
 * Map Phase 4 API response to UI data structure
 */
function mapApiToUi(api: Phase4Response): InsightsData {
  const totalSessions = api.overview.workBreakdown.totalSessions
  const hasEnoughData = totalSessions >= 20

  return {
    heroInsight: api.topInsight ? {
      id: api.topInsight.patternId,
      title: api.topInsight.title,
      description: api.topInsight.body,
      impactScore: api.topInsight.impactScore,
      category: api.topInsight.category,
      metric: {
        value: api.topInsight.evidence.comparisonValues['value'] ?? 0,
        comparison: api.topInsight.evidence.comparisonValues['comparison'] ?? 0,
        unit: 're-edit rate',  // derive from pattern type if needed
        improvement: api.topInsight.evidence.comparisonValues['improvement_pct'] ?? 0,
      },
      sampleSize: api.topInsight.evidence.sampleSize,
    } : null,

    quickStats: {
      // Note: Phase 4 provides session counts, not category percentages
      // Display as session breakdown instead of work type breakdown
      workBreakdown: {
        total: api.overview.workBreakdown.totalSessions,
        withCommits: api.overview.workBreakdown.withCommits,
        exploration: api.overview.workBreakdown.exploration,
        avgMinutes: api.overview.workBreakdown.avgSessionMinutes,
      },
      efficiency: {
        editsPerFile: api.overview.efficiency.avgEditVelocity,
        trend: api.overview.efficiency.trendPct,
        reeditRate: api.overview.efficiency.avgReeditRate,
        trendDirection: api.overview.efficiency.trend,
      },
      patterns: {
        bestTime: api.overview.bestTime.timeSlot,
        bestDay: api.overview.bestTime.dayOfWeek,
        improvementPct: api.overview.bestTime.improvementPct,
      },
    },

    patterns: [
      ...api.patterns.high,
      ...api.patterns.medium,
      ...api.patterns.observations,
    ],

    meta: {
      totalSessions,
      filteredSessions: api.meta.patternsReturned,
      minSessionsRequired: 20,
      hasEnoughData,
    },
  }
}
```

#### Hook Implementation

```typescript
import { useQuery } from '@tanstack/react-query'

interface UseInsightsOptions {
  timeRange: TimeRange
  customRange?: { start: Date; end: Date }
}

export function useInsights({ timeRange, customRange }: UseInsightsOptions) {
  const { from, to } = timeRangeToTimestamps(timeRange, customRange)

  return useQuery({
    queryKey: ['insights', from, to],
    queryFn: async (): Promise<InsightsData> => {
      const params = new URLSearchParams({
        from: from.toString(),
        to: to.toString(),
      })

      const response = await fetch(`/api/insights?${params}`)
      if (!response.ok) {
        throw new Error('Failed to fetch insights')
      }

      const apiResponse: Phase4Response = await response.json()
      return mapApiToUi(apiResponse)
    },
    staleTime: 60_000, // 1 minute
    refetchOnWindowFocus: false,
  })
}
```

### Expected API Response (from Phase 4)

The Phase 4 API (`GET /api/insights`) returns this structure. The `use-insights` hook maps it to UI types.

```json
{
  "topInsight": {
    "patternId": "skill_tdd_reedit",
    "category": "Workflow Patterns",
    "title": "TDD Skill Usage",
    "body": "TDD sessions: 0.18 re-edit rate vs 0.38 without (+52% better)",
    "recommendation": "Consider using TDD for complex features",
    "impactScore": 0.85,
    "impactTier": "high",
    "evidence": {
      "sampleSize": 247,
      "timeRangeDays": 30,
      "comparisonValues": {
        "value": 0.18,
        "comparison": 0.38,
        "improvement_pct": 52
      }
    }
  },
  "overview": {
    "workBreakdown": {
      "totalSessions": 247,
      "withCommits": 189,
      "exploration": 58,
      "avgSessionMinutes": 32
    },
    "efficiency": {
      "avgReeditRate": 0.18,
      "avgEditVelocity": 1.4,
      "trend": "improving",
      "trendPct": -23
    },
    "bestTime": {
      "dayOfWeek": "Tuesday",
      "timeSlot": "9-11am",
      "improvementPct": 43
    }
  },
  "patterns": {
    "high": [
      {
        "patternId": "skill_tdd_reedit",
        "category": "Workflow Patterns",
        "title": "TDD Skill Usage",
        "body": "TDD sessions: 0.18 re-edit rate vs 0.38 without (+52% better)",
        "recommendation": "Consider using TDD for complex features",
        "impactScore": 0.85,
        "impactTier": "high",
        "evidence": { "sampleSize": 247, "timeRangeDays": 30, "comparisonValues": {} }
      }
    ],
    "medium": [],
    "observations": []
  },
  "classificationStatus": {
    "classified": 200,
    "total": 247,
    "pendingClassification": 47,
    "classificationPct": 81
  },
  "meta": {
    "computedAt": 1738745123,
    "timeRangeStart": 1736153123,
    "timeRangeEnd": 1738745123,
    "patternsEvaluated": 60,
    "patternsReturned": 15
  }
}
```

---

## State Management

### Page-Level State

```typescript
// InsightsPage.tsx
const [timeRange, setTimeRange] = useState<TimeRange>('30d')
const [activeTab, setActiveTab] = useState<TabId>('patterns')
const [expandedPatternId, setExpandedPatternId] = useState<string | null>(null)

// URL sync for time range (optional, for shareability)
const [searchParams, setSearchParams] = useSearchParams()
useEffect(() => {
  const rangeFromUrl = searchParams.get('range') as TimeRange | null
  if (rangeFromUrl && ['7d', '30d', '90d', 'all'].includes(rangeFromUrl)) {
    setTimeRange(rangeFromUrl)
  }
}, [])

const handleTimeRangeChange = (range: TimeRange) => {
  setTimeRange(range)
  setSearchParams({ range })
}
```

### Data Flow

```
User selects time range
    ↓
TimeRangeFilter.onChange(range)
    ↓
InsightsPage.setTimeRange(range)
    ↓
useInsights queryKey changes
    ↓
React Query refetches /api/insights?range=X
    ↓
Components re-render with new data
```

---

## Testing Strategy

### Unit Tests

**File:** `src/components/insights/HeroInsight.test.tsx`

```typescript
import { render, screen } from '@testing-library/react'
import { HeroInsight } from './HeroInsight'

describe('HeroInsight', () => {
  it('renders loading skeleton when isLoading is true', () => {
    render(<HeroInsight insight={null} isLoading={true} />)
    expect(screen.getByRole('status')).toBeInTheDocument()
  })

  it('renders empty state when insight is null', () => {
    render(<HeroInsight insight={null} isLoading={false} />)
    expect(screen.getByText(/not enough data/i)).toBeInTheDocument()
  })

  it('renders insight content when populated', () => {
    const mockInsight = {
      id: 'test-1',
      title: 'Test Insight',
      description: 'Test description',
      impactScore: 0.85,
      category: 'test',
      metric: { value: 0.18, comparison: 0.38, unit: 're-edit rate', improvement: 52 },
      sampleSize: 100,
    }
    render(<HeroInsight insight={mockInsight} isLoading={false} />)
    expect(screen.getByText('Test Insight')).toBeInTheDocument()
    expect(screen.getByText('Based on 100 sessions')).toBeInTheDocument()
  })

  it('calls onViewDetails when button is clicked', async () => {
    const onViewDetails = vi.fn()
    const mockInsight = { /* ... */ }
    render(<HeroInsight insight={mockInsight} isLoading={false} onViewDetails={onViewDetails} />)

    await userEvent.click(screen.getByText('View Details'))
    expect(onViewDetails).toHaveBeenCalled()
  })
})
```

### Integration Tests

**File:** `src/components/InsightsPage.test.tsx`

```typescript
import { render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { MemoryRouter } from 'react-router-dom'
import { InsightsPage } from './InsightsPage'
import { http, HttpResponse } from 'msw'
import { setupServer } from 'msw/node'

const mockInsightsResponse = { /* full response object */ }

const server = setupServer(
  http.get('/api/insights', () => HttpResponse.json(mockInsightsResponse))
)

beforeAll(() => server.listen())
afterEach(() => server.resetHandlers())
afterAll(() => server.close())

describe('InsightsPage', () => {
  it('renders all sections when data loads', async () => {
    render(
      <QueryClientProvider client={new QueryClient()}>
        <MemoryRouter>
          <InsightsPage />
        </MemoryRouter>
      </QueryClientProvider>
    )

    await waitFor(() => {
      expect(screen.getByText('YOUR #1 INSIGHT')).toBeInTheDocument()
      expect(screen.getByText('WORK BREAKDOWN')).toBeInTheDocument()
      expect(screen.getByText('HIGH IMPACT')).toBeInTheDocument()
    })
  })

  it('changes time range and refetches', async () => {
    // ...
  })
})
```

### E2E Tests (Playwright)

**File:** `e2e/insights.spec.ts`

```typescript
import { test, expect } from '@playwright/test'

test.describe('Insights Page', () => {
  test('navigates from sidebar', async ({ page }) => {
    await page.goto('/')
    await page.click('text=Insights')
    await expect(page).toHaveURL('/insights')
    await expect(page.locator('h1')).toContainText('Insights')
  })

  test('time range filter updates content', async ({ page }) => {
    await page.goto('/insights')
    await page.click('text=This Week')
    await expect(page).toHaveURL('/insights?range=7d')
  })

  test('shows empty state when not enough data', async ({ page }) => {
    // Mock API to return hasEnoughData: false
    await page.route('/api/insights*', route =>
      route.fulfill({
        json: { meta: { hasEnoughData: false, totalSessions: 5, minSessionsRequired: 20 } }
      })
    )
    await page.goto('/insights')
    await expect(page.locator('text=Not Enough Data')).toBeVisible()
  })
})
```

---

## Acceptance Criteria

### Must Have

- [ ] `/insights` route accessible and renders
- [ ] Insights link appears in sidebar navigation (between History and Projects)
- [ ] Hero insight displays #1 pattern with title, description, sample size
- [ ] Quick stats row shows 3 cards: Work Breakdown, Efficiency, Patterns
- [ ] Patterns tab displays patterns grouped by impact (HIGH, MEDIUM, OBSERVATIONS)
- [ ] Time range filter works (This Week, This Month, Last 90 Days, All Time)
- [ ] Loading state shows skeleton animations
- [ ] Empty state shows when < 20 sessions
- [ ] Error state shows with retry button

### Should Have

- [ ] Pattern cards show visual bar chart for metric comparison
- [ ] Impact score visualized on each pattern card
- [ ] Confidence indicator (high/medium/low) on pattern cards
- [ ] Smooth 150-300ms transitions on filter changes
- [ ] URL reflects current time range for shareability

### Nice to Have

- [ ] Custom date range picker
- [ ] Expand/collapse pattern cards for more details
- [ ] Pattern detail modal/drawer
- [ ] Keyboard navigation between patterns

---

## Implementation Notes

### Styling Considerations

1. **Dark Mode**: All components must support dark mode using Tailwind's `dark:` prefix
2. **Responsive**: Cards should stack on mobile (single column)
3. **Accessibility**: All interactive elements need focus styles and ARIA labels

### Performance Considerations

1. **Data Caching**: Use React Query's `staleTime` to avoid unnecessary refetches
2. **Lazy Loading**: Consider lazy loading tab content for Trends/Categories/Benchmarks
3. **Skeleton Matching**: Skeleton dimensions should match actual content to prevent layout shift

### Future Considerations

- Phase 6 will add content to the Categories tab
- Phase 7 will add content to the Trends tab
- Phase 8 will add content to the Benchmarks tab
- These tabs should be rendered as "Coming Soon" or disabled until implemented

---

## Related Documents

- Master design: `../2026-02-05-theme4-chat-insights-design.md`
- Dependency: `phase4-pattern-engine.md` (provides `/api/insights` endpoint)
- Progress tracker: `PROGRESS.md`
