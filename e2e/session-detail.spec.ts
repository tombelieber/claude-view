import { test, expect } from '@playwright/test'

test.describe('Session Detail', () => {
  test('shows session detail with conversation', async ({ page }) => {
    // Navigate to history to find a session
    await page.goto('/history')
    await page.waitForLoadState('domcontentloaded')

    // Wait for sessions to load
    await page.waitForSelector('article', { timeout: 30000 })

    // Click first session
    const firstSessionLink = page.locator('a[href*="/session/"]').first()
    if (await firstSessionLink.isVisible({ timeout: 2000 }).catch(() => false)) {
      await firstSessionLink.click()

      // Wait for conversation to load
      await page.waitForTimeout(2000)

      // Verify back button exists
      const backButton = page.locator('button:has-text("Back to sessions")')
      await expect(backButton).toBeVisible({ timeout: 5000 })

      // Verify export buttons exist
      await expect(page.locator('button:has-text("HTML")')).toBeVisible()
      await expect(page.locator('button:has-text("PDF")')).toBeVisible()

      // Take screenshot
      await page.screenshot({ path: 'e2e/screenshots/session-detail.png' })
    }
  })

  test('session detail loading shows skeleton', async ({ page }) => {
    // Navigate to history first
    await page.goto('/history')
    await page.waitForLoadState('domcontentloaded')

    // Wait for sessions to load
    await page.waitForSelector('article', { timeout: 30000 })

    // Intercept session API to delay response
    await page.route('**/api/session/**', async (route) => {
      await new Promise(resolve => setTimeout(resolve, 500))
      await route.continue()
    })

    // Click first session
    const firstSessionLink = page.locator('a[href*="/session/"]').first()
    if (await firstSessionLink.isVisible({ timeout: 2000 }).catch(() => false)) {
      await firstSessionLink.click()

      // Verify loading skeleton appears with proper accessibility
      const skeleton = page.locator('[aria-busy="true"]')
      await expect(skeleton).toBeVisible({ timeout: 2000 })
    }
  })

  test('back button navigates correctly', async ({ page }) => {
    await page.goto('/history')
    await page.waitForLoadState('domcontentloaded')

    // Wait for sessions to load
    await page.waitForSelector('article', { timeout: 30000 })

    // Click first session
    const firstSessionLink = page.locator('a[href*="/session/"]').first()
    if (await firstSessionLink.isVisible({ timeout: 2000 }).catch(() => false)) {
      await firstSessionLink.click()

      // Wait for session detail page
      await page.waitForSelector('button:has-text("Back to sessions")', { timeout: 10000 })

      // Click back button
      await page.locator('button:has-text("Back to sessions")').click()

      // Verify we're back on history page
      await expect(page).toHaveURL(/\/history/)
    }
  })
})
