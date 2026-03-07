import { act, renderHook } from '@testing-library/react'
import type { SerializedDockview } from 'dockview-react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useLayoutPresets } from '../use-layout-presets'

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

describe('useLayoutPresets', () => {
  it('starts with empty presets', () => {
    const { result } = renderHook(() => useLayoutPresets())
    expect(result.current.customPresets).toEqual({})
  })

  it('saves a preset to state and localStorage', () => {
    const { result } = renderHook(() => useLayoutPresets())
    const mockLayout = { grid: {}, panels: {} } as unknown as SerializedDockview
    act(() => result.current.savePreset('My Layout', mockLayout))
    expect(result.current.customPresets['My Layout']).toEqual(mockLayout)
    const stored = JSON.parse(mockStorage.get('claude-view:monitor-presets') ?? '{}')
    expect(stored['My Layout']).toEqual(mockLayout)
  })

  it('deletes a preset from state and localStorage', () => {
    const { result } = renderHook(() => useLayoutPresets())
    const mockLayout = { grid: {}, panels: {} } as unknown as SerializedDockview
    act(() => result.current.savePreset('To Delete', mockLayout))
    expect(result.current.customPresets['To Delete']).toBeDefined()
    act(() => result.current.deletePreset('To Delete'))
    expect(result.current.customPresets['To Delete']).toBeUndefined()
  })

  it('restores presets from localStorage', () => {
    const presets = { Saved: { grid: {}, panels: {} } }
    mockStorage.set('claude-view:monitor-presets', JSON.stringify(presets))
    const { result } = renderHook(() => useLayoutPresets())
    expect(result.current.customPresets).toEqual(presets)
  })

  it('handles invalid JSON in localStorage gracefully', () => {
    mockStorage.set('claude-view:monitor-presets', 'invalid')
    const { result } = renderHook(() => useLayoutPresets())
    expect(result.current.customPresets).toEqual({})
  })
})
