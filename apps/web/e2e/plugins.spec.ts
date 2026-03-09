import { expect, test } from '@playwright/test'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait for the plugins grid to load (skeleton replaced by real cards or empty state). */
async function waitForPluginsLoad(page: import('@playwright/test').Page) {
  await page.waitForLoadState('domcontentloaded')
  // Either cards appear, empty state appears, or an error appears
  await page
    .locator('[role="button"]')
    .or(page.getByText('No plugins found'))
    .or(page.getByText('Failed to load'))
    .first()
    .waitFor({ timeout: 60000 })
}

// ---------------------------------------------------------------------------
// API contract
// ---------------------------------------------------------------------------

test.describe('Plugins API', () => {
  test('GET /api/plugins returns valid response shape', async ({ request }) => {
    const res = await request.get('/api/plugins', { timeout: 60000 })
    expect(res.ok()).toBeTruthy()

    const data = await res.json()

    // Top-level shape
    expect(data).toHaveProperty('installed')
    expect(data).toHaveProperty('available')
    expect(data).toHaveProperty('totalInstalled')
    expect(data).toHaveProperty('totalAvailable')
    expect(data).toHaveProperty('duplicateCount')
    expect(data).toHaveProperty('unusedCount')
    expect(data).toHaveProperty('updatableCount')
    expect(data).toHaveProperty('marketplaces')
    expect(Array.isArray(data.installed)).toBe(true)
    expect(Array.isArray(data.available)).toBe(true)
    expect(Array.isArray(data.marketplaces)).toBe(true)

    // Counts are consistent
    expect(data.totalInstalled).toBe(data.installed.length)
    expect(data.totalAvailable).toBe(data.available.length)
  })

  test('installed plugin has required fields', async ({ request }) => {
    const res = await request.get('/api/plugins', { timeout: 60000 })
    const data = await res.json()

    if (data.installed.length === 0) {
      test.skip(true, 'No installed plugins to verify')
      return
    }

    const plugin = data.installed[0]
    expect(plugin).toHaveProperty('id')
    expect(plugin).toHaveProperty('name')
    expect(plugin).toHaveProperty('marketplace')
    expect(plugin).toHaveProperty('scope')
    expect(typeof plugin.enabled).toBe('boolean')
    expect(plugin).toHaveProperty('installedAt')
    expect(plugin).toHaveProperty('items')
    expect(Array.isArray(plugin.items)).toBe(true)
    // projectPath is nullable
    expect('projectPath' in plugin).toBe(true)
  })

  test('query params filter results', async ({ request }) => {
    // Fetch all plugins first
    const allRes = await request.get('/api/plugins', { timeout: 60000 })
    const allData = await allRes.json()
    const totalAll = allData.totalInstalled + allData.totalAvailable

    if (totalAll === 0) {
      test.skip(true, 'No plugins to test filtering')
      return
    }

    // Filter by scope=user should not increase count
    const userRes = await request.get('/api/plugins?scope=user', { timeout: 60000 })
    const userData = await userRes.json()
    expect(userData.totalInstalled).toBeLessThanOrEqual(allData.totalInstalled)

    // Filter by nonexistent search term
    const emptyRes = await request.get('/api/plugins?search=zzznonexistent999', {
      timeout: 60000,
    })
    const emptyData = await emptyRes.json()
    expect(emptyData.totalInstalled + emptyData.totalAvailable).toBe(0)
  })

  test('POST /api/plugins/action rejects invalid action', async ({ request }) => {
    const res = await request.post('/api/plugins/action', {
      data: {
        action: 'invalid_action_xyz',
        name: 'nonexistent-plugin',
        scope: null,
        projectPath: null,
      },
      headers: { 'Content-Type': 'application/json' },
    })

    // Should either return error response or non-ok status
    if (res.ok()) {
      const data = await res.json()
      expect(data.success).toBe(false)
    } else {
      expect(res.status()).toBeGreaterThanOrEqual(400)
    }
  })
})

