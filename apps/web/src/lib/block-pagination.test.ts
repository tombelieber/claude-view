import { describe, expect, it } from 'vitest'
import { BLOCK_PAGE_SIZE, computeInitialPage, computePreviousPage } from './block-pagination'

describe('computeInitialPage', () => {
  it('loads all blocks for small sessions (total <= BLOCK_PAGE_SIZE)', () => {
    expect(computeInitialPage(0)).toEqual({ offset: 0, size: 0 })
    expect(computeInitialPage(3)).toEqual({ offset: 0, size: 3 })
    expect(computeInitialPage(BLOCK_PAGE_SIZE)).toEqual({ offset: 0, size: BLOCK_PAGE_SIZE })
  })

  it('caps initial page to 60% for sessions just above BLOCK_PAGE_SIZE', () => {
    const result = computeInitialPage(51)
    expect(result.size).toBe(30)
    expect(result.offset).toBe(21)
    expect(result.offset).toBeGreaterThan(0)
  })

  it('caps to BLOCK_PAGE_SIZE for large sessions (60% > BLOCK_PAGE_SIZE)', () => {
    const result = computeInitialPage(255)
    expect(result.size).toBe(BLOCK_PAGE_SIZE)
    expect(result.offset).toBe(255 - BLOCK_PAGE_SIZE)
  })

  it('never returns size > total', () => {
    for (const total of [0, 1, 10, 49, 50, 51, 100, 500, 1000]) {
      const { offset, size } = computeInitialPage(total)
      expect(size).toBeLessThanOrEqual(total)
      expect(offset + size).toBe(total)
      expect(offset).toBeGreaterThanOrEqual(0)
    }
  })

  it('guarantees offset > 0 for any session with total > BLOCK_PAGE_SIZE', () => {
    for (const total of [51, 77, 97, 100, 120, 255, 500]) {
      const { offset } = computeInitialPage(total)
      expect(offset).toBeGreaterThan(0)
    }
  })
})

describe('computePreviousPage', () => {
  it('returns undefined at the beginning (offset=0)', () => {
    expect(computePreviousPage(0)).toBeUndefined()
  })

  it('returns full page when enough room', () => {
    expect(computePreviousPage(200)).toEqual({ offset: 150, limit: 50 })
  })

  it('clamps limit to avoid overlap for partial first page', () => {
    expect(computePreviousPage(30)).toEqual({ offset: 0, limit: 30 })
  })

  it('pages never overlap — each covers exactly its range', () => {
    let { offset } = computeInitialPage(255)
    const pages: { offset: number; limit: number }[] = []
    while (offset > 0) {
      const prev = computePreviousPage(offset)
      if (!prev) break
      pages.push(prev)
      offset = prev.offset
    }
    for (let i = 0; i < pages.length - 1; i++) {
      expect(pages[i + 1].offset + pages[i + 1].limit).toBe(pages[i].offset)
    }
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
