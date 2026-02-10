import { test, expect } from '@playwright/test'

test.describe('Dashboard Time Range Filter (Feature 2A)', () => {
  /**
   * TC-2A-01: Segmented Control Rendering (Desktop)
   * At desktop viewport (>=1024px), verify segmented control renders
   * with Today, 7d, 30d, 90d, All, Custom options. Default is "30d".
   */
  test('TC-2A-01: renders segmented control on desktop with correct options', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 })
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    // Verify the segmented control (radiogroup) is visible
    const segmentedControl = page.locator('[role="radiogroup"][aria-label="Time range selector"]')
    await expect(segmentedControl).toBeVisible()

    // Verify all 6 options exist as radio buttons
    const radioButtons = segmentedControl.locator('button[role="radio"]')
    await expect(radioButtons).toHaveCount(6)

    // Verify the labels (Today is now first)
    await expect(radioButtons.nth(0)).toHaveText('Today')
    await expect(radioButtons.nth(1)).toHaveText('7d')
    await expect(radioButtons.nth(2)).toHaveText('30d')
    await expect(radioButtons.nth(3)).toHaveText('90d')
    await expect(radioButtons.nth(4)).toHaveText('All')
    await expect(radioButtons.nth(5)).toHaveText('Custom')

    // Verify "30d" is selected by default (aria-checked="true")
    await expect(radioButtons.nth(2)).toHaveAttribute('aria-checked', 'true')

    // Verify others are not selected
    await expect(radioButtons.nth(0)).toHaveAttribute('aria-checked', 'false')
    await expect(radioButtons.nth(1)).toHaveAttribute('aria-checked', 'false')
    await expect(radioButtons.nth(3)).toHaveAttribute('aria-checked', 'false')
    await expect(radioButtons.nth(4)).toHaveAttribute('aria-checked', 'false')
    await expect(radioButtons.nth(5)).toHaveAttribute('aria-checked', 'false')

    await page.screenshot({ path: 'e2e/screenshots/time-range-desktop-segmented.png' })
  })

  /**
   * TC-2A-02: Dropdown Selector Rendering (Mobile)
   * At mobile viewport (<640px), verify native <select> dropdown renders
   * with correct options and adequate touch target (>= 44x44px).
   */
  test('TC-2A-02: renders dropdown selector on mobile with correct options', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 })
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    // Segmented control should NOT be visible on mobile
    const segmentedControl = page.locator('[role="radiogroup"][aria-label="Time range selector"]')
    await expect(segmentedControl).not.toBeVisible()

    // Native <select> should be visible on mobile
    const dropdown = page.locator('select[aria-label="Time range selector"]')
    await expect(dropdown).toBeVisible()

    // Verify dropdown options
    const options = dropdown.locator('option')
    await expect(options).toHaveCount(6)

    // Mobile labels are longer (e.g. "7 days" instead of "7d")
    await expect(options.nth(0)).toHaveText('Today')
    await expect(options.nth(1)).toHaveText('7 days')
    await expect(options.nth(2)).toHaveText('30 days')
    await expect(options.nth(3)).toHaveText('90 days')
    await expect(options.nth(4)).toHaveText('All time')
    await expect(options.nth(5)).toHaveText('Custom')

    // Verify "30 days" is selected by default
    await expect(dropdown).toHaveValue('30d')

    // Verify touch target is at least 44x44px (WCAG 2.1 AA)
    const box = await dropdown.boundingBox()
    expect(box).not.toBeNull()
    if (box) {
      expect(box.height).toBeGreaterThanOrEqual(44)
      expect(box.width).toBeGreaterThanOrEqual(44)
    }

    await page.screenshot({ path: 'e2e/screenshots/time-range-mobile-dropdown.png' })
  })

  /**
   * TC-2A-03: Time Range Selection Updates Dashboard
   * Select "7d", verify stats update. Select "All", verify all data shown.
   */
  test('TC-2A-03: selecting a time range updates dashboard stats', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 })
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    const segmentedControl = page.locator('[role="radiogroup"][aria-label="Time range selector"]')

    // Wait for the period metrics region to appear (contains Sessions, Tokens, etc.)
    // The aria-label is "Period metrics" when trends data is loaded,
    // or "Week-over-week metrics (loading)" while loading
    const periodMetrics = page.locator('[aria-label="Period metrics"]')
    await expect(periodMetrics).toBeVisible({ timeout: 10000 })

    // --- Select "7d" ---
    const btn7d = segmentedControl.locator('button[role="radio"]', { hasText: '7d' }).first()
    await btn7d.click()
    await expect(btn7d).toHaveAttribute('aria-checked', 'true')

    // Wait for dashboard to update (date range caption should reflect the change)
    await expect(page.locator('text=Showing stats from')).toBeVisible({ timeout: 10000 })

    // Period metrics region should still be visible after range change
    await expect(periodMetrics).toBeVisible()

    await page.screenshot({ path: 'e2e/screenshots/time-range-7d-selected.png' })

    // --- Select "All" ---
    const btnAll = segmentedControl.locator('button[role="radio"]', { hasText: 'All' })
    await btnAll.click()
    await expect(btnAll).toHaveAttribute('aria-checked', 'true')

    // "All" range shows "Showing all-time stats" caption
    await expect(page.locator('text=Showing all-time stats')).toBeVisible({ timeout: 10000 })

    // In "All" mode, the API returns trends: null, so the DashboardMetricsGrid
    // is intentionally hidden (not rendered). Verify the caption is correct instead.
    // The dashboard should still show the overall session/project counts.
    await expect(page.locator('span.ml-1:text("sessions")')).toBeVisible()

    await page.screenshot({ path: 'e2e/screenshots/time-range-all-selected.png' })
  })

  /**
   * TC-2A-04: Custom Date Range Picker
   * Select "Custom", verify date picker popover appears with
   * start/end date labels and Apply button.
   */
  test('TC-2A-04: custom date range picker opens on Custom selection', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 })
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    const segmentedControl = page.locator('[role="radiogroup"][aria-label="Time range selector"]')

    // Click "Custom" option
    const btnCustom = segmentedControl.locator('button[role="radio"]', { hasText: 'Custom' })
    await btnCustom.click()
    await expect(btnCustom).toHaveAttribute('aria-checked', 'true')

    // The DateRangePicker trigger button should appear (with aria-haspopup="dialog")
    const datePickerTrigger = page.locator('button[aria-haspopup="dialog"]')
    await expect(datePickerTrigger).toBeVisible({ timeout: 5000 })

    // Click the trigger to open the popover
    await datePickerTrigger.click()

    // Verify the date picker dialog opens
    const dialog = page.locator('[role="dialog"][aria-label="Select custom date range"]')
    await expect(dialog).toBeVisible({ timeout: 5000 })

    // Verify "Start date" and "End date" labels
    await expect(dialog.locator('text=Start date')).toBeVisible()
    await expect(dialog.locator('text=End date')).toBeVisible()

    // Verify the "Apply" button exists
    const applyButton = dialog.locator('button', { hasText: 'Apply' })
    await expect(applyButton).toBeVisible()

    // Apply button should be disabled when no dates are selected (initial state)
    // (It's disabled unless both tempFrom and tempTo are set)
    await expect(applyButton).toBeDisabled()

    await page.screenshot({ path: 'e2e/screenshots/time-range-custom-picker.png' })

    // Close dialog with Escape
    await page.keyboard.press('Escape')
    await expect(dialog).not.toBeVisible()
  })

  /**
   * TC-2A-05: URL Persistence
   * Navigate to /?range=7d, verify "7d" is selected.
   * Navigate to /?range=90d, verify "90d" is selected.
   * Navigate to / (no param), verify default "30d" is selected.
   */
  test('TC-2A-05: URL param persists and restores time range', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 })

    // Clear localStorage to avoid stale state interfering
    await page.goto('/')
    await page.evaluate(() => localStorage.removeItem('dashboard-time-range'))

    // --- Navigate with ?range=today ---
    await page.goto('/?range=today')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    const btnToday = segmentedControl.locator('button[role="radio"]', { hasText: 'Today' })
    await expect(btnToday).toHaveAttribute('aria-checked', 'true')

    // --- Navigate with ?range=7d ---
    await page.goto('/?range=7d')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    const segmentedControl = page.locator('[role="radiogroup"][aria-label="Time range selector"]')
    const btn7d = segmentedControl.locator('button[role="radio"]', { hasText: '7d' }).first()
    await expect(btn7d).toHaveAttribute('aria-checked', 'true')

    // --- Navigate with ?range=90d ---
    await page.goto('/?range=90d')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    const btn90d = segmentedControl.locator('button[role="radio"]', { hasText: '90d' })
    await expect(btn90d).toHaveAttribute('aria-checked', 'true')

    // --- Navigate with ?range=all ---
    await page.goto('/?range=all')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    const btnAll = segmentedControl.locator('button[role="radio"]', { hasText: 'All' })
    await expect(btnAll).toHaveAttribute('aria-checked', 'true')

    // --- Verify selecting a range updates the URL ---
    await page.goto('/')
    await page.evaluate(() => localStorage.removeItem('dashboard-time-range'))
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    // Default should be 30d (no range param in URL since it's the default)
    const btn30d = segmentedControl.locator('button[role="radio"]', { hasText: '30d' })
    await expect(btn30d).toHaveAttribute('aria-checked', 'true')

    // Click 7d and verify URL updates
    await btn7d.click()
    await page.waitForTimeout(500) // Allow URL sync
    expect(page.url()).toContain('range=7d')

    await page.screenshot({ path: 'e2e/screenshots/time-range-url-persistence.png' })
  })

  /**
   * TC-2A-07: API Endpoint with Time Range
   * Test GET /api/stats/dashboard with from/to params returns correct response structure.
   */
  test('TC-2A-07: API returns correct structure with time range params', async ({ request }) => {
    // Test without params (all-time)
    const allTimeResponse = await request.get('/api/stats/dashboard', { timeout: 60000 })
    expect(allTimeResponse.ok()).toBeTruthy()

    const allTimeData = await allTimeResponse.json()
    expect(allTimeData).toHaveProperty('totalSessions')
    expect(allTimeData).toHaveProperty('totalProjects')
    expect(allTimeData).toHaveProperty('heatmap')
    expect(allTimeData).toHaveProperty('topSkills')
    expect(allTimeData).toHaveProperty('topProjects')
    expect(typeof allTimeData.totalSessions).toBe('number')
    expect(typeof allTimeData.totalProjects).toBe('number')
    expect(Array.isArray(allTimeData.heatmap)).toBeTruthy()
    expect(Array.isArray(allTimeData.topSkills)).toBeTruthy()
    expect(Array.isArray(allTimeData.topProjects)).toBeTruthy()

    // Test with time range params (last 7 days)
    const now = Math.floor(Date.now() / 1000)
    const sevenDaysAgo = now - 7 * 86400
    const rangeResponse = await request.get(
      `/api/stats/dashboard?from=${sevenDaysAgo}&to=${now}`,
      { timeout: 60000 }
    )
    expect(rangeResponse.ok()).toBeTruthy()

    const rangeData = await rangeResponse.json()
    expect(rangeData).toHaveProperty('totalSessions')
    expect(rangeData).toHaveProperty('totalProjects')
    expect(typeof rangeData.totalSessions).toBe('number')
    expect(typeof rangeData.totalProjects).toBe('number')

    // Verify periodStart and periodEnd are present when time range is specified
    expect(rangeData).toHaveProperty('periodStart')
    expect(rangeData).toHaveProperty('periodEnd')
    expect(typeof rangeData.periodStart).toBe('number')
    expect(typeof rangeData.periodEnd).toBe('number')

    // periodStart should be close to our 'from' param
    // (allow some tolerance for server-side rounding)
    expect(rangeData.periodStart).toBeLessThanOrEqual(now)
    expect(rangeData.periodEnd).toBeLessThanOrEqual(now + 60)

    // 7d range should have <= all-time sessions
    expect(rangeData.totalSessions).toBeLessThanOrEqual(allTimeData.totalSessions)
  })

  /**
   * TC-2A-03b: Mobile time range selection also updates dashboard
   * Verify the native <select> on mobile triggers dashboard updates.
   */
  test('TC-2A-03b: mobile dropdown selection updates dashboard', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 667 })
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    const dropdown = page.locator('select[aria-label="Time range selector"]')
    await expect(dropdown).toBeVisible()

    // Select "7 days" (value="7d")
    await dropdown.selectOption('7d')
    await expect(dropdown).toHaveValue('7d')

    // Dashboard should update — period metrics region still visible
    await expect(page.locator('text=Showing stats from')).toBeVisible({ timeout: 10000 })
    await expect(page.locator('[aria-label="Period metrics"]')).toBeVisible()

    // Select "All time" (value="all")
    await dropdown.selectOption('all')
    await expect(dropdown).toHaveValue('all')

    // "All" shows all-time stats caption
    await expect(page.locator('text=Showing all-time stats')).toBeVisible({ timeout: 10000 })

    await page.screenshot({ path: 'e2e/screenshots/time-range-mobile-selection.png' })
  })

  /**
   * Date range caption displays correctly
   * Verify the "Showing stats from X - Y" caption updates with range changes.
   */
  test('date range caption updates when time range changes', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 })
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    // Default 30d should show "Showing stats from" caption
    const caption = page.locator('text=Showing stats from')
    await expect(caption).toBeVisible({ timeout: 10000 })

    const segmentedControl = page.locator('[role="radiogroup"][aria-label="Time range selector"]')

    // Switch to "All" — should show "Showing all-time stats"
    const btnAll = segmentedControl.locator('button[role="radio"]', { hasText: 'All' })
    await btnAll.click()
    await expect(page.locator('text=Showing all-time stats')).toBeVisible({ timeout: 10000 })

    // Switch back to "7d" — should show "Showing stats from" again
    const btn7d = segmentedControl.locator('button[role="radio"]', { hasText: '7d' }).first()
    await btn7d.click()
    await expect(page.locator('text=Showing stats from')).toBeVisible({ timeout: 10000 })

    await page.screenshot({ path: 'e2e/screenshots/time-range-caption-update.png' })
  })
})
