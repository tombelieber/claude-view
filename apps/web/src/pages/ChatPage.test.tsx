import { describe, expect, it } from 'vitest'
import type { LiveStatus } from '../lib/live-status'
import { getContextLimit } from '../lib/model-context-windows'

/**
 * Tests for the `liveStatus` derivation in ChatPage.
 *
 * From ChatPage.tsx:
 *   const liveStatus: LiveStatus = liveSession == null ? 'inactive'
 *     : liveSession.control != null ? 'cc_agent_sdk_owned'
 *     : 'cc_owned'
 *
 * We test this as a pure logic function to avoid mocking router/outlet/query.
 */

function deriveLiveStatus(
  liveSession: { id: string; control: string | null } | undefined,
): LiveStatus {
  if (liveSession == null) return 'inactive'
  return liveSession.control != null ? 'cc_agent_sdk_owned' : 'cc_owned'
}

describe('liveStatus derivation', () => {
  it('cc_owned when session is live AND control is null (CLI/VS Code)', () => {
    expect(deriveLiveStatus({ id: 's1', control: null })).toBe('cc_owned')
  })

  it('cc_agent_sdk_owned when session is live AND control is non-null (sidecar)', () => {
    expect(deriveLiveStatus({ id: 's1', control: 'ctrl-123' })).toBe('cc_agent_sdk_owned')
  })

  it('inactive when no liveSession found', () => {
    expect(deriveLiveStatus(undefined)).toBe('inactive')
  })

  it('inactive when liveSession is null', () => {
    expect(deriveLiveStatus(null as any)).toBe('inactive')
  })
})

/**
 * Tests for contextPercent priority chain in ChatSession.
 *
 * Priority (most authoritative first — never show wrong data):
 *  1. statuslineUsedPct — pre-computed by Claude Code
 *  2. liveContextData.contextWindowTokens + statuslineContextWindowSize
 *  3. richData.contextWindowTokens (JSONL history)
 *  4. undefined — show "--"
 *
 * Extracted as pure function to test without rendering ChatSession.
 */

interface LiveContextData {
  contextWindowTokens: number
  statuslineContextWindowSize: number | null
  statuslineUsedPct: number | null
}

function deriveContextPercent(
  liveContextData: LiveContextData | undefined,
  richDataContextWindowTokens: number | undefined,
): number | undefined {
  if (liveContextData?.statuslineUsedPct != null) {
    return Math.round(liveContextData.statuslineUsedPct)
  }
  if (liveContextData && liveContextData.contextWindowTokens > 0) {
    const limit = getContextLimit(
      null,
      liveContextData.contextWindowTokens,
      liveContextData.statuslineContextWindowSize,
    )
    return Math.round((liveContextData.contextWindowTokens / limit) * 100)
  }
  if (richDataContextWindowTokens != null && richDataContextWindowTokens > 0) {
    const limit = getContextLimit(null, richDataContextWindowTokens)
    return Math.round((richDataContextWindowTokens / limit) * 100)
  }
  return undefined
}

describe('contextPercent priority chain', () => {
  it('P1: uses statuslineUsedPct when available (most authoritative)', () => {
    const result = deriveContextPercent(
      {
        statuslineUsedPct: 42.7,
        contextWindowTokens: 999_999,
        statuslineContextWindowSize: 200_000,
      },
      50_000,
    )
    expect(result).toBe(43)
  })

  it('P1: statuslineUsedPct = 0 is valid (not skipped)', () => {
    const result = deriveContextPercent(
      { statuslineUsedPct: 0, contextWindowTokens: 0, statuslineContextWindowSize: 200_000 },
      undefined,
    )
    expect(result).toBe(0)
  })

  it('P2: falls back to liveContextData fill + statusline denominator', () => {
    const result = deriveContextPercent(
      {
        statuslineUsedPct: null,
        contextWindowTokens: 100_000,
        statuslineContextWindowSize: 1_000_000,
      },
      undefined,
    )
    expect(result).toBe(10)
  })

  it('P2: uses getContextLimit inference when statuslineContextWindowSize is null', () => {
    // 100K fill, no statuslineSize → getContextLimit infers 200K default
    const result = deriveContextPercent(
      { statuslineUsedPct: null, contextWindowTokens: 100_000, statuslineContextWindowSize: null },
      undefined,
    )
    expect(result).toBe(50)
  })

  it('P3: falls back to richData when no live context data', () => {
    const result = deriveContextPercent(undefined, 50_000)
    // 50K / 200K default = 25%
    expect(result).toBe(25)
  })

  it('P3: richData with large fill infers 1M context window', () => {
    const result = deriveContextPercent(undefined, 250_000)
    // 250K > 200K → getContextLimit returns 1M → 250K/1M = 25%
    expect(result).toBe(25)
  })

  it('P4: returns undefined when no data available', () => {
    expect(deriveContextPercent(undefined, undefined)).toBeUndefined()
    expect(deriveContextPercent(undefined, 0)).toBeUndefined()
  })

  it('never uses WS totalInputTokens (intentionally excluded)', () => {
    // Simulate: live session with statuslineUsedPct=12, but if someone
    // were to use WS totalInputTokens they'd get ~84%. Verify we get 12.
    const result = deriveContextPercent(
      { statuslineUsedPct: 12, contextWindowTokens: 24_000, statuslineContextWindowSize: 200_000 },
      24_000,
    )
    expect(result).toBe(12)
  })
})
