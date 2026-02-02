import { describe, it, expect } from 'vitest'
import { renderHook, act } from '@testing-library/react'
import { ThreadHighlightProvider, useThreadHighlight } from './ThreadHighlightContext'
import type { ReactNode } from 'react'

const wrapper = ({ children }: { children: ReactNode }) => (
  <ThreadHighlightProvider>{children}</ThreadHighlightProvider>
)

describe('ThreadHighlightContext', () => {
  it('starts with empty highlighted set', () => {
    const { result } = renderHook(() => useThreadHighlight(), { wrapper })
    expect(result.current.highlightedUuids.size).toBe(0)
  })

  it('highlights a set of uuids', () => {
    const { result } = renderHook(() => useThreadHighlight(), { wrapper })
    act(() => result.current.setHighlightedUuids(new Set(['a', 'b', 'c'])))
    expect(result.current.highlightedUuids).toEqual(new Set(['a', 'b', 'c']))
  })

  it('clears highlight', () => {
    const { result } = renderHook(() => useThreadHighlight(), { wrapper })
    act(() => result.current.setHighlightedUuids(new Set(['a'])))
    act(() => result.current.clearHighlight())
    expect(result.current.highlightedUuids.size).toBe(0)
  })

  it('replaces previous highlight set entirely', () => {
    const { result } = renderHook(() => useThreadHighlight(), { wrapper })
    act(() => result.current.setHighlightedUuids(new Set(['a'])))
    act(() => result.current.setHighlightedUuids(new Set(['b', 'c'])))
    expect(result.current.highlightedUuids).toEqual(new Set(['b', 'c']))
  })

  it('returns safe defaults when used outside provider', () => {
    const { result } = renderHook(() => useThreadHighlight())
    expect(result.current.highlightedUuids.size).toBe(0)
    // Should not throw — returns no-op functions
    act(() => result.current.setHighlightedUuids(new Set(['test'])))
    act(() => result.current.clearHighlight())
  })
})
