---
status: pending
date: 2026-02-05
theme: "Theme 2: Dashboard & Analytics Enhancements"
---

# E2E Test Plan — Dashboard Analytics Features

> **Purpose:** Comprehensive end-to-end test plan for verifying all features implemented in the `feature/dashboard-analytics` branch. This document serves as a checklist for manual and automated testing using Playwright MCP tools.

## Test Infrastructure

- **Framework:** Playwright
- **Config:** `/playwright.config.ts`
- **Test Directory:** `/e2e/`
- **Base URL:** `http://localhost:47892`
- **Server:** `cargo run -p vibe-recall-server`
- **Timeout:** 180s (accounts for filesystem scanning)

## Test Data Requirements

Before running tests, ensure:
- [ ] `~/.claude/` directory exists with session data
- [ ] At least 1 project with sessions exists
- [ ] Backend server is running at port 47892
- [ ] Database has been indexed (run sync at least once)

---

## Feature 2B: Heatmap Hover Tooltips

### Test Scenarios

#### TC-2B-01: Tooltip Appears on Hover
- [ ] **Navigate:** Dashboard page (`/`)
- [ ] **Wait:** Activity calendar loads
- [ ] **Action:** Hover over a calendar cell with sessions > 0
- [ ] **Verify:** Tooltip appears with:
  - Day name and date (e.g., "Wed, Jan 29, 2026")
  - Session count (e.g., "8 sessions")
  - "Click to filter" hint in muted color
- [ ] **Verify:** Tooltip has arrow pointing to cell

```typescript
// Playwright MCP test
await browser_navigate({ url: 'http://localhost:47892/' })
await browser_snapshot({})
// Find calendar cell with sessions, hover over it
await browser_hover({ ref: '<calendar-cell-ref>', element: 'Calendar cell with sessions' })
await browser_snapshot({}) // Verify tooltip content
```

#### TC-2B-02: Tooltip 150ms Close Delay
- [ ] **Action:** Hover over calendar cell (tooltip appears)
- [ ] **Action:** Move mouse away from cell
- [ ] **Verify:** Tooltip stays visible for ~150ms before fading
- [ ] **Verify:** No flickering when moving between adjacent cells

#### TC-2B-03: Keyboard Navigation (ARIA Grid Pattern)
- [ ] **Action:** Tab to focus the calendar grid
- [ ] **Action:** Press Arrow keys (Right, Left, Down, Up)
- [ ] **Verify:** Focus moves to adjacent cells
- [ ] **Verify:** Tooltip shows on focused cell
- [ ] **Action:** Press Home key
- [ ] **Verify:** Focus moves to first cell
- [ ] **Action:** Press End key
- [ ] **Verify:** Focus moves to last cell

#### TC-2B-04: Accessibility Attributes
- [ ] **Verify:** Calendar has `role="grid"`
- [ ] **Verify:** Cells have `role="gridcell"`
- [ ] **Verify:** Each cell has `aria-label` with date and session count
- [ ] **Verify:** Tooltip has `role="tooltip"`
- [ ] **Verify:** Cells have `aria-describedby` linking to tooltip

#### TC-2B-05: Zero Sessions Cell
- [ ] **Action:** Hover over a cell with 0 sessions
- [ ] **Verify:** Tooltip shows "0 sessions"
- [ ] **Verify:** Cell has appropriate empty styling (gray-50)

---

## Feature 2C: Sync Button Redesign

### Test Scenarios

#### TC-2C-01: Labeled Sync Button Visibility
- [ ] **Navigate:** Dashboard page (`/`)
- [ ] **Verify:** Status bar footer is visible
- [ ] **Verify:** Button shows "Sync Now" label (not just icon)
- [ ] **Verify:** RefreshCw icon is visible next to label
- [ ] **Verify:** Button has `data-testid="sync-button"`

```typescript
await browser_navigate({ url: 'http://localhost:47892/' })
await browser_snapshot({})
// Verify sync button with label is visible
```

