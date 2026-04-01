import { afterEach, describe, expect, it, vi } from 'vitest'
import { BLOCK_PAGE_SIZE, computeInitialPage } from './block-pagination'
import { fetchInitialHistory } from './fetch-initial-history'

describe('fetchInitialHistory — boundary tests', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('fetches all blocks for small session (total <= BLOCK_PAGE_SIZE)', async () => {
    const total = 30
    const blocks = Array.from({ length: total }, (_, i) => ({ id: `b${i}`, type: 'user' }))

    vi.stubGlobal(
      'fetch',
      vi
        .fn()
        .mockResolvedValueOnce({
          ok: true,
          json: () => Promise.resolve({ total, blocks: [blocks[0]] }),
        })
        .mockResolvedValueOnce({
          ok: true,
          json: () => Promise.resolve({ total, blocks, offset: 0 }),
        }),
    )

    const result = await fetchInitialHistory('sess-small')

    // Verify tail fetch used offset=0 (all blocks fit)
    const tailCall = vi.mocked(fetch).mock.calls[1][0] as string
    const url = new URL(tailCall, 'http://localhost')
    expect(url.searchParams.get('offset')).toBe('0')
    expect(url.searchParams.get('limit')).toBe(String(total))
    expect(result).toMatchObject({ type: 'HISTORY_OK', total, offset: 0 })
  })

  it('fetches tail page for large session (total > BLOCK_PAGE_SIZE)', async () => {
    const total = 200
    const { offset: expectedOffset, size: expectedSize } = computeInitialPage(total)

    vi.stubGlobal(
      'fetch',
      vi
        .fn()
        .mockResolvedValueOnce({
          ok: true,
          json: () => Promise.resolve({ total, blocks: [] }),
        })
        .mockResolvedValueOnce({
          ok: true,
          json: () => Promise.resolve({ total, blocks: [], offset: expectedOffset }),
        }),
    )

    const result = await fetchInitialHistory('sess-large')

    const tailCall = vi.mocked(fetch).mock.calls[1][0] as string
    const url = new URL(tailCall, 'http://localhost')
    expect(url.searchParams.get('offset')).toBe(String(expectedOffset))
    expect(url.searchParams.get('limit')).toBe(String(expectedSize))
    expect(Number(url.searchParams.get('offset'))).toBeGreaterThan(0)
    expect(result).toMatchObject({ type: 'HISTORY_OK', total, offset: expectedOffset })
  })

  it('returns empty result for empty session (no tail fetch)', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ total: 0, blocks: [] }),
      }),
    )

    const result = await fetchInitialHistory('sess-empty')

    expect(vi.mocked(fetch)).toHaveBeenCalledTimes(1) // only probe, no tail fetch
    expect(result).toEqual({ type: 'HISTORY_OK', blocks: [], total: 0, offset: 0 })
  })

  it('never requests limit > BLOCK_PAGE_SIZE for large sessions', async () => {
    for (const total of [51, 100, 200, 500, 1000]) {
      vi.stubGlobal(
        'fetch',
        vi
          .fn()
          .mockResolvedValueOnce({
            ok: true,
            json: () => Promise.resolve({ total, blocks: [] }),
          })
          .mockResolvedValueOnce({
            ok: true,
            json: () => Promise.resolve({ total, blocks: [], offset: 0 }),
          }),
      )

      await fetchInitialHistory(`sess-${total}`)

      const tailCall = vi.mocked(fetch).mock.calls[1][0] as string
      const url = new URL(tailCall, 'http://localhost')
      const requestedLimit = Number(url.searchParams.get('limit'))
      expect(requestedLimit).toBeLessThanOrEqual(BLOCK_PAGE_SIZE)
      vi.restoreAllMocks()
    }
  })

  it('coerces non-numeric total to 0 (NaN guard)', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ total: 'garbage', blocks: [] }),
      }),
    )

    const result = await fetchInitialHistory('sess-nan')

    expect(vi.mocked(fetch)).toHaveBeenCalledTimes(1) // treated as empty
    expect(result).toEqual({ type: 'HISTORY_OK', blocks: [], total: 0, offset: 0 })
  })
})
