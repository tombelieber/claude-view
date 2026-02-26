import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { createElement, type ReactNode } from 'react'
import { useRecentSessions, type RecentSession } from './use-recent-sessions'

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  })
  return function Wrapper({ children }: { children: ReactNode }) {
    return createElement(QueryClientProvider, { client: queryClient }, children)
  }
}

const mockSessions: RecentSession[] = [
  {
    id: 'session-1',
    preview: 'Refactored auth module',
    modifiedAt: 1700000000,
    gitBranch: 'feature/auth',
    project: 'my-app',
  },
  {
    id: 'session-2',
    preview: 'Fixed CSS layout bug',
    modifiedAt: 1699999000,
    gitBranch: 'main',
    project: 'my-app',
  },
]

beforeEach(() => {
  vi.restoreAllMocks()
})

describe('useRecentSessions', () => {
  it('returns empty array when no project is selected', () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch')

    const { result } = renderHook(() => useRecentSessions(null, null), {
      wrapper: createWrapper(),
    })

    expect(result.current.data).toEqual([])
    expect(result.current.isLoading).toBe(false)
    expect(fetchSpy).not.toHaveBeenCalled()
  })

  it('fetches recent sessions when project is selected', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      new Response(JSON.stringify({ sessions: mockSessions, total: 2 }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    )

    const { result } = renderHook(() => useRecentSessions('my-app', null), {
      wrapper: createWrapper(),
    })

    await waitFor(() => {
      expect(result.current.data).toHaveLength(2)
    })

    expect(result.current.data![0].id).toBe('session-1')
    expect(result.current.data![1].id).toBe('session-2')
  })

  it('includes branch filter in API call when provided', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      new Response(JSON.stringify({ sessions: mockSessions, total: 2 }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    )

    const { result } = renderHook(
      () => useRecentSessions('my-app', 'feature/auth'),
      { wrapper: createWrapper() },
    )

    await waitFor(() => {
      expect(result.current.data).toHaveLength(2)
    })

    const calledUrl = fetchSpy.mock.calls[0][0] as string
    expect(calledUrl).toContain('branch=feature%2Fauth')
  })

  it('limits to 5 results', async () => {
    const tenSessions: RecentSession[] = Array.from({ length: 10 }, (_, i) => ({
      id: `session-${i}`,
      preview: `Session ${i} preview`,
      modifiedAt: 1700000000 - i * 1000,
      project: 'my-app',
    }))

    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      new Response(JSON.stringify({ sessions: tenSessions, total: 10 }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    )

    const { result } = renderHook(() => useRecentSessions('my-app', null), {
      wrapper: createWrapper(),
    })

    await waitFor(() => {
      expect(result.current.data!.length).toBeGreaterThan(0)
    })

    expect(result.current.data).toHaveLength(5)
  })
})
