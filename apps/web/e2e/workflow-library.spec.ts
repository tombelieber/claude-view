import { expect, test } from '@playwright/test'

test.describe('Workflow Library', () => {
  test('library page loads and shows workflow list', async ({ page }) => {
    await page.goto('/workflows')
    // Page has "Workflows" heading
    await expect(page.locator('h1:has-text("Workflows")')).toBeVisible({ timeout: 10000 })
  })

  test('"New Workflow" button navigates to editor', async ({ page }) => {
    await page.goto('/workflows')
    await page.click('a[href="/workflows/new"]:has-text("New Workflow")')
    await expect(page).toHaveURL(/\/workflows\/new/)
  })

  test('official workflows are displayed', async ({ page }) => {
    await page.goto('/workflows')
    await expect(page.locator('h3:has-text("Plan Polisher")')).toBeVisible({ timeout: 10000 })
    await expect(page.locator('h3:has-text("Plan Executor")')).toBeVisible({ timeout: 10000 })
  })

  test('clicking View navigates to detail page', async ({ page }) => {
    await page.goto('/workflows')
    // Wait for workflows to render (no networkidle — SSE keeps connections open)
    await expect(page.locator('text=Plan Polisher')).toBeVisible({ timeout: 15000 })
    await page.locator('button:has-text("View")').first().click()
    await expect(page).toHaveURL(/\/workflows\/plan-[a-z]+/, { timeout: 10000 })
  })
})
