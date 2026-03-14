import { describe, expect, it } from 'vitest'
import { formatDayLabel } from './DayDivider'

describe('formatDayLabel', () => {
  it('returns "Today" for today', () => {
    expect(formatDayLabel(new Date())).toBe('Today')
  })

  it('returns "Yesterday" for yesterday', () => {
    const yesterday = new Date()
    yesterday.setDate(yesterday.getDate() - 1)
    expect(formatDayLabel(yesterday)).toBe('Yesterday')
  })

  it('returns weekday name for 2-6 days ago', () => {
    const threeDaysAgo = new Date()
    threeDaysAgo.setDate(threeDaysAgo.getDate() - 3)
    const label = formatDayLabel(threeDaysAgo)
    const weekdays = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday']
    expect(weekdays).toContain(label)
  })

  it('returns short date for 7+ days ago this year', () => {
    const now = new Date()
    // Use a date guaranteed to be >7 days ago and same year
    const target = new Date(now.getFullYear(), 0, 1) // Jan 1 of current year
    // Skip if Jan 1 is within 7 days (early January edge case)
    const diffDays = Math.round((now.getTime() - target.getTime()) / 86_400_000)
    if (diffDays < 7) return

    const label = formatDayLabel(target)
    // Should NOT be "Today", "Yesterday", or a bare weekday
    expect(label).not.toBe('Today')
    expect(label).not.toBe('Yesterday')
    // Should contain month abbreviation
    expect(label).toMatch(/Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec/)
    // Should NOT contain year (same year)
    expect(label).not.toContain(String(now.getFullYear()))
  })

  it('includes year for dates in a different year', () => {
    const lastYear = new Date()
    lastYear.setFullYear(lastYear.getFullYear() - 1)
    const label = formatDayLabel(lastYear)
    expect(label).toContain(String(lastYear.getFullYear()))
  })
})
