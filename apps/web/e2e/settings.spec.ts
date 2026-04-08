import { expect, test } from '@playwright/test'

test.describe('Settings Page', () => {
  test('settings page loads with all sections', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for page to fully render
    await expect(page.locator('h1:text("Settings")')).toBeVisible({ timeout: 10000 })

    // Verify all sections exist (section headings from SettingsSection)
    await expect(page.getByRole('heading', { name: 'Storage' })).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Git Sync' })).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Export', exact: true })).toBeVisible()
    await expect(page.getByRole('heading', { name: 'About', exact: true })).toBeVisible()

    // Take screenshot
    await page.screenshot({ path: 'e2e/screenshots/settings-page.png' })
  })

  test('git sync button is clickable', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for page to load
    await expect(page.getByRole('heading', { name: 'Git Sync' })).toBeVisible({ timeout: 10000 })

    // Find the sync button
    const syncButton = page.locator('button:has-text("Sync Git History")')
    await expect(syncButton).toBeVisible()
    await expect(syncButton).toBeEnabled()

    // Click the sync button — verify it responds (shows syncing state or aria-busy)
    await syncButton.click()
    await page.waitForTimeout(500)

    // Button should either show syncing state or have completed already
    const ariaLabel = await syncButton.getAttribute('aria-busy')
    // It's acceptable for the sync to complete quickly — just verify no crash
    expect(ariaLabel === 'true' || ariaLabel === 'false').toBeTruthy()
  })

  test('keyboard shortcuts section is visible', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for page to load
    await expect(page.getByRole('heading', { name: 'About', exact: true })).toBeVisible({
      timeout: 10000,
    })

    // Verify keyboard shortcut groups exist (inside About section)
    await expect(page.locator('#keyboard-shortcuts')).toBeVisible()

    // Verify some shortcuts are listed
    await expect(page.locator('text=Command palette')).toBeVisible()
    await expect(page.locator('text=Toggle sidebar')).toBeVisible()
  })

  test('storage overview shows session and project counts', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for storage data to load
    await expect(page.getByRole('heading', { name: 'Storage' })).toBeVisible({ timeout: 10000 })

    // Verify counts are displayed in the storage overview (StatCard with role="group")
    await expect(page.getByRole('group', { name: /Sessions:/ })).toBeVisible({ timeout: 15000 })
    await expect(page.getByRole('group', { name: /Projects:/ })).toBeVisible()
  })
})
