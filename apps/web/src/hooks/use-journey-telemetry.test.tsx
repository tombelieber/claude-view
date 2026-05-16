import { renderHook } from '@testing-library/react'
import type { ReactNode } from 'react'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useJourneyTelemetry } from './use-journey-telemetry'

const opened = vi.hoisted(() => vi.fn())
vi.mock('@/lib/telemetry-track', () => ({
  trackFeatureOpened: opened,
  trackFeatureAction: vi.fn(),
}))

function wrapperFor(path: string) {
  return ({ children }: { children: ReactNode }) => (
    <MemoryRouter initialEntries={[path]}>{children}</MemoryRouter>
  )
}

describe('useJourneyTelemetry', () => {
  afterEach(() => opened.mockReset())

  it('fires feature_opened with the mapped surface on mount', () => {
    renderHook(() => useJourneyTelemetry(), { wrapper: wrapperFor('/analytics') })
    expect(opened).toHaveBeenCalledExactlyOnceWith('analytics')
  })

  it('dedupes the same surface within a session (re-render = no re-fire)', () => {
    const { rerender } = renderHook(() => useJourneyTelemetry(), {
      wrapper: wrapperFor('/search'),
    })
    rerender()
    rerender()
    expect(opened).toHaveBeenCalledExactlyOnceWith('search')
  })

  it('does not fire for an unknown route (never leaks a raw path)', () => {
    renderHook(() => useJourneyTelemetry(), { wrapper: wrapperFor('/totally-unknown') })
    expect(opened).not.toHaveBeenCalled()
  })
})
