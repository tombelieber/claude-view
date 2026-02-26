import { describe, it, expect } from 'vitest'
import { formatTokens, formatLineCount } from './use-ai-generation'

describe('formatTokens', () => {
  it('returns -- for null/undefined', () => {
    expect(formatTokens(null)).toBe('--')
    expect(formatTokens(undefined)).toBe('--')
  })

  it('formats billions with B suffix', () => {
    expect(formatTokens(1_000_000_000)).toBe('1.00B')
    expect(formatTokens(7_177_700_000)).toBe('7.18B')
    expect(formatTokens(12_500_000_000)).toBe('12.5B')
  })

  it('formats millions with M suffix', () => {
    expect(formatTokens(1_000_000)).toBe('1.0M')
    expect(formatTokens(4_900_000)).toBe('4.9M')
    expect(formatTokens(456_800_000)).toBe('456.8M')
    expect(formatTokens(999_000_000)).toBe('999.0M')
  })

  it('formats thousands with k suffix', () => {
    expect(formatTokens(1_000)).toBe('1.0k')
    expect(formatTokens(5_400)).toBe('5.4k')
    expect(formatTokens(45_000)).toBe('45k')
    expect(formatTokens(450_000)).toBe('450k')
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
