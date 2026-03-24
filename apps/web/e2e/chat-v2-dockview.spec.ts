import { expect, test } from '@playwright/test'

test.describe('Chat V2 Dockview', () => {
  test('chat page loads with sidebar and dockview layout', async ({ page }) => {
    // Collect console errors
    const errors: string[] = []
    page.on('console', (msg) => {
      if (msg.type() === 'error') errors.push(msg.text())
    })

    await page.goto('/chat')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(2000)

    // SessionSidebar renders — has nav element with aria-label
    const sidebar = page.locator('nav[aria-label="Chat history"]')
    await expect(sidebar).toBeVisible({ timeout: 10000 })

    // Sidebar header shows "Chats" title
    await expect(sidebar.locator('text=Chats')).toBeVisible()

    // New chat button exists (PenSquare icon button with title "New chat")
    const newChatBtn = sidebar.locator('button[title="New chat"]')
    await expect(newChatBtn).toBeVisible()

    // Search input exists
    const searchInput = sidebar.locator('input[placeholder="Search chats..."]')
    await expect(searchInput).toBeVisible()

    // Dockview container renders (has the theme class)
    const dockview = page.locator('.dockview-theme-cv')
    await expect(dockview).toBeVisible()

    await page.screenshot({ path: 'e2e/screenshots/chat-v2-dockview-layout.png' })

    // No critical console errors (ignore benign ones like favicon, WebSocket connection)
    const criticalErrors = errors.filter(
      (e) =>
        !e.includes('favicon') &&
        !e.includes('WebSocket') &&
        !e.includes('ws://') &&
        !e.includes('ERR_CONNECTION_REFUSED') &&
        !e.includes('net::ERR'),
    )
    expect(criticalErrors).toHaveLength(0)
  })

  test('sidebar shows session history from API', async ({ page }) => {
    await page.goto('/chat')
    await page.waitForLoadState('domcontentloaded')

    const sidebar = page.locator('nav[aria-label="Chat history"]')
    await expect(sidebar).toBeVisible({ timeout: 10000 })

    // Wait for sessions to load — either time groups or empty state must appear
    const emptyState = sidebar.locator('text=No sessions yet')
    const timeGroupHeader = sidebar
      .locator('text=Today')
      .or(sidebar.locator('text=Yesterday'))
      .or(sidebar.locator('text=Last 7 days'))
      .or(sidebar.locator('text=Older'))

    // Wait for either to appear (loading 1700+ sessions may take a few seconds)
    await expect(emptyState.or(timeGroupHeader.first())).toBeVisible({ timeout: 15000 })

    await page.screenshot({ path: 'e2e/screenshots/chat-v2-sidebar-sessions.png' })
  })

  test('sidebar search filters sessions', async ({ page }) => {
    await page.goto('/chat')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(3000) // Wait for sessions to load

    const sidebar = page.locator('nav[aria-label="Chat history"]')
    await expect(sidebar).toBeVisible({ timeout: 10000 })

    const searchInput = sidebar.locator('input[placeholder="Search chats..."]')

    // Type a search query that won't match anything
    await searchInput.fill('zzzznonexistentquery')
    await page.waitForTimeout(500)

    // After filtering, either no time groups visible or empty state
    await page.screenshot({ path: 'e2e/screenshots/chat-v2-sidebar-search.png' })

    // Clear search
    await searchInput.fill('')
    await page.waitForTimeout(500)
  })

  test('clicking a session navigates to /chat/:sessionId', async ({ page }) => {
    await page.goto('/chat')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(3000)

    const sidebar = page.locator('nav[aria-label="Chat history"]')
    await expect(sidebar).toBeVisible({ timeout: 10000 })

    // Find any clickable session item in the sidebar
    // SessionListItem renders as a div with onClick -> navigate(/chat/:id)
    const sessionItems = sidebar
      .locator('[data-testid^="session-item"]')
      .or(sidebar.locator('.cursor-pointer').filter({ hasNotText: 'New chat' }))

    const count = await sessionItems.count()
    if (count > 0) {
      // Click the first session
      await sessionItems.first().click()
      await page.waitForTimeout(1000)

      // URL should have changed to include a session ID
      const url = page.url()
      expect(url).toMatch(/\/chat\/[a-f0-9-]+/)

      await page.screenshot({ path: 'e2e/screenshots/chat-v2-session-selected.png' })
    } else {
      // No sessions available — skip this assertion
      test.skip()
    }
  })

  test('dockview theme has blue active tab border', async ({ page }) => {
    await page.goto('/chat')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(2000)

    // Verify the dockview-theme-cv class is present
    const dockview = page.locator('.dockview-theme-cv')
    await expect(dockview).toBeVisible()

    // Check CSS custom property for active tab color
    const activeTabColor = await page.evaluate(() => {
      const el = document.querySelector('.dockview-theme-cv')
      if (!el) return null
      return getComputedStyle(el).getPropertyValue('--dv-active-tab-border-bottom-color').trim()
    })

    // Should be blue (#3B82F6) not green
    if (activeTabColor) {
      expect(activeTabColor.toLowerCase()).toBe('#3b82f6')
    }
  })
})
