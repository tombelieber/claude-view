import { describe, expect, it } from 'vitest'
import { formatContextWindow } from './format-model'

describe('formatContextWindow', () => {
  // === Unit tests: formatting correctness ===

  it('formats 1M tokens', () => {
    expect(formatContextWindow(1_000_000)).toBe('1M')
  })

  it('formats 200K tokens', () => {
    expect(formatContextWindow(200_000)).toBe('200K')
  })

  it('formats 128K tokens', () => {
    expect(formatContextWindow(128_000)).toBe('128K')
  })

  it('returns undefined for null', () => {
    expect(formatContextWindow(null)).toBeUndefined()
  })

  it('formats small values as raw numbers', () => {
    expect(formatContextWindow(500)).toBe('500')
  })

  it('formats 2M tokens', () => {
    expect(formatContextWindow(2_000_000)).toBe('2M')
  })

  // === Regression: edge cases ===

  it('formats 1.5M tokens with decimal', () => {
    expect(formatContextWindow(1_500_000)).toBe('1.5M')
  })

  it('formats exactly 1000 as 1K', () => {
    expect(formatContextWindow(1_000)).toBe('1K')
  })

  it('handles zero', () => {
    expect(formatContextWindow(0)).toBe('0')
  })
})