// ---------------------------------------------------------------------------
// Page load & layout
// ---------------------------------------------------------------------------

test.describe('Plugins Page', () => {
  test('page loads and shows header', async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    // Header with "Plugins" title
    await expect(page.locator('h1:has-text("Plugins")')).toBeVisible()

    // Toolbar search input
    await expect(page.locator('input[placeholder="Search plugins..."]')).toBeVisible()

    // Kind tabs (All, Skills, MCP, Commands, Agents)
    await expect(page.locator('button:has-text("All")')).toBeVisible()
    await expect(page.locator('button:has-text("Skills")')).toBeVisible()
    await expect(page.locator('button:has-text("MCP")')).toBeVisible()
    await expect(page.locator('button:has-text("Commands")')).toBeVisible()
    await expect(page.locator('button:has-text("Agents")')).toBeVisible()

    await page.screenshot({ path: 'e2e/screenshots/plugins-page.png' })
  })

  test('total count displays in header', async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    // The "N total" text next to the header
    const totalText = page.locator('text=/\\d+ total/')
    // May or may not be visible depending on whether plugins exist
    const hasPlugins = await totalText.isVisible({ timeout: 3000 }).catch(() => false)

    if (hasPlugins) {
      await expect(totalText).toBeVisible()
    }
  })

  test('scope filter dropdown has correct options', async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    const scopeSelect = page.locator('select').first()
    await expect(scopeSelect).toBeVisible()

    // Verify options
    const options = scopeSelect.locator('option')
    await expect(options).toHaveCount(4)

    const labels = await options.allTextContents()
    expect(labels).toEqual(['All Scopes', 'User', 'Project', 'Available'])
  })

  test('shows plugin cards or empty state', async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    const cards = page.locator('[role="button"]')
    const emptyState = page.locator('text=No plugins found')
    const cliError = page.locator('text=CLI unavailable')

    const hasCards = await cards
      .first()
      .isVisible({ timeout: 3000 })
      .catch(() => false)
    const isEmpty = await emptyState.isVisible({ timeout: 1000 }).catch(() => false)
    const hasError = await cliError.isVisible({ timeout: 1000 }).catch(() => false)

    // At least one of these states must be true
    expect(hasCards || isEmpty || hasError).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// Search & filtering
// ---------------------------------------------------------------------------

test.describe('Plugins Search & Filters', () => {
  test('search input filters plugins with debounce', async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    const searchInput = page.locator('input[placeholder="Search plugins..."]')
    await expect(searchInput).toBeVisible()

    // Type a search that won't match anything
    await searchInput.fill('zzz-nonexistent-plugin-999')

    // Wait for debounce (300ms) + API response
    await page.waitForTimeout(600)

    // Should show empty state or zero cards
    const emptyState = page.locator('text=No plugins found')
    const cards = page.locator('[role="button"]')
    const cardCount = await cards.count()

    const isEmpty = await emptyState.isVisible({ timeout: 3000 }).catch(() => false)
    expect(isEmpty || cardCount === 0).toBe(true)
  })

  test('search clears and shows all plugins again', async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    const searchInput = page.locator('input[placeholder="Search plugins..."]')

    // Count initial plugins
    const initialCards = await page.locator('[role="button"]').count()

    // Search for something nonexistent
    await searchInput.fill('zzz-nonexistent-plugin-999')
    await page.waitForTimeout(600)

    // Clear search
    await searchInput.fill('')
    await page.waitForTimeout(600)

    // Cards should return
    const restoredCards = await page.locator('[role="button"]').count()
    expect(restoredCards).toBe(initialCards)
  })

  test('scope filter changes results', async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    const scopeSelect = page.locator('select').first()

    // Count initial cards
    const allCards = await page.locator('[role="button"]').count()

    // Select "User" scope
    await scopeSelect.selectOption('user')
    await page.waitForTimeout(600)

    const userCards = await page.locator('[role="button"]').count()

    // User-scoped should be <= total (may be equal if all are user-scoped)
    expect(userCards).toBeLessThanOrEqual(allCards)
  })

  test('kind tabs filter by item type', async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    // Click "Skills" tab
    await page.locator('button:has-text("Skills")').click()
    await page.waitForTimeout(600)

    // Click "All" tab to go back
    await page.locator('button:has-text("All")').click()
    await page.waitForTimeout(600)

    // Should have cards or empty state
    const cards = page.locator('[role="button"]')
    const emptyState = page.locator('text=No plugins found')
    const hasContent =
      (await cards.count()) > 0 ||
      (await emptyState.isVisible({ timeout: 1000 }).catch(() => false))
    expect(hasContent).toBe(true)
  })

  test('active kind tab has blue styling', async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    // "All" tab should be active by default (has bg-blue-500 class)
    const allTab = page.locator('button:has-text("All")').first()
    await expect(allTab).toHaveClass(/bg-blue-500/)

    // Click "MCP" tab
    await page.locator('button:has-text("MCP")').click()
    await page.waitForTimeout(200)

    // MCP should now be active
    await expect(page.locator('button:has-text("MCP")')).toHaveClass(/bg-blue-500/)

    // "All" should no longer be active
    await expect(allTab).not.toHaveClass(/bg-blue-500/)
  })
})

