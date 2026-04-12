import { renderHook, act } from '@testing-library/react'
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest'
import {
  useInteractionResponder,
  type InteractRequest,
} from '@claude-view/shared/hooks/useInteractionResponder'
import type { SessionOwnership } from '@claude-view/shared/types/generated/SessionOwnership'

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
const sdkOwnership: SessionOwnership = {
  sdk: { controlId: 'ctrl-1' },
  source: null,
  entrypoint: null,
}

const tmuxOwnership: SessionOwnership = {
  tmux: { cliSessionId: 'cli-1' },
  source: null,
  entrypoint: null,
}

const observedOwnership: SessionOwnership = {
  source: null,
  entrypoint: null,
}

const permissionRequest: InteractRequest = {
  variant: 'permission',
  requestId: 'req-1',
  allowed: true,
}

describe('useInteractionResponder', () => {
  it('returns undefined when ownership is null', () => {
    const { result } = renderHook(() => useInteractionResponder('sess-1', null))
    expect(result.current).toBeUndefined()
  })

  it('returns undefined when ownership is undefined', () => {
    const { result } = renderHook(() => useInteractionResponder('sess-1', undefined))
    expect(result.current).toBeUndefined()
  })

  it('returns undefined when ownership tier is observed', () => {
    const { result } = renderHook(() => useInteractionResponder('sess-1', observedOwnership))
    expect(result.current).toBeUndefined()
  })

  it('returns a function when ownership tier is sdk', () => {
    const { result } = renderHook(() => useInteractionResponder('sess-1', sdkOwnership))
    expect(typeof result.current).toBe('function')
  })

  it('returns a function when ownership tier is tmux', () => {
    const { result } = renderHook(() => useInteractionResponder('sess-1', tmuxOwnership))
    expect(typeof result.current).toBe('function')
  })

  it('calls fetch with correct URL and body', async () => {
    mockFetch.mockResolvedValueOnce({ ok: true })

    const { result } = renderHook(() => useInteractionResponder('sess-42', sdkOwnership))

    await act(async () => {
      await result.current!(permissionRequest)
    })

    expect(mockFetch).toHaveBeenCalledOnce()
    expect(mockFetch).toHaveBeenCalledWith('/api/sessions/sess-42/interact', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(permissionRequest),
    })
  })

  it('returns { ok: true } on 200 response', async () => {
    mockFetch.mockResolvedValueOnce({ ok: true })

    const { result } = renderHook(() => useInteractionResponder('sess-1', sdkOwnership))

    let interactResult: unknown
    await act(async () => {
      interactResult = await result.current!(permissionRequest)
    })

    expect(interactResult).toEqual({ ok: true })
  })

  it('returns { ok: false, status, reason } on 409 response', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 409,
      text: async () => 'Conflict: already resolved',
    })

    const { result } = renderHook(() => useInteractionResponder('sess-1', sdkOwnership))

    let interactResult: unknown
    await act(async () => {
      interactResult = await result.current!(permissionRequest)
    })

    expect(interactResult).toEqual({
      ok: false,
      status: 409,
      reason: 'Conflict: already resolved',
    })
  })

  it('returns { ok: false, status: 0, reason } on network error', async () => {
    mockFetch.mockRejectedValueOnce(new Error('Network failed'))

    const { result } = renderHook(() => useInteractionResponder('sess-1', sdkOwnership))

    let interactResult: unknown
    await act(async () => {
      interactResult = await result.current!(permissionRequest)
    })

    expect(interactResult).toEqual({
      ok: false,
      status: 0,
      reason: 'Error: Network failed',
    })
  })
})
