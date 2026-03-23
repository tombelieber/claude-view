import { expect, test } from '@playwright/test'

test.describe('Chat V2 Resume', () => {
  test('clicking a history session opens it in a dockview panel', async ({ page }) => {
    await page.goto('/chat')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(3000)

    const sidebar = page.locator('nav[aria-label="Chat history"]')
    await expect(sidebar).toBeVisible({ timeout: 10000 })

    // Check if we have any sessions — the sidebar shows them as cursor-pointer divs
    // Look for time group headers to confirm sessions loaded
    const timeGroupHeaders = sidebar
      .locator('text=Today')
      .or(sidebar.locator('text=Yesterday'))
      .or(sidebar.locator('text=Last 7 days'))
      .or(sidebar.locator('text=Older'))

    const hasGroups = await timeGroupHeaders
      .first()
      .isVisible()
      .catch(() => false)
    if (!hasGroups) {
      test.skip(true, 'No history sessions available to test resume')
      return
    }

    // Click the first session item (skip the group header)
    // Session items have py-1.5 and cursor-pointer classes
    const firstSession = sidebar.locator('[role="button"], .cursor-pointer').first()
    if (await firstSession.isVisible().catch(() => false)) {
      await firstSession.click()
      await page.waitForTimeout(1500)

      // URL should contain a session ID
      expect(page.url()).toMatch(/\/chat\/[a-f0-9-]+/)

      // A dockview tab should now be visible
      const tabs = page.locator('.dv-tab')
      const tabCount = await tabs.count()
      expect(tabCount).toBeGreaterThanOrEqual(1)

      await page.screenshot({ path: 'e2e/screenshots/chat-v2-resume-session.png' })
    }
  })

  test('resumed session shows conversation thread area', async ({ page }) => {
    await page.goto('/chat')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(3000)

    const sidebar = page.locator('nav[aria-label="Chat history"]')
    const timeGroupHeaders = sidebar
      .locator('text=Today')
      .or(sidebar.locator('text=Yesterday'))
      .or(sidebar.locator('text=Last 7 days'))
      .or(sidebar.locator('text=Older'))

    const hasGroups = await timeGroupHeaders
      .first()
      .isVisible()
      .catch(() => false)
    if (!hasGroups) {
      test.skip(true, 'No history sessions available')
      return
    }

    const firstSession = sidebar.locator('[role="button"], .cursor-pointer').first()
    if (await firstSession.isVisible().catch(() => false)) {
      await firstSession.click()
      await page.waitForTimeout(1500)

      // The panel should contain a chat input area
      const chatInput = page.locator('[data-testid="chat-input"]')
      await expect(chatInput).toBeVisible({ timeout: 5000 })

      await page.screenshot({ path: 'e2e/screenshots/chat-v2-resumed-panel.png' })
    }
  })
})
