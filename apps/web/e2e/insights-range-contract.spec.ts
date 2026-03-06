import { expect, test } from '@playwright/test'

type EffectiveRangeResponse = {
  meta?: {
    effectiveRange?: {
      from?: number
      to?: number
      source?: string
    }
  }
}

test.describe('Insights range contract', () => {
  test('deep-link /insights renders Insights page without analytics redirect', async ({ page }) => {
    await page.goto('/insights')
    await page.waitForLoadState('domcontentloaded')

    const url = new URL(page.url())
    expect(url.pathname).toBe('/insights')
    expect(url.search).not.toContain('tab=insights')
    await expect(page.getByRole('heading', { name: 'Insights' })).toBeVisible()
  })

  test('default insights endpoints expose effective all-time range metadata', async ({
    request,
  }) => {
    const endpoints = [
      '/api/insights',
      '/api/insights/categories',
      '/api/insights/trends?metric=sessions&granularity=week',
    ]

    for (const endpoint of endpoints) {
      const response = await request.get(endpoint, { timeout: 60000 })
      expect(response.ok(), `${endpoint} should return 2xx`).toBeTruthy()

      const body = (await response.json()) as EffectiveRangeResponse
      expect(body.meta).toBeDefined()
      expect(body.meta?.effectiveRange).toBeDefined()
      expect(typeof body.meta?.effectiveRange?.from).toBe('number')
      expect(typeof body.meta?.effectiveRange?.to).toBe('number')
      expect(body.meta?.effectiveRange?.from).toBeLessThanOrEqual(
        body.meta?.effectiveRange?.to ?? 0,
      )
      expect(body.meta?.effectiveRange?.source).toBe('default_all_time')
    }
  })

  test('one-sided insights range params are rejected in strict mode', async ({ request }) => {
    const urls = [
      '/api/insights?from=1700000000',
      '/api/insights/categories?to=1700000000',
      '/api/insights/trends?metric=sessions&granularity=week&from=1700000000',
    ]

    for (const url of urls) {
      const response = await request.get(url, { timeout: 60000 })
      expect(response.status(), `${url} should return 400`).toBe(400)
    }
  })
})
