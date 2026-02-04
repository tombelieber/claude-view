# Session Discovery E2E Test Plan

**Status:** Ready for execution
**Branch:** `feature/session-discovery`
**Date:** 2026-02-05
**Spec:** [docs/plans/2026-02-04-session-discovery-design.md](../plans/2026-02-04-session-discovery-design.md)

## Overview

This document provides a comprehensive E2E test plan for all session discovery features implemented in phases A-F. Tests use the MCP Playwright browser tool to verify functionality in a real browser environment.

**Test Environment:**
- Frontend: http://localhost:5173
- Backend API: http://localhost:47892
- Browser: Chromium (via Playwright)

**Prerequisites:**
- Backend server must be running with indexed sessions
- Frontend dev server must be running
- Test data should include sessions with various branches, commits, and LOC values

---

## Phase A: Session Card Enhancements

### Test A1: Branch Badge Display

**Objective:** Verify branch badges appear correctly on session cards with git branches.

**Steps:**
1. Navigate to http://localhost:5173
2. Take a snapshot of the page
3. Locate a session card with a branch badge
4. Verify badge contains GitBranch icon and branch name
5. Hover over badge to check for tooltip with full branch name

**Expected Results:**
- Branch badge visible in card header before time range
- Badge styled as muted pill (gray background)
- GitBranch icon visible
- Long branch names truncated with "..." and full name in title tooltip

**Acceptance Criteria:**
- AC-1.1: Session with gitBranch shows badge with icon
- AC-1.2: Session with null gitBranch shows no badge
- AC-1.3: Branch names >20 chars truncated with tooltip
- AC-1.4: Badge has dark mode compatible colors

**Success:** ✅ Badge visible, properly styled, truncation works
**Failure:** ❌ No badge shown, wrong styling, no tooltip, poor contrast

---

### Test A2: Top Files Touched Display

**Objective:** Verify top 3 files edited shown on session cards.

**Steps:**
1. Navigate to http://localhost:5173
2. Take a snapshot of the page
3. Locate a session card with files edited
4. Verify file list shows up to 3 basenames (not full paths)
5. If session has >3 files, verify "+N more" overflow indicator

**Expected Results:**
- FileEdit icon visible
- Up to 3 filenames displayed as basenames only (e.g., "Button.tsx" not "/src/components/Button.tsx")
- Files separated by middle dot "·"
- Overflow indicator "+N more" when >3 files
- No file row when 0 files edited

**Acceptance Criteria:**
- AC-3.1: Session with 3 files shows all 3 basenames
- AC-3.2: Session with 8 files shows 3 + "+5 more"
- AC-3.3: Session with 0 files shows no file row
- AC-3.4: Full paths converted to basenames

**Success:** ✅ Correct file count, basenames only, overflow works
**Failure:** ❌ Wrong count, full paths shown, no overflow, crashes

---

### Test A3: LOC Display Format

**Objective:** Verify lines of code display in +N/-N format with proper coloring.

**Steps:**
1. Navigate to http://localhost:5173
2. Take a snapshot of the page
3. Locate a session card with LOC data (linesAdded/linesRemoved > 0)
4. Verify format is "+N / -N" with green for additions, red for removals
5. Check for GitCommit icon if locSource = 2 (git-verified)

