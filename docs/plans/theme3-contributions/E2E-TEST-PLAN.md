---
status: pending
date: 2026-02-05
purpose: Theme 3 E2E Test Plan for MCP Verification
---

# Theme 3: Git Integration & AI Contribution Tracking — E2E Test Plan

> **Purpose:** Comprehensive end-to-end test plan to verify all Theme 3 features work correctly via browser automation (Playwright).

## Test Environment Setup

### Prerequisites

1. **Server running** at `http://localhost:47892` (or configured port)
2. **Database seeded** with test data (sessions, contributions, commits)
3. **Playwright** installed and configured

### Test Data Requirements

The test suite requires seeded data with:

- At least 5 sessions across different dates
- At least 2 branches with sessions
- At least 3 different AI models used
- At least 1 session with commits linked
- At least 1 session with uncommitted work
- Sessions with different work types (DeepWork, QuickAsk, Planning, BugFix, Standard)
- Sessions with and without skills used

---

## Test Suite A: API Endpoints

### A1: GET /api/contributions (Main Endpoint)

| Test Case | Description | Expected Result |
|-----------|-------------|-----------------|
| A1.1 | Default request (week range) | Returns ContributionsResponse with 200 OK |
| A1.2 | Range: today | Returns data with Cache-Control: max-age=60 |
| A1.3 | Range: week | Returns data with Cache-Control: max-age=300 |
| A1.4 | Range: month | Returns data with Cache-Control: max-age=900 |
| A1.5 | Range: 90days | Returns data with Cache-Control: max-age=1800 |
| A1.6 | Range: all | Returns data with Cache-Control: max-age=1800 |
| A1.7 | Custom range with from/to | Returns data filtered to custom dates |
| A1.8 | Invalid range parameter | Falls back to 'week' range |
| A1.9 | Project filter | Returns data filtered to specific project |
| A1.10 | Empty database | Returns valid response with zero counts |

**Response Structure Validation (A1.11):**

```typescript
{
  overview: {
    fluency: { sessions, promptsPerSession, trend, insight },
    output: { linesAdded, linesRemoved, filesCount, commitsCount, insight },
    effectiveness: { commitRate, reeditRate, insight }
  },
  trend: DailyTrendPoint[],
  efficiency: { totalCost, totalLines, costPerLine, costPerCommit, costTrend, insight },
  byModel: ModelStats[],
  learningCurve: { periods, currentAvg, improvement, insight },
  byBranch: BranchBreakdown[],
  bySkill: SkillStats[],
  skillInsight: string,
  uncommitted: UncommittedWork[],
  uncommittedInsight: string,
  warnings: ContributionWarning[]
}
```

### A2: GET /api/contributions/sessions/:id (Session Detail)

| Test Case | Description | Expected Result |
|-----------|-------------|-----------------|
| A2.1 | Valid session ID | Returns SessionContributionResponse with 200 OK |
| A2.2 | Non-existent session ID | Returns 404 Not Found with error message |
| A2.3 | Cache header | Returns Cache-Control: max-age=300 |
| A2.4 | Session with commits | Response includes linked commits array |
| A2.5 | Session with file impacts | Response includes files array with paths |
| A2.6 | Session with work type | workType field populated |
| A2.7 | Session metrics | duration, promptCount, aiLinesAdded/Removed populated |

**Response Structure Validation (A2.8):**

```typescript
{
  sessionId: string,
  workType: string | null,
  duration: number,
  promptCount: number,
  aiLinesAdded: number,
  aiLinesRemoved: number,
  filesEditedCount: number,
  files: FileImpact[],
  commits: LinkedCommit[],
  commitRate: number | null,
  reeditRate: number | null,
  insight: Insight
}
```

### A3: GET /api/contributions/branches/:name/sessions (Branch Sessions)

| Test Case | Description | Expected Result |
|-----------|-------------|-----------------|
| A3.1 | Valid branch name | Returns BranchSessionsResponse with 200 OK |
| A3.2 | URL-encoded branch name (feature/test) | Correctly decodes and returns data |
| A3.3 | Limit parameter | Respects limit (max 50) |
| A3.4 | Range filter | Returns sessions within time range |
| A3.5 | Non-existent branch | Returns empty sessions array |
| A3.6 | Cache header | Returns Cache-Control: max-age=300 |