#### TC-2C-02: Sync Button Click - Success Toast
- [ ] **Action:** Click "Sync Now" button
- [ ] **Verify:** Button text changes to "Syncing..."
- [ ] **Verify:** Icon spins (animate-spin class)
- [ ] **Verify:** Button is disabled during sync
- [ ] **Wait:** Sync completes
- [ ] **Verify:** Success toast appears in top-right
- [ ] **Verify:** Toast content includes:
  - "Sync completed" title
  - Session count (e.g., "6,748 sessions")
  - New sessions count if any (e.g., "+12 new")
  - Commit count if any (e.g., "1,247 commits")
- [ ] **Verify:** Toast auto-dismisses after 4 seconds

```typescript
await browser_click({ ref: '<sync-button-ref>', element: 'Sync Now button' })
await browser_snapshot({}) // Verify syncing state
await browser_wait_for({ text: 'Sync completed' })
await browser_snapshot({}) // Verify success toast
```

#### TC-2C-03: Sync Button - Error Toast with Retry
- [ ] **Setup:** Block `/api/sync/git` endpoint to return 500
- [ ] **Action:** Click "Sync Now" button
- [ ] **Verify:** Error toast appears
- [ ] **Verify:** Toast shows error message
- [ ] **Verify:** "Retry" action button is visible
- [ ] **Action:** Click "Retry"
- [ ] **Verify:** Sync retries

#### TC-2C-04: Sync Button - Conflict Toast (Already Syncing)
- [ ] **Action:** Click "Sync Now" button
- [ ] **Action:** Immediately click again
- [ ] **Verify:** Info toast shows "Sync already in progress"
- [ ] **Verify:** Toast auto-dismisses after 3 seconds

#### TC-2C-05: Status Bar Data Display
- [ ] **Navigate:** Dashboard page
- [ ] **Verify:** Status bar shows:
  - "Last update: X ago" (relative time)
  - Session count (e.g., "6,742 sessions")
  - Commit icon + count (if commits exist)

---

## Feature 2A: Dashboard Time Range Filter

### Test Scenarios

#### TC-2A-01: Segmented Control Rendering (Desktop)
- [ ] **Navigate:** Dashboard page at desktop viewport (>=1024px)
- [ ] **Verify:** Segmented control is visible with options:
  - 7d, 30d, 90d, All, Custom
- [ ] **Verify:** Default selection is "30d"

```typescript
await browser_resize({ width: 1200, height: 800 })
await browser_navigate({ url: 'http://localhost:47892/' })
await browser_snapshot({})
// Verify segmented control with all options
```

#### TC-2A-02: Dropdown Selector Rendering (Mobile)
- [ ] **Navigate:** Dashboard page at mobile viewport (<640px)
- [ ] **Verify:** Native `<select>` dropdown is rendered
- [ ] **Verify:** Dropdown shows current selection
- [ ] **Verify:** Chevron icon is visible
- [ ] **Verify:** Touch target is at least 44x44px

```typescript
await browser_resize({ width: 375, height: 667 })
await browser_navigate({ url: 'http://localhost:47892/' })
await browser_snapshot({})
// Verify dropdown selector
```

#### TC-2A-03: Time Range Selection Updates Dashboard
- [ ] **Action:** Select "7d" option
- [ ] **Verify:** Dashboard stats update
- [ ] **Verify:** URL updates to include `?range=7d`
- [ ] **Verify:** Activity calendar shows last 7 days data
- [ ] **Action:** Select "All"
- [ ] **Verify:** URL updates to `?range=all`
- [ ] **Verify:** All historical data is shown

#### TC-2A-04: Custom Date Range Picker
- [ ] **Action:** Select "Custom" option
- [ ] **Verify:** Date picker popover appears
- [ ] **Verify:** Start and end date inputs are visible
- [ ] **Action:** Select custom date range
- [ ] **Action:** Click "Apply"
- [ ] **Verify:** URL updates with `from` and `to` params
- [ ] **Verify:** Dashboard filters to selected range