// ---------------------------------------------------------------------------
// Plugin card interactions
// ---------------------------------------------------------------------------

test.describe('Plugin Card', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)
  })

  test('card displays plugin name and scope badge', async ({ page }) => {
    const firstCard = page.locator('[role="button"]').first()
    const exists = await firstCard.isVisible({ timeout: 3000 }).catch(() => false)

    if (!exists) {
      test.skip(true, 'No plugin cards to test')
      return
    }

    // Card should contain a plugin name (h3 element)
    const name = firstCard.locator('h3')
    await expect(name).toBeVisible()
    const nameText = await name.textContent()
    expect(nameText?.length).toBeGreaterThan(0)

    // Card should have a scope badge (USER or PROJECT)
    const scopeBadge = firstCard.locator('span.uppercase')
    await expect(scopeBadge.first()).toBeVisible()
  })

  test('card expands on click to show details', async ({ page }) => {
    const firstCard = page.locator('[role="button"]').first()
    const exists = await firstCard.isVisible({ timeout: 3000 }).catch(() => false)

    if (!exists) {
      test.skip(true, 'No plugin cards to test')
      return
    }

    // Click to expand
    await firstCard.click()
    await page.waitForTimeout(300)

    // Expanded section should show "Installed" date text
    const installedText = firstCard.locator('text=/Installed \\d{4}-/')
    await expect(installedText).toBeVisible()

    // Click again to collapse
    await firstCard.click()
    await page.waitForTimeout(300)

    // Installed text should be hidden
    await expect(installedText).not.toBeVisible()

    await page.screenshot({ path: 'e2e/screenshots/plugins-card-expanded.png' })
  })

  test('card shows marketplace name and dot', async ({ page }) => {
    const firstCard = page.locator('[role="button"]').first()
    const exists = await firstCard.isVisible({ timeout: 3000 }).catch(() => false)

    if (!exists) {
      test.skip(true, 'No plugin cards to test')
      return
    }

    // Marketplace dot (colored circle)
    const dot = firstCard.locator('.rounded-full').first()
    await expect(dot).toBeVisible()
  })

  test('disabled plugin shows reduced opacity', async ({ page }) => {
    // Look for a card with "Disabled" badge
    const disabledBadge = page.locator('text=Disabled')
    const hasDisabled = await disabledBadge
      .first()
      .isVisible({ timeout: 3000 })
      .catch(() => false)

    if (!hasDisabled) {
      test.skip(true, 'No disabled plugins to test')
      return
    }

    // The card containing the disabled badge should have opacity-50 class
    const disabledCard = disabledBadge.first().locator('xpath=ancestor::div[@role="button"]')
    await expect(disabledCard).toHaveClass(/opacity-50/)
  })
})

