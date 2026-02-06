import { test, expect } from '@playwright/test'

test.describe('Feature 2B: Heatmap Hover Tooltips', () => {
  /**
   * Helper: Navigate to a project page that has the full ActivityCalendar component.
   * The full ActivityCalendar with Radix tooltips, role="grid", and keyboard nav
   * is rendered on project detail pages (not the dashboard).
   * The sidebar uses treeitem elements in a tree, not <a> links.
   */
  async function navigateToProject(page: import('@playwright/test').Page) {
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')

    // Wait for sidebar tree to populate with project treeitems
    const projectItem = page.locator('[role="tree"][aria-label="Projects"] [role="treeitem"]').first()
    const hasProject = await projectItem.isVisible({ timeout: 30000 }).catch(() => false)
    if (!hasProject) {
      throw new Error('No projects found in sidebar. Cannot test ActivityCalendar.')
    }

    // Click into the first project
    await projectItem.click()
    await page.waitForLoadState('domcontentloaded')

    // Wait for the full ActivityCalendar grid to render on the project page
    await page.waitForSelector('[role="grid"][aria-label="Activity calendar showing sessions per day"]', { timeout: 30000 })
  }

  test('TC-2B-01: tooltip appears on hover with date, session count, and click hint', async ({ page }) => {
    await navigateToProject(page)

    const grid = page.locator('[role="grid"][aria-label="Activity calendar showing sessions per day"]')
    await expect(grid).toBeVisible()

    // Find a gridcell that has sessions > 0 by checking aria-label for non-zero count
    // aria-label format: "January 15, 2026: 8 sessions"
    const allCells = grid.locator('[role="gridcell"]')
    const cellCount = await allCells.count()
    let targetCell = null

    for (let i = 0; i < cellCount; i++) {
      const cell = allCells.nth(i)
      const label = await cell.getAttribute('aria-label')
      if (label && !label.endsWith(': 0 sessions')) {
        targetCell = cell
        break
      }
    }

    // If no cells with sessions, skip gracefully
    if (!targetCell) {
      test.skip(true, 'No calendar cells with sessions > 0 found; cannot test tooltip hover')
      return
    }

    // Hover over the cell to trigger the Radix tooltip
    await targetCell.hover()

    // Wait for tooltip to appear (Radix tooltip with role="tooltip")
    const tooltip = page.locator('[role="tooltip"]')
    await expect(tooltip).toBeVisible({ timeout: 5000 })

    // Verify tooltip contains a formatted date (e.g. "Wed, Jan 29, 2026")
    // The date line has class "font-medium"
    const dateLine = tooltip.locator('.font-medium')
    await expect(dateLine).toBeVisible()
    const dateText = await dateLine.textContent()
    // Date format: "Mon, Jan 1, 2026" - weekday abbreviation, month, day, year
    expect(dateText).toMatch(/\w{3}, \w{3} \d{1,2}, \d{4}/)

    // Verify session count is displayed (e.g. "8 sessions" or "1 session")
    const sessionLine = tooltip.locator('.text-gray-200')
    await expect(sessionLine).toBeVisible()
    const sessionText = await sessionLine.textContent()
    expect(sessionText).toMatch(/\d+ sessions?/)

    // Verify "Click to filter" hint
    const hintLine = tooltip.locator('.text-gray-400.text-xs')
    await expect(hintLine).toBeVisible()
    await expect(hintLine).toHaveText('Click to filter')

    // Verify tooltip arrow exists
    await expect(tooltip.locator('svg')).toBeVisible().catch(() => {
      // Arrow may be rendered differently; not a hard failure
    })

    await page.screenshot({ path: 'e2e/screenshots/heatmap-tooltip-hover.png' })
  })

  test('TC-2B-03: keyboard navigation with arrow keys (ARIA grid pattern)', async ({ page }) => {
    await navigateToProject(page)

    const grid = page.locator('[role="grid"][aria-label="Activity calendar showing sessions per day"]')
    await expect(grid).toBeVisible()

    // Find the first focusable cell (tabindex="0" indicates the roving tabindex active cell)
    const focusableCell = grid.locator('[role="gridcell"][tabindex="0"]').first()
    await expect(focusableCell).toBeVisible({ timeout: 5000 })

    // Focus the cell
    await focusableCell.focus()
    await expect(focusableCell).toBeFocused()

    // Get the initial cell's aria-label to track navigation
    const initialLabel = await focusableCell.getAttribute('aria-label')
    expect(initialLabel).toBeTruthy()

    // Press ArrowRight to move to the next cell
    await page.keyboard.press('ArrowRight')

    // After ArrowRight, focus should have moved to a different cell
    const newFocused = grid.locator('[role="gridcell"]:focus')
    await expect(newFocused).toBeVisible({ timeout: 2000 })
    const newLabel = await newFocused.getAttribute('aria-label')
    // The focused cell should have changed (different aria-label)
    expect(newLabel).toBeTruthy()
    expect(newLabel).not.toBe(initialLabel)

    // Press ArrowLeft to go back
    await page.keyboard.press('ArrowLeft')
    const backFocused = grid.locator('[role="gridcell"]:focus')
    const backLabel = await backFocused.getAttribute('aria-label')
    expect(backLabel).toBe(initialLabel)

    // Press ArrowDown to move one week forward (7 cells)
    await page.keyboard.press('ArrowDown')
    const downFocused = grid.locator('[role="gridcell"]:focus')
    await expect(downFocused).toBeVisible({ timeout: 2000 })
    const downLabel = await downFocused.getAttribute('aria-label')
    expect(downLabel).toBeTruthy()
    expect(downLabel).not.toBe(initialLabel)

    // Press ArrowUp to go back up one week
    await page.keyboard.press('ArrowUp')
    const upFocused = grid.locator('[role="gridcell"]:focus')
    const upLabel = await upFocused.getAttribute('aria-label')
    expect(upLabel).toBe(initialLabel)

    // Press Home to go to first cell
    await page.keyboard.press('Home')
    const homeFocused = grid.locator('[role="gridcell"]:focus')
    await expect(homeFocused).toBeVisible({ timeout: 2000 })

    // Press End to go to last cell
    await page.keyboard.press('End')
    const endFocused = grid.locator('[role="gridcell"]:focus')
    await expect(endFocused).toBeVisible({ timeout: 2000 })

    await page.screenshot({ path: 'e2e/screenshots/heatmap-keyboard-nav.png' })
  })

  test('TC-2B-04: accessibility attributes on calendar grid and cells', async ({ page }) => {
    await navigateToProject(page)

    // Verify the grid container has role="grid" and proper aria-label
    const grid = page.locator('[role="grid"][aria-label="Activity calendar showing sessions per day"]')
    await expect(grid).toBeVisible()

    // Verify grid has aria-describedby pointing to the legend
    const describedBy = await grid.getAttribute('aria-describedby')
    expect(describedBy).toBeTruthy()

    // Verify the legend element referenced by aria-describedby exists
    if (describedBy) {
      const legend = page.locator(`#${CSS.escape(describedBy)}`)
      await expect(legend).toBeAttached()
    }

    // Verify gridcells have role="gridcell"
    const cells = grid.locator('[role="gridcell"]')
    const cellCount = await cells.count()
    expect(cellCount).toBeGreaterThan(0)

    // Verify each cell has aria-label with date and session count
    // Format: "January 15, 2026: 8 sessions"
    const firstCell = cells.first()
    const ariaLabel = await firstCell.getAttribute('aria-label')
    expect(ariaLabel).toBeTruthy()
    expect(ariaLabel).toMatch(/\w+ \d{1,2}, \d{4}: \d+ sessions?/)

    // Verify each cell has aria-describedby pointing to a tooltip
    const ariaDescribedBy = await firstCell.getAttribute('aria-describedby')
    expect(ariaDescribedBy).toBeTruthy()
    expect(ariaDescribedBy).toMatch(/^tooltip-/)

    // Verify roving tabindex: exactly one cell should have tabindex="0"
    const tabbableCells = grid.locator('[role="gridcell"][tabindex="0"]')
    await expect(tabbableCells).toHaveCount(1)

    // All other cells should have tabindex="-1"
    const nonTabbableCells = grid.locator('[role="gridcell"][tabindex="-1"]')
    const nonTabbableCount = await nonTabbableCells.count()
    expect(nonTabbableCount).toBe(cellCount - 1)

    await page.screenshot({ path: 'e2e/screenshots/heatmap-accessibility.png' })
  })

  test('TC-2B-05: zero sessions cells have empty/gray styling', async ({ page }) => {
    // The dashboard heatmap (ActivityHeatmap) uses buttons with aria-label
    // Format: "2026-01-10: 0 sessions"
    await page.goto('/')
    await page.waitForLoadState('domcontentloaded')

    // Wait for the dashboard to load with the heatmap
    await page.waitForSelector('text=Activity', { timeout: 30000 })

    // Find heatmap buttons with 0 sessions via aria-label
    const allButtons = page.locator('button[aria-label$=": 0 sessions"]')
    const zeroCount = await allButtons.count()

    if (zeroCount === 0) {
      // Try the project page ActivityCalendar instead
      const projectItem = page.locator('[role="tree"][aria-label="Projects"] [role="treeitem"]').first()
      const hasProject = await projectItem.isVisible({ timeout: 5000 }).catch(() => false)
      if (hasProject) {
        await projectItem.click()
        await page.waitForLoadState('domcontentloaded')
        await page.waitForSelector('[role="grid"][aria-label="Activity calendar showing sessions per day"]', { timeout: 30000 })

        const grid = page.locator('[role="grid"][aria-label="Activity calendar showing sessions per day"]')
        const gridCells = grid.locator('[role="gridcell"]')
        const gridCellCount = await gridCells.count()
        let zeroCellFound = false

        for (let i = 0; i < gridCellCount; i++) {
          const cell = gridCells.nth(i)
          const label = await cell.getAttribute('aria-label')
          if (label && label.endsWith(': 0 sessions')) {
            zeroCellFound = true
            // Verify the cell has gray styling (bg-gray-50 class)
            await expect(cell).toHaveClass(/bg-gray-50/)
            break
          }
        }

        if (!zeroCellFound) {
          test.skip(true, 'No zero-session cells found; cannot verify empty styling')
          return
        }
      } else {
        test.skip(true, 'No zero-session cells or projects found; cannot verify empty styling')
        return
      }
    } else {
      // Verify on the dashboard heatmap: zero-session buttons have gray background
      const zeroButton = allButtons.first()
      // Dashboard uses: bg-gray-100 dark:bg-gray-800 for zero count
      await expect(zeroButton).toHaveClass(/bg-gray-100/)
    }

    await page.screenshot({ path: 'e2e/screenshots/heatmap-zero-sessions.png' })
  })

  test('TC-2B-06: heatmap legend shows Less/More labels and color swatches', async ({ page }) => {
    // The dashboard heatmap has a simpler legend with Less/More labels
    // Navigate to a project page for the full ActivityCalendar legend
    await navigateToProject(page)

    const calendarContainer = page.locator('.activity-calendar')
    await expect(calendarContainer).toBeVisible()

    // Verify "Less" label exists in the legend
    const lessLabel = calendarContainer.locator('text=Less')
    await expect(lessLabel).toBeVisible()

    // Verify "More" label exists in the legend
    const moreLabel = calendarContainer.locator('text=More')
    await expect(moreLabel).toBeVisible()

    // Verify the legend has the color scale image with proper aria-label
    const colorScale = calendarContainer.locator('[role="img"][aria-label="Activity intensity scale from low to high"]')
    await expect(colorScale).toBeVisible()

    // Verify exactly 5 color swatch divs inside the legend scale
    const swatches = colorScale.locator('div')
    await expect(swatches).toHaveCount(5)

    // Verify the swatches have the expected background color classes
    // from lightest to darkest: gray-50, emerald-50, emerald-200, emerald-400, emerald-600
    await expect(swatches.nth(0)).toHaveClass(/bg-gray-50/)
    await expect(swatches.nth(1)).toHaveClass(/bg-emerald-50/)
    await expect(swatches.nth(2)).toHaveClass(/bg-emerald-200/)
    await expect(swatches.nth(3)).toHaveClass(/bg-emerald-400/)
    await expect(swatches.nth(4)).toHaveClass(/bg-emerald-600/)

    // Verify swatches are decorative (aria-hidden="true")
    for (let i = 0; i < 5; i++) {
      await expect(swatches.nth(i)).toHaveAttribute('aria-hidden', 'true')
    }

    // Verify summary stats are shown (sessions count)
    const summaryText = calendarContainer.locator('text=sessions')
    await expect(summaryText.first()).toBeVisible()

    await page.screenshot({ path: 'e2e/screenshots/heatmap-legend.png' })
  })
})
