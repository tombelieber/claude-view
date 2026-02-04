# Session Discovery Testing Guide

**Quick Links:**
- [E2E Test Plan](./session-discovery-e2e-tests.md) - Full test specification with 50+ test cases
- [Design Spec](../plans/2026-02-04-session-discovery-design.md) - Original design document
- [Implementation Progress](../plans/PROGRESS.md) - Project status

## Overview

This guide helps you execute the comprehensive E2E test plan for session discovery features (Phases A-F). All tests use the MCP Playwright browser tool for real browser automation.

## Prerequisites

### 1. Environment Setup

```bash
# Start backend (from repo root)
cd crates/server
cargo run

# Start frontend (from repo root)
npm run dev

# Verify services running
curl http://localhost:47892/api/health  # Backend health
curl http://localhost:5173              # Frontend dev server
```

### 2. Test Data Requirements

Your test database should include:
- ✅ At least 50 sessions total
- ✅ Sessions across 3+ different git branches
- ✅ Sessions with `gitBranch = null` (no branch)
- ✅ Sessions with commits (`commitCount > 0`)
- ✅ Sessions with LOC data (`linesAdded`/`linesRemoved > 0`)
- ✅ Sessions with files edited (`filesEditedCount > 0`)
- ✅ Sessions with various durations (short <10min, medium 10-30min, long >30min)
- ✅ Ideally 400+ sessions for grouping performance tests

**Create test data if needed:**
```bash
# Index your ~/.claude/ directory
cargo run -p server -- index

# Or use test fixtures (if available)
cargo test --test integration -- setup_test_data
```

### 3. Browser Requirements

The Playwright MCP tool uses Chromium by default. No manual installation needed - the tool manages the browser.

## Test Execution Workflow

### Quick Test (20 minutes)

Execute critical path tests only:

1. **Phase A (5 min):** Session card enhancements
   - Test A1: Branch badges
   - Test A2: Top files
   - Test A3: LOC display

2. **Phase B (8 min):** Filters and grouping
   - Test B2: Filter popover open/close
   - Test B3: Filter apply
   - Test B7: Group by branch
   - Test B9: View mode toggle

3. **Phase D (4 min):** Table view
   - Test D1: Table layout
   - Test D3: Column sorting

4. **Phase E (3 min):** Sidebar
   - Test E1: Branch list
   - Test E3: Tree view toggle

### Full Test Suite (90 minutes)

Execute all 50+ tests in order:

1. **Phase A Tests (15 min)** - Session card enhancements
2. **Phase B Tests (30 min)** - Toolbar, filters, grouping
3. **Phase C Tests (8 min)** - LOC display
4. **Phase D Tests (12 min)** - Table view
5. **Phase E Tests (15 min)** - Sidebar branches and tree
6. **Phase F Tests (5 min)** - Git-verified stats
7. **Integration Tests (10 min)** - Cross-phase features
8. **Accessibility Tests (20 min)** - A11y compliance
9. **Performance Tests (8 min)** - Speed benchmarks
10. **Error Handling Tests (10 min)** - Graceful failures

### Automated Test Execution (Future)

Eventually, these tests should be automated in CI:

```bash
# Run E2E tests (not yet implemented)
npm run test:e2e

# Run specific phase
npm run test:e2e -- --grep "Phase A"

# Run with headed browser (see what's happening)
npm run test:e2e -- --headed
```

## Test Case Format

Each test follows this structure:

```markdown
### Test ID: Description

**Objective:** What we're testing

**Steps:**
1. Action to take
2. Next action
3. Verification step

**Expected Results:**
- What should happen
- What UI should show
- What values should be present

**Acceptance Criteria:**
- AC-X.Y: Reference to design spec

**Success:** ✅ What constitutes passing
**Failure:** ❌ What indicates failure
```

## Using Playwright Browser Tool

### Basic Commands