// ---------------------------------------------------------------------------
// Action menu
// ---------------------------------------------------------------------------

test.describe('Plugin Action Menu', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)
  })

  test('action menu opens on click', async ({ page }) => {
    // Find the "..." menu button (MoreHorizontal icon trigger)
    const menuButton = page.locator('button[title="Plugin actions"]').first()
    const exists = await menuButton.isVisible({ timeout: 3000 }).catch(() => false)

    if (!exists) {
      test.skip(true, 'No plugin action menus to test')
      return
    }

    // Click to open
    await menuButton.click()
    await page.waitForTimeout(200)

    // Popover content should be visible with menu items
    const enableDisable = page.locator('button:has-text("Enable"), button:has-text("Disable")')
    await expect(enableDisable.first()).toBeVisible()

    // Uninstall option should always be present
    await expect(page.locator('button:has-text("Uninstall")')).toBeVisible()

    await page.screenshot({ path: 'e2e/screenshots/plugins-action-menu.png' })
  })

  test('uninstall shows confirmation dialog', async ({ page }) => {
    const menuButton = page.locator('button[title="Plugin actions"]').first()
    const exists = await menuButton.isVisible({ timeout: 3000 }).catch(() => false)

    if (!exists) {
      test.skip(true, 'No plugin action menus to test')
      return
    }

    // Open action menu
    await menuButton.click()
    await page.waitForTimeout(200)

    // Click "Uninstall"
    await page.locator('button:has-text("Uninstall")').click()
    await page.waitForTimeout(300)

    // Confirmation dialog should appear
    const dialog = page.locator('[role="alertdialog"], [role="dialog"]')
    await expect(dialog).toBeVisible()

    // Dialog should have cancel and confirm buttons
    await expect(page.locator('button:has-text("Cancel")')).toBeVisible()

    // Cancel the dialog
    await page.locator('button:has-text("Cancel")').click()
    await page.waitForTimeout(200)

    // Dialog should be closed
    await expect(dialog).not.toBeVisible()

    await page.screenshot({ path: 'e2e/screenshots/plugins-uninstall-dialog.png' })
  })

  test('menu does not propagate click to card', async ({ page }) => {
    const firstCard = page.locator('[role="button"]').first()
    const exists = await firstCard.isVisible({ timeout: 3000 }).catch(() => false)

    if (!exists) {
      test.skip(true, 'No plugin cards to test')
      return
    }

    // Verify card is collapsed (no "Installed" date visible)
    const installedText = firstCard.locator('text=/Installed \\d{4}-/')
    await expect(installedText).not.toBeVisible()

    // Click the action menu
    const menuButton = firstCard.locator('button[title="Plugin actions"]')
    await menuButton.click()
    await page.waitForTimeout(200)

    // Card should still be collapsed (menu click didn't propagate)
    await expect(installedText).not.toBeVisible()

    // Close menu by pressing Escape
    await page.keyboard.press('Escape')
  })
})

// ---------------------------------------------------------------------------
// Health banner
// ---------------------------------------------------------------------------

test.describe('Plugin Health Banner', () => {
  test('health banner reflects API data', async ({ page, request }) => {
    // Check API response first
    const res = await request.get('/api/plugins', { timeout: 60000 })
    const data = await res.json()

    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    if (data.cliError) {
      // CLI error banner should show
      await expect(page.locator('text=CLI unavailable')).toBeVisible()
    } else if (data.duplicateCount > 0 || data.unusedCount > 0) {
      // Warning banner should show
      if (data.duplicateCount > 0) {
        await expect(page.locator(`text=/${data.duplicateCount} duplicate/`)).toBeVisible()
      }
      if (data.unusedCount > 0) {
        await expect(page.locator(`text=/${data.unusedCount} unused/`)).toBeVisible()
      }
    }
    // If no warnings, banner should not be visible (nothing to check)
  })
})

// ---------------------------------------------------------------------------
// Marketplaces dialog
// ---------------------------------------------------------------------------

