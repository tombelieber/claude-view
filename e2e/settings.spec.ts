import { test, expect } from '@playwright/test'

test.describe('Settings Page', () => {
  test('settings page loads with all sections', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for page to fully render
    await expect(page.locator('h1:text("Settings")')).toBeVisible({ timeout: 10000 })

    // Verify all four sections exist
    await expect(page.locator('text=Data Status')).toBeVisible()
    await expect(page.locator('text=Git Sync')).toBeVisible()
    await expect(page.locator('text=Export Data')).toBeVisible()
    await expect(page.locator('text=About')).toBeVisible()

    // Take screenshot
    await page.screenshot({ path: 'e2e/screenshots/settings-page.png' })
  })

  test('git sync button is clickable and shows feedback', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for page to load
    await expect(page.locator('text=Git Sync')).toBeVisible({ timeout: 10000 })

    // Find and click the sync button
    const syncButton = page.locator('button:has-text("Sync Git History")')
    await expect(syncButton).toBeVisible()
    await expect(syncButton).toBeEnabled()

    // Click the sync button
    await syncButton.click()

    // Verify loading state or success message appears
    const syncingButton = page.locator('button:has-text("Syncing")')
    const successMessage = page.locator('text=Sync started successfully')

    // Wait for either syncing state or success
    await expect(
      syncingButton.or(successMessage)
    ).toBeVisible({ timeout: 10000 })
  })

  test('keyboard shortcuts section is visible', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for page to load
    await expect(page.locator('text=About')).toBeVisible({ timeout: 10000 })

    // Verify keyboard shortcuts section
    await expect(page.locator('text=Keyboard Shortcuts')).toBeVisible()

    // Verify some shortcuts are listed
    await expect(page.locator('text=Command palette')).toBeVisible()
    await expect(page.locator('text=Focus search')).toBeVisible()
  })

  test('data status shows index information', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for status data to load
    const dataStatusHeading = page.locator('h2:text("Data Status")')
    await expect(dataStatusHeading).toBeVisible({ timeout: 10000 })

    // Scope assertions to the Data Status section to avoid ambiguity
    // with other sections (e.g. "Data & Storage") that also contain "Sessions"
    const dataStatusSection = dataStatusHeading.locator('..').locator('..')
    await expect(dataStatusSection.locator('text=Last indexed')).toBeVisible({ timeout: 15000 })
    await expect(dataStatusSection.locator('text=Sessions')).toBeVisible()
    await expect(dataStatusSection.locator('text=Projects')).toBeVisible()
  })
})
