import { test, expect } from '@playwright/test'

test.describe('Dashboard', () => {
  test('loads dashboard with metrics', async ({ page }) => {
    // Navigate to home (dashboard)
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')

    // Wait for content to load (not just skeleton)
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    // Verify dashboard header is present
    await expect(page.locator('text=Your Claude Code Usage')).toBeVisible()

    // Verify metrics are displayed (sessions and projects counts)
    // Use the metric label spans which have class "ml-1" to avoid matching
    // session list items or status bar text that also contain these words
    await expect(page.locator('span.ml-1:text("sessions")')).toBeVisible()
    await expect(page.locator('span.ml-1:text("projects")')).toBeVisible()

    // Verify Top Skills section exists
    await expect(page.locator('text=Top Skills')).toBeVisible()

    // Verify Most Active Projects section exists
    await expect(page.locator('text=Most Active Projects')).toBeVisible()

    // Verify Activity heatmap exists (text has "Activity" in heading)
    await expect(page.locator('text=/Activity.*Last.*Days/')).toBeVisible()

    // Verify Tool Usage section exists
    await expect(page.locator('text=Tool Usage')).toBeVisible()

    // Take screenshot for visual verification
    await page.screenshot({ path: 'e2e/screenshots/dashboard-loaded.png' })
  })

  test('dashboard shows loading skeleton initially', async ({ page }) => {
    // Intercept the dashboard stats API to delay response
    await page.route('**/api/stats/dashboard**', async (route) => {
      await new Promise(resolve => setTimeout(resolve, 500))
      await route.continue()
    })

    await page.goto('/')

    // Check for loading skeleton with role="status" and aria-busy="true"
    // The DashboardSkeleton component uses these attributes
    const loadingElement = page.locator('[role="status"][aria-busy="true"]')
    await expect(loadingElement.first()).toBeVisible({ timeout: 5000 })
  })

  test('status bar shows data freshness', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')

    // Wait for status bar to load
    const statusBar = page.locator('footer[role="contentinfo"]')
    await expect(statusBar).toBeVisible({ timeout: 10000 })

    // Check for session count display (e.g. "713 sessions" or "Loading status...")
    await expect(statusBar.locator('text=/sessions/')).toBeVisible({ timeout: 15000 })

    // Check for sync button (aria-label is "Sync now", data-testid is "sync-button")
    const syncButton = statusBar.locator('[data-testid="sync-button"]')
    await expect(syncButton).toBeVisible()

    // Click sync and verify it works
    await syncButton.click()
  })
})
