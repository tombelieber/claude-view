import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { useTweenedValue } from '../use-tweened-value'

describe('useTweenedValue', () => {
  it('returns initial value immediately', () => {
    const { result } = renderHook(() => useTweenedValue(42))
    expect(result.current).toBe(42)
  })

  it('jumps directly to target when prefers-reduced-motion is enabled', () => {
    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: query === '(prefers-reduced-motion: reduce)',
        media: query,
        onchange: null,
        addListener: vi.fn(),
        removeListener: vi.fn(),
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    })
    const { result, rerender } = renderHook(
      ({ target }: { target: number }) => useTweenedValue(target),
      {
        initialProps: { target: 0 },
      },
    )
    act(() => {
      rerender({ target: 100 })
    })
    expect(result.current).toBe(100)
  })
})