**Response Structure Validation (A3.7):**

```typescript
{
  branch: string,
  sessions: BranchSession[]
}
```

### A4: Warning Detection

| Test Case | Description | Expected Result |
|-----------|-------------|-----------------|
| A4.1 | GitSyncIncomplete | Warning when sessions exist but no commits correlated |
| A4.2 | CostUnavailable | Warning when sessions exist but no token/cost data |
| A4.3 | PartialData | Warning when trend has fewer days than expected |
| A4.4 | No warnings | Empty warnings array when data is complete |

---

## Test Suite B: UI Foundation (ContributionsPage)

### B1: Page Load & Navigation

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| B1.1 | Navigate to /contributions | Click sidebar link or navigate directly | Page loads without error |
| B1.2 | Loading state | Navigate while data loading | Shows DashboardSkeleton |
| B1.3 | Error state | API returns error | Shows ErrorState with retry button |
| B1.4 | Empty state | No sessions in range | Shows ContributionsEmptyState |
| B1.5 | URL persistence | Change range, refresh page | Range preserved in URL |

### B2: Time Range Filter

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| B2.1 | Default range | Load page fresh | "Week" selected by default |
| B2.2 | Switch to Today | Click "Today" | Data refetches, URL updates |
| B2.3 | Switch to Month | Click "Month" | Data refetches, URL updates |
| B2.4 | Switch to 90 Days | Click "90 Days" | Data refetches, URL updates |
| B2.5 | Switch to All Time | Click "All Time" | Data refetches, URL updates |
| B2.6 | Filter applies to all sections | Change range | All cards/charts update |

### B3: Overview Cards (3 Pillars)

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| B3.1 | Fluency card renders | Load page | Shows sessions count, prompts/session avg |
| B3.2 | Fluency trend badge | Load with trend data | Shows trend % with up/down icon |
| B3.3 | Output card renders | Load page | Shows +lines/-lines, files, commits |
| B3.4 | Effectiveness card renders | Load page | Shows commit rate %, re-edit rate |
| B3.5 | Insight lines present | Load page | Each card has InsightLine component |
| B3.6 | Large number formatting | Load with 1500+ lines | Shows "1.5K" format |
| B3.7 | Null handling | Load with missing data | Shows "--" for null values |

### B4: Trend Chart

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| B4.1 | Chart renders | Load page with data | Chart visible with data points |
| B4.2 | Toggle between lines/sessions | Click toggle | Chart updates to show selected metric |
| B4.3 | Hover tooltip | Hover over data point | Shows date and value |
| B4.4 | Empty data | Load with no trend data | Shows appropriate empty state |
| B4.5 | Insight below chart | Load with data | Shows generated insight text |

### B5: Efficiency Metrics Section

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| B5.1 | Section renders | Load page | Shows total cost, cost per line, cost per commit |
| B5.2 | Cost trend chart | Load page | Mini sparkline chart visible |
| B5.3 | Efficiency insight | Load page | Insight about cost efficiency shown |
| B5.4 | Null cost handling | Load with no cost data | Shows "unavailable" text |

### B6: Model Comparison Table

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| B6.1 | Table renders | Load page | Shows model stats table |
| B6.2 | Multiple models | Load with 3+ models | All models listed with stats |
| B6.3 | Sortable columns | Click column header | Table re-sorts by that column |
| B6.4 | Re-edit rate highlighting | Load with varied rates | Lower rates highlighted positively |
| B6.5 | Cost per line column | Load with cost data | Shows cost per line per model |

### B7: Learning Curve Section

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| B7.1 | Chart renders | Load page | Shows re-edit rate over time periods |
| B7.2 | Current average shown | Load page | Displays current avg re-edit rate |
| B7.3 | Improvement percentage | Load with improving data | Shows % improvement |
| B7.4 | Insight text | Load page | Shows learning curve insight |

