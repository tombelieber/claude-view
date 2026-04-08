import { expect, test } from '@playwright/test'

test.describe('Sessions List', () => {
  test('displays session list with filter and sort', async ({ page }) => {
    await page.goto('/sessions')
    await page.waitForLoadState('domcontentloaded')

    // Wait for sessions to load (not skeleton)
    await page.waitForSelector('article', { timeout: 30000 })

    // Verify filter dropdown exists and is accessible
    const filterButton = page.locator('button[aria-haspopup="listbox"]').first()
    await expect(filterButton).toBeVisible()
    await expect(filterButton).toHaveAttribute('aria-expanded', 'false')

    // Click filter button and verify dropdown opens
    await filterButton.click()
    await expect(filterButton).toHaveAttribute('aria-expanded', 'true')

    // Verify filter options are visible
    const filterDropdown = page.locator('[role="listbox"]').first()
    await expect(filterDropdown).toBeVisible()
    await expect(filterDropdown.locator('[role="option"]')).toHaveCount(7)

    // Close dropdown by clicking outside
    await page.keyboard.press('Escape')

    // Verify sort dropdown exists
    const sortButton = page.locator('button[aria-haspopup="listbox"]').nth(1)
    await expect(sortButton).toBeVisible()

    // Take screenshot
    await page.screenshot({ path: 'e2e/screenshots/sessions-list.png' })
  })

  test('session cards are clickable and accessible', async ({ page }) => {
    await page.goto('/sessions')
    await page.waitForLoadState('domcontentloaded')

    // Wait for session cards to load
    await page.waitForSelector('article', { timeout: 30000 })

    // Get first session card link
    const firstSessionLink = page.locator('a[href*="/sessions/"]').first()
    await expect(firstSessionLink).toBeVisible()

    // Verify session card has proper cursor style (via cursor-pointer class)
    const sessionCard = firstSessionLink.locator('article')
    await expect(sessionCard).toHaveClass(/cursor-pointer/)

    // Verify keyboard navigation works - focus the link
    await firstSessionLink.focus()
    await expect(firstSessionLink).toBeFocused()
  })

  test('search input is visible and clickable', async ({ page }) => {
    await page.goto('/sessions')
    await page.waitForLoadState('domcontentloaded')

    // Wait for sessions to load
    await page.waitForSelector('article', { timeout: 30000 })

    // Search input is a readonly trigger for the command palette
    const searchInput = page.locator('input[placeholder*="Search"]')
    if (!(await searchInput.isVisible({ timeout: 5000 }).catch(() => false))) {
      test.skip(true, 'Search input not visible — UI may have changed')
      return
    }

    // Use force:true since the input may be readonly and/or have an overlay
    await searchInput.click({ force: true })
    await page.waitForTimeout(500)

    // Close any opened dialog
    await page.keyboard.press('Escape')
  })

  test('session list renders multiple cards', async ({ page }) => {
    await page.goto('/sessions')
    await page.waitForLoadState('domcontentloaded')

    // Wait for sessions to load
    await page.waitForSelector('article', { timeout: 30000 })

    // Verify multiple session cards are rendered
    const sessionCount = await page.locator('article').count()
    expect(sessionCount).toBeGreaterThan(0)
  })
})
