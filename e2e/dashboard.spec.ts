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
    await expect(page.locator('text=sessions')).toBeVisible()
    await expect(page.locator('text=projects')).toBeVisible()

    // Verify Top Skills section exists
    await expect(page.locator('text=Top Skills')).toBeVisible()

    // Verify Most Active Projects section exists
    await expect(page.locator('text=Most Active Projects')).toBeVisible()

    // Verify Activity heatmap exists
    await expect(page.locator('text=Activity (Last 30 Days)')).toBeVisible()

    // Verify Tool Usage section exists
    await expect(page.locator('text=Tool Usage')).toBeVisible()

    // Take screenshot for visual verification
    await page.screenshot({ path: 'e2e/screenshots/dashboard-loaded.png' })
  })

  test('dashboard shows loading skeleton initially', async ({ page }) => {
    // Intercept the API to delay response
    await page.route('/api/dashboard/stats', async (route) => {
      await new Promise(resolve => setTimeout(resolve, 500))
      await route.continue()
    })

    await page.goto('/')

    // Check for aria-busy attribute on loading state
    const loadingElement = page.locator('[aria-busy="true"]')
    await expect(loadingElement).toBeVisible({ timeout: 2000 })
  })

  test('status bar shows data freshness', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')

    // Wait for status bar to load
    const statusBar = page.locator('footer[role="contentinfo"]')
    await expect(statusBar).toBeVisible({ timeout: 10000 })

    // Check for session count display
    await expect(statusBar.locator('text=/\\d+ sessions/')).toBeVisible()

    // Check for refresh button
    const refreshButton = statusBar.locator('button[aria-label="Refresh status"]')
    await expect(refreshButton).toBeVisible()

    // Click refresh and verify it works
    await refreshButton.click()
  })
})
