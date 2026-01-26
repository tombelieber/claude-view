import { test, expect } from '@playwright/test'

test.describe('Smoke Tests', () => {
  test('app loads and navigates', async ({ page }) => {
    // Collect console errors
    const errors: string[] = []
    page.on('console', msg => {
      if (msg.type() === 'error') errors.push(msg.text())
    })

    // Visit home page
    await page.goto('/')
    // Wait for DOM content to load (faster than networkidle which can hang on long-polling)
    await page.waitForLoadState('domcontentloaded')
    // Wait a bit for React hydration
    await page.waitForTimeout(2000)
    await page.screenshot({ path: 'e2e/screenshots/01-home.png' })

    // Verify basic UI elements - check for the title (Claude View)
    await expect(page).toHaveTitle(/Claude View/i)

    // Try to click first project in sidebar if exists
    // Projects are rendered as Link elements with FolderOpen icon
    const project = page.locator('aside a[href^="/project/"]').first()
    if (await project.isVisible({ timeout: 2000 }).catch(() => false)) {
      await project.click()
      await page.waitForTimeout(1000)
      await page.screenshot({ path: 'e2e/screenshots/02-project.png' })

      // Try to click first session card if exists
      // Sessions are rendered as buttons with class containing 'rounded-lg border'
      const session = page.locator('button.rounded-lg.border').first()
      if (await session.isVisible({ timeout: 2000 }).catch(() => false)) {
        await session.click()
        await page.waitForTimeout(1000)
        await page.screenshot({ path: 'e2e/screenshots/03-session.png' })
      }
    }

    // Verify no console errors (excluding favicon 404 which is benign)
    expect(errors.filter(e => !e.includes('favicon'))).toHaveLength(0)
  })

  test('health endpoint returns ok', async ({ request }) => {
    const response = await request.get('/api/health')
    expect(response.ok()).toBeTruthy()

    const data = await response.json()
    expect(data.status).toBe('ok')
    expect(data.version).toBeDefined()
  })

  test('projects endpoint returns array', async ({ request }) => {
    // Projects endpoint scans ~/.claude/projects which can take 60-90s on first load
    // with many sessions (large ~/.claude directory)
    const response = await request.get('/api/projects', { timeout: 120000 })
    expect(response.ok()).toBeTruthy()

    const data = await response.json()
    expect(Array.isArray(data)).toBeTruthy()
  })
})