#### TC-2A-05: URL Persistence
- [ ] **Navigate:** `/?range=7d`
- [ ] **Verify:** "7d" is selected in control
- [ ] **Navigate:** `/?from=1706400000&to=1707004800`
- [ ] **Verify:** "Custom" is selected
- [ ] **Verify:** Date picker shows the correct dates

#### TC-2A-06: localStorage Persistence
- [ ] **Action:** Select "90d"
- [ ] **Action:** Refresh page (no URL params)
- [ ] **Verify:** "90d" is still selected (from localStorage)

#### TC-2A-07: API Endpoint with Time Range
- [ ] **Request:** `GET /api/stats/dashboard?from=1706400000&to=1707004800`
- [ ] **Verify:** Response includes `periodStart`, `periodEnd`
- [ ] **Verify:** Session count reflects filtered data
- [ ] **Verify:** Response includes `dataStartDate` (earliest session)

```typescript
// API test
const response = await fetch('/api/stats/dashboard?from=1706400000&to=1707004800')
const data = await response.json()
// Verify response structure
```

---

## Feature 2E: Storage Overview (Settings Page)

### Test Scenarios

#### TC-2E-01: Storage Section Visibility
- [ ] **Navigate:** Settings page (`/settings`)
- [ ] **Verify:** "Storage Overview" section exists
- [ ] **Verify:** Section shows loading state initially
- [ ] **Wait:** Data loads
- [ ] **Verify:** Storage bars appear

```typescript
await browser_navigate({ url: 'http://localhost:47892/settings' })
await browser_snapshot({})
// Verify Storage Overview section
```

#### TC-2E-02: Storage Breakdown Progress Bars
- [ ] **Verify:** Three progress bars are visible:
  - "JSONL Sessions" with size (e.g., "512 MB")
  - "SQLite Database" with size
  - "Search Index" with size
- [ ] **Verify:** Progress bars show correct proportions
- [ ] **Verify:** Total storage is displayed below bars

#### TC-2E-03: Counts Grid Display
- [ ] **Verify:** 6 stat cards are visible:
  - Sessions count
  - Projects count
  - Commits count
  - Oldest Session date
  - Index Built timestamp
  - Last Git Sync timestamp
- [ ] **Verify:** Numbers are formatted with commas (e.g., "6,742")
- [ ] **Verify:** Dates are human-readable

#### TC-2E-04: Responsive Grid Layout
- [ ] **Mobile (<640px):** 2 columns grid
- [ ] **Tablet (640-1024px):** 3 columns grid
- [ ] **Desktop (>=1024px):** 6 columns grid

```typescript
// Test at different viewports
await browser_resize({ width: 375, height: 667 }) // Mobile
await browser_snapshot({})

await browser_resize({ width: 768, height: 1024 }) // Tablet
await browser_snapshot({})

await browser_resize({ width: 1200, height: 800 }) // Desktop
await browser_snapshot({})
```

#### TC-2E-05: Rebuild Index Button
- [ ] **Verify:** "Rebuild Index" button is visible
- [ ] **Verify:** Button has RefreshCw icon
- [ ] **Verify:** Button is enabled
- [ ] **Action:** Click "Rebuild Index"
- [ ] **Verify:** Button shows loading spinner
- [ ] **Verify:** Toast appears "Index rebuild started"
- [ ] **Wait:** Rebuild completes
- [ ] **Verify:** Success icon appears briefly

#### TC-2E-06: Clear Cache Button (Disabled)
- [ ] **Verify:** "Clear Cache" button is visible
- [ ] **Verify:** Button is disabled (grayed out)
- [ ] **Verify:** Button has `title` tooltip explaining it's not implemented

