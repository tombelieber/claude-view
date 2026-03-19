import { expect, test } from '@playwright/test'

test.describe('Workflow Router Guards', () => {
  test('/workflows/new renders editor (NOT detail page for id="new")', async ({ page }) => {
    await page.goto('/workflows/new')
    // Should NOT show workflow detail page
    await expect(page.locator('text=Workflow not found')).not.toBeVisible()
    // Should show editor
    await expect(
      page.locator('[data-testid="workflow-editor"]').or(page.locator('text=Workflow Editor')),
    ).toBeVisible({ timeout: 10000 })
  })

  test('/workflows/abc renders detail page (not editor)', async ({ page }) => {
    await page.goto('/workflows/abc')
    // Should show detail or 404, NOT the editor
    await expect(page.locator('text=Workflow Editor')).not.toBeVisible()
  })
})
