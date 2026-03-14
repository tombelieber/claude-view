import { describe, expect, it } from 'vitest'
import { formatBytes, formatUptime } from './format-utils'

describe('formatBytes (binary / 1024-based)', () => {
  it('formats sub-megabyte as KB', () => {
    expect(formatBytes(0)).toBe('0 KB')
    expect(formatBytes(1024)).toBe('1 KB')
    expect(formatBytes(512_000)).toBe('500 KB')
  })

  it('formats megabytes without decimals', () => {
    expect(formatBytes(1024 ** 2)).toBe('1 MB')
    expect(formatBytes(50 * 1024 ** 2)).toBe('50 MB')
  })

  it('formats gigabytes with one decimal', () => {
    expect(formatBytes(1024 ** 3)).toBe('1.0 GB')
    expect(formatBytes(1.5 * 1024 ** 3)).toBe('1.5 GB')
  })

  it('shows 64 GB for a 64 GiB machine', () => {
    // 64 GiB = 68,719,476,736 bytes — must show "64.0 GB", not "68.7 GB"
    expect(formatBytes(64 * 1024 ** 3)).toBe('64.0 GB')
  })

  it('handles exact boundaries', () => {
    expect(formatBytes(1024 ** 2)).toBe('1 MB')
    expect(formatBytes(1024 ** 3)).toBe('1.0 GB')
  })
})

describe('formatUptime', () => {
  it('formats seconds', () => {
    expect(formatUptime(0)).toBe('0s')
    expect(formatUptime(59)).toBe('59s')
  })

  it('formats minutes', () => {
    expect(formatUptime(60)).toBe('1m')
    expect(formatUptime(3599)).toBe('59m')
  })

  it('formats hours', () => {
    expect(formatUptime(3600)).toBe('1h')
    expect(formatUptime(86399)).toBe('23h')
  })

  it('formats days', () => {
    expect(formatUptime(86400)).toBe('1d')
    expect(formatUptime(259200)).toBe('3d')
  })
})