#### TC-2E-07: Index Performance Stats
- [ ] **Verify:** "Index Performance" section exists
- [ ] **Verify:** Shows "Last deep index: Xs" duration
- [ ] **Verify:** Shows throughput (e.g., "X.X MB/s")
- [ ] **Verify:** Shows last git sync timestamp

#### TC-2E-08: API Endpoint - Storage Stats
- [ ] **Request:** `GET /api/stats/storage`
- [ ] **Verify:** Response includes:
  - `jsonlBytes`, `sqliteBytes`, `indexBytes`
  - `sessionCount`, `projectCount`, `commitCount`
  - `oldestSessionDate`, `lastIndexAt`, `lastGitSyncAt`
  - `lastIndexDurationMs`, `lastIndexSessionCount`

---

## Feature 2D: AI Generation Breakdown

### Test Scenarios

#### TC-2D-01: AI Generation Section Visibility
- [ ] **Navigate:** Dashboard page (`/`)
- [ ] **Verify:** "AI Code Generation" section exists (if data available)
- [ ] **Verify:** Section has Sparkles icon in header

```typescript
await browser_navigate({ url: 'http://localhost:47892/' })
await browser_snapshot({})
// Look for AI Generation section
```

#### TC-2D-02: Metric Cards Display
- [ ] **Verify:** Three metric cards are visible:
  1. **Lines Generated**
     - Primary value: lines added (e.g., "+12,847")
     - Sub-value: lines removed (e.g., "-3,201 removed")
     - Footer: net lines (e.g., "net: +9,646")
  2. **Files Created**
     - Primary value: count
     - Sub-value: "written by AI"
  3. **Tokens Used**
     - Primary value: total tokens (e.g., "4.8M")
     - Sub-value: input tokens
     - Footer: output tokens

#### TC-2D-03: Token Usage by Model Progress Bars
- [ ] **Verify:** "Token Usage by Model" section exists
- [ ] **Verify:** Progress bars show for each model:
  - Model name (friendly format, e.g., "Claude Opus 4.5")
  - Percentage bar
  - Token count (e.g., "3.5M")
- [ ] **Verify:** Bars are sorted by usage (highest first)

#### TC-2D-04: Top Projects by Token Usage
- [ ] **Verify:** "Top Projects by Token Usage" section exists
- [ ] **Verify:** Progress bars show for top 5 projects
- [ ] **Verify:** Project names are displayed
- [ ] **Verify:** Token counts are formatted correctly

#### TC-2D-05: Responsive Layout
- [ ] **Mobile:** Metric cards stack vertically (1 column)
- [ ] **Tablet:** Metric cards 2 columns
- [ ] **Desktop:** Metric cards 3 columns
- [ ] **All sizes:** Token breakdown sections are 2 columns on md+

#### TC-2D-06: Time Range Filtering Integration
- [ ] **Action:** Change time range to "7d"
- [ ] **Verify:** AI Generation stats update for 7-day period
- [ ] **Action:** Change time range to "All"
- [ ] **Verify:** Stats show all-time data

#### TC-2D-07: API Endpoint - AI Generation Stats
- [ ] **Request:** `GET /api/stats/ai-generation`
- [ ] **Verify:** Response includes:
  - `linesAdded`, `linesRemoved`, `filesCreated`
  - `totalInputTokens`, `totalOutputTokens`
  - `tokensByModel[]` array with model breakdown
  - `tokensByProject[]` array with project breakdown
- [ ] **Request:** `GET /api/stats/ai-generation?from=X&to=Y`
- [ ] **Verify:** Response is filtered by time range

#### TC-2D-08: Empty State
- [ ] **Setup:** No AI generation data available
- [ ] **Verify:** AI Generation section is hidden (not shown)

---

## Database Migration & Indexes

### Test Scenarios

#### TC-DB-01: Timestamp Index Performance
- [ ] **Request:** `GET /api/stats/dashboard?from=X&to=Y`
- [ ] **Measure:** Response time < 500ms for large datasets
- [ ] **Verify:** Uses idx_sessions_timestamp index

