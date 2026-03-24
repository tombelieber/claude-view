import { expect, test } from '@playwright/test'

test.describe('Chat V2 Keyboard', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/chat')
    await page.waitForLoadState('domcontentloaded')
    await page.waitForTimeout(2000)
    // Verify the dockview container is ready
    await expect(page.locator('.dockview-theme-cv')).toBeVisible({ timeout: 10000 })
  })

  test('Ctrl+T opens new session tab', async ({ page }) => {
    // Initially no panels open (dockview shows watermark or empty state)
    const panelsBefore = await page.locator('.dv-tab').count()

    // Press Ctrl+T (click on the dockview area first to ensure focus is not in an input)
    await page.locator('.dockview-theme-cv').click()
    await page.keyboard.press('Control+t')
    await page.waitForTimeout(1000)

    // A new tab should have appeared (dockview uses .dv-tab class)
    const panelsAfter = await page.locator('.dv-tab').count()
    expect(panelsAfter).toBeGreaterThan(panelsBefore)

    await page.screenshot({ path: 'e2e/screenshots/chat-v2-ctrl-t.png' })
  })

  test('Ctrl+W closes active tab', async ({ page }) => {
    // First open a tab with Ctrl+T
    await page.locator('.dockview-theme-cv').click()
    await page.keyboard.press('Control+t')
    await page.waitForTimeout(1000)

    const panelsBeforeClose = await page.locator('.dv-tab').count()
    expect(panelsBeforeClose).toBeGreaterThanOrEqual(1)

    // Now close with Ctrl+W — click on the dockview area (not in textarea)
    await page.locator('.dockview-theme-cv').click({ position: { x: 10, y: 10 } })
    await page.keyboard.press('Control+w')
    await page.waitForTimeout(500)

    const panelsAfterClose = await page.locator('.dv-tab').count()
    expect(panelsAfterClose).toBeLessThan(panelsBeforeClose)

    await page.screenshot({ path: 'e2e/screenshots/chat-v2-ctrl-w.png' })
  })
})
