import { describe, expect, it } from 'vitest'
import { deriveDropdownActions, getStatusBadge, getStatusDotColor } from './session-list-helpers'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const liveAutonomous = {
  agentState: { group: 'autonomous' as const },
  status: 'working' as const,
  control: null,
}

const liveNeedsYou = {
  agentState: { group: 'needs_you' as const },
  status: 'paused' as const,
  control: null,
}

const liveSidecarManaged = {
  ...liveAutonomous,
  control: { controlId: 'c1', boundAt: 100 },
}

// ---------------------------------------------------------------------------
// getStatusDotColor — aligned with Live Monitor StatusDot
// ---------------------------------------------------------------------------

describe('getStatusDotColor', () => {
  it('amber for needs_you', () => {
    expect(getStatusDotColor({ liveData: liveNeedsYou })).toBe('bg-amber-500')
  })

  it('green for autonomous', () => {
    expect(getStatusDotColor({ liveData: liveAutonomous })).toBe('bg-green-500')
  })

  it('green for sidecar-managed autonomous', () => {
    expect(getStatusDotColor({ liveData: liveSidecarManaged, isSidecarManaged: true })).toBe(
      'bg-green-500',
    )
  })

  it('gray when no live data', () => {
    expect(getStatusDotColor({})).toBe('bg-gray-300 dark:bg-gray-600')
  })
})

// ---------------------------------------------------------------------------
// getStatusBadge — Live/Watching with color matching agent state
// ---------------------------------------------------------------------------

describe('getStatusBadge', () => {
  it('"Live" amber when sidecar-managed + needs_you', () => {
    const badge = getStatusBadge({ liveData: liveNeedsYou, isSidecarManaged: true })
    expect(badge?.text).toBe('Live')
    expect(badge?.className).toContain('amber')
  })

  it('"Live" green when sidecar-managed + autonomous', () => {
    const badge = getStatusBadge({ liveData: liveSidecarManaged, isSidecarManaged: true })
    expect(badge?.text).toBe('Live')
    expect(badge?.className).toContain('green')
  })

  it('"Watching" amber when external + needs_you', () => {
    const badge = getStatusBadge({ liveData: liveNeedsYou, isSidecarManaged: false })
    expect(badge?.text).toBe('Watching')
    expect(badge?.className).toContain('amber')
  })

  it('"Watching" green when external + autonomous', () => {
    const badge = getStatusBadge({ liveData: liveAutonomous, isSidecarManaged: false })
    expect(badge?.text).toBe('Watching')
    expect(badge?.className).toContain('green')
  })

  it('null when no live data', () => {
    expect(getStatusBadge({})).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// deriveDropdownActions — button visibility per session mode
// ---------------------------------------------------------------------------

describe('deriveDropdownActions', () => {
  it('HISTORY session: Resume + Fork + Archive', () => {
    const actions = deriveDropdownActions({
      isActive: false,
      isWatching: false,
      isSidecarManaged: false,
      liveData: null,
    })
    expect(actions.resume).toBe(true)
    expect(actions.fork).toBe(true)
    expect(actions.archive).toBe(true)
    expect(actions.takeOver).toBe(false)
    expect(actions.shutDown).toBe(false)
    expect(actions.openInMonitor).toBe(false)
  })

  it('WATCHING session: Take Over + Fork + Open in Monitor', () => {
    const actions = deriveDropdownActions({
      isActive: true,
      isWatching: true,
      isSidecarManaged: false,
      liveData: liveAutonomous,
    })
    expect(actions.takeOver).toBe(true)
    expect(actions.fork).toBe(true)
    expect(actions.openInMonitor).toBe(true)
    expect(actions.resume).toBe(false)
    expect(actions.shutDown).toBe(false)
    expect(actions.archive).toBe(false)
  })

  it('OWN session: Fork + Shut Down + Open in Monitor', () => {
    const actions = deriveDropdownActions({
      isActive: true,
      isWatching: false,
      isSidecarManaged: true,
      liveData: liveSidecarManaged,
    })
    expect(actions.fork).toBe(true)
    expect(actions.shutDown).toBe(true)
    expect(actions.openInMonitor).toBe(true)
    expect(actions.resume).toBe(false)
    expect(actions.takeOver).toBe(false)
    expect(actions.archive).toBe(false)
  })
})