#### TC-DB-02: Primary Model Column
- [ ] **Verify:** Sessions have `primary_model` field populated
- [ ] **Verify:** Model breakdown queries work correctly

---

## Feature Flags

### Test Scenarios

#### TC-FF-01: Feature Flags Configuration
- [ ] **Read:** `src/config/features.ts`
- [ ] **Verify:** All 5 features are defined:
  - `timeRange`
  - `heatmapTooltip`
  - `syncRedesign`
  - `aiGeneration`
  - `storageOverview`
- [ ] **Verify:** All default to `true`

#### TC-FF-02: Disable Feature via Environment Variable
- [ ] **Start server:** `VITE_FEATURE_TIME_RANGE=false bun run dev`
- [ ] **Navigate:** Dashboard page
- [ ] **Verify:** Time range selector is NOT visible
- [ ] **Verify:** Other features still work

#### TC-FF-03: Feature Flag Helpers
- [ ] **Verify:** `isFeatureEnabled('timeRange')` returns boolean
- [ ] **Verify:** `getEnabledFeatures()` returns array
- [ ] **Verify:** `getDisabledFeatures()` returns array

---

## Backend Observability (Metrics)

### Test Scenarios

#### TC-OBS-01: Prometheus Metrics Endpoint
- [ ] **Request:** `GET /metrics`
- [ ] **Verify:** Response is Prometheus text format
- [ ] **Verify:** HTTP status 200

```typescript
const response = await fetch('http://localhost:47892/metrics')
const text = await response.text()
// Verify contains Prometheus format metrics
```

#### TC-OBS-02: Metrics Content
- [ ] **Verify:** Metrics include (if implemented):
  - Request counters by endpoint
  - Request duration histograms
  - Error counts
  - Index operation metrics

---

## Mobile Responsive Design

### Test Scenarios

#### TC-MR-01: Mobile Viewport (375x667)
- [ ] **Navigate:** Dashboard page
- [ ] **Verify:** No horizontal scrolling
- [ ] **Verify:** All content fits in viewport
- [ ] **Verify:** Touch targets >= 44x44px

```typescript
await browser_resize({ width: 375, height: 667 })
await browser_navigate({ url: 'http://localhost:47892/' })
await browser_take_screenshot({ type: 'png', filename: 'mobile-dashboard.png' })
```

#### TC-MR-02: Tablet Viewport (768x1024)
- [ ] **Navigate:** Dashboard page
- [ ] **Verify:** Grid layouts use tablet columns
- [ ] **Verify:** Sidebar is collapsible/hidden

#### TC-MR-03: Desktop Viewport (1920x1080)
- [ ] **Navigate:** Dashboard page
- [ ] **Verify:** Full sidebar visible
- [ ] **Verify:** All sections use desktop layouts

#### TC-MR-04: useMediaQuery Hook
- [ ] **Verify:** `useIsMobile()` returns true on mobile
- [ ] **Verify:** `useIsTablet()` returns true on tablet
- [ ] **Verify:** `useIsDesktop()` returns true on desktop

---

## Error Handling Scenarios

### Test Scenarios

#### TC-ERR-01: API Error on Dashboard Load
- [ ] **Setup:** Block `/api/stats/dashboard` to return 500
- [ ] **Navigate:** Dashboard page
- [ ] **Verify:** Error state is shown (not crash)
- [ ] **Verify:** User can still navigate

#### TC-ERR-02: Network Timeout
- [ ] **Setup:** Delay API responses > 30s
- [ ] **Navigate:** Dashboard page
- [ ] **Verify:** Loading state shows
- [ ] **Verify:** Eventually shows error or partial data

#### TC-ERR-03: Partial Data Sync
- [ ] **Setup:** Sync partially completes (some sessions, no git)
- [ ] **Verify:** Toast shows "Partial success" message
- [ ] **Verify:** Available data is displayed

