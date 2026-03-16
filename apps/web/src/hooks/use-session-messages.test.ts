import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fetchMessages } from './use-session-messages'

describe('fetchMessages — 404 handling', () => {
  beforeEach(() => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        json: () =>
          Promise.resolve({
            messages: [{ role: 'user', content: 'hello' }],
            total: 1,
            offset: 0,
            limit: 100,
            hasMore: false,
          }),
      }),
    )
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  // --- Regression: 404 + suppressNotFound=true returns empty (new session) ---
  // Root cause: brand-new sessions have no JSONL file yet. Without suppression,
  // 404 throws HttpError → "Failed to load messages. Retry" in the UI.
  it('returns empty PaginatedMessages on 404 when suppressNotFound=true', async () => {
    vi.mocked(fetch).mockResolvedValueOnce({
      ok: false,
      status: 404,
    } as Response)

    const result = await fetchMessages('new-session-id', 0, 100, false, true)

    expect(result).toEqual({
      messages: [],
      total: 0,
      offset: 0,
      limit: 100,
      hasMore: false,
    })
  })

  // --- Regression: 404 + suppressNotFound=false throws (genuinely missing) ---
  it('throws HttpError on 404 when suppressNotFound=false (default)', async () => {
    vi.mocked(fetch).mockResolvedValueOnce({
      ok: false,
      status: 404,
    } as Response)

    await expect(fetchMessages('deleted-session', 0, 100, false)).rejects.toThrow(
      'Failed to fetch messages',
    )
  })

  // --- Regression: 404 + suppressNotFound=false throws (explicit false) ---
  it('throws HttpError on 404 when suppressNotFound is explicitly false', async () => {
    vi.mocked(fetch).mockResolvedValueOnce({
      ok: false,
      status: 404,
    } as Response)

    await expect(fetchMessages('deleted-session', 0, 100, false, false)).rejects.toThrow(
      'Failed to fetch messages',
    )
  })

  // --- Unit: non-404 errors throw regardless of suppressNotFound ---
  it('throws HttpError on 500 even with suppressNotFound=true', async () => {
    vi.mocked(fetch).mockResolvedValueOnce({
      ok: false,
      status: 500,
    } as Response)

    await expect(fetchMessages('sess-1', 0, 100, false, true)).rejects.toThrow(
      'Failed to fetch messages',
    )
  })

  // --- Unit: 200 returns parsed JSON normally ---
  it('returns parsed messages on 200', async () => {
    const result = await fetchMessages('sess-1', 0, 100, false)

    expect(result.messages).toHaveLength(1)
    expect(result.total).toBe(1)
  })

  // --- Regression: suppressed 404 preserves the limit parameter ---
  it('preserves limit parameter in empty 404 response', async () => {
    vi.mocked(fetch).mockResolvedValueOnce({
      ok: false,
      status: 404,
    } as Response)

    const result = await fetchMessages('new-session-id', 0, 50, false, true)
    expect(result.limit).toBe(50)
  })
})