### B8: Skill Effectiveness Table

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| B8.1 | Table renders | Load page | Shows skill stats table |
| B8.2 | "(no skill)" row | Load with mixed skill usage | Shows comparison row |
| B8.3 | Sessions count | Load page | Each skill shows session count |
| B8.4 | Re-edit rate per skill | Load page | Shows re-edit rate per skill |
| B8.5 | Skill insight | Load page | Shows generated skill insight |

---

## Test Suite C: Branch & Session Features

### C1: Branch List

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| C1.1 | Branch list renders | Load page | Shows all branches with data |
| C1.2 | Sort by AI Lines | Click sort dropdown, select "AI Lines" | Branches sorted by lines desc |
| C1.3 | Sort by Sessions | Select "Sessions" | Branches sorted by session count |
| C1.4 | Sort by Commits | Select "Commits" | Branches sorted by commit count |
| C1.5 | Sort by Recent | Select "Recent" | Branches sorted by last activity |
| C1.6 | Empty branch list | Load with no branch data | Shows "No branch data" message |

### C2: Branch Card Expansion

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| C2.1 | Expand branch | Click branch card | Card expands, sessions list appears |
| C2.2 | Collapse branch | Click expanded branch | Card collapses |
| C2.3 | Only one expanded | Expand branch A, then B | A collapses, B expands |
| C2.4 | Session list loads | Expand branch | API called for branch sessions |
| C2.5 | AI share progress bar | View expanded card | Shows AI contribution % bar |
| C2.6 | Session click | Click session in expanded list | Triggers drill-down |

### C3: Session Drill-Down Modal

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| C3.1 | Modal opens | Click session | Modal overlay appears |
| C3.2 | Loading state | Open modal | Shows loading spinner |
| C3.3 | Work type badge | Open session with work type | Badge shown (e.g., "DeepWork") |
| C3.4 | Summary stats grid | Open session | Shows duration, prompts, lines, commits |
| C3.5 | Files impacted list | Open session with files | Shows file paths with +/- lines |
| C3.6 | Linked commits | Open session with commits | Shows commit hashes and messages |
| C3.7 | Effectiveness bars | Open session | Shows commit rate bar, re-edit rate |
| C3.8 | Insight line | Open session | Shows effectiveness insight |
| C3.9 | Close modal - X button | Click X | Modal closes |
| C3.10 | Close modal - back button | Click back arrow | Modal closes |
| C3.11 | Close modal - backdrop | Click outside modal | Modal closes |
| C3.12 | Open full session | Click "Open Full Session" | Navigates to /sessions/:id |

### C4: Duration & Number Formatting

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| C4.1 | Duration < 60s | View session with 45s | Shows "45s" |
| C4.2 | Duration < 1h | View session with 15min | Shows "15 min" |
| C4.3 | Duration >= 1h | View session with 1h 30m | Shows "1h 30m" |
| C4.4 | Lines >= 1000 | View session with 1500 lines | Shows "1.5K" |
| C4.5 | Lines >= 1M | View session with 1.2M lines | Shows "1.2M" |

---

## Test Suite D: Dashboard Integration

### D1: Contribution Summary Card

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| D1.1 | Card renders on dashboard | Navigate to / | ContributionSummaryCard visible |
| D1.2 | Loading state | Load dashboard | Shows skeleton loader |
| D1.3 | Error state | API fails | Shows minimal card with link |
| D1.4 | Progress bar | Load with data | Shows AI contribution % bar |
| D1.5 | Metrics display | Load with data | Shows lines, commits, re-edit rate |
| D1.6 | Trend icon | Load with positive trend | Shows green TrendingUp icon |
| D1.7 | Insight text | Load with data | Shows insight from API |
| D1.8 | Click navigates | Click card | Navigates to /contributions |

