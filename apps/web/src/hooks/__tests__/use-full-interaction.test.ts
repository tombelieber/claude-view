import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { createElement, type ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  useFullInteraction,
  type PendingInteractionMeta,
  type InteractionBlock,
} from '../use-full-interaction'

// --- Test wrapper with QueryClient ---
function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
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

// --- Helpers ---
const pendingMeta: PendingInteractionMeta = {
  variant: 'permission',
  requestId: 'req-abc',
  preview: 'Allow tool_use: bash?',
}

const interactionBlock: InteractionBlock = {
  id: 'block-1',
  variant: 'permission',
  requestId: 'req-abc',
  resolved: false,
  historicalSource: null,
  data: { tool: 'bash', command: 'ls' },
}

describe('useFullInteraction', () => {
  it('returns null when pendingMeta is null (query disabled)', async () => {
    const { result } = renderHook(() => useFullInteraction('sess-1', null), {
      wrapper: createWrapper(),
    })

    // Give React Query a tick to settle
    await waitFor(() => {
      expect(result.current).toBeNull()
    })

    // fetch should NOT have been called since the query is disabled
    expect(mockFetch).not.toHaveBeenCalled()
  })

  it('returns null when pendingMeta is undefined (query disabled)', async () => {
    const { result } = renderHook(() => useFullInteraction('sess-1', undefined), {
      wrapper: createWrapper(),
    })

    await waitFor(() => {
      expect(result.current).toBeNull()
    })

    expect(mockFetch).not.toHaveBeenCalled()
  })

  it('fetches data when pendingMeta is provided', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => interactionBlock,
    })

    const { result } = renderHook(() => useFullInteraction('sess-42', pendingMeta), {
      wrapper: createWrapper(),
    })

    await waitFor(() => {
      expect(result.current).not.toBeNull()
    })

    expect(mockFetch).toHaveBeenCalledOnce()
    expect(mockFetch).toHaveBeenCalledWith('/api/sessions/sess-42/interaction')
  })

  it('returns the fetched InteractionBlock', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: async () => interactionBlock,
    })

    const { result } = renderHook(() => useFullInteraction('sess-42', pendingMeta), {
      wrapper: createWrapper(),
    })

    await waitFor(() => {
      expect(result.current).toEqual(interactionBlock)
    })
  })

  it('returns null when fetch fails', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 404,
    })

    const { result } = renderHook(() => useFullInteraction('sess-42', pendingMeta), {
      wrapper: createWrapper(),
    })

    // Query will error but the hook returns null on no data
    await waitFor(() => {
      expect(result.current).toBeNull()
    })
  })
})
