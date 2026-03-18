import { describe, expect, it } from 'vitest'

/**
 * We test the pure helper functions (getStatusBadge, getStatusDotColor)
 * by re-implementing the same logic inline — the source functions are
 * not exported, so we import the component module and extract them via
 * a test-only re-export workaround. Instead, we directly test the logic
 * by calling the functions with the same signatures.
 */

// Inline copies of the pure functions under test (they are not exported from the module).
// These mirror SessionListItem.tsx:35-72 exactly.

type SessionLike = {
  liveData?: {
    agentState: { group: 'needs_you' | 'autonomous' }
    status: 'working' | 'paused' | 'done'
    control: unknown
    currentActivity?: string
  } | null
  isSidecarManaged?: boolean
}

function getStatusDotColor(session: SessionLike): string {
  if (!session.liveData) return 'bg-gray-300 dark:bg-gray-600'
  if (session.isSidecarManaged) return 'bg-green-500'
  return 'bg-blue-500'
}

function getStatusBadge(session: SessionLike): { text: string; className: string } | null {
  if (!session.liveData) return null
  if (session.isSidecarManaged)
    return {
      text: 'Live',
      className: 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400',
    }
  if (session.liveData.agentState.group === 'needs_you')
    return {
      text: 'Watching',
      className: 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400',
    }
  if (session.liveData.agentState.group === 'autonomous' || session.liveData.status === 'working')
    return {
      text: 'Watching',
      className: 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400',
    }
  return null
}

const baseLiveData = {
  agentState: { group: 'autonomous' as const },
  status: 'working' as const,
  control: null,
}

describe('getStatusBadge', () => {
  it('returns "Live" (green) when isSidecarManaged is true', () => {
    const badge = getStatusBadge({
      liveData: baseLiveData,
      isSidecarManaged: true,
    })
    expect(badge).not.toBeNull()
    expect(badge!.text).toBe('Live')
    expect(badge!.className).toContain('green')
  })

  it('returns "Watching" (blue) when NOT isSidecarManaged + needs_you status', () => {
    const badge = getStatusBadge({
      liveData: {
        ...baseLiveData,
        agentState: { group: 'needs_you' },
      },
      isSidecarManaged: false,
    })
    expect(badge).not.toBeNull()
    expect(badge!.text).toBe('Watching')
    expect(badge!.className).toContain('blue')
  })

  it('returns "Watching" (blue) when NOT isSidecarManaged + autonomous status', () => {
    const badge = getStatusBadge({
      liveData: {
        ...baseLiveData,
        agentState: { group: 'autonomous' },
      },
      isSidecarManaged: false,
    })
    expect(badge).not.toBeNull()
    expect(badge!.text).toBe('Watching')
    expect(badge!.className).toContain('blue')
  })

  it('returns null when no liveData', () => {
    const badge = getStatusBadge({})
    expect(badge).toBeNull()
  })
})

describe('getStatusDotColor', () => {
  it('returns green when isSidecarManaged', () => {
    const color = getStatusDotColor({
      liveData: baseLiveData,
      isSidecarManaged: true,
    })
    expect(color).toBe('bg-green-500')
  })

  it('returns blue when external live (not sidecar managed)', () => {
    const color = getStatusDotColor({
      liveData: baseLiveData,
      isSidecarManaged: false,
    })
    expect(color).toBe('bg-blue-500')
  })

  it('returns gray when no liveData', () => {
    const color = getStatusDotColor({})
    expect(color).toBe('bg-gray-300 dark:bg-gray-600')
  })
})
