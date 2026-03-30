import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  PAGE_SIZE,
  computeInitialPage,
  computePreviousPage,
  fetchMessages,
} from './use-session-messages'

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

    expect('messages' in result && result.messages).toHaveLength(1)
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

// ── computeInitialPage — ensures first page never swallows entire dataset ──
describe('computeInitialPage', () => {
  it('loads all blocks for small sessions (total ≤ PAGE_SIZE)', () => {
    expect(computeInitialPage(0)).toEqual({ offset: 0, size: 0 })
    expect(computeInitialPage(3)).toEqual({ offset: 0, size: 3 })
    expect(computeInitialPage(PAGE_SIZE)).toEqual({ offset: 0, size: PAGE_SIZE })
  })

  it('caps initial page to 60% for sessions just above PAGE_SIZE', () => {
    // 51 blocks → 60% = 30, offset = 21
    const result = computeInitialPage(51)
    expect(result.size).toBe(30)
    expect(result.offset).toBe(21)
    expect(result.offset).toBeGreaterThan(0) // guarantees hasPreviousPage
  })

  it('caps to PAGE_SIZE for large sessions (60% > PAGE_SIZE)', () => {
    // 255 blocks → 60% = 153, capped to PAGE_SIZE
    const result = computeInitialPage(255)
    expect(result.size).toBe(PAGE_SIZE)
    expect(result.offset).toBe(255 - PAGE_SIZE)
  })

  it('never returns size > total', () => {
    for (const total of [0, 1, 10, 49, 50, 51, 100, 500, 1000]) {
      const { offset, size } = computeInitialPage(total)
      expect(size).toBeLessThanOrEqual(total)
      expect(offset + size).toBe(total)
      expect(offset).toBeGreaterThanOrEqual(0)
    }
  })

  it('guarantees hasPreviousPage for any session with total > PAGE_SIZE', () => {
    for (const total of [51, 77, 97, 100, 120, 255, 500]) {
      const { offset } = computeInitialPage(total)
      expect(offset).toBeGreaterThan(0)
    }
  })
})

// ── computePreviousPage — non-overlapping page params ──
describe('computePreviousPage', () => {
  it('returns undefined at the beginning (offset=0)', () => {
    expect(computePreviousPage(0)).toBeUndefined()
  })

  it('returns full page when enough room', () => {
    const result = computePreviousPage(200)
    expect(result).toEqual({ offset: 150, limit: 50 })
  })

  it('clamps limit to avoid overlap for partial first page', () => {
    // offset=30 → prevOffset=0, limit=30 (not PAGE_SIZE=50)
    const result = computePreviousPage(30)
    expect(result).toEqual({ offset: 0, limit: 30 })
  })

  it('pages never overlap — each page covers exactly its range', () => {
    // Simulate full pagination chain from total=255
    let { offset } = computeInitialPage(255)
    const pages: { offset: number; limit: number }[] = []

    while (offset > 0) {
      const prev = computePreviousPage(offset)
      if (!prev) break
      pages.push(prev)
      offset = prev.offset
    }

    // Verify no gaps or overlaps
    for (let i = 0; i < pages.length - 1; i++) {
      const current = pages[i]
      const next = pages[i + 1]
      // Current page ends exactly where the next page starts
      expect(next.offset + next.limit).toBe(current.offset)
    }

    // First page in chain should reach offset=0
    expect(pages[pages.length - 1].offset).toBe(0)
  })

  it('covers all items when chain completes', () => {
    const total = 177
    const initial = computeInitialPage(total)
    let covered = initial.size
    let offset = initial.offset

    while (offset > 0) {
      const prev = computePreviousPage(offset)
      if (!prev) break
      covered += prev.limit
      offset = prev.offset
    }

    expect(covered).toBe(total)
  })
})
