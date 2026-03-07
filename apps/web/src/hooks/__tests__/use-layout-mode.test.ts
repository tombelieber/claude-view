import { act, renderHook } from '@testing-library/react'
import type { SerializedDockview } from 'dockview-react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useLayoutMode } from '../use-layout-mode'

// Mock localStorage
const mockStorage = new Map<string, string>()
beforeEach(() => {
  mockStorage.clear()
  vi.spyOn(Storage.prototype, 'getItem').mockImplementation((key) => mockStorage.get(key) ?? null)
  vi.spyOn(Storage.prototype, 'setItem').mockImplementation((key, value) => {
    mockStorage.set(key, value)
  })
  vi.spyOn(Storage.prototype, 'removeItem').mockImplementation((key) => {
    mockStorage.delete(key)
  })
})

describe('useLayoutMode', () => {
  it('defaults to auto-grid when no stored value', () => {
    const { result } = renderHook(() => useLayoutMode())
    expect(result.current.mode).toBe('auto-grid')
    expect(result.current.savedLayout).toBeNull()
    expect(result.current.activePreset).toBeNull()
  })

  it('restores mode from localStorage', () => {
    mockStorage.set('claude-view:monitor-layout-mode', 'custom')
    const { result } = renderHook(() => useLayoutMode())
    expect(result.current.mode).toBe('custom')
  })

  it('persists mode change to localStorage', () => {
    const { result } = renderHook(() => useLayoutMode())
    act(() => result.current.setMode('custom'))
    expect(result.current.mode).toBe('custom')
    expect(mockStorage.get('claude-view:monitor-layout-mode')).toBe('custom')
  })

  it('toggleMode switches between modes', () => {
    const { result } = renderHook(() => useLayoutMode())
    expect(result.current.mode).toBe('auto-grid')
    act(() => result.current.toggleMode())
    expect(result.current.mode).toBe('custom')
    act(() => result.current.toggleMode())
    expect(result.current.mode).toBe('auto-grid')
  })

  it('setSavedLayout persists to localStorage and clears activePreset', () => {
    const { result } = renderHook(() => useLayoutMode())
    const mockLayout = { grid: {}, panels: {} } as unknown as SerializedDockview
    act(() => result.current.setActivePreset('2x2'))
    expect(result.current.activePreset).toBe('2x2')
    act(() => result.current.setSavedLayout(mockLayout))
    expect(result.current.savedLayout).toEqual(mockLayout)
    expect(result.current.activePreset).toBeNull()
    expect(mockStorage.get('claude-view:monitor-layout')).toBeTruthy()
  })

  it('setSavedLayout(null) removes from localStorage', () => {
    const { result } = renderHook(() => useLayoutMode())
    const mockLayout = { grid: {}, panels: {} } as unknown as SerializedDockview
    act(() => result.current.setSavedLayout(mockLayout))
    expect(mockStorage.has('claude-view:monitor-layout')).toBe(true)
    act(() => result.current.setSavedLayout(null))
    expect(result.current.savedLayout).toBeNull()
    expect(mockStorage.has('claude-view:monitor-layout')).toBe(false)
  })

  it('restores saved layout from localStorage', () => {
    const mockLayout = { grid: { test: true }, panels: {} }
    mockStorage.set('claude-view:monitor-layout', JSON.stringify(mockLayout))
    const { result } = renderHook(() => useLayoutMode())
    expect(result.current.savedLayout).toEqual(mockLayout)
  })

  it('handles invalid JSON in localStorage gracefully', () => {
    mockStorage.set('claude-view:monitor-layout', 'not-json')
    const { result } = renderHook(() => useLayoutMode())
    expect(result.current.savedLayout).toBeNull()
  })
})