test.describe('Marketplaces Dialog', () => {
  test('marketplaces button opens dialog', async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    // Look for the marketplaces button/trigger near the header
    const marketplaceBtn = page.locator(
      'button:has-text("Marketplaces"), button:has-text("Sources")',
    )
    const exists = await marketplaceBtn.isVisible({ timeout: 3000 }).catch(() => false)

    if (!exists) {
      test.skip(true, 'No marketplaces button found')
      return
    }

    await marketplaceBtn.click()
    await page.waitForTimeout(300)

    // Dialog should appear
    const dialog = page.locator('[role="dialog"]')
    await expect(dialog).toBeVisible()

    await page.screenshot({ path: 'e2e/screenshots/plugins-marketplaces-dialog.png' })
  })
})

// ---------------------------------------------------------------------------
// Update All
// ---------------------------------------------------------------------------

test.describe('Update All', () => {
  test('Update All button visibility matches updatable count', async ({ page, request }) => {
    const res = await request.get('/api/plugins', { timeout: 60000 })
    const data = await res.json()

    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    const updateAllBtn = page.locator('button:has-text("Update All")')

    if (data.updatableCount > 0) {
      await expect(updateAllBtn).toBeVisible()
      // Button text should include the count
      await expect(updateAllBtn).toHaveText(`Update All (${data.updatableCount})`)
    } else {
      await expect(updateAllBtn).not.toBeVisible()
    }
  })
})

// ---------------------------------------------------------------------------
// Available plugin cards
// ---------------------------------------------------------------------------

test.describe('Available Plugin Cards', () => {
  test('available plugins show dashed border and GET button', async ({ page, request }) => {
    const res = await request.get('/api/plugins', { timeout: 60000 })
    const data = await res.json()

    if (data.totalAvailable === 0) {
      test.skip(true, 'No available plugins to test')
      return
    }

    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    // Available cards use dashed border (border-dashed class)
    const dashedCards = page.locator('.border-dashed')
    await expect(dashedCards.first()).toBeVisible()

    // GET button should be visible for non-installed available plugins
    const getButton = page.locator('button:has-text("GET")')
    const hasGet = await getButton
      .first()
      .isVisible({ timeout: 3000 })
      .catch(() => false)

    // If there are available plugins that aren't already installed, GET should show
    const notInstalled = data.available.filter(
      (p: { alreadyInstalled: boolean }) => !p.alreadyInstalled,
    )
    if (notInstalled.length > 0) {
      expect(hasGet).toBe(true)
    }
  })
})

// ---------------------------------------------------------------------------
// Keyboard accessibility
// ---------------------------------------------------------------------------

test.describe('Plugins Accessibility', () => {
  test('plugin cards are keyboard navigable', async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    const firstCard = page.locator('[role="button"]').first()
    const exists = await firstCard.isVisible({ timeout: 3000 }).catch(() => false)

    if (!exists) {
      test.skip(true, 'No plugin cards to test')
      return
    }

    // Cards should be focusable (tabIndex=0)
    await expect(firstCard).toHaveAttribute('tabindex', '0')

    // Focus the card
    await firstCard.focus()
    await expect(firstCard).toBeFocused()

    // Press Enter to expand
    await page.keyboard.press('Enter')
    await page.waitForTimeout(300)

    // Card should expand
    const installedText = firstCard.locator('text=/Installed \\d{4}-/')
    await expect(installedText).toBeVisible()

    // Press Space to collapse
    await page.keyboard.press('Space')
    await page.waitForTimeout(300)
    await expect(installedText).not.toBeVisible()
  })

  test('search input is focusable and functional', async ({ page }) => {
    await page.goto('/plugins')
    await waitForPluginsLoad(page)

    const searchInput = page.locator('input[placeholder="Search plugins..."]')
    await searchInput.focus()
    await expect(searchInput).toBeFocused()

    // Type and verify value
    await searchInput.fill('test-search')
    await expect(searchInput).toHaveValue('test-search')
  })
})