### D2: Work Type Badge (Session List)

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| D2.1 | Badge renders | View session list | WorkTypeBadge visible on sessions |
| D2.2 | DeepWork badge | View DeepWork session | Shows Briefcase icon, "Deep Work" |
| D2.3 | QuickAsk badge | View QuickAsk session | Shows Zap icon, "Quick Ask" |
| D2.4 | Planning badge | View Planning session | Shows ClipboardList icon, "Planning" |
| D2.5 | BugFix badge | View BugFix session | Shows Bug icon, "Bug Fix" |
| D2.6 | Standard badge | View Standard session | Shows Sparkles icon, "Standard" |
| D2.7 | No work type | View session without type | No badge rendered |
| D2.8 | Tooltip on hover | Hover badge | Shows work type description |

---

## Test Suite E: Warnings & Alerts

### E1: Warning Banner

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| E1.1 | Banner renders | Load with warnings | WarningBanner visible at top |
| E1.2 | GitSyncIncomplete warning | Load with sync warning | Shows sync message with action |
| E1.3 | CostUnavailable warning | Load with cost warning | Shows cost unavailable message |
| E1.4 | PartialData warning | Load with partial data | Shows partial data message |
| E1.5 | Multiple warnings | Load with 2+ warnings | All warnings displayed |
| E1.6 | Sync button | Click sync on GitSync warning | Triggers refetch |
| E1.7 | No warnings | Load with complete data | No banner rendered |

### E2: Uncommitted Work Section

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| E2.1 | Section renders | Load with uncommitted work | UncommittedWork section visible |
| E2.2 | Project list | Load with uncommitted | Shows list of projects |
| E2.3 | Lines count | Load with uncommitted | Shows uncommitted line count per project |
| E2.4 | Insight text | Load with uncommitted | Shows uncommitted insight |
| E2.5 | View session button | Click view session | Opens session drill-down |
| E2.6 | Refresh button | Click refresh | Triggers refetch |
| E2.7 | No uncommitted | Load with all committed | Section not rendered |

---

## Test Suite F: Edge Cases & Error Handling

### F1: Network Errors

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| F1.1 | API timeout | Slow network | Shows error state after timeout |
| F1.2 | Server 500 | Force server error | Shows error with message |
| F1.3 | Network offline | Disable network | Shows offline error |
| F1.4 | Retry after error | Click retry button | Refetches data |

### F2: Data Edge Cases

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| F2.1 | Zero sessions | Load with empty DB | Shows empty state |
| F2.2 | One session | Load with 1 session | All components render correctly |
| F2.3 | Very large numbers | Load with 1M+ lines | Numbers formatted correctly |
| F2.4 | Negative trend | Load with declining activity | Shows negative % with down icon |
| F2.5 | 100% commit rate | Load with all committed | Shows 100% correctly |
| F2.6 | 0% commit rate | Load with no commits | Shows 0% or "--" |
| F2.7 | Very long branch name | Load with 100+ char branch | Truncated with ellipsis |
| F2.8 | Special chars in branch | Load with "feature/test-123" | Correctly displayed and URL-encoded |

### F3: Concurrent Actions

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| F3.1 | Rapid range switching | Click through ranges quickly | Final range loads correctly |
| F3.2 | Multiple branch expansions | Expand branches rapidly | Only last expanded shows |
| F3.3 | Modal open during range change | Open modal, change range | Modal closes, new data loads |

---

## Test Suite G: Accessibility

### G1: Keyboard Navigation

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| G1.1 | Tab through time filter | Press Tab | Focus moves through options |
| G1.2 | Enter to select range | Focus range, press Enter | Range selected |
| G1.3 | Tab through cards | Press Tab | Focus moves through cards |
| G1.4 | Enter on branch card | Focus branch, press Enter | Branch expands |
| G1.5 | Escape closes modal | Open modal, press Escape | Modal closes |
| G1.6 | Tab trap in modal | Open modal, Tab | Focus stays within modal |

### G2: ARIA & Screen Reader

| Test Case | Description | Steps | Expected Result |
|-----------|-------------|-------|-----------------|
| G2.1 | Card headings | Inspect | Proper heading hierarchy (h1, h2, h3) |
| G2.2 | Progress bar labels | Inspect commit rate bar | Has role="progressbar", aria-valuenow |
| G2.3 | Sort dropdown | Inspect | Has aria-haspopup, aria-expanded |
| G2.4 | Modal labeling | Inspect modal | Proper aria-labelledby |
| G2.5 | Icon aria-hidden | Inspect icons | Decorative icons have aria-hidden |

