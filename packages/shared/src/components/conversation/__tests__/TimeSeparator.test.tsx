import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { TimeSeparator, formatRelativeDate, shouldShowSeparator } from '../TimeSeparator'

describe('TimeSeparator', () => {
  describe('shouldShowSeparator', () => {
    it('returns false for gap < 30 minutes (in seconds)', () => {
      const now = 1700000000
      const twentyMinutesLater = now + 20 * 60 // 20 minutes in seconds
      expect(shouldShowSeparator(now, twentyMinutesLater)).toBe(false)
    })

    it('returns true for gap > 30 minutes (in seconds)', () => {
      const now = 1700000000
      const twoHoursLater = now + 2 * 60 * 60 // 2 hours in seconds
      expect(shouldShowSeparator(now, twoHoursLater)).toBe(true)
    })

    it('returns false when timestamps are missing', () => {
      expect(shouldShowSeparator(undefined, 1700000000)).toBe(false)
      expect(shouldShowSeparator(1700000000, undefined)).toBe(false)
      expect(shouldShowSeparator(undefined, undefined)).toBe(false)
    })

    it('returns true for exactly 30 minutes + 1 second gap', () => {
      const now = 1700000000
      const thirtyMinutesAndOne = now + 30 * 60 + 1
      expect(shouldShowSeparator(now, thirtyMinutesAndOne)).toBe(true)
    })

    it('returns false for exactly 30 minutes gap (boundary)', () => {
      const now = 1700000000
      const exactlyThirtyMinutes = now + 30 * 60
      expect(shouldShowSeparator(now, exactlyThirtyMinutes)).toBe(false)
    })
  })

  describe('formatRelativeDate', () => {
    it('converts unix seconds to readable relative date', () => {
      // 2023-11-14T16:13:20Z = 1700000000
      const formatted = formatRelativeDate(1700000000)
      // Should contain month and time components (day varies by timezone)
      expect(formatted).toMatch(/Nov/)
      expect(formatted).toMatch(/\d{1,2}/)
      // Verify it produces a parseable date string
      expect(formatted.length).toBeGreaterThan(5)
    })
  })

  describe('TimeSeparator component', () => {
    it('renders separator line with formatted date', () => {
      render(<TimeSeparator timestamp={1700000000} />)
      // Should render the formatted date text
      const text = screen.getByText(/Nov/)
      expect(text).toBeDefined()
    })
  })
})
