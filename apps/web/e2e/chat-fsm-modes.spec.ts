import { expect, test } from '@playwright/test'

const CHAT_INPUT = '[data-testid="chat-input"]'

async function sendMessage(page: import('@playwright/test').Page, text: string) {
  const input = page.locator(CHAT_INPUT)
  await input.fill(text)
  await page.keyboard.press('Enter')
}

async function getPanelMode(page: import('@playwright/test').Page) {
  const panel = page.locator('[data-panel-mode]').first()
  return {
    mode: await panel.getAttribute('data-panel-mode'),
    substate: await panel.getAttribute('data-panel-substate'),
  }
}

test.describe('Chat Panel FSM Modes', () => {
  test.beforeAll(async ({ request }) => {
    const res = await request.get('/api/sessions').catch(() => null)
    if (!res || !res.ok()) {
      throw new Error(
        'Sidecar not available. Start the full stack with `bun dev` before running FSM E2E tests.\n' +
          'These tests require a running sidecar + ANTHROPIC_API_KEY.',
      )
    }
  })

  // ─── FSM-01: BLANK → CONNECTING → OWN(active) → OWN(streaming) → OWN(active) ───

  test('FSM-01: new chat transitions through blank → connecting → own lifecycle', async ({
    page,
  }) => {
    await page.goto('/chat')
    await page.waitForSelector(CHAT_INPUT, { timeout: 10_000 })

    const input = page.locator(CHAT_INPUT)
    await expect(input).toBeVisible()

    await sendMessage(page, 'Say exactly: "fsm-test-response"')

    await expect(page).toHaveURL(/\/chat\/[a-f0-9-]+/, { timeout: 15_000 })

    const disabledInput = page.locator(`${CHAT_INPUT}[disabled]`)
    await expect(disabledInput).toBeVisible({ timeout: 15_000 })

    await page.locator('[data-testid="assistant-message"]').first().waitFor({
      state: 'visible',
      timeout: 30_000,
    })

    const cursor = page.locator('.animate-pulse.rounded-sm')
    await expect(cursor).toBeHidden({ timeout: 60_000 })

    await expect(input).not.toBeDisabled({ timeout: 5_000 })

    const threadText = await page.locator('[data-testid="message-thread"]').innerText()
    expect(threadText.toLowerCase()).toContain('fsm-test-response')

    const panelModeAttr = await page
      .locator('[data-panel-mode]')
      .first()
      .getAttribute('data-panel-mode')
      .catch(() => null)
    if (panelModeAttr) {
      expect(panelModeAttr).toBe('own')
    }
  })

  // ─── FSM-02: HISTORY → resume → OWN ───

  test('FSM-02: history session resumes to own mode when user types', async ({ page }) => {
    await page.goto('/chat')
    await page.waitForSelector(CHAT_INPUT, { timeout: 10_000 })
    await sendMessage(page, 'Say "resume-test" and nothing else.')
    await expect(page).toHaveURL(/\/chat\/[a-f0-9-]+/, { timeout: 15_000 })

    await page.locator('[data-testid="assistant-message"]').first().waitFor({
      state: 'visible',
      timeout: 30_000,
    })
    const cursor = page.locator('.animate-pulse.rounded-sm')
    await expect(cursor).toBeHidden({ timeout: 60_000 })

    const sessionId = page.url().split('/chat/')[1]
    expect(sessionId).toBeTruthy()

    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(2_000)

    await page.goto(`/chat/${sessionId}`)
    await page.waitForLoadState('domcontentloaded')

    const input = page.locator(CHAT_INPUT)
    await expect(input).toBeVisible({ timeout: 10_000 })
    await expect(input).not.toBeDisabled({ timeout: 5_000 })

    await sendMessage(page, 'Say "resumed" and nothing else.')

    await page.waitForTimeout(3_000)
    const thread = page.locator('[data-testid="message-thread"]')
    const assistantMessages = thread.locator('[data-testid="assistant-message"]')
    await expect(assistantMessages).toHaveCount(2, { timeout: 30_000 })
  })

  // ─── FSM-03: Input disabled during streaming, re-enabled after ───

  test('FSM-03: input bar disabled during OWN(streaming), enabled during OWN(active)', async ({
    page,
  }) => {
    await page.goto('/chat')
    await page.waitForSelector(CHAT_INPUT, { timeout: 10_000 })
    await sendMessage(page, 'Write a long paragraph about the color blue.')
    await expect(page).toHaveURL(/\/chat\/[a-f0-9-]+/, { timeout: 15_000 })

    const input = page.locator(CHAT_INPUT)
    await expect(input).toBeDisabled({ timeout: 15_000 })

    const placeholder = await input.getAttribute('placeholder')
    expect(placeholder).toMatch(/responding|processing/i)

    const cursor = page.locator('.animate-pulse.rounded-sm')
    await expect(cursor).toBeHidden({ timeout: 60_000 })

    await expect(input).not.toBeDisabled({ timeout: 5_000 })

    const activePlaceholder = await input.getAttribute('placeholder')
    expect(activePlaceholder).toMatch(/send|message|command/i)
  })

  // ─── FSM-04: P1 Regression — dormant resume doesn't false-fail ───

  test('FSM-04: dormant session resume does not show Failed status (P1 timer regression)', async ({
    page,
  }) => {
    await page.goto('/chat')
    await page.waitForSelector(CHAT_INPUT, { timeout: 10_000 })
    await sendMessage(page, 'Say "timer-test" and nothing else.')
    await expect(page).toHaveURL(/\/chat\/[a-f0-9-]+/, { timeout: 15_000 })

    const cursor = page.locator('.animate-pulse.rounded-sm')
    await page.locator('[data-testid="assistant-message"]').first().waitFor({
      state: 'visible',
      timeout: 30_000,
    })
    await expect(cursor).toBeHidden({ timeout: 60_000 })

    const sessionId = page.url().split('/chat/')[1]

    await page.goto('/')
    await page.waitForTimeout(2_000)

    await page.goto(`/chat/${sessionId}`)
    await page.waitForSelector(CHAT_INPUT, { timeout: 10_000 })

    await sendMessage(page, 'Say "timer-ok" and nothing else.')

    await page.waitForTimeout(25_000)

    const thread = page.locator('[data-testid="message-thread"]')
    const failedIndicator = thread.locator('text=Failed')
    await expect(failedIndicator).toBeHidden({ timeout: 5_000 })

    const assistantMessages = thread.locator('[data-testid="assistant-message"]')
    const count = await assistantMessages.count()
    expect(count).toBeGreaterThanOrEqual(2)
  })

  // ─── FSM-05: No page crash on any mode transition ───

  test('FSM-05: full lifecycle produces zero console errors', async ({ page }) => {
    const errors: string[] = []
    page.on('pageerror', (err) => {
      errors.push(err.message)
    })

    await page.goto('/chat')
    await page.waitForSelector(CHAT_INPUT, { timeout: 10_000 })

    await sendMessage(page, 'Reply with just "ok".')
    await expect(page).toHaveURL(/\/chat\/[a-f0-9-]+/, { timeout: 15_000 })
    await page.locator('[data-testid="assistant-message"]').first().waitFor({
      state: 'visible',
      timeout: 30_000,
    })
    const cursor = page.locator('.animate-pulse.rounded-sm')
    await expect(cursor).toBeHidden({ timeout: 60_000 })

    await sendMessage(page, 'Reply with just "ok" again.')
    await expect(cursor).toBeVisible({ timeout: 15_000 })
    await expect(cursor).toBeHidden({ timeout: 60_000 })

    const fatalErrors = errors.filter(
      (e) =>
        !e.includes('fetch') &&
        !e.includes('WebSocket') &&
        !e.includes('ERR_CONNECTION') &&
        !e.includes('NetworkError') &&
        !e.includes('Failed to fetch'),
    )
    expect(fatalErrors).toHaveLength(0)
  })
})
