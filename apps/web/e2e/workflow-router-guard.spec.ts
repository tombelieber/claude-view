import { expect, test } from '@playwright/test'

test.describe('Workflow Router Guards', () => {
  test('/workflows/new renders editor (NOT detail page for id="new")', async ({ page }) => {
    await page.goto('/workflows/new')
    // Should show editor elements: chat textarea + "New Workflow" header
    await expect(page.locator('textarea')).toBeVisible({ timeout: 10000 })
    await expect(page.locator('text=New Workflow')).toBeVisible()
  })

  test('/workflows/abc renders detail page (not editor)', async ({ page }) => {
    await page.goto('/workflows/abc')
    // Should show detail or 404, NOT the editor textarea
    await expect(page.locator('textarea')).not.toBeVisible({ timeout: 5000 })
  })
})
