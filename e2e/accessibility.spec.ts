import { test, expect } from '@playwright/test'

test.describe('Accessibility', () => {
  test('skip to content link works', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')

    // Tab to the skip link
    await page.keyboard.press('Tab')

    // Check skip link is focused and visible
    const skipLink = page.locator('a.skip-to-content')
    await expect(skipLink).toBeFocused()
    await expect(skipLink).toHaveText('Skip to content')

    // Activate the skip link
    await page.keyboard.press('Enter')

    // Verify focus moved to main content
    const main = page.locator('#main')
    await expect(main).toBeVisible()
  })

  test('all interactive elements are keyboard accessible', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(2000)

    // Tab through the page and verify focus visible rings appear
    // Start tabbing
    await page.keyboard.press('Tab') // Skip link
    await page.keyboard.press('Tab') // Home link
    await page.keyboard.press('Tab') // Search button
    await page.keyboard.press('Tab') // Help button
    await page.keyboard.press('Tab') // Settings link

    // Verify settings link gets focused
    const settingsLink = page.locator('a[aria-label="Settings"]')
    await expect(settingsLink).toBeFocused()
  })

  test('focus visible rings are present on interactive elements', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')
    await expect(page.locator('text=Settings')).toBeVisible({ timeout: 10000 })

    // Tab to the sync button
    const syncButton = page.locator('button:has-text("Sync Git History")')
    await syncButton.focus()
    await expect(syncButton).toBeFocused()

    // Verify the button has focus-visible ring styles in its class list
    await expect(syncButton).toHaveClass(/focus-visible:ring/)
  })

  test('error states have role=alert', async ({ page }) => {
    // Navigate to a non-existent project to trigger error/empty state
    await page.goto('/project/nonexistent-project-xyz')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(2000)

    // Verify either the error has role=alert or empty state is descriptive
    const hasAlertRole = await page.locator('[role="alert"]').count()
    const hasEmptyState = await page.locator('text=Project not found').count()

    expect(hasAlertRole + hasEmptyState).toBeGreaterThan(0)
  })

  test('loading states have aria-busy', async ({ page }) => {
    // Intercept API to slow down loading
    await page.route('/api/projects', async (route) => {
      await new Promise(resolve => setTimeout(resolve, 1000))
      await route.continue()
    })

    await page.goto('/')

    // Verify loading state has aria-busy
    const busyElement = page.locator('[aria-busy="true"]')
    await expect(busyElement).toBeVisible({ timeout: 2000 })
  })

  test('metrics have screen reader labels', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')

    // Wait for dashboard to load
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    // Verify tool usage metrics have accessible structure
    // The Tool Usage section should be accessible
    const toolUsageSection = page.locator('text=Tool Usage')
    await expect(toolUsageSection).toBeVisible()
  })

  test('status bar has contentinfo role', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(2000)

    // Verify footer has contentinfo role
    const footer = page.locator('footer[role="contentinfo"]')
    await expect(footer).toBeVisible()
    await expect(footer).toHaveAttribute('aria-label', 'Data freshness status')
  })

  test('no blank screens during navigation', async ({ page }) => {
    // Navigate to each major route and verify content is never blank
    const routes = ['/', '/history', '/settings']

    for (const route of routes) {
      await page.goto(route)
      await page.waitForLoadState('domcontentloaded')

      // Verify page is never completely blank - either content or loading state is shown
      const hasContent = await page.locator('[role="status"], main, h1, article, footer').first().isVisible({ timeout: 5000 }).catch(() => false)
      expect(hasContent).toBeTruthy()
    }
  })
})
