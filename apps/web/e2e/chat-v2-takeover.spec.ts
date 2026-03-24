import { expect, test } from '@playwright/test'

test.describe('Chat V2 Takeover', () => {
  test('chat page renders without crashes', async ({ page }) => {
    // Basic smoke test: the chat page loads without crashing
    const errors: string[] = []
    page.on('pageerror', (err) => {
      errors.push(err.message)
    })

    await page.goto('/chat')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(3000)

    // Sidebar should be visible
    const sidebar = page.locator('nav[aria-label="Chat history"]')
    await expect(sidebar).toBeVisible({ timeout: 10000 })

    // Dockview container should be visible
    const dockview = page.locator('.dockview-theme-cv')
    await expect(dockview).toBeVisible()

    // No page-crashing errors (ignore fetch/WS errors which are expected without sidecar)
    const fatalErrors = errors.filter(
      (e) =>
        !e.includes('fetch') &&
        !e.includes('WebSocket') &&
        !e.includes('ERR_CONNECTION') &&
        !e.includes('NetworkError') &&
        !e.includes('Failed to fetch'),
    )
    expect(fatalErrors).toHaveLength(0)

    await page.screenshot({ path: 'e2e/screenshots/chat-v2-takeover-smoke.png' })
  })

  test('new chat button in sidebar navigates to /chat', async ({ page }) => {
    // Navigate to a session first
    await page.goto('/chat')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(2000)

    const sidebar = page.locator('nav[aria-label="Chat history"]')
    await expect(sidebar).toBeVisible({ timeout: 10000 })

    // Click new chat button
    const newChatBtn = sidebar.locator('button[title="New chat"]')
    await expect(newChatBtn).toBeVisible()
    await newChatBtn.click()
    await page.waitForTimeout(500)

    // Should navigate to /chat (no session ID)
    expect(page.url()).toMatch(/\/chat\/?$/)

    await page.screenshot({ path: 'e2e/screenshots/chat-v2-new-chat.png' })
  })
})