**Expected Results:**
- LOC displayed in format "+100 / -20" after metrics row
- Green color (#10B981 or similar) for additions
- Red color (#EF4444 or similar) for removals
- GitCommit icon visible for git-verified stats (locSource=2)
- No icon for tool-call estimates (locSource=1)
- "±0" or "--" in muted gray when linesAdded=0 and linesRemoved=0
- Large numbers use K/M suffix (e.g., "+1.2K / -340")

**Acceptance Criteria:**
- AC-2.1: Session with linesAdded=100, linesRemoved=20 shows "+100 / -20" in green/red
- AC-2.2: Session with 0/0 shows "±0" in gray
- AC-2.3: Session with locSource=2 shows GitCommit icon
- AC-2.4: Session with locSource=1 shows no icon
- AC-2.5: Large numbers formatted with K suffix

**Success:** ✅ Correct format, colors, icon logic, number formatting
**Failure:** ❌ Wrong format, colors incorrect, icon missing/wrong, no K suffix

---

## Phase B: SessionToolbar, Group-by, and Filters

### Test B1: SessionToolbar Visibility

**Objective:** Verify SessionToolbar renders with all controls.

**Steps:**
1. Navigate to http://localhost:5173 (HistoryView)
2. Take a snapshot of the page
3. Verify toolbar contains:
   - Filter button/popover trigger
   - Sort dropdown
   - Group by dropdown
   - View mode toggle (List/Table)
   - Time range filters (All time/Today/7d/30d)

**Expected Results:**
- All toolbar controls visible and styled consistently
- Controls aligned properly (left-aligned filters, right-aligned view toggle)
- Responsive on smaller screens

**Acceptance Criteria:**
- All 5 control groups present
- Proper spacing and alignment
- Dark mode compatible

**Success:** ✅ All controls visible, properly aligned, responsive
**Failure:** ❌ Missing controls, misaligned, broken layout

---

### Test B2: Filter Popover Open/Close

**Objective:** Verify filter popover opens, closes, and responds to interactions.

**Steps:**
1. Navigate to http://localhost:5173
2. Take a snapshot of the page
3. Click the "Filter" button
4. Verify popover opens with all filter sections
5. Click outside popover area
6. Verify popover closes without applying changes
7. Re-open popover
8. Press Escape key
9. Verify popover closes without applying changes

**Expected Results:**
- Popover opens on button click
- Popover contains all 8 filter sections (Commits, Duration, Branch, Model, Skills, Re-edit, Files, Tokens)
- Header shows "Filters" title and "Clear" link
- Footer shows active filter count and "Apply" button
- Clicking outside closes without applying
- Escape key closes without applying

**Acceptance Criteria:**
- AC-4.1: Click Filter button opens popover
- AC-4.6: Escape key closes without applying
- AC-4.7: Click outside closes without applying

**Success:** ✅ Opens/closes correctly, no application on dismiss
**Failure:** ❌ Doesn't open, doesn't close, applies on dismiss

---

### Test B3: Filter Popover Apply

**Objective:** Verify filter selections apply correctly and update URL.

**Steps:**
1. Navigate to http://localhost:5173
2. Click "Filter" button
3. Select "Has commits: Yes" radio button
4. Select "Duration: >30m" radio button
5. Click "Apply" button
6. Verify popover closes
7. Check URL contains filter params (?hasCommits=true&minDuration=1800)
8. Verify session list filtered to only show sessions matching criteria
9. Verify Filter button shows badge with count (e.g., "Filter (2)")

**Expected Results:**
- Popover closes on Apply
- URL updated with query params
- Sessions filtered correctly
- Filter button shows active count badge
- Badge styled with blue highlight

**Acceptance Criteria:**
- AC-4.2: Radio selections persist until Apply
- AC-4.3: Apply closes popover and updates URL
- AC-4.4: Filter button shows "Filter (2)" with blue highlight

**Success:** ✅ Filters apply, URL updates, count badge correct
**Failure:** ❌ No filtering, URL unchanged, no badge, wrong count

---

### Test B4: Clear Filters

**Objective:** Verify Clear button resets all filters.

**Steps:**
1. Navigate to http://localhost:5173
2. Apply 2+ filters (e.g., hasCommits=yes, minDuration=1800)
3. Click "Filter" button
4. Click "Clear" link in popover header
5. Verify popover closes
6. Verify URL params removed
7. Verify all sessions visible again
8. Verify Filter button no longer shows count badge

**Expected Results:**
- Clear link resets all filter selections
- Popover closes
- URL params removed
- Full session list restored
- No count badge on Filter button

**Acceptance Criteria:**
- AC-4.5: Clear resets all filters and removes URL params

**Success:** ✅ All filters cleared, URL clean, full list shown
**Failure:** ❌ Filters remain, URL not cleared, list still filtered

---

### Test B5: Branch Filter with Search

**Objective:** Verify searchable branch filter with debounce.

**Steps:**
1. Navigate to http://localhost:5173
2. Click "Filter" button
3. Scroll to "Branch" section
4. Type "feat" in branch search input
5. Wait 200ms (debounce period)
6. Verify branch list filtered to only show branches containing "feat"
7. Clear search input
8. Verify full branch list restored
9. Check 2 branches (e.g., "main", "feature/auth")
10. Click "Apply"
11. Verify URL contains ?branches=main,feature/auth
12. Verify sessions filtered to those branches

**Expected Results:**
- Branch search filters list after 150ms debounce
- Search is case-insensitive
- Clearing search restores full list
- Multi-select checkboxes work
- Apply updates URL and filters sessions
- If no branches match search, shows "No branches found" empty state
- If 50+ branches, list scrollable with max-height 200px

**Acceptance Criteria:**
- AC-5.1: Search filters branch list
- AC-5.2: Multiple branches selectable
- AC-5.3: Apply filters sessions to selected branches
- AC-5.4: Empty state shown when no matches
- AC-5.5: List scrollable when many branches
- AC-5.6: Debounce prevents jank (150ms)
- AC-5.7: Clear search restores full list

**Success:** ✅ Search works, debounce smooth, multi-select works
**Failure:** ❌ No filtering, jank on typing, checkboxes broken, no scrolling

---

### Test B6: Filter Persistence on Reload

**Objective:** Verify filters persist across page refreshes.

**Steps:**
1. Navigate to http://localhost:5173
2. Apply filters: hasCommits=yes, branches=main,feature/auth
3. Note current URL (should be ?hasCommits=true&branches=main,feature/auth)
4. Refresh page (F5 or navigate to same URL)
5. Verify filters still active (Filter button shows count badge)
6. Open Filter popover
7. Verify previous selections restored (Has commits: Yes, main+feature/auth checked)
8. Verify session list still filtered

**Expected Results:**
- Filters survive page refresh
- URL params preserved
- Filter selections restored in popover
- Filtered session list matches

**Acceptance Criteria:**
- AC-4.8: Page refresh with filters in URL restores filters

**Success:** ✅ Filters restored, sessions filtered, UI reflects state
**Failure:** ❌ Filters lost, URL ignored, default state shown

---

### Test B7: Group By Branch

**Objective:** Verify group-by dropdown and branch grouping.

**Steps:**
1. Navigate to http://localhost:5173
2. Take a snapshot of the page
3. Click "Group by" dropdown
4. Select "Branch" option
5. Verify dropdown closes
6. Verify sessions grouped by branch with section headers
7. Verify group headers show aggregate stats (session count, tokens, files, LOC)
8. Verify sessions with null gitBranch appear in "(no branch)" group at bottom
9. Click a group header
10. Verify group collapses (sessions hidden)
11. Click again to expand

**Expected Results:**
- Group by dropdown shows 7 options (None, Branch, Project, Model, Day, Week, Month)
- Selecting Branch groups sessions by git_branch
- Group headers format: "feature/auth-flow --- 12 sessions . 145K tokens . 23 files . +1.2K / -340 lines"
- Sessions with null branch in "(no branch)" section (italic)
- Groups collapsible by clicking header
- All groups start expanded

**Acceptance Criteria:**
- AC-6.1: Select "Group by: Branch" groups sessions
- AC-6.2: Group headers show aggregate stats
- AC-6.3: Click header collapses/expands group
- AC-6.4: null branch sessions in "(no branch)" group at bottom
- AC-6.5: Switch to "None" returns to flat list

**Success:** ✅ Grouping works, stats correct, collapse works
**Failure:** ❌ No grouping, wrong stats, can't collapse, wrong order

---

### Test B8: Group By with >500 Sessions

**Objective:** Verify grouping disabled with warning when too many sessions.

**Steps:**
1. If test database has <500 sessions, skip this test
2. Navigate to http://localhost:5173
3. Ensure 500+ sessions loaded (check total count)
4. Click "Group by" dropdown
5. Select "Branch"
6. Verify warning message appears: "Too many sessions for grouping. Use filters to narrow results."
7. Verify grouping not applied (flat list still shown)
8. Apply filters to reduce to <500 sessions
9. Retry grouping
10. Verify grouping now works

**Expected Results:**
- Grouping disabled when total sessions > 500
- Info banner shown (not error state)
- Filtering below threshold re-enables grouping

**Acceptance Criteria:**
- AC-6.7: Total >500 disables grouping with warning
- AC-6.8: Filter to <500 re-enables grouping

**Success:** ✅ Warning shown, grouping disabled, works after filtering
**Failure:** ❌ No warning, crashes, always disabled

---

### Test B9: View Mode Toggle

**Objective:** Verify view mode toggle switches between Timeline and Table.

**Steps:**
1. Navigate to http://localhost:5173
2. Verify default view is Timeline (card-based layout)
3. Click "Table" icon in view mode toggle
4. Verify view switches to compact table
5. Verify URL updated with ?viewMode=table
6. Click "List" icon
7. Verify view switches back to timeline cards
8. Verify URL param removed (default)

**Expected Results:**
- Default view is Timeline (cards)
- Table view shows compact table with 9 columns
- View mode persisted in URL
- Switching between modes is instant (<100ms)

**Acceptance Criteria:**
- AC-7.1: Default view is Timeline
- AC-7.2: Click Table icon switches to table view
- AC-7.7: View mode persisted in URL (?viewMode=table)

**Success:** ✅ Toggle works, instant switch, URL correct
**Failure:** ❌ No switch, slow, URL wrong, crashes

---

## Phase C: LOC Estimation Display

### Test C1: LOC Format Verification

**Objective:** Verify LOC display format matches spec in both views.

**Steps:**
1. Navigate to http://localhost:5173
2. In Timeline view, locate session with LOC data
3. Verify format is "+N / -N" (e.g., "+342 / -89")
4. Verify green color for additions, red for removals
5. Switch to Table view
6. Verify LOC column shows same format
7. Verify tabular-nums font for alignment
8. Locate session with 0 LOC
9. Verify shows "--" in gray

**Expected Results:**
- Consistent format in both views
- Color coding correct (green +, red -)
- Monospace/tabular numbers for alignment
- Zero LOC shown as "--"

**Acceptance Criteria:**
- AC-2.1: Format "+N / -N" with colors
- AC-2.2: Zero LOC shows "±0" or "--"
- AC-2.5: Large numbers use K/M suffix

**Success:** ✅ Format correct, colors right, alignment good
**Failure:** ❌ Wrong format, no colors, misaligned, no suffix

---

### Test C2: Git-Verified LOC Icon

**Objective:** Verify GitCommit icon appears for git-verified LOC.

**Steps:**
1. Navigate to http://localhost:5173
2. Locate session with commits (commitCount > 0)
3. If locSource=2, verify GitCommit icon visible next to LOC
4. Hover over icon to check for tooltip explaining "Git-verified"
5. Locate session with locSource=1 (tool estimate)
6. Verify no GitCommit icon shown

**Expected Results:**
- GitCommit icon visible only when locSource=2
- Icon positioned after LOC numbers
- Tooltip explains icon meaning
- No icon for tool estimates (locSource=1)

**Acceptance Criteria:**
- AC-2.3: locSource=2 shows GitCommit icon
- AC-2.4: locSource=1 shows no icon

**Success:** ✅ Icon logic correct, tooltip works
**Failure:** ❌ Icon always/never shown, wrong logic, no tooltip

---

## Phase D: Compact Table View

### Test D1: Table Layout Verification

**Objective:** Verify compact table shows all 9 columns correctly.

**Steps:**
1. Navigate to http://localhost:5173
2. Switch to Table view
3. Take a snapshot of the table
4. Verify table headers (9 columns):
   - Time (140px)
   - Branch (120px)
   - Preview (flex)
   - Prompts (60px)
   - Tokens (70px)
   - Files (50px)
   - LOC (80px)
   - Commits (60px)
   - Duration (70px)
5. Verify all columns visible without horizontal scroll (on desktop)
6. On narrow viewport (<1024px), verify horizontal scroll enabled

**Expected Results:**
- All 9 columns visible
- Column widths match spec
- Headers properly labeled
- Horizontal scroll on narrow viewports
- Table uses proper semantic HTML (<table>, <th>, <td>)

**Acceptance Criteria:**
- AC-7.3: Table shows all 9 columns
- All columns have expected widths
- Responsive with horizontal scroll

**Success:** ✅ All columns visible, widths correct, scrollable
**Failure:** ❌ Missing columns, wrong widths, no scroll, broken layout

---

### Test D2: Table Row Click Navigation

**Objective:** Verify clicking table rows navigates to session detail.

**Steps:**
1. Navigate to http://localhost:5173
2. Switch to Table view
3. Click any table row
4. Verify navigation to session detail page (/session/:id)
5. Use browser back button
6. Verify returned to table view

**Expected Results:**
- Row click navigates to detail
- Entire row is clickable (not just specific cells)
- Cursor changes to pointer on hover
- Row highlights on hover (bg-gray-50 dark:bg-gray-800)
- Navigation preserves filter/sort state in URL

**Acceptance Criteria:**
- AC-7.4: Row click navigates to session detail
- AC-7.5: Click column header sorts (not row navigation)

**Success:** ✅ Navigation works, hover correct, state preserved
**Failure:** ❌ No navigation, wrong page, state lost

---

### Test D3: Column Sorting

**Objective:** Verify all 9 columns sortable with direction toggle.

**Steps:**
1. Navigate to http://localhost:5173
2. Switch to Table view
3. For each sortable column (Time, Branch, Prompts, Tokens, Files, LOC, Commits, Duration):
   a. Click column header
   b. Verify sort direction indicator (up/down arrow)
   c. Verify aria-sort attribute set (ascending/descending)
   d. Verify rows re-ordered accordingly
   e. Click again
   f. Verify sort direction toggles
4. Preview column is not sortable (no click handler)

**Expected Results:**
- Sortable columns: 8 out of 9 (all except Preview)
- Click header sorts ascending (default)
- Click again sorts descending
- Arrow icon shows direction (ArrowUp/ArrowDown)
- aria-sort attribute correct ("ascending", "descending", "none")
- Numeric columns sort numerically (not alphabetically)

**Acceptance Criteria:**
- AC-7.5: Click column header sorts
- AC-7.6: Column header shows sort arrow with aria-sort

**Success:** ✅ All columns sort correctly, direction toggles, accessible
**Failure:** ❌ No sorting, wrong order, no indicators, missing aria

---

## Phase E: Sidebar Branch List and Tree View

### Test E1: Sidebar Branch List on Expand

**Objective:** Verify expanding project in sidebar loads and displays branch list.

**Steps:**
1. Navigate to http://localhost:5173
2. In sidebar, click a project to expand
3. Verify loading state (skeleton) appears briefly
4. Verify branch list loads and displays
5. Verify branches shown with session counts (e.g., "main 28", "feature/auth-flow 8")
6. Verify "(no branch)" shown in italic for null branches
7. Verify branches sorted by session count descending

**Expected Results:**
- API call to GET /api/projects/:id/branches on expand
- Skeleton loader while fetching
- Branch list displays with counts
- Branches sorted by count (highest first)
- "(no branch)" entry for sessions without branches
- Long branch names truncated with tooltip

**Acceptance Criteria:**
- AC-8.1: Expand project loads branch list with skeleton
- AC-8.2: Branch list shows branch names with session counts
- AC-8.4: "(no branch)" shown in italic for null

**Success:** ✅ Loads correctly, skeleton shown, sorted, styled
**Failure:** ❌ No load, no skeleton, wrong sort, no "(no branch)"

---

### Test E2: Branch Click Navigation

**Objective:** Verify clicking branch in sidebar navigates to filtered view.

**Steps:**
1. Navigate to http://localhost:5173
2. Expand a project in sidebar
3. Click a branch (e.g., "main")
4. Verify navigation to ProjectView (/project/:id?branches=main)
5. Verify sessions filtered to selected branch
6. Verify Filter popover shows branch filter active

**Expected Results:**
- Click branch navigates to /project/:id with ?branches=<branch> param
- ProjectView loads with branch pre-filtered
- Filter popover reflects active branch filter
- Filter count badge shows on Filter button

**Acceptance Criteria:**
- AC-8.3: Click branch navigates to ProjectView with ?branches=<branch>

**Success:** ✅ Navigation correct, filter applied, URL right
**Failure:** ❌ No navigation, no filter, wrong URL

---

### Test E3: Sidebar Tree View Toggle

**Objective:** Verify tree/list view toggle and tree structure.

**Steps:**
1. Navigate to http://localhost:5173
2. In sidebar header, locate view toggle (List/Tree icons)
3. Verify default is List view (flat alphabetical list)
4. Click Tree icon
5. Verify projects grouped by directory structure
6. Verify shared parent directories shown (e.g., "@vicky-ai/")
7. Verify single-child parents flattened (not shown)
8. Verify session counts on all rows (parents and children)
9. Click List icon
10. Verify return to flat list

**Expected Results:**
- Toggle switches between List (default) and Tree views
- Tree groups projects by directory (e.g., "@vicky-ai/claude-view", "@vicky-ai/fluffy")
- Parent nodes collapsible
- Session counts aggregate from children
- Single-child flattening works
- Standalone projects appear at root

**Acceptance Criteria:**
- AC-9.1: Toggle to Tree view groups by directory
- AC-9.2: Projects in same dir share parent node
- AC-9.3: Single-child parents flattened
- AC-9.4: Session counts on every row
- AC-9.5: Toggle to List restores flat view

**Success:** ✅ Toggle works, tree structure correct, counts right
**Failure:** ❌ No toggle, wrong structure, no counts, broken

---

### Test E4: Sidebar Branch List Error Handling

**Objective:** Verify error state when branch list fails to load.

**Steps:**
1. Stop backend server (simulate API failure)
2. Navigate to http://localhost:5173
3. Click a project in sidebar to expand
4. Verify error state shown (not silent failure)
5. Verify "Retry" button or error message
6. Restart backend server
7. Click retry or re-expand project
8. Verify branch list loads successfully

**Expected Results:**
- Error state displayed on API failure
- Error message clear (e.g., "Failed to load branches")
- Retry mechanism available
- Success after retry with server running

**Acceptance Criteria:**
- AC-8.5: API error shows error state with retry

**Success:** ✅ Error shown, retry works, recovers after fix
**Failure:** ❌ Silent failure, no retry, crashes, doesn't recover

---

## Phase F: Git Diff Stats Overlay

### Test F1: Git-Verified LOC Display

**Objective:** Verify git-verified LOC displayed with GitCommit icon.

**Steps:**
1. Navigate to http://localhost:5173
2. Locate session with commits (commitCount > 0)
3. Verify LOC displayed (may be tool estimate or git-verified)
4. If GitCommit icon present, hover to see tooltip
5. Verify tooltip indicates "Git-verified" or similar
6. Compare LOC value with raw git diff stats (if available)

**Expected Results:**
- Sessions with commits may have locSource=2 (git-verified)
- GitCommit icon visible for git-verified stats
- Tooltip explains provenance
- LOC values match git diff --numstat output

**Acceptance Criteria:**
- AC-2.3: locSource=2 shows GitCommit icon
- Git diff stats accurate (from Phase F implementation)

**Success:** ✅ Icon shows, tooltip works, stats accurate
**Failure:** ❌ No icon, no tooltip, stats wrong

---

### Test F2: Tool Estimate vs Git-Verified

**Objective:** Verify both tool estimates and git-verified stats display correctly.

**Steps:**
1. Navigate to http://localhost:5173
2. Locate session WITHOUT commits (commitCount=0)
3. Verify LOC shown (if any) is tool estimate (locSource=1)
4. Verify NO GitCommit icon
5. Locate session WITH commits
6. Verify LOC may show GitCommit icon (locSource=2)
7. Verify accurate LOC from git

**Expected Results:**
- Sessions without commits: tool estimate, no icon
- Sessions with commits: may have git-verified stats with icon
- Tool estimates still valuable for uncommitted work
- Git stats override tool estimates when available

**Acceptance Criteria:**
- AC-2.4: locSource=1 shows no icon
- AC-2.3: locSource=2 shows GitCommit icon

**Success:** ✅ Both types display correctly, logic works
**Failure:** ❌ Wrong icon logic, stats missing, confusion

---

## Cross-Phase Integration Tests

### Test INT1: Filter + Group By + View Mode

**Objective:** Verify all toolbar features work together.

**Steps:**
1. Navigate to http://localhost:5173
2. Apply filters: branches=main,feature/auth, hasCommits=yes
3. Set groupBy=branch
4. Switch to Table view
5. Verify:
   - Sessions filtered correctly
   - Groups visible in table (if implemented) or flat list
   - Table columns show filtered data
   - URL reflects all params
6. Refresh page
7. Verify all settings restored

**Expected Results:**
- All features compose correctly
- No conflicts between filter/group/view
- URL contains all params
- Page refresh restores state

**Success:** ✅ All features work together, state persists
**Failure:** ❌ Conflicts, crashes, state lost, wrong results

---

### Test INT2: Sidebar Branch → ProjectView → Table

**Objective:** Verify navigation flow from sidebar to filtered table view.

**Steps:**
1. Navigate to http://localhost:5173
2. In sidebar, expand project
3. Click branch (e.g., "feature/auth")
4. Verify navigation to /project/:id?branches=feature/auth
5. Verify sessions filtered to branch
6. Switch to Table view
7. Verify table shows only branch sessions
8. Sort by LOC
9. Verify sorting works on filtered set

**Expected Results:**
- Seamless navigation flow
- Filter applied from sidebar click
- View mode switch preserves filter
- Sorting works on filtered data

**Success:** ✅ Navigation smooth, filters persist, sorting works
**Failure:** ❌ Navigation broken, filters lost, sorting fails

---

### Test INT3: Group By + Sort + Collapse

**Objective:** Verify grouping with sorting and collapse behavior.

**Steps:**
1. Navigate to http://localhost:5173
2. Set groupBy=branch
3. Set sort=tokens (highest first)
4. Verify sessions within each group sorted by tokens
5. Collapse 2 groups
6. Change sort to prompts
7. Verify collapsed groups remain collapsed
8. Expand groups
9. Verify sessions re-sorted by new criteria

**Expected Results:**
- Sorting applies within groups
- Collapse state independent of sort changes
- Re-sorting doesn't expand collapsed groups
- Group aggregate stats update after filter/sort

**Success:** ✅ Sorting works within groups, collapse persists
**Failure:** ❌ Wrong sort, collapse lost, stats wrong

---

## Accessibility Tests

### Test A11Y1: Keyboard Navigation in Filter Popover

**Objective:** Verify filter popover fully keyboard accessible.

**Steps:**
1. Navigate to http://localhost:5173
2. Tab to Filter button
3. Press Enter to open popover
4. Press Tab repeatedly
5. Verify focus cycles through all interactive elements:
   - Radio buttons
   - Checkboxes
   - Search input
   - Apply button
   - Clear link
6. Press Escape
7. Verify popover closes and focus returns to trigger button

**Expected Results:**
- All elements focusable
- Focus visible (outline/ring)
- Tab order logical
- Focus trap active (Tab doesn't leave popover)
- Escape closes and returns focus

**Acceptance Criteria:**
- AC-14.2: Radio groups have proper roles and labels
- AC-14.7: Keyboard navigation works
- AC-14.9: Focus trap in popover

**Success:** ✅ All keyboard navigation works, focus visible
**Failure:** ❌ Elements not focusable, no focus trap, wrong order

---

### Test A11Y2: Screen Reader Announcements

**Objective:** Verify filter changes announced to screen readers.

**Steps:**
1. Enable screen reader (VoiceOver, NVDA, etc.)
2. Navigate to http://localhost:5173
3. Apply filter (e.g., hasCommits=yes)
4. Listen for announcement like "Filtered to 23 sessions"
5. Clear filter
6. Listen for announcement like "Showing all sessions"

**Expected Results:**
- Live region exists for announcements (role="status" or aria-live="polite")
- Filter changes announced
- Clear filter announced
- Group collapse/expand announced

**Acceptance Criteria:**
- AC-14.10: Filter changes announced via live region

**Success:** ✅ All changes announced, clear messages
**Failure:** ❌ No announcements, unclear, wrong timing

---

### Test A11Y3: Table Accessibility

**Objective:** Verify compact table meets accessibility standards.

**Steps:**
1. Navigate to http://localhost:5173
2. Switch to Table view
3. Inspect table structure (Dev Tools or screen reader)
4. Verify:
   - <table> element used (not divs)
   - <thead> and <tbody> present
   - <th scope="col"> on all headers
   - aria-sort on sorted column
   - Row hover doesn't break keyboard focus
5. Tab through table rows
6. Press Enter on focused row
7. Verify navigation to detail

**Expected Results:**
- Proper semantic HTML
- Column headers have scope="col"
- aria-sort indicates sort column and direction
- Keyboard navigation works
- No focus loss on hover

**Acceptance Criteria:**
- AC-14.3: Table uses proper semantics
- AC-14.4: Sort column has aria-sort

**Success:** ✅ Semantic HTML, aria-sort correct, keyboard works
**Failure:** ❌ Div soup, no aria-sort, keyboard broken

---

### Test A11Y4: Color Contrast

**Objective:** Verify all UI elements meet WCAG AA contrast (4.5:1).

**Steps:**
1. Navigate to http://localhost:5173
2. Using browser DevTools or contrast checker:
   - Measure branch badge text/background
   - Measure LOC +N (green) against background
   - Measure LOC -N (red) against background
   - Measure filter button active state (blue)
   - Measure group headers
3. Test in dark mode
4. Verify all elements ≥4.5:1 contrast

**Expected Results:**
- All text ≥4.5:1 contrast
- Interactive elements ≥3:1 (if not text)
- Dark mode also meets standards

**Acceptance Criteria:**
- AC-1.4: Branch badge has sufficient contrast
- AC-14.5: All interactive elements meet contrast

**Success:** ✅ All elements meet WCAG AA
**Failure:** ❌ Low contrast, hard to read, dark mode fails

---

### Test A11Y5: Reduced Motion

**Objective:** Verify animations disabled when prefers-reduced-motion set.

**Steps:**
1. Enable prefers-reduced-motion in OS settings or browser DevTools
2. Navigate to http://localhost:5173
3. Open filter popover
4. Observe animation (should be instant/minimal)
5. Collapse/expand group
6. Observe animation (should be instant)
7. Switch view modes
8. Observe transition (should be instant)

**Expected Results:**
- All animations respect prefers-reduced-motion
- Transitions become instant or <100ms
- Functionality preserved (no breakage)

**Acceptance Criteria:**
- AC-14.8: prefers-reduced-motion respected

**Success:** ✅ Animations disabled, no jank, works correctly
**Failure:** ❌ Animations still run, breaks layout, doesn't respect setting

---

## Performance Tests

### Test PERF1: Filter Popover Render Time

**Objective:** Verify filter popover opens in <16ms (60fps).

**Steps:**
1. Navigate to http://localhost:5173
2. Open browser Performance tab
3. Start recording
4. Click Filter button
5. Stop recording when popover fully visible
6. Measure time from click to paint

**Expected Results:**
- Popover opens in <16ms (no frame drops)
- No jank or stuttering
- Smooth 60fps animation (if any)

**Acceptance Criteria:**
- AC-13.5: Filter popover open <16ms

**Success:** ✅ Opens instantly, smooth, no jank
**Failure:** ❌ Slow (>50ms), jank, frame drops

---

### Test PERF2: Client-Side Grouping Performance

**Objective:** Verify grouping 500 sessions completes in <50ms.

**Steps:**
1. Ensure test database has 400-500 sessions
2. Navigate to http://localhost:5173
3. Open browser Performance tab
4. Start recording
5. Select groupBy=branch
6. Stop recording when groups rendered
7. Measure time from click to paint

**Expected Results:**
- Grouping computation <50ms
- No blocking/freezing
- Smooth transition to grouped view

**Acceptance Criteria:**
- AC-13.4: Client-side grouping <50ms for 500 sessions

**Success:** ✅ Fast grouping, no freeze, smooth
**Failure:** ❌ Slow (>100ms), freezes, jank

---

### Test PERF3: Branch Search Debounce

**Objective:** Verify branch search debounce prevents excessive re-renders.

**Steps:**
1. Navigate to http://localhost:5173
2. Open Filter popover
3. Open browser Performance tab
4. Start recording
5. Rapidly type "feature-branch-with-long-name" in branch search
6. Stop recording after 2 seconds
7. Count number of branch list re-renders

**Expected Results:**
- 150ms debounce active
- Re-renders only after debounce delay
- Typing doesn't cause jank
- Final render matches typed query

**Acceptance Criteria:**
- AC-5.6: 150ms debounce prevents jank

**Success:** ✅ Smooth typing, debounce works, no jank
**Failure:** ❌ Re-renders on every keystroke, jank, slow

---

## Error Handling Tests

### Test ERR1: Empty Filter Results

**Objective:** Verify clear empty state when filters match nothing.

**Steps:**
1. Navigate to http://localhost:5173
2. Apply very restrictive filters (e.g., branches=nonexistent-branch)
3. Click Apply
4. Verify empty state shown
5. Verify message like "No sessions match your filters"
6. Verify "Clear filters" button visible
7. Click "Clear filters"
8. Verify full session list restored

**Expected Results:**
- Empty state with clear message
- Clear filters button prominent
- role="status" on empty state message
- Clicking Clear restores list

**Acceptance Criteria:**
- AC-15.4: Empty result shows message with clear filters button

**Success:** ✅ Clear message, button works, restores list
**Failure:** ❌ No message, blank page, no clear button, doesn't restore

---

### Test ERR2: Network Error on Branch List

**Objective:** Verify graceful handling of API failures.

**Steps:**
1. Stop backend server
2. Navigate to http://localhost:5173
3. Expand project in sidebar
4. Verify error state shown (not spinner forever)
5. Verify error message like "Failed to load branches"
6. Restart server
7. Click retry or re-expand
8. Verify branch list loads

**Expected Results:**
- Error state after reasonable timeout (2-5s)
- Clear error message
- Retry mechanism available
- Sidebar doesn't crash (can still navigate projects)
- Recovery after server restart

**Acceptance Criteria:**
- AC-15.1: Branches endpoint fails → error state with retry

**Success:** ✅ Error shown, retry works, doesn't crash
**Failure:** ❌ Spinner forever, no retry, crash, can't recover

---

### Test ERR3: Invalid URL Parameters

**Objective:** Verify graceful handling of malformed URL params.

**Steps:**
1. Navigate to http://localhost:5173?branches=main%2Cfeature%2Fauth&minDuration=abc&invalidParam=foo
2. Verify page loads (not crash)
3. Verify invalid params ignored (minDuration=abc becomes null)
4. Verify valid params applied (branches)
5. Verify unknown params ignored (invalidParam)

**Expected Results:**
- Page loads successfully
- Invalid param values ignored with fallback to defaults
- Unknown params ignored
- No console errors or crashes

**Acceptance Criteria:**
- AC-15.3: Invalid URL params gracefully ignored

**Success:** ✅ Loads, invalid ignored, no crashes
**Failure:** ❌ Crash, errors, wrong behavior

---

## Test Execution Checklist

Before running tests:
- [ ] Backend running at http://localhost:47892
- [ ] Frontend running at http://localhost:5173
- [ ] Database indexed with test sessions including:
  - [ ] Sessions with branches (at least 3 different branches)
  - [ ] Sessions without branches (gitBranch=null)
  - [ ] Sessions with commits (commitCount >0)
  - [ ] Sessions with LOC data (linesAdded/linesRemoved >0)
  - [ ] Sessions with files edited (filesEditedCount >0)
  - [ ] At least 50 sessions total
  - [ ] If possible, 400+ sessions for grouping tests
- [ ] Browser console clear of errors

---

## Success Criteria Summary

**Phase A (Session Card):**
- ✅ Branch badges display correctly (AC-1.1-1.5)
- ✅ Top files display with overflow (AC-3.1-3.4)
- ✅ LOC display formatted and colored (AC-2.1-2.5)

**Phase B (Toolbar/Filters):**
- ✅ SessionToolbar renders all controls (AC-4.1-4.8)
- ✅ Filter popover works with apply/clear (AC-4.1-4.7)
- ✅ Branch filter searchable with debounce (AC-5.1-5.8)
- ✅ Group-by works with all 6 options (AC-6.1-6.10)
- ✅ View mode toggle works (AC-7.1-7.7)

**Phase C (LOC):**
- ✅ LOC displayed in both views (AC-2.1-2.5)
- ✅ Git-verified icon logic correct (AC-2.3-2.4)

**Phase D (Table View):**
- ✅ Table shows 9 columns (AC-7.3)
- ✅ Row click navigates (AC-7.4)
- ✅ Column sorting works (AC-7.5-7.6)

**Phase E (Sidebar):**
- ✅ Branch list loads on expand (AC-8.1-8.5)
- ✅ Branch click navigates with filter (AC-8.3)
- ✅ Tree view groups correctly (AC-9.1-9.5)

**Phase F (Git Stats):**
- ✅ Git-verified LOC with icon (AC-2.3)
- ✅ Tool estimates without icon (AC-2.4)

**Cross-Phase:**
- ✅ All features compose correctly
- ✅ URL state persistence works
- ✅ Performance targets met (AC-13.1-13.5)
- ✅ Accessibility standards met (AC-14.1-14.12)
- ✅ Error handling graceful (AC-15.1-15.7)

---

## Test Report Template

After execution, document results:

```markdown
## Session Discovery E2E Test Results

**Date:** YYYY-MM-DD
**Tester:** [Name]
**Environment:** [OS, Browser, Screen Size]
**Backend Version:** [commit hash]
**Frontend Version:** [commit hash]

### Summary
- Total Tests: 50
- Passed: __
- Failed: __
- Skipped: __

### Phase Results
- Phase A: ✅ / ❌
- Phase B: ✅ / ❌
- Phase C: ✅ / ❌
- Phase D: ✅ / ❌
- Phase E: ✅ / ❌
- Phase F: ✅ / ❌
- Integration: ✅ / ❌
- Accessibility: ✅ / ❌
- Performance: ✅ / ❌
- Error Handling: ✅ / ❌

### Failed Tests
1. Test ID: [e.g., B5]
   - Failure: [description]
   - Screenshot: [path]
   - Console Log: [excerpt]
   - Severity: Critical / Major / Minor

### Recommendations
- [Fix priorities]
- [Blockers for merge]
- [Nice-to-have improvements]
```

---

## Notes for Test Executor

1. **Use Playwright browser tool** for all visual verification
2. **Take snapshots** before each interaction to see current state
3. **Check console** for errors after each test
4. **Measure timings** using browser Performance tab
5. **Test both light and dark modes** where relevant
6. **Test on different viewport sizes** (desktop, tablet, mobile)
7. **Document all failures** with screenshots and logs
8. **Re-test failed cases** after fixes to verify
9. **Update this document** if tests reveal spec gaps

**Ready to execute!** 🚀
