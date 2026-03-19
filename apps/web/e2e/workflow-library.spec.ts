import { expect, test } from '@playwright/test'

test.describe('Workflow Library', () => {
  test('library page loads and shows workflow list', async ({ page }) => {
    await page.goto('/workflows')
    await expect(page.locator('h1')).toContainText('Workflows')
  })

  test('"New Workflow" button navigates to editor', async ({ page }) => {
    await page.goto('/workflows')
    await page.click('text=New Workflow')
    await expect(page).toHaveURL(/\/workflows\/new/)
  })

  test('official workflows are displayed', async ({ page }) => {
    await page.goto('/workflows')
    // Official workflows should be seeded on server start
    await expect(page.locator('[data-testid="workflow-card"]').first()).toBeVisible({
      timeout: 10000,
    })
  })

  test('clicking a workflow navigates to detail page', async ({ page }) => {
    await page.goto('/workflows')
    const firstCard = page.locator('[data-testid="workflow-card"]').first()
    await firstCard.click()
    await expect(page).toHaveURL(/\/workflows\/[a-z0-9-]+$/)
  })
})
