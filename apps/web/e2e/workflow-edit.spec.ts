import { expect, test } from '@playwright/test'

test.describe('Workflow Editor', () => {
  test('new workflow editor loads with chat and canvas', async ({ page }) => {
    await page.goto('/workflows/new')
    // Editor renders: empty canvas message + textarea for chat
    await expect(page.locator('text=No workflow yet')).toBeVisible({ timeout: 15000 })
    await expect(page.locator('textarea')).toBeVisible({ timeout: 5000 })
  })

  test('editor page has send button', async ({ page }) => {
    await page.goto('/workflows/new')
    await expect(page.locator('button:has-text("Send")')).toBeVisible({ timeout: 15000 })
  })
})
