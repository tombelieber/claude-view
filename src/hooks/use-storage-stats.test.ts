import { describe, it, expect } from 'vitest'
import { formatBytes, formatTimestamp, formatDurationMs } from './use-storage-stats'

describe('use-storage-stats', () => {
  describe('formatBytes', () => {
    it('should format null as --', () => {
      expect(formatBytes(null)).toBe('--')
    })

    it('should format bytes < 1KB', () => {
      expect(formatBytes(512)).toBe('512 B')
      expect(formatBytes(0)).toBe('0 B')
    })

    it('should format kilobytes', () => {
      expect(formatBytes(1024)).toBe('1.0 KB')
      expect(formatBytes(2048)).toBe('2.0 KB')
      expect(formatBytes(10240)).toBe('10.0 KB')
    })

    it('should format megabytes', () => {
      expect(formatBytes(1048576)).toBe('1.0 MB')
      expect(formatBytes(245 * 1024 * 1024)).toBe('245.0 MB')
    })

    it('should format gigabytes', () => {
      expect(formatBytes(1073741824)).toBe('1.0 GB')
      expect(formatBytes(11.8 * 1024 * 1024 * 1024)).toBe('11.8 GB')
    })

    it('should handle bigint values', () => {
      expect(formatBytes(BigInt(1073741824))).toBe('1.0 GB')
    })
  })

  describe('formatTimestamp', () => {
    it('should format null as Never', () => {
      expect(formatTimestamp(null)).toBe('Never')
    })

    it('should format seconds ago', () => {
      const nowSecs = Math.floor(Date.now() / 1000)
      const result = formatTimestamp(BigInt(nowSecs - 30))
      expect(result).toBe('30s ago')
    })

    it('should format minutes ago', () => {
      const nowSecs = Math.floor(Date.now() / 1000)
      const result = formatTimestamp(BigInt(nowSecs - 180)) // 3 minutes
      expect(result).toBe('3m ago')
    })

    it('should format hours ago', () => {
      const nowSecs = Math.floor(Date.now() / 1000)
      const result = formatTimestamp(BigInt(nowSecs - 7200)) // 2 hours
      expect(result).toBe('2h ago')
    })

    it('should format days ago', () => {
      const nowSecs = Math.floor(Date.now() / 1000)
      const result = formatTimestamp(BigInt(nowSecs - 172800)) // 2 days
      expect(result).toBe('2d ago')
    })

    it('should format older dates as actual date', () => {
      const nowSecs = Math.floor(Date.now() / 1000)
      const result = formatTimestamp(BigInt(nowSecs - 864000)) // 10 days
      // Should return something like "Jan 26" or similar
      expect(result).toMatch(/^[A-Z][a-z]{2} \d{1,2}/)
    })
  })

  describe('formatDurationMs', () => {
    it('should format null as --', () => {
      expect(formatDurationMs(null)).toBe('--')
    })

    it('should format milliseconds', () => {
      expect(formatDurationMs(BigInt(150))).toBe('150ms')
      expect(formatDurationMs(BigInt(999))).toBe('999ms')
    })

    it('should format seconds', () => {
      expect(formatDurationMs(BigInt(1000))).toBe('1.0s')
      expect(formatDurationMs(BigInt(3200))).toBe('3.2s')
      expect(formatDurationMs(BigInt(1500))).toBe('1.5s')
    })
  })
})
