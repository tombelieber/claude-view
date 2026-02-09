import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { useTimeRange } from './use-time-range'

// Mock localStorage
const localStorageMock = (() => {
  let store: Record<string, string> = {}
  return {
    getItem: vi.fn((key: string) => store[key] || null),
    setItem: vi.fn((key: string, value: string) => {
      store[key] = value
    }),
    removeItem: vi.fn((key: string) => {
      delete store[key]
    }),
    clear: vi.fn(() => {
      store = {}
    }),
  }
})()

Object.defineProperty(window, 'localStorage', { value: localStorageMock })

function wrapper({ children }: { children: React.ReactNode }) {
  return <MemoryRouter>{children}</MemoryRouter>
}

describe('useTimeRange', () => {
  beforeEach(() => {
    localStorageMock.clear()
    vi.clearAllMocks()
  })

  afterEach(() => {
    localStorageMock.clear()
  })

  describe('initialization', () => {
    it('defaults to 30d when no URL or localStorage', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      expect(result.current.state.preset).toBe('30d')
      expect(result.current.state.customRange).toBeNull()
    })

    it('returns timestamps for non-all presets', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      expect(result.current.state.fromTimestamp).not.toBeNull()
      expect(result.current.state.toTimestamp).not.toBeNull()
    })
  })

  describe('setPreset', () => {
    it('updates preset to 7d', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      act(() => {
        result.current.setPreset('7d')
      })

      expect(result.current.state.preset).toBe('7d')
    })

    it('updates preset to all with null timestamps', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      act(() => {
        result.current.setPreset('all')
      })

      expect(result.current.state.preset).toBe('all')
      expect(result.current.state.fromTimestamp).toBeNull()
      expect(result.current.state.toTimestamp).toBeNull()
    })

    it('clears customRange when switching to preset', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      // Set custom range
      act(() => {
        result.current.setCustomRange({
          from: new Date('2026-01-01'),
          to: new Date('2026-01-15'),
        })
      })

      expect(result.current.state.preset).toBe('custom')

      // Switch to preset
      act(() => {
        result.current.setPreset('7d')
      })

      expect(result.current.state.preset).toBe('7d')
      expect(result.current.state.customRange).toBeNull()
    })
  })

  describe('setCustomRange', () => {
    it('sets custom range and switches preset to custom', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      const from = new Date('2026-01-01')
      const to = new Date('2026-01-15')

      act(() => {
        result.current.setCustomRange({ from, to })
      })

      expect(result.current.state.preset).toBe('custom')
      expect(result.current.state.customRange).toEqual({ from, to })
    })

    it('calculates timestamps from custom range', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      const from = new Date('2026-01-01')
      const to = new Date('2026-01-15')

      act(() => {
        result.current.setCustomRange({ from, to })
      })

      expect(result.current.state.fromTimestamp).not.toBeNull()
      expect(result.current.state.toTimestamp).not.toBeNull()
      // fromTimestamp should be start of day
      // toTimestamp should be end of day
      expect(result.current.state.fromTimestamp! < result.current.state.toTimestamp!).toBe(true)
    })
  })

  describe('labels', () => {
    it('returns "All time" for all preset', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      act(() => {
        result.current.setPreset('all')
      })

      expect(result.current.label).toBe('All time')
      expect(result.current.comparisonLabel).toBeNull()
    })

    it('returns comparison label for presets', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      act(() => {
        result.current.setPreset('7d')
      })

      expect(result.current.comparisonLabel).toBe('vs prev 7d')
    })

    it('returns comparison label for custom range', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      const from = new Date('2026-01-01')
      const to = new Date('2026-01-15') // 15 days (inclusive makes it ~14)

      act(() => {
        result.current.setCustomRange({ from, to })
      })

      // Should show days in comparison label
      expect(result.current.comparisonLabel).toMatch(/vs prev \d+d/)
    })
  })

  describe('today preset', () => {
    it('updates preset to today with correct timestamps', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      act(() => {
        result.current.setPreset('today')
      })

      expect(result.current.state.preset).toBe('today')
      expect(result.current.state.fromTimestamp).not.toBeNull()
      expect(result.current.state.toTimestamp).not.toBeNull()
      const midnight = new Date()
      midnight.setHours(0, 0, 0, 0)
      expect(result.current.state.fromTimestamp).toBe(Math.floor(midnight.getTime() / 1000))
    })

    it('returns "Today" label for today preset', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      act(() => {
        result.current.setPreset('today')
      })

      expect(result.current.label).toBe('Today')
      expect(result.current.comparisonLabel).toBe('vs yesterday')
    })
  })

  describe('legacy URL migration', () => {
    it('migrates legacy ?range=week to 7d', () => {
      function legacyWrapper({ children }: { children: React.ReactNode }) {
        return <MemoryRouter initialEntries={['/?range=week']}>{children}</MemoryRouter>
      }
      const { result } = renderHook(() => useTimeRange(), { wrapper: legacyWrapper })
      expect(result.current.state.preset).toBe('7d')
    })

    it('migrates legacy ?range=month to 30d', () => {
      function legacyWrapper({ children }: { children: React.ReactNode }) {
        return <MemoryRouter initialEntries={['/?range=month']}>{children}</MemoryRouter>
      }
      const { result } = renderHook(() => useTimeRange(), { wrapper: legacyWrapper })
      expect(result.current.state.preset).toBe('30d')
    })

    it('migrates legacy ?range=90days to 90d', () => {
      function legacyWrapper({ children }: { children: React.ReactNode }) {
        return <MemoryRouter initialEntries={['/?range=90days']}>{children}</MemoryRouter>
      }
      const { result } = renderHook(() => useTimeRange(), { wrapper: legacyWrapper })
      expect(result.current.state.preset).toBe('90d')
    })
  })

  describe('localStorage persistence', () => {
    it('persists preset to localStorage', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      act(() => {
        result.current.setPreset('90d')
      })

      expect(localStorageMock.setItem).toHaveBeenCalled()
      const stored = JSON.parse(localStorageMock.setItem.mock.calls.at(-1)?.[1] || '{}')
      expect(stored.preset).toBe('90d')
    })

    it('persists custom range to localStorage', () => {
      const { result } = renderHook(() => useTimeRange(), { wrapper })

      const from = new Date('2026-01-01')
      const to = new Date('2026-01-15')

      act(() => {
        result.current.setCustomRange({ from, to })
      })

      expect(localStorageMock.setItem).toHaveBeenCalled()
      const stored = JSON.parse(localStorageMock.setItem.mock.calls.at(-1)?.[1] || '{}')
      expect(stored.preset).toBe('custom')
      expect(stored.customRange).toBeDefined()
    })
  })
})
