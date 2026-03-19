import { expect, test } from '@playwright/test'

/**
 * Chat lifecycle E2E tests.
 *
 * These require a running sidecar with an API key.
 * The beforeAll hook verifies the sidecar is reachable and fails loudly if not.
 *
 * Run locally:
 *   bun dev   # start the full stack (server + sidecar)
 *   npx playwright test chat-lifecycle
 */

const CHAT_INPUT = '[data-testid="chat-input"]'

/** Type a message and press Enter to send it. */
async function sendMessage(page: import('@playwright/test').Page, text: string) {
  const input = page.locator(CHAT_INPUT)
  await input.fill(text)
  await page.keyboard.press('Enter')
}

/** Wait for at least one assistant text segment to appear in the thread. */
async function waitForAssistantText(page: import('@playwright/test').Page, timeoutMs = 30_000) {
  await page.locator('[data-testid="assistant-message"]').first().waitFor({
    state: 'visible',
    timeout: timeoutMs,
  })
}

test.describe('Chat Lifecycle', () => {
  test.beforeAll(async ({ request }) => {
    // Verify sidecar is available — fail loudly if not
    const res = await request.get('/api/control/sessions').catch(() => null)
    if (!res || !res.ok()) {
      throw new Error(
        'Sidecar not available. Start the full stack with `bun dev` before running chat E2E tests.\n' +
          'These tests require a running sidecar + ANTHROPIC_API_KEY.',
      )
    }
  })

  test('send message, streaming renders, no vanish after turn completes', async ({ page }) => {
    await page.goto('/chat')
    await page.waitForSelector(CHAT_INPUT, { timeout: 10_000 })

    // Send a simple prompt
    await sendMessage(page, 'Say exactly: "hello e2e test"')

    // The URL should transition from /chat to /chat/<sessionId>
    await expect(page).toHaveURL(/\/chat\/[a-f0-9-]+/, { timeout: 15_000 })

    // Assistant text appears during streaming
    await waitForAssistantText(page)

    // The streaming cursor (animate-pulse bar) should eventually disappear
    // when the turn completes, indicating turn_complete was received.
    const cursor = page.locator('.animate-pulse.rounded-sm')
    await expect(cursor).toBeHidden({ timeout: 60_000 })

    // Critical assertion: text REMAINS after turn completes (no vanish bug).
    // Wait 2s then re-check that assistant content is still in the DOM.
    await page.waitForTimeout(2_000)
    const threadText = await page.locator('[data-testid="message-thread"]').innerText()
    expect(threadText.toLowerCase()).toContain('hello')

    // Regression: doubled text (Bug #1). The assistant's response should NOT
    // contain the same text repeated back-to-back.
    const assistantText = await page.locator('[data-testid="message-thread"]').innerText()
    expect(assistantText).not.toContain('hello e2e testhello e2e test')

    // No stuck loading spinner
    const spinner = page.locator('.animate-spin')
    await expect(spinner).toBeHidden({ timeout: 5_000 })
  })

  test('page refresh reconnects and shows all messages from snapshot', async ({ page }) => {
    await page.goto('/chat')
    await page.waitForSelector(CHAT_INPUT, { timeout: 10_000 })

    // Send message and wait for the response to complete
    await sendMessage(page, 'Repeat this word exactly once: "reconnect-test"')
    await expect(page).toHaveURL(/\/chat\/[a-f0-9-]+/, { timeout: 15_000 })
    await waitForAssistantText(page)

    // Wait for turn to complete (streaming cursor gone)
    const cursor = page.locator('.animate-pulse.rounded-sm')
    await expect(cursor).toBeHidden({ timeout: 60_000 })

    // Capture message count before reload
    const threadContainer = page.locator('[data-testid="message-thread"]')
    const blockCountBefore = await threadContainer
      .locator('[data-testid="assistant-message"], [data-testid="user-message"]')
      .count()
    expect(blockCountBefore).toBeGreaterThan(0)

    // Reload the page
    await page.reload()
    await page.waitForLoadState('domcontentloaded')

    // All previous messages should be visible within 3s (snapshot restore)
    await page.waitForTimeout(3_000)
    const threadAfterReload = page.locator('[data-testid="message-thread"]')
    const blockCountAfter = await threadAfterReload
      .locator('[data-testid="assistant-message"], [data-testid="user-message"]')
      .count()
    expect(blockCountAfter).toBeGreaterThanOrEqual(blockCountBefore)

    // Content still present
    const pageText = await page.locator('[data-testid="message-thread"]').innerText()
    expect(pageText.toLowerCase()).toContain('reconnect')

    // No duplicate messages: each user bubble text should appear exactly once
    const userBubbles = page.locator('[data-testid="user-message"]')
    const userTexts = await userBubbles.allInnerTexts()
    const uniqueTexts = new Set(userTexts)
    expect(uniqueTexts.size).toBe(userTexts.length)
  })

  test('live to history transition shows messages via JSONL', async ({ page }) => {
    // Start a session and send a message
    await page.goto('/chat')
    await page.waitForSelector(CHAT_INPUT, { timeout: 10_000 })
    await sendMessage(page, 'Reply with the word "history-check" and nothing else.')
    await expect(page).toHaveURL(/\/chat\/[a-f0-9-]+/, { timeout: 15_000 })
    await waitForAssistantText(page)

    // Wait for turn to complete
    const cursor = page.locator('.animate-pulse.rounded-sm')
    await expect(cursor).toBeHidden({ timeout: 60_000 })

    // Extract the session ID from the URL
    const url = page.url()
    const sessionId = url.split('/chat/')[1]
    expect(sessionId).toBeTruthy()

    // Navigate away to a different page
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(1_000)

    // Navigate back to the session — this should load via history (JSONL)
    await page.goto(`/chat/${sessionId}`)
    await page.waitForLoadState('domcontentloaded')

    // Messages should be visible via history loading
    await page.waitForTimeout(3_000)
    const threadText = await page.locator('[data-testid="message-thread"]').innerText()
    expect(threadText.toLowerCase()).toContain('history')

    // No 404 error state — check that the empty state placeholder is NOT shown
    const emptyState = page.getByText('Start a conversation')
    await expect(emptyState).toBeHidden({ timeout: 5_000 })

    // Regression: "Failed to load messages" error banner (Bug #2).
    // Should NOT show any error state during the init->history transition.
    const errorBanner = page.getByText('Failed to load')
    await expect(errorBanner).toBeHidden({ timeout: 5_000 })
  })

  test('new session appears in sidebar active section', async ({ page }) => {
    await page.goto('/chat')
    await page.waitForSelector(CHAT_INPUT, { timeout: 10_000 })

    // Send a message to create a new session
    await sendMessage(page, 'Say "sidebar-test" and nothing else.')
    await expect(page).toHaveURL(/\/chat\/[a-f0-9-]+/, { timeout: 15_000 })

    // The session sidebar (nav with aria-label="Chat history") should show the
    // new session. The sidebar fetches sidecar sessions every 5s, so we give
    // it up to 10s to appear.
    const sidebar = page.locator('nav[aria-label="Chat history"]')
    await expect(sidebar).toBeVisible({ timeout: 10_000 })

    // The "Active" section header should be present
    const activeHeader = sidebar.getByText('Active', { exact: false })
    await expect(activeHeader).toBeVisible({ timeout: 10_000 })

    // At least one session item should be visible in the sidebar
    // SessionListItem renders items with a status dot + text content
    const sessionItems = sidebar.locator('.cursor-pointer')
    await expect(sessionItems.first()).toBeVisible({ timeout: 10_000 })
  })
})