```typescript
// Navigate to page
browser_navigate({ url: "http://localhost:5173" })

// Take snapshot of current page
browser_snapshot()

// Click an element (use ref from snapshot)
browser_click({
  ref: "button[Filter]",
  element: "Filter button"
})

// Type in input
browser_type({
  ref: "input[branch-search]",
  text: "feature",
  element: "Branch search input"
})

// Select dropdown option
browser_select_option({
  ref: "select[group-by]",
  values: ["branch"],
  element: "Group by dropdown"
})

// Wait for content
browser_wait_for({ text: "12 sessions" })

// Take screenshot
browser_take_screenshot({
  filename: "phase-a-test-1.png"
})
```

### Workflow Example

```typescript
// 1. Navigate and inspect
browser_navigate({ url: "http://localhost:5173" })
browser_snapshot() // See what's on page

// 2. Interact
browser_click({ ref: "button[Filter]", element: "Filter button" })
browser_wait_for({ text: "Filters" }) // Wait for popover

// 3. Verify
browser_snapshot() // Check popover contents
browser_take_screenshot({ filename: "filter-popover-open.png" })

// 4. Continue testing
browser_click({ ref: "button[Apply]", element: "Apply button" })
browser_wait_for({ textGone: "Filters" }) // Popover closed

// 5. Verify results
browser_snapshot() // Check filtered sessions
```

## Recording Test Results

### During Execution

For each test, record:
1. **Test ID** (e.g., A1, B3, D1)
2. **Status** (✅ Pass / ❌ Fail / ⏭️ Skip)
3. **Duration** (how long test took)
4. **Screenshots** (save to `docs/testing/screenshots/`)
5. **Notes** (observations, edge cases found)

### After Execution

Fill out the Test Report Template (in session-discovery-e2e-tests.md):

```markdown
## Session Discovery E2E Test Results

**Date:** 2026-02-05
**Tester:** Claude / Your Name
**Environment:** macOS 14.2, Chromium 120, 1920x1080
**Backend Version:** fa0cb9b
**Frontend Version:** fa0cb9b

### Summary
- Total Tests: 52
- Passed: 48 ✅
- Failed: 2 ❌
- Skipped: 2 ⏭️

### Failed Tests

#### Test B8: Group By with >500 Sessions
- **Failure:** Warning message not displayed when grouping >500 sessions
- **Screenshot:** `screenshots/b8-no-warning.png`
- **Console Log:** No errors
- **Severity:** Major (AC-6.7 not met)
- **Root Cause:** Safeguard check missing in `useSessionFilters` hook

#### Test PERF2: Client-Side Grouping Performance
- **Failure:** Grouping 500 sessions took 73ms (target: <50ms)
- **Screenshot:** N/A (performance test)
- **Console Log:** Performance tab shows 73ms blocking time
- **Severity:** Minor (still usable, just slower than target)
- **Root Cause:** Unoptimized `groupSessions()` utility

### Recommendations
1. **Blockers for merge:**
   - Fix Test B8: Add >500 session safeguard check
2. **Performance improvements (post-merge):**
   - Optimize grouping algorithm (Test PERF2)
   - Consider React.memo for session cards
3. **Nice-to-have:**
   - Add loading state for slow grouping operations
```

## Common Issues and Solutions

### Issue: Backend not responding
**Symptom:** `browser_navigate` times out
**Solution:** Check backend running on port 47892
```bash
curl http://localhost:47892/api/health
# Should return: {"status":"ok"}
```

### Issue: Frontend not loading
**Symptom:** Blank page or build errors
**Solution:** Restart Vite dev server
```bash
npm run dev
# Should show: ➜ Local: http://localhost:5173/
```

### Issue: No test data
**Symptom:** "No sessions found" empty state
**Solution:** Index your Claude sessions
```bash
cd crates/server
cargo run -- index
```

