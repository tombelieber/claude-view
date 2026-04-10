import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { createElement, type ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useCliSessions, useCreateCliSession, useKillCliSession } from '../use-cli-sessions'

// --- Test wrapper with QueryClient ---
function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  })
  return function Wrapper({ children }: { children: ReactNode }) {
    return createElement(QueryClientProvider, { client: queryClient }, children)
  }
}

// --- Mock fetch ---
const mockFetch = vi.fn()

beforeEach(() => {
  mockFetch.mockReset()
  vi.stubGlobal('fetch', mockFetch)
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('useCliSessions', () => {
  it('returns empty array when API returns no sessions', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => ({ sessions: [] }),
    })

    const { result } = renderHook(() => useCliSessions(), {
      wrapper: createWrapper(),
    })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual([])
  })

  it('returns sessions from API', async () => {
    const mockSessions = [
      { id: 'sess-1', createdAt: 1000, status: 'running', projectDir: '/tmp', args: [] },
      { id: 'sess-2', createdAt: 2000, status: 'exited', projectDir: null, args: ['--help'] },
    ]
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => ({ sessions: mockSessions }),
    })

    const { result } = renderHook(() => useCliSessions(), {
      wrapper: createWrapper(),
    })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toEqual(mockSessions)
    expect(result.current.data).toHaveLength(2)
  })

  it('sets error when API fails', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 500,
    })

    const { result } = renderHook(() => useCliSessions(), {
      wrapper: createWrapper(),
    })

    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(result.current.error).toBeInstanceOf(Error)
  })
})

describe('useCreateCliSession', () => {
  it('calls POST /api/cli-sessions with correct body', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => ({ session: { id: 'new-sess' } }),
    })

    const { result } = renderHook(() => useCreateCliSession(), {
      wrapper: createWrapper(),
    })

    await result.current.mutateAsync({ projectDir: '/home/user/project' })

    expect(mockFetch).toHaveBeenCalledWith('/api/cli-sessions', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ projectDir: '/home/user/project' }),
    })
  })

  it('throws on non-ok response', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 500,
      json: async () => ({ error: 'Internal error' }),
    })

    const { result } = renderHook(() => useCreateCliSession(), {
      wrapper: createWrapper(),
    })

    await expect(result.current.mutateAsync({ projectDir: '/tmp' })).rejects.toThrow(
      'Internal error',
    )
  })
})

describe('useKillCliSession', () => {
  it('calls DELETE /api/cli-sessions/:id', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => ({ ok: true }),
    })

    const { result } = renderHook(() => useKillCliSession(), {
      wrapper: createWrapper(),
    })

    await result.current.mutateAsync('sess-to-kill')

    expect(mockFetch).toHaveBeenCalledWith('/api/cli-sessions/sess-to-kill', {
      method: 'DELETE',
    })
  })

  it('throws on non-ok response', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 404,
    })

    const { result } = renderHook(() => useKillCliSession(), {
      wrapper: createWrapper(),
    })

    await expect(result.current.mutateAsync('nonexistent')).rejects.toThrow(
      'Failed to kill session',
    )
  })
})
