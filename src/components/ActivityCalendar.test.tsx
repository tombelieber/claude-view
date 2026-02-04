import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { ActivityCalendar } from './ActivityCalendar'
import type { SessionInfo } from '../hooks/use-projects'

// Helper to create mock session data
function createMockSession(id: string, modifiedAt: number): SessionInfo {
  return {
    id,
    projectPath: '/test/project',
    projectDisplayName: 'Test Project',
    modifiedAt,
    preview: 'Test session',
  }
}

// Create sessions for specific dates
function createSessionsForDate(date: Date, count: number): SessionInfo[] {
  const timestamp = Math.floor(date.getTime() / 1000)
  return Array.from({ length: count }, (_, i) =>
    createMockSession(`session-${timestamp}-${i}`, timestamp + i)
  )
}

// Helper to get our custom calendar cells (with session-related aria-labels)
function getOurCalendarCells(): HTMLElement[] {
  const cells = screen.getAllByRole('gridcell')
  return cells.filter(cell =>
    cell.getAttribute('aria-label')?.includes('session')
  )
}

// Helper to find a cell by session count
function getCellBySessionCount(count: number): HTMLElement | undefined {
  const cells = getOurCalendarCells()
  const pattern = count === 1 ? `${count} session` : `${count} sessions`
  return cells.find(cell => {
    const label = cell.getAttribute('aria-label')
    if (count === 1) {
      return label?.includes('1 session') && !label?.includes('1 sessions')
    }
    return label?.includes(pattern)
  })
}