### Issue: Snapshot shows old UI
**Symptom:** Changes not visible in snapshots
**Solution:** Clear browser cache or hard refresh
```bash
# In browser console:
location.reload(true)
```

### Issue: Element not found in snapshot
**Symptom:** `browser_click` fails with "ref not found"
**Solution:** Take a fresh snapshot and find the correct ref
```typescript
// 1. Get fresh snapshot
browser_snapshot()

// 2. Read snapshot carefully for exact ref
// 3. Use correct ref from snapshot
browser_click({ ref: "correct-ref-from-snapshot", element: "Description" })
```

## Best Practices

### 1. Always Snapshot Before Interacting
```typescript
// ❌ Bad: Blind clicking
browser_click({ ref: "button", element: "Some button" })

// ✅ Good: See what you're clicking
browser_snapshot()
// Read snapshot, find correct ref
browser_click({ ref: "button[Filter]", element: "Filter button" })
```

### 2. Wait for UI Updates
```typescript
// ❌ Bad: Immediate check after click
browser_click({ ref: "button[Apply]", element: "Apply" })
browser_snapshot() // Might see old state

// ✅ Good: Wait for update
browser_click({ ref: "button[Apply]", element: "Apply" })
browser_wait_for({ text: "Showing 23 sessions" })
browser_snapshot() // See new state
```

### 3. Take Screenshots for Failures
```typescript
// If test fails, capture evidence
browser_take_screenshot({
  filename: `failure-test-b3-${Date.now()}.png`
})
```

### 4. Check Console for Errors
```typescript
// After each major interaction
browser_console_messages({ level: "error" })
// Should be empty (or document unexpected errors)
```

### 5. Test Both Light and Dark Modes
```typescript
// Light mode
browser_snapshot()
browser_take_screenshot({ filename: "test-a1-light.png" })

// Switch to dark mode
browser_evaluate({
  function: "() => document.documentElement.classList.add('dark')"
})
browser_snapshot()
browser_take_screenshot({ filename: "test-a1-dark.png" })
```

## Acceptance Criteria Reference

Quick lookup for AC codes in test plan:

| AC Code | Description |
|---------|-------------|
| AC-1.x  | Branch badge display |
| AC-2.x  | LOC display format |
| AC-3.x  | Top files display |
| AC-4.x  | Filter popover behavior |
| AC-5.x  | Branch filter search |
| AC-6.x  | Group-by functionality |
| AC-7.x  | View mode toggle and table |
| AC-8.x  | Sidebar branch list |
| AC-9.x  | Sidebar tree view |
| AC-10.x | Backend filter params |
| AC-11.x | Branches API endpoint |
| AC-12.x | LOC parsing logic |
| AC-13.x | Performance benchmarks |
| AC-14.x | Accessibility standards |
| AC-15.x | Error handling |
| AC-16.x | Migration 13 |

## Test Coverage Goals

**Minimum for merge approval:**
- ✅ All Phase A tests pass (session card)
- ✅ All Phase B critical tests pass (filters, grouping)
- ✅ Core navigation tests pass (E1, E2)
- ✅ No critical accessibility failures (AC-14.1-14.9)
- ✅ No crashes or console errors during normal use

**Full coverage (100% confidence):**
- ✅ All 52 test cases executed
- ✅ All phases tested
- ✅ Performance tests meet targets
- ✅ Accessibility audit clean
- ✅ Error states verified

## Next Steps After Testing

1. **Document results** using template above
2. **File issues** for any failures
3. **Tag severity** (critical/major/minor)
4. **Update PROGRESS.md** with test status
5. **Create PR** if tests pass
6. **Assign reviewer** familiar with the features

## Questions?

- Check [Design Spec](../plans/2026-02-04-session-discovery-design.md) for feature details
- Check [E2E Test Plan](./session-discovery-e2e-tests.md) for test specifics
- Check [Implementation](../../src/components/) for code reference
- Ask the agent who created this plan for clarification

---

**Happy Testing!** 🧪
