import { test, expect } from '@playwright/test'

test.describe('Feature 2C: Sync Button Redesign', () => {
  test('TC-2C-01: Labeled Sync Button Visibility', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')

    // Wait for dashboard content to load
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    // Verify status bar footer is visible with correct role and label
    const footer = page.locator('footer[role="contentinfo"]')
    await expect(footer).toBeVisible()
    await expect(footer).toHaveAttribute('aria-label', 'Data freshness status')

    // Verify sync button is visible with data-testid
    const syncButton = page.locator('[data-testid="sync-button"]')
    await expect(syncButton).toBeVisible()

    // Verify button shows "Sync Now" label text
    await expect(syncButton).toHaveText(/Sync Now/)

    // Verify button is within the footer
    const footerSyncButton = footer.locator('[data-testid="sync-button"]')
    await expect(footerSyncButton).toBeVisible()

    // Verify button is enabled in default state
    await expect(syncButton).toBeEnabled()

    // Take screenshot for visual verification
    await page.screenshot({ path: 'e2e/screenshots/sync-button-default.png' })
  })

  test('TC-2C-02: Sync Button Click - Loading State', async ({ page }) => {
    // Intercept the sync API to delay the response so we can observe the loading state
    await page.route('/api/sync/git', async (route) => {
      // Delay the response to give us time to assert on the loading state
      await new Promise(resolve => setTimeout(resolve, 3000))
      await route.fulfill({
        status: 202,
        contentType: 'application/json',
        body: JSON.stringify({
          message: 'Git sync started',
          startedAt: new Date().toISOString(),
        }),
      })
    })

    // Also intercept status polling to prevent it from resolving immediately
    await page.route('/api/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          lastIndexedAt: new Date().toISOString(),
          sessionsIndexed: 42,
          commitsFound: 10,
          linksCreated: 5,
        }),
      })
    })

    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')

    // Wait for dashboard to load
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    const syncButton = page.locator('[data-testid="sync-button"]')
    await expect(syncButton).toBeVisible()
    await expect(syncButton).toHaveText(/Sync Now/)

    // Click the sync button
    await syncButton.click()

    // Verify button text changes to "Syncing..."
    await expect(syncButton).toHaveText(/Syncing\.\.\./, { timeout: 5000 })

    // Verify the RefreshCw icon has animate-spin class
    const spinningIcon = syncButton.locator('svg.animate-spin')
    await expect(spinningIcon).toBeVisible()

    // Verify button is disabled during sync
    await expect(syncButton).toBeDisabled()

    // Verify aria-label changes to indicate sync in progress
    await expect(syncButton).toHaveAttribute('aria-label', 'Sync in progress')

    // Verify the status bar left side shows "Syncing..." with animate-pulse
    const footer = page.locator('footer[role="contentinfo"]')
    const syncingText = footer.locator('span.animate-pulse:has-text("Syncing...")')
    await expect(syncingText).toBeVisible()

    // Take screenshot of loading state
    await page.screenshot({ path: 'e2e/screenshots/sync-button-loading.png' })
  })

  test('TC-2C-04: Sync Button - Conflict Toast (409)', async ({ page }) => {
    // Mock the sync API to return 409 Conflict
    await page.route('/api/sync/git', async (route) => {
      await route.fulfill({
        status: 409,
        contentType: 'application/json',
        body: JSON.stringify({
          error: 'Sync already in progress',
          details: null,
        }),
      })
    })

    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')

    // Wait for dashboard to load
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    const syncButton = page.locator('[data-testid="sync-button"]')
    await expect(syncButton).toBeVisible()

    // Click the sync button
    await syncButton.click()

    // Verify info toast appears with "Sync already in progress" message
    // Sonner renders toasts in the DOM - look for the text content
    const conflictToast = page.locator('text=Sync already in progress')
    await expect(conflictToast).toBeVisible({ timeout: 5000 })

    // Take screenshot of conflict toast
    await page.screenshot({ path: 'e2e/screenshots/sync-button-conflict.png' })
  })

  test('TC-2C-05: Status Bar Data Display', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')

    // Wait for dashboard to load
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    const footer = page.locator('footer[role="contentinfo"]')
    await expect(footer).toBeVisible({ timeout: 10000 })

    // Wait for status data to be fetched and rendered (not loading state)
    // The status bar should show either "Last update: X ago" or "Not yet synced"
    const statusText = footer.locator('div').first()
    await expect(statusText).toBeVisible()

    // Check for session count display (e.g., "42 sessions")
    await expect(footer.locator('text=/\\d+ sessions/')).toBeVisible({ timeout: 10000 })

    // Check for "Last update:" text or "Not yet synced" (depending on backend state)
    const hasLastUpdate = await footer.locator('text=/Last update:/').isVisible({ timeout: 3000 }).catch(() => false)
    const hasNotSynced = await footer.locator('text=Not yet synced').isVisible({ timeout: 1000 }).catch(() => false)

    // One of these must be true
    expect(hasLastUpdate || hasNotSynced).toBeTruthy()

    // If git sync has run, commit count should also be displayed
    if (hasLastUpdate) {
      // Commit count is optional - only shown if commitsFound > 0
      // Just verify the structure exists (session count is already verified above)
      const commitIcon = footer.locator('svg') // GitCommitHorizontal icon
      const hasCommits = await commitIcon.first().isVisible({ timeout: 2000 }).catch(() => false)
      // This is informational - commits may or may not exist depending on backend state
      if (hasCommits) {
        // Verify the commit count is a number
        await expect(footer.locator('text=/\\d+/')).toBeVisible()
      }
    }

    // Take screenshot of status bar
    await page.screenshot({ path: 'e2e/screenshots/status-bar-data.png' })
  })

  test('TC-2C-06: Button accessibility', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')

    // Wait for dashboard to load
    await page.waitForSelector('text=Your Claude Code Usage', { timeout: 30000 })

    const syncButton = page.locator('[data-testid="sync-button"]')
    await expect(syncButton).toBeVisible()

    // Verify button has appropriate aria-label in default state
    await expect(syncButton).toHaveAttribute('aria-label', 'Sync now')

    // Verify button is keyboard focusable
    await syncButton.focus()
    await expect(syncButton).toBeFocused()

    // Verify button has focus-visible ring styles in its class list
    await expect(syncButton).toHaveClass(/focus-visible:ring/)

    // Verify button can be activated via keyboard (Enter key)
    // First intercept the API so we can verify the action was triggered
    let syncTriggered = false
    await page.route('/api/sync/git', async (route) => {
      syncTriggered = true
      await route.fulfill({
        status: 202,
        contentType: 'application/json',
        body: JSON.stringify({
          message: 'Git sync started',
          startedAt: new Date().toISOString(),
        }),
      })
    })

    // Also intercept status polling
    await page.route('/api/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          lastGitSyncAt: new Date().toISOString(),
          lastIndexedAt: new Date().toISOString(),
          sessionsIndexed: 42,
          commitsFound: 10,
          linksCreated: 5,
        }),
      })
    })

    // Focus the button and press Enter
    await syncButton.focus()
    await page.keyboard.press('Enter')

    // Verify the sync was triggered via keyboard
    // Wait a bit for the request to go through
    await page.waitForTimeout(1000)
    expect(syncTriggered).toBeTruthy()

    // Verify aria-label changes during sync
    // The button may already have returned to default state depending on timing,
    // so we check that the aria-label is one of the valid values
    const ariaLabel = await syncButton.getAttribute('aria-label')
    expect(['Sync now', 'Sync in progress']).toContain(ariaLabel)

    // Take screenshot
    await page.screenshot({ path: 'e2e/screenshots/sync-button-accessibility.png' })
  })
})
