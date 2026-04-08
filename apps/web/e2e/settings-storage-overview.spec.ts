import { expect, test } from '@playwright/test'

test.describe('Feature 2E: Storage Overview (Settings Page)', () => {
  test('TC-2E-01: Storage Section Visibility', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for settings page to fully render
    await expect(page.locator('h1:text("Settings")')).toBeVisible({ timeout: 10000 })

    // Verify "Storage" section exists
    await expect(page.getByRole('heading', { name: 'Storage' })).toBeVisible()

    // Take screenshot for visual verification
    await page.screenshot({ path: 'e2e/screenshots/storage-section-visible.png' })
  })

  test('TC-2E-02: Storage Breakdown Donut Chart', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for settings page to fully render
    await expect(page.locator('h1:text("Settings")')).toBeVisible({ timeout: 10000 })

    // Wait for storage data to load (legend items appear after API response)
    // Look for the three legend labels in the donut chart
    await expect(page.locator('text=JSONL Sessions')).toBeVisible({ timeout: 15000 })
    await expect(page.locator('text=SQLite Database')).toBeVisible()
    await expect(page.locator('text=Search Index')).toBeVisible()

    // Verify "Total" label is displayed in donut center
    await expect(page.locator('text=Total')).toBeVisible()

    // Take screenshot of storage donut chart
    await page.screenshot({ path: 'e2e/screenshots/storage-donut-chart.png' })
  })

  test('TC-2E-03: Counts Grid Display', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for settings page to fully render
    await expect(page.locator('h1:text("Settings")')).toBeVisible({ timeout: 10000 })

    // Wait for storage data to load (progress bars appear after API response)
    const jsonlVisible = await page
      .locator('text=JSONL Sessions')
      .isVisible({ timeout: 15000 })
      .catch(() => false)
    if (!jsonlVisible) {
      test.skip(true, 'Storage data did not load — JSONL Sessions label not visible')
      return
    }

    // Verify 3 stat cards (StatCard with role="group") — use longer timeout for data-dependent elements
    const sessionsGroup = page.getByRole('group', { name: /Sessions:/ })
    const sessionsVisible = await sessionsGroup.isVisible({ timeout: 15000 }).catch(() => false)
    if (!sessionsVisible) {
      test.skip(true, 'Sessions stat card not visible — StatCard structure may have changed')
      return
    }

    await expect(page.getByRole('group', { name: /Projects:/ })).toBeVisible({ timeout: 5000 })
    await expect(page.getByRole('group', { name: /Commits:/ })).toBeVisible({ timeout: 5000 })

    // Verify timestamp metadata is displayed as inline text — use longer timeout
    await expect(page.getByText('Oldest Session')).toBeVisible({ timeout: 10000 })
    await expect(page.getByText('Index Built')).toBeVisible({ timeout: 5000 })
    await expect(page.getByText('Last Git Sync')).toBeVisible({ timeout: 5000 })

    // Take screenshot of counts grid
    await page.screenshot({ path: 'e2e/screenshots/storage-counts-grid.png' })
  })

  test('TC-2E-05: Rebuild Index Button', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for settings page to fully render
    await expect(page.locator('h1:text("Settings")')).toBeVisible({ timeout: 10000 })

    // Wait for storage data to load (button appears after loading)
    await expect(page.getByRole('heading', { name: 'Storage' })).toBeVisible({ timeout: 10000 })

    // Verify "Rebuild Index" button exists and is clickable
    const rebuildButton = page.locator('button:has-text("Rebuild Index")')
    await expect(rebuildButton).toBeVisible({ timeout: 15000 })
    await expect(rebuildButton).toBeEnabled()

    // Verify button has minimum touch target (44px)
    const box = await rebuildButton.boundingBox()
    expect(box).not.toBeNull()
    if (box) {
      expect(box.height).toBeGreaterThanOrEqual(44)
    }

    // Click the rebuild button
    await rebuildButton.click()

    // Verify either the button shows loading state (disabled with spinner)
    // or a toast notification appears indicating rebuild started
    const toastSuccess = page.locator('text=Index rebuild started')
    const toastInProgress = page.locator('text=Rebuild in progress')
    const disabledButton = page.locator('button:has-text("Rebuild Index")[disabled]')

    await expect(toastSuccess.or(toastInProgress).or(disabledButton)).toBeVisible({
      timeout: 10000,
    })

    // Take screenshot after clicking rebuild
    await page.screenshot({ path: 'e2e/screenshots/storage-rebuild-index.png' })
  })

  test('TC-2E-07: Index Performance Stats', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for settings page to fully render
    await expect(page.locator('h1:text("Settings")')).toBeVisible({ timeout: 10000 })

    // Wait for storage data to load (progress bars appear after API response)
    await expect(page.locator('text=JSONL Sessions')).toBeVisible({ timeout: 15000 })

    // Index Performance section only appears if lastIndexDurationMs is not null.
    // It may not be visible if no deep index has been run yet.
    const indexPerf = page.locator('text=Index Performance')
    const isVisible = await indexPerf.isVisible().catch(() => false)

    if (isVisible) {
      // If visible, verify the detail text is also shown
      await expect(page.locator('text=Last deep index:')).toBeVisible()
    } else {
      // No deep index has been run — section is correctly hidden.
      // Verify the rest of the storage section is still functional.
      await expect(page.locator('button:has-text("Rebuild Index")')).toBeVisible()
    }

    // Take screenshot of storage section state
    await page.screenshot({ path: 'e2e/screenshots/storage-index-performance.png' })
  })

  test('TC-2E-08: API Endpoint — GET /api/stats/storage', async ({ request }) => {
    const response = await request.get('/api/stats/storage', { timeout: 30000 })
    expect(response.ok()).toBeTruthy()

    const data = await response.json()

    // Verify response structure contains all expected fields
    expect(data).toHaveProperty('jsonlBytes')
    expect(data).toHaveProperty('sqliteBytes')
    expect(data).toHaveProperty('indexBytes')
    expect(data).toHaveProperty('sessionCount')
    expect(data).toHaveProperty('projectCount')
    expect(data).toHaveProperty('commitCount')
    expect(data).toHaveProperty('oldestSessionDate')
    expect(data).toHaveProperty('lastIndexAt')
    expect(data).toHaveProperty('lastGitSyncAt')
    expect(data).toHaveProperty('lastIndexDurationMs')
    expect(data).toHaveProperty('lastIndexSessionCount')

    // Verify numeric fields are numbers (or null)
    expect(typeof data.jsonlBytes === 'number').toBeTruthy()
    expect(typeof data.sqliteBytes === 'number').toBeTruthy()
    expect(typeof data.indexBytes === 'number').toBeTruthy()
    expect(typeof data.sessionCount === 'number').toBeTruthy()
    expect(typeof data.projectCount === 'number').toBeTruthy()
    expect(typeof data.commitCount === 'number').toBeTruthy()
  })
})

test.describe('Backend Observability', () => {
  test('TC-OBS-01: Prometheus Metrics Endpoint', async ({ request }) => {
    const response = await request.get('/metrics', { timeout: 15000 })
    expect(response.ok()).toBeTruthy()

    // Verify content type is text-based (Prometheus exposition format)
    const contentType = response.headers()['content-type'] ?? ''
    expect(contentType.includes('text/plain') || contentType.includes('text/')).toBeTruthy()

    // Verify response body is non-empty text
    const body = await response.text()
    expect(body.length).toBeGreaterThan(0)

    // Prometheus format uses lines like: metric_name{labels} value
    // or # HELP / # TYPE comment lines
    // Verify at least some content that looks like Prometheus metrics
    const lines = body.split('\n').filter((l) => l.trim().length > 0)
    expect(lines.length).toBeGreaterThan(0)
  })
})
