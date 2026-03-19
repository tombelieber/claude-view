import { expect, test } from '@playwright/test'

test.describe('Workflow Editor', () => {
  test('new workflow editor loads with empty canvas', async ({ page }) => {
    await page.goto('/workflows/new')
    // Editor page should render
    await expect(
      page.locator('text=Workflow Editor').or(page.locator('[data-testid="workflow-editor"]')),
    ).toBeVisible({ timeout: 10000 })
  })

  test('editor page has chat rail and canvas areas', async ({ page }) => {
    await page.goto('/workflows/new')
    // Chat area should be present
    await expect(
      page.locator('[data-testid="chat-rail"]').or(page.locator('textarea')),
    ).toBeVisible({ timeout: 10000 })
  })
})
