import { act, renderHook, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { useWsQuery } from './use-ws-query'

describe('useWsQuery', () => {
  it('starts with loading=false when queryFn is null', () => {
    const { result } = renderHook(() => useWsQuery(null))
    expect(result.current.loading).toBe(false)
    expect(result.current.data).toBeNull()
    expect(result.current.error).toBeNull()
  })

  it('starts with loading=true when queryFn provided and autoFetch enabled', () => {
    const queryFn = vi.fn().mockReturnValue(new Promise(() => {}))
    const { result } = renderHook(() => useWsQuery(queryFn))
    expect(result.current.loading).toBe(true)
  })

  it('auto-fetches on mount by default', async () => {
    const queryFn = vi.fn().mockResolvedValue(['model-a'])
    const { result } = renderHook(() => useWsQuery(queryFn))
    await waitFor(() => expect(result.current.data).toEqual(['model-a']))
    expect(queryFn).toHaveBeenCalledTimes(1)
  })

  it('does NOT auto-fetch when autoFetch is false', () => {
    const queryFn = vi.fn().mockResolvedValue(['model-a'])
    renderHook(() => useWsQuery(queryFn, { autoFetch: false }))
    expect(queryFn).not.toHaveBeenCalled()
  })

  it('sets error on rejection', async () => {
    const queryFn = vi.fn().mockRejectedValue(new Error('network fail'))
    const { result } = renderHook(() => useWsQuery(queryFn))
    await waitFor(() => expect(result.current.error?.message).toBe('network fail'))
    expect(result.current.loading).toBe(false)
  })

  it('refresh() re-fetches data', async () => {
    let callCount = 0
    const queryFn = vi.fn().mockImplementation(() => Promise.resolve(`call-${++callCount}`))
    const { result } = renderHook(() => useWsQuery(queryFn))
    await waitFor(() => expect(result.current.data).toBe('call-1'))

    act(() => result.current.refresh())
    await waitFor(() => expect(result.current.data).toBe('call-2'))
  })

  it('refresh is a no-op when queryFn is null', () => {
    const { result } = renderHook(() => useWsQuery(null))
    expect(() => act(() => result.current.refresh())).not.toThrow()
  })
})
