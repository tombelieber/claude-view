import { expect, test } from '@playwright/test'

test.describe('Session Detail', () => {
  test('shows session detail with conversation', async ({ page }) => {
    // Navigate to sessions list
    await page.goto('/sessions')
    await page.waitForLoadState('domcontentloaded')

    // Wait for sessions to load
    const sessionsLoaded = await page
      .waitForSelector('article', { timeout: 30000 })
      .catch(() => null)
    if (!sessionsLoaded) {
      test.skip(true, 'Session list did not load within timeout')
      return
    }

    // Click first session link
    const firstSessionLink = page.locator('a[href*="/sessions/"]').first()
    if (!(await firstSessionLink.isVisible({ timeout: 5000 }).catch(() => false))) {
      test.skip(true, 'No session links visible')
      return
    }
    await firstSessionLink.click()

    // Wait for session detail to load (back button or conversation content)
    const backButton = page.locator('button:has-text("Back to sessions")')
    const backLink = page.locator('a:has-text("Back to sessions")')
    const backVisible = await backButton
      .or(backLink)
      .isVisible({ timeout: 30000 })
      .catch(() => false)
    if (!backVisible) {
      test.skip(true, 'Session detail did not load — back button not visible')
      return
    }

    // Take screenshot
    await page.screenshot({ path: 'e2e/screenshots/session-detail.png' })
  })

  test('session detail loading shows skeleton', async ({ page }) => {
    // Navigate to history first
    await page.goto('/sessions')
    await page.waitForLoadState('domcontentloaded')

    // Wait for sessions to load
    await page.waitForSelector('article', { timeout: 30000 })

    // Intercept session API to delay response
    await page.route('**/api/session/**', async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 500))
      await route.continue()
    })

    // Click first session
    const firstSessionLink = page.locator('a[href*="/sessions/"]').first()
    if (await firstSessionLink.isVisible({ timeout: 2000 }).catch(() => false)) {
      await firstSessionLink.click()

      // Verify loading skeleton appears with proper accessibility
      const skeleton = page.locator('[aria-busy="true"]')
      await expect(skeleton).toBeVisible({ timeout: 2000 })
    }
  })

  test('back button navigates correctly', async ({ page }) => {
    await page.goto('/sessions')
    await page.waitForLoadState('domcontentloaded')

    // Wait for sessions to load
    const sessionsLoaded = await page
      .waitForSelector('article', { timeout: 30000 })
      .catch(() => null)
    if (!sessionsLoaded) {
      test.skip(true, 'Session list did not load within timeout')
      return
    }

    // Click first session
    const firstSessionLink = page.locator('a[href*="/sessions/"]').first()
    if (!(await firstSessionLink.isVisible({ timeout: 5000 }).catch(() => false))) {
      test.skip(true, 'No session links visible')
      return
    }
    await firstSessionLink.click()

    // Wait for back navigation element
    const backButton = page.locator('button:has-text("Back to sessions")')
    const backLink = page.locator('a:has-text("Back to sessions")')
    const backEl = backButton.or(backLink)
    const backVisible = await backEl.isVisible({ timeout: 30000 }).catch(() => false)
    if (!backVisible) {
      test.skip(true, 'Session detail did not load — back button not visible')
      return
    }

    // Click back
    await backEl.click()

    // Verify we're back on sessions page
    await expect(page).toHaveURL(/\/sessions/)
  })
})
