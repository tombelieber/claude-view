import { expect, test } from '@playwright/test'

test.describe('Command Palette', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/chat')
    await page.waitForSelector('[data-testid="chat-input"]', { timeout: 10_000 })
  })

  test('slash key opens command palette', async ({ page }) => {
    const input = page.locator('[data-testid="chat-input"]')
    await input.fill('/')
    await expect(page.locator('[data-testid="command-palette"]')).toBeVisible()
  })

  test('palette shows section headers', async ({ page }) => {
    const input = page.locator('[data-testid="chat-input"]')
    await input.fill('/')
    await expect(page.getByText('Context')).toBeVisible()
    await expect(page.getByText('Model')).toBeVisible()
    await expect(page.getByText('Customize')).toBeVisible()
  })

  test('Escape closes palette', async ({ page }) => {
    const input = page.locator('[data-testid="chat-input"]')
    await input.fill('/')
    await expect(page.locator('[data-testid="command-palette"]')).toBeVisible()
    await page.keyboard.press('Escape')
    await expect(page.locator('[data-testid="command-palette"]')).not.toBeVisible()
  })

  test('filtering narrows visible items', async ({ page }) => {
    const input = page.locator('[data-testid="chat-input"]')
    await input.fill('/com')
    await expect(page.getByText('/commit')).toBeVisible()
  })

  test('clicking Clear conversation invokes action', async ({ page }) => {
    const input = page.locator('[data-testid="chat-input"]')
    await input.fill('/')
    await page.getByText('Clear conversation').click()
    await expect(page.locator('[data-testid="command-palette"]')).not.toBeVisible()
  })
})
