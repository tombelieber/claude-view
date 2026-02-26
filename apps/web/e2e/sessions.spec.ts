import { test, expect } from '@playwright/test'

test.describe('Sessions List', () => {
  test('displays session list with filter and sort', async ({ page }) => {
    await page.goto('/history')
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
    await expect(filterDropdown.locator('[role="option"]')).toHaveCount(4) // all, has_commits, high_reedit, long_session

    // Close dropdown by clicking outside
    await page.keyboard.press('Escape')

    // Verify sort dropdown exists
    const sortButton = page.locator('button[aria-haspopup="listbox"]').nth(1)
    await expect(sortButton).toBeVisible()

    // Take screenshot
    await page.screenshot({ path: 'e2e/screenshots/sessions-list.png' })
  })

  test('session cards are clickable and accessible', async ({ page }) => {
    await page.goto('/history')
    await page.waitForLoadState('domcontentloaded')

    // Wait for session cards to load
    await page.waitForSelector('article', { timeout: 30000 })

    // Get first session card link
    const firstSessionLink = page.locator('a[href*="/session/"]').first()
    await expect(firstSessionLink).toBeVisible()

    // Verify session card has proper cursor style (via cursor-pointer class)
    const sessionCard = firstSessionLink.locator('article')
    await expect(sessionCard).toHaveClass(/cursor-pointer/)

    // Verify keyboard navigation works - focus the link
    await firstSessionLink.focus()
    await expect(firstSessionLink).toBeFocused()
  })

  test('search filters sessions', async ({ page }) => {
    await page.goto('/history')
    await page.waitForLoadState('domcontentloaded')

    // Wait for sessions to load
    await page.waitForSelector('article', { timeout: 30000 })

    // Get initial session count from the filter summary
    const sessionCountBefore = await page.locator('article').count()

    // Type in search box
    const searchInput = page.locator('input[placeholder*="Search"]')
    await expect(searchInput).toBeVisible()
    await searchInput.fill('nonexistent-search-term-xyz123')

    // Wait for filtering
    await page.waitForTimeout(500)

    // Verify empty state or fewer results
    const emptyState = page.locator('text=No sessions found')
    const sessionCountAfter = await page.locator('article').count()

    // Either empty state is shown or count decreased
    expect(await emptyState.isVisible() || sessionCountAfter < sessionCountBefore).toBeTruthy()
  })

  test('empty state shows when no sessions match filter', async ({ page }) => {
    await page.goto('/history')
    await page.waitForLoadState('domcontentloaded')

    // Wait for sessions to load
    await page.waitForSelector('article', { timeout: 30000 })

    // Search for something that definitely won't match
    const searchInput = page.locator('input[placeholder*="Search"]')
    await searchInput.fill('zzz-impossible-search-term-that-matches-nothing-12345')

    // Verify empty state appears
    await expect(page.locator('text=No sessions found')).toBeVisible({ timeout: 5000 })
    await expect(page.locator('text=Try adjusting your filters')).toBeVisible()

    // Verify clear filters button exists
    await expect(page.locator('button:has-text("Clear filters")')).toBeVisible()
  })
})