---

## Test Execution Checklist

### Pre-flight Checks

- [ ] Server running on correct port
- [ ] Database seeded with test data
- [ ] Playwright configured
- [ ] Network inspector available for API tests

### Test Run Order

1. **API Tests (Suite A)** — Run first to verify backend
2. **UI Foundation (Suite B)** — Core page functionality
3. **Branch & Session (Suite C)** — Interactive features
4. **Dashboard Integration (Suite D)** — Cross-page features
5. **Warnings & Alerts (Suite E)** — Edge case UI
6. **Edge Cases (Suite F)** — Robustness testing
7. **Accessibility (Suite G)** — Compliance testing

### Reporting

For each test case, record:

- **Status:** Pass / Fail / Blocked
- **Notes:** Any observations or deviations
- **Screenshot:** Capture for failures

---

## Coverage Summary

| Area | Test Cases | Priority |
|------|------------|----------|
| API Endpoints | 28 | Critical |
| UI Foundation | 32 | Critical |
| Branch & Session | 22 | High |
| Dashboard Integration | 16 | High |
| Warnings & Alerts | 13 | Medium |
| Edge Cases & Errors | 14 | Medium |
| Accessibility | 11 | Medium |
| **Total** | **136** | — |

### Coverage by Component

| Component | Tests |
|-----------|-------|
| ContributionsPage | 15 |
| TimeRangeFilter | 6 |
| OverviewCards | 7 |
| TrendChart | 5 |
| EfficiencyMetrics | 4 |
| ModelComparison | 5 |
| LearningCurve | 4 |
| SkillEffectiveness | 5 |
| BranchList | 6 |
| BranchCard | 6 |
| SessionDrillDown | 12 |
| ContributionSummaryCard | 8 |
| WorkTypeBadge | 8 |
| WarningBanner | 7 |
| UncommittedWork | 7 |
| API /contributions | 11 |
| API /contributions/sessions/:id | 8 |
| API /contributions/branches/:name/sessions | 7 |

---

## Playwright Test Implementation Notes

### API Test Example

```typescript
test('GET /api/contributions returns valid response', async ({ request }) => {
  const response = await request.get('/api/contributions?range=week');
  expect(response.ok()).toBeTruthy();

  const data = await response.json();
  expect(data.overview).toBeDefined();
  expect(data.overview.fluency.sessions).toBeGreaterThanOrEqual(0);
  expect(data.trend).toBeInstanceOf(Array);

  const cacheControl = response.headers()['cache-control'];
  expect(cacheControl).toContain('max-age=300');
});
```

### UI Test Example

```typescript
test('ContributionsPage loads and displays overview cards', async ({ page }) => {
  await page.goto('/contributions');

  // Wait for data to load
  await page.waitForSelector('[data-testid="fluency-card"]');

  // Verify 3 overview cards
  const cards = await page.locator('.overview-card').count();
  expect(cards).toBe(3);

  // Verify session count is displayed
  const sessionsText = await page.textContent('[data-testid="sessions-count"]');
  expect(parseInt(sessionsText || '0')).toBeGreaterThan(0);
});
```

### Modal Test Example

```typescript
test('Session drill-down modal opens and closes', async ({ page }) => {
  await page.goto('/contributions');

  // Expand a branch
  await page.click('[data-testid="branch-card"]:first-child');

  // Wait for sessions to load
  await page.waitForSelector('[data-testid="branch-session"]');

  // Click a session
  await page.click('[data-testid="branch-session"]:first-child');

  // Verify modal opened
  await expect(page.locator('[data-testid="session-drilldown"]')).toBeVisible();

  // Close with X button
  await page.click('[aria-label="Close drill-down"]');

  // Verify modal closed
  await expect(page.locator('[data-testid="session-drilldown"]')).not.toBeVisible();
});
```

---

## Maintenance Notes

- Update test data requirements when new features added
- Review edge cases quarterly for relevance
- Keep Playwright selectors in sync with component changes
- Add data-testid attributes to components for stable selectors
