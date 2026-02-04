import { describe, it, expect } from 'vitest'
import { formatTokens, formatLineCount } from './use-ai-generation'

describe('formatTokens', () => {
  it('returns -- for null/undefined', () => {
    expect(formatTokens(null)).toBe('--')
    expect(formatTokens(undefined)).toBe('--')
  })

  it('formats millions with M suffix', () => {
    expect(formatTokens(1_000_000)).toBe('1.0M')
    expect(formatTokens(1_500_000)).toBe('1.5M')
    expect(formatTokens(2_900_000)).toBe('2.9M')
  })

  it('formats thousands with K suffix', () => {
    expect(formatTokens(1_000)).toBe('1K')
    expect(formatTokens(450_000)).toBe('450K')
    expect(formatTokens(999_999)).toBe('1000K')
  })

  it('formats small numbers without suffix', () => {
    expect(formatTokens(0)).toBe('0')
    expect(formatTokens(100)).toBe('100')
    expect(formatTokens(999)).toBe('999')
  })
})

describe('formatLineCount', () => {
  it('adds + prefix for positive numbers by default', () => {
    expect(formatLineCount(100)).toBe('+100')
    expect(formatLineCount(12847)).toBe('+12,847')
  })

  it('does not add + prefix when showPlus is false', () => {
    expect(formatLineCount(100, false)).toBe('100')
    expect(formatLineCount(-3201, false)).toBe('-3,201')
  })

  it('formats negative numbers correctly', () => {
    expect(formatLineCount(-100)).toBe('-100')
    expect(formatLineCount(-3201)).toBe('-3,201')
  })

  it('handles zero', () => {
    expect(formatLineCount(0)).toBe('0')
    expect(formatLineCount(0, true)).toBe('0')
  })
})