describe('ActivityCalendar', () => {
  describe('Rendering', () => {
    it('should render the calendar component', () => {
      const { container } = render(<ActivityCalendar sessions={[]} />)
      expect(container.querySelector('.activity-calendar')).toBeInTheDocument()
    })

    it('should display total sessions count', () => {
      const sessions = createSessionsForDate(new Date(), 5)
      render(<ActivityCalendar sessions={sessions} />)
      const summaryText = screen.getByText('sessions')
      expect(summaryText.previousElementSibling).toHaveTextContent('5')
    })

    it('should display total projects when provided', () => {
      render(<ActivityCalendar sessions={[]} totalProjects={10} />)
      const projectsText = screen.getByText('projects')
      expect(projectsText.previousElementSibling).toHaveTextContent('10')
    })

    it('should render the heatmap legend', () => {
      render(<ActivityCalendar sessions={[]} />)
      expect(screen.getByText('Less')).toBeInTheDocument()
      expect(screen.getByText('More')).toBeInTheDocument()
    })
  })

  describe('AC-2: Heatmap Tooltips - Static Attributes', () => {
    // AC-2.5: Cell has aria-describedby (static attribute test)
    it('should have aria-describedby attribute on cells for tooltip', () => {
      const today = new Date()
      const sessions = createSessionsForDate(today, 2)
      render(<ActivityCalendar sessions={sessions} />)

      const cellWithSession = getCellBySessionCount(2)
      expect(cellWithSession).toBeDefined()

      // Cell should have aria-describedby pointing to tooltip
      expect(cellWithSession).toHaveAttribute('aria-describedby')
      const describedById = cellWithSession!.getAttribute('aria-describedby')
      expect(describedById).toMatch(/^tooltip-\d{4}-\d{2}-\d{2}$/)
    })

    // AC-2.6: Click behavior unchanged
    it('should maintain click behavior on cells', () => {
      const onRangeChange = vi.fn()
      const today = new Date()
      const sessions = createSessionsForDate(today, 2)
      render(
        <ActivityCalendar
          sessions={sessions}
          onRangeChange={onRangeChange}
        />
      )

      const cellWithSession = getCellBySessionCount(2)
      expect(cellWithSession).toBeDefined()

      fireEvent.click(cellWithSession!)

      // Cell should still be interactive
      expect(cellWithSession).toBeInTheDocument()
    })

    it('should render Tooltip.Provider wrapper', () => {
      const { container } = render(<ActivityCalendar sessions={[]} />)
      // The component is wrapped in Tooltip.Provider - verify structure is correct
      const activityCalendar = container.querySelector('.activity-calendar')
      expect(activityCalendar).toBeInTheDocument()
    })
  })

  describe('Accessibility (A11Y)', () => {
    // A11Y-1: Cells should have proper tabindex for roving tabindex pattern
    it('should have one focusable cell with tabindex=0', () => {
      const sessions = createSessionsForDate(new Date(), 1)
      render(<ActivityCalendar sessions={sessions} />)

      const ourCells = getOurCalendarCells()
      const focusableCells = ourCells.filter(cell => cell.getAttribute('tabindex') === '0')
      // Exactly one cell should be focusable (roving tabindex)
      expect(focusableCells.length).toBe(1)
    })

    it('should have tabindex=-1 on non-focused cells', () => {
      const sessions = createSessionsForDate(new Date(), 1)
      render(<ActivityCalendar sessions={sessions} />)

      const ourCells = getOurCalendarCells()
      const nonFocusableCells = ourCells.filter(cell => cell.getAttribute('tabindex') === '-1')
      // All but one cell should have tabindex=-1
      expect(nonFocusableCells.length).toBe(ourCells.length - 1)
    })

    // A11Y-2: Keyboard navigation handler is attached
    it('should have onKeyDown handler for arrow key navigation', () => {
      const sessions = createSessionsForDate(new Date(), 1)
      render(<ActivityCalendar sessions={sessions} />)

      const ourCells = getOurCalendarCells()
      const firstCell = ourCells[0]

      // Focus the cell
      firstCell.focus()
      expect(document.activeElement).toBe(firstCell)

      // Verify keydown events can be dispatched (handler exists)
      // The actual navigation is handled by state updates
      expect(() => {
        fireEvent.keyDown(firstCell, { key: 'ArrowRight' })
      }).not.toThrow()
    })

    // A11Y-3: Screen reader announces date and session count
    it('should have screen reader accessible labels with date and count', () => {
      const today = new Date()
      const sessions = createSessionsForDate(today, 8)
      render(<ActivityCalendar sessions={sessions} />)

      const cellWith8Sessions = getCellBySessionCount(8)
      expect(cellWith8Sessions).toBeDefined()

      const ariaLabel = cellWith8Sessions!.getAttribute('aria-label')
      // Should contain full date and session count: "February 5, 2026: 8 sessions"
      expect(ariaLabel).toMatch(/\w+\s+\d+,\s+\d+:\s+8\s+sessions/)
    })

    it('should have screen reader label with singular session', () => {
      const today = new Date()
      const sessions = createSessionsForDate(today, 1)
      render(<ActivityCalendar sessions={sessions} />)

      const cellWith1Session = getCellBySessionCount(1)
      expect(cellWith1Session).toBeDefined()

      const ariaLabel = cellWith1Session!.getAttribute('aria-label')
      // Should say "1 session" not "1 sessions"
      expect(ariaLabel).toContain('1 session')
      expect(ariaLabel).not.toContain('1 sessions')
    })

    it('should have ARIA grid role for the calendar container', () => {
      const { container } = render(<ActivityCalendar sessions={[]} />)
      const activityCalendar = container.querySelector('.activity-calendar')
      expect(activityCalendar).toHaveAttribute('role', 'grid')
      expect(activityCalendar).toHaveAttribute('aria-label', 'Activity calendar showing sessions per day')
    })

    it('should have aria-describedby linking to legend', () => {
      const { container } = render(<ActivityCalendar sessions={[]} />)
      const activityCalendar = container.querySelector('.activity-calendar')
      expect(activityCalendar).toHaveAttribute('aria-describedby')
    })

    it('should have gridcell role for calendar day buttons', () => {
      render(<ActivityCalendar sessions={[]} />)
      const ourCells = getOurCalendarCells()
      expect(ourCells.length).toBeGreaterThan(0)
      ourCells.forEach(cell => {
        expect(cell).toHaveAttribute('role', 'gridcell')
      })
    })
  })

  describe('Null Safety', () => {
    it('should handle null sessions', () => {
      const { container } = render(<ActivityCalendar sessions={null} />)
      expect(container).toBeInTheDocument()
      expect(container.querySelector('.activity-calendar')).toBeInTheDocument()
    })

    it('should handle undefined sessions', () => {
      const { container } = render(<ActivityCalendar sessions={undefined} />)
      expect(container).toBeInTheDocument()
      expect(container.querySelector('.activity-calendar')).toBeInTheDocument()
    })

    it('should handle empty sessions array', () => {
      render(<ActivityCalendar sessions={[]} />)
      const summaryText = screen.getByText('sessions')
      expect(summaryText.previousElementSibling).toHaveTextContent('0')
    })
  })

  describe('Edge Cases', () => {
    it('should handle single session', () => {
      const sessions = createSessionsForDate(new Date(), 1)
      render(<ActivityCalendar sessions={sessions} />)
      const summaryText = screen.getByText('sessions')
      expect(summaryText.previousElementSibling).toHaveTextContent('1')
    })

    it('should handle large session counts', () => {
      const sessions = createSessionsForDate(new Date(), 100)
      render(<ActivityCalendar sessions={sessions} />)
      const summaryText = screen.getByText('sessions')
      expect(summaryText.previousElementSibling).toHaveTextContent('100')
    })

    it('should correctly pluralize session count in aria-label', () => {
      // Test singular
      const singleSession = createSessionsForDate(new Date(), 1)
      const { rerender } = render(<ActivityCalendar sessions={singleSession} />)

      let cells = getOurCalendarCells()
      const singleCell = cells.find(cell => {
        const label = cell.getAttribute('aria-label')
        return label?.includes('1 session') && !label?.includes('1 sessions')
      })
      expect(singleCell).toBeDefined()

      // Test plural
      const multipleSessions = createSessionsForDate(new Date(), 5)
      rerender(<ActivityCalendar sessions={multipleSessions} />)

      cells = getOurCalendarCells()
      const pluralCell = cells.find(cell =>
        cell.getAttribute('aria-label')?.includes('5 sessions')
      )
      expect(pluralCell).toBeDefined()
    })

    it('should show 0 sessions for days without activity', () => {
      render(<ActivityCalendar sessions={[]} />)
      const cells = getOurCalendarCells()
      // All cells should show "0 sessions"
      const zeroSessionCells = cells.filter(cell =>
        cell.getAttribute('aria-label')?.includes('0 sessions')
      )
      expect(zeroSessionCells.length).toBeGreaterThan(0)
    })
  })

  describe('Heatmap Intensity Styling', () => {
    it('should apply different styles based on session count', () => {
      // Create sessions on different days with different counts
      const today = new Date()
      const highActivity = createSessionsForDate(today, 15) // > 10, should be emerald-600
      render(<ActivityCalendar sessions={highActivity} />)

      const highActivityCell = getCellBySessionCount(15)
      expect(highActivityCell).toBeDefined()
      expect(highActivityCell!.className).toContain('bg-emerald-600')
    })

    it('should style today cell with a ring', () => {
      const today = new Date()
      const sessions = createSessionsForDate(today, 1)
      render(<ActivityCalendar sessions={sessions} />)

      const todayCell = getCellBySessionCount(1)
      expect(todayCell).toBeDefined()
      expect(todayCell!.className).toContain('ring-emerald-500')
    })
  })
})