---

## Accessibility Tests

### Test Scenarios

#### TC-A11Y-01: ARIA Landmarks
- [ ] **Verify:** Main content area has `role="main"` or `<main>`
- [ ] **Verify:** Navigation has `role="navigation"` or `<nav>`
- [ ] **Verify:** Status bar has `role="contentinfo"`

#### TC-A11Y-02: Focus Management
- [ ] **Action:** Tab through entire page
- [ ] **Verify:** Focus order is logical
- [ ] **Verify:** Focus ring is visible on all interactive elements
- [ ] **Verify:** No focus traps

#### TC-A11Y-03: Screen Reader Compatibility
- [ ] **Verify:** All images have alt text
- [ ] **Verify:** Icons have aria-label or aria-hidden
- [ ] **Verify:** Progress bars have aria-valuenow/min/max
- [ ] **Verify:** Loading states have aria-busy="true"

#### TC-A11Y-04: Color Contrast
- [ ] **Verify:** Text meets WCAG AA contrast (4.5:1)
- [ ] **Verify:** Progress bar fill is distinguishable
- [ ] **Verify:** Heatmap colors are accessible

---

## Cross-Browser Testing

### Browsers to Test

- [ ] Chrome (latest)
- [ ] Firefox (latest)
- [ ] Safari (latest, macOS only)

### Test Each Browser

- [ ] Dashboard loads correctly
- [ ] Time range selector works
- [ ] Tooltips appear on hover
- [ ] Sync button works
- [ ] Settings page loads

---

## Performance Tests

### Test Scenarios

#### TC-PERF-01: Dashboard Load Time
- [ ] **Measure:** Time from navigation to content visible
- [ ] **Target:** < 2 seconds on localhost
- [ ] **Verify:** Skeleton loaders show during load

#### TC-PERF-02: Time Range Filter Response
- [ ] **Measure:** Time from range change to data update
- [ ] **Target:** < 500ms

#### TC-PERF-03: Large Dataset Handling
- [ ] **Setup:** 10,000+ sessions in database
- [ ] **Verify:** Dashboard still loads in < 5 seconds
- [ ] **Verify:** No memory issues in browser

---

## Test Execution Checklist

### Pre-Test Setup
- [ ] Backend server running
- [ ] Database indexed
- [ ] Test data present in ~/.claude/

### Execute Test Suites
- [ ] Feature 2B: Heatmap Tooltips
- [ ] Feature 2C: Sync Button Redesign
- [ ] Feature 2A: Time Range Filter
- [ ] Feature 2E: Storage Overview
- [ ] Feature 2D: AI Generation Breakdown
- [ ] Database Migration
- [ ] Feature Flags
- [ ] Backend Observability
- [ ] Mobile Responsive
- [ ] Error Handling
- [ ] Accessibility
- [ ] Cross-Browser
- [ ] Performance

### Post-Test
- [ ] Document failures
- [ ] Create issues for bugs
- [ ] Update test plan as needed

---

## Automated Test Commands

```bash
# Run all E2E tests
bun run test:e2e

# Run specific test file
bunx playwright test e2e/dashboard.spec.ts

# Run tests with UI
bunx playwright test --ui

# Generate test report
bunx playwright show-report
```

---

## Test Coverage Summary

| Feature | Scenarios | Priority |
|---------|-----------|----------|
| 2B: Heatmap Tooltips | 5 | High |
| 2C: Sync Button | 5 | High |
| 2A: Time Range Filter | 7 | High |
| 2E: Storage Overview | 8 | Medium |
| 2D: AI Generation | 8 | Medium |
| Feature Flags | 3 | Low |
| Metrics Endpoint | 2 | Low |
| Mobile Responsive | 4 | Medium |
| Error Handling | 3 | Medium |
| Accessibility | 4 | High |
| Performance | 3 | Medium |

**Total Test Scenarios: 52**
