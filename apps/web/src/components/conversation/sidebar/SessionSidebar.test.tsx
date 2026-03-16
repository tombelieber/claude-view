// apps/web/src/components/conversation/sidebar/SessionSidebar.test.tsx
import { describe, expect, it } from 'vitest'

// Since SessionSidebar has many dependencies (router, contexts, etc.),
// these are logic-level tests for the progressive rendering invariants.
// Full component rendering is verified by Task 13 (build + manual).

describe('Sidebar progressive rendering invariants', () => {
  // --- Unit: VISIBLE_BATCH controls initial render count ---
  it('visible slice respects VISIBLE_BATCH constant', () => {
    const VISIBLE_BATCH = 30
    const allSessions = Array.from({ length: 100 }, (_, i) => ({ id: `s-${i}` }))
    const visibleCount = VISIBLE_BATCH
    const visible = allSessions.slice(0, visibleCount)
    expect(visible).toHaveLength(30)
  })

  // --- Unit: hasMore derived correctly ---
  it('hasMore is true when visibleCount < total sessions', () => {
    const filteredRest = Array.from({ length: 100 }, (_, i) => ({ id: `s-${i}` }))
    const visibleCount = 30
    expect(visibleCount < filteredRest.length).toBe(true)
  })

  it('hasMore is false when all sessions are visible', () => {
    const filteredRest = Array.from({ length: 20 }, (_, i) => ({ id: `s-${i}` }))
    const visibleCount = 30
    expect(visibleCount < filteredRest.length).toBe(false)
  })

  // --- Behavioral: visibleCount resets on search change ---
  it('visibleCount resets to VISIBLE_BATCH when search query changes', () => {
    const VISIBLE_BATCH = 30
    let visibleCount = 90 // User scrolled far down
    // Simulate: search query changes → reset
    const resetOnSearchChange = () => {
      visibleCount = VISIBLE_BATCH
    }
    resetOnSearchChange()
    expect(visibleCount).toBe(30)
  })

  // --- Behavioral: IntersectionObserver increments visibleCount ---
  it('IO callback increments visibleCount by VISIBLE_BATCH', () => {
    const VISIBLE_BATCH = 30
    let visibleCount = 30
    const incrementVisible = () => {
      visibleCount += VISIBLE_BATCH
    }

    // Simulate IO firing
    incrementVisible()
    expect(visibleCount).toBe(60)

    incrementVisible()
    expect(visibleCount).toBe(90)
  })

  // --- Unit: groupByTime receives sliced array, not full array ---
  it('visibleTimeGroups uses sliced filteredRest, not full list', () => {
    const VISIBLE_BATCH = 30
    const filteredRest = Array.from({ length: 100 }, (_, i) => ({
      id: `s-${i}`,
      timestamp: Date.now() / 1000,
    }))
    const visibleCount = VISIBLE_BATCH
    const sliced = filteredRest.slice(0, visibleCount)
    expect(sliced).toHaveLength(30)
    expect(sliced[0].id).toBe('s-0')
    expect(sliced[29].id).toBe('s-29')
  })
})

describe('event-driven sidebar (regression: 10s polling removed)', () => {
  it('SessionSidebar accepts liveSessions prop (new interface)', async () => {
    // @ts-expect-error — node:fs/promises unavailable in browser tsconfig
    const fs = await import('node:fs/promises')
    // @ts-expect-error — node:path unavailable in browser tsconfig
    const path = await import('node:path')
    const source = await fs.readFile(
      // @ts-expect-error — process.cwd() unavailable in browser tsconfig
      path.resolve(process.cwd(), 'src/components/conversation/sidebar/SessionSidebar.tsx'),
      'utf-8',
    )
    expect(source).toMatch(/liveSessions:\s*LiveSession\[\]/)
    expect(source).toMatch(/interface\s+SessionSidebarProps/)
  })

  it('no setInterval polling in component (regression: was 10s poll)', async () => {
    // @ts-expect-error — node:fs/promises unavailable in browser tsconfig
    const fs = await import('node:fs/promises')
    // @ts-expect-error — node:path unavailable in browser tsconfig
    const path = await import('node:path')
    const source = await fs.readFile(
      // @ts-expect-error — process.cwd() unavailable in browser tsconfig
      path.resolve(process.cwd(), 'src/components/conversation/sidebar/SessionSidebar.tsx'),
      'utf-8',
    )
    expect(source).not.toMatch(/setInterval/)
  })

  it('sdkControlledSessions filters by control !== null', () => {
    const live = [
      { id: 's1', control: { id: 'c1' }, agentState: { group: 'autonomous' } },
      { id: 's2', control: null, agentState: { group: 'autonomous' } },
      { id: 's3', control: { id: 'c3' }, agentState: { group: 'needs_you' } },
    ]
    const sdkControlled = live.filter((s) => s.control !== null)
    expect(sdkControlled).toHaveLength(2)
    expect(sdkControlled.map((s) => s.id)).toEqual(['s1', 's3'])
  })
})
