import { test, expect } from '@playwright/test'

test.describe('Export Functionality', () => {
  test('export JSON download works from settings', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for settings to load
    await expect(page.locator('text=Settings')).toBeVisible({ timeout: 10000 })

    // Verify Export Data section exists
    await expect(page.locator('text=Export Data')).toBeVisible()

    // Verify JSON is selected by default
    const jsonRadio = page.locator('input[value="json"]')
    await expect(jsonRadio).toBeChecked()

    // Set up download listener
    const downloadPromise = page.waitForEvent('download', { timeout: 30000 })

    // Click Download Export button
    await page.locator('button:has-text("Download Export")').click()

    // Wait for download
    const download = await downloadPromise

    // Verify download filename contains .json
    expect(download.suggestedFilename()).toContain('.json')

    // Take screenshot
    await page.screenshot({ path: 'e2e/screenshots/export-json.png' })
  })

  test('export CSV download works from settings', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for settings to load
    await expect(page.locator('text=Settings')).toBeVisible({ timeout: 10000 })

    // Select CSV format
    const csvRadio = page.locator('input[value="csv"]')
    await csvRadio.check()
    await expect(csvRadio).toBeChecked()

    // Set up download listener
    const downloadPromise = page.waitForEvent('download', { timeout: 30000 })

    // Click Download Export button
    await page.locator('button:has-text("Download Export")').click()

    // Wait for download
    const download = await downloadPromise

    // Verify download filename contains .csv
    expect(download.suggestedFilename()).toContain('.csv')

    // Take screenshot
    await page.screenshot({ path: 'e2e/screenshots/export-csv.png' })
  })

  test('export button shows loading state', async ({ page }) => {
    await page.goto('/settings')
    await page.waitForLoadState('domcontentloaded')

    // Wait for settings to load
    await expect(page.locator('text=Settings')).toBeVisible({ timeout: 10000 })

    // Intercept export API to delay response
    await page.route('/api/export*', async (route) => {
      await new Promise(resolve => setTimeout(resolve, 1000))
      await route.continue()
    })

    // Click Download Export button
    await page.locator('button:has-text("Download Export")').click()

    // Verify button shows loading state (aria-busy and Exporting text)
    const exportButton = page.locator('button:has-text("Exporting")')
    await expect(exportButton).toBeVisible({ timeout: 2000 })
    await expect(exportButton).toHaveAttribute('aria-busy', 'true')
  })
})
