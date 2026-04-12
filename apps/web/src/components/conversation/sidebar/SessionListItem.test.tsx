import { describe, expect, it } from 'vitest'
import {
  deriveDropdownActions,
  getSessionSource,
  getStatusDotColor,
  getUrgencyGroup,
  groupByUrgency,
} from './session-list-helpers'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const liveAutonomous = {
  agentState: { group: 'autonomous' as const },
  status: 'working' as const,
  ownership: null,
}

const liveNeedsYou = {
  agentState: { group: 'needs_you' as const },
  status: 'paused' as const,
  ownership: null,
}

const liveSdkManaged = {
  ...liveAutonomous,
  ownership: { sdk: { controlId: 'ctrl-1' }, source: null, entrypoint: null },
}

const liveTmuxManaged = {
  ...liveAutonomous,
  ownership: {
    tmux: { cliSessionId: 'cv-test-1' },
    source: null,
    entrypoint: null,
  },
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

  it('green for sdk-managed autonomous', () => {
    expect(getStatusDotColor({ liveData: liveSdkManaged })).toBe('bg-green-500')
  })

  it('gray when no live data', () => {
    expect(getStatusDotColor({})).toBe('bg-gray-300 dark:bg-gray-600')
  })
})

// ---------------------------------------------------------------------------
// getSessionSource — terminal vs sdk based on ownership tier
// ---------------------------------------------------------------------------

describe('getSessionSource', () => {
  it('sdk when ownership tier is sdk', () => {
    expect(getSessionSource({ liveData: liveSdkManaged })).toBe('sdk')
  })

  it('terminal when ownership tier is tmux', () => {
    expect(getSessionSource({ liveData: liveTmuxManaged })).toBe('terminal')
  })

  it('terminal when no ownership (default)', () => {
    expect(getSessionSource({ liveData: liveAutonomous })).toBe('terminal')
  })
})

// ---------------------------------------------------------------------------
// getUrgencyGroup + groupByUrgency — Mission Control grouping
// ---------------------------------------------------------------------------

describe('getUrgencyGroup', () => {
  it('needs_you when agentState.group is needs_you', () => {
    expect(getUrgencyGroup({ liveData: liveNeedsYou })).toBe('needs_you')
  })

  it('working when agentState.group is autonomous', () => {
    expect(getUrgencyGroup({ liveData: liveAutonomous })).toBe('working')
  })

  it('working when no agentState', () => {
    expect(getUrgencyGroup({ liveData: { status: 'working' } })).toBe('working')
  })
})

describe('groupByUrgency', () => {
  it('splits sessions into needsYou and working', () => {
    const sessions = [
      { liveData: liveNeedsYou, isActive: true },
      { liveData: liveAutonomous, isActive: true },
      { liveData: liveSdkManaged, isActive: true },
    ]
    const { needsYou, working } = groupByUrgency(sessions)
    expect(needsYou).toHaveLength(1)
    expect(working).toHaveLength(2)
  })

  it('returns empty arrays when no sessions', () => {
    const { needsYou, working } = groupByUrgency([])
    expect(needsYou).toHaveLength(0)
    expect(working).toHaveLength(0)
  })
})

// ---------------------------------------------------------------------------
// deriveDropdownActions — button visibility per session mode
// ---------------------------------------------------------------------------

describe('deriveDropdownActions', () => {
  it('HISTORY session: Resume + Archive (fork/takeOver hidden)', () => {
    const actions = deriveDropdownActions({
      isActive: false,
      liveData: null,
    })
    expect(actions.resume).toBe(true)
    expect(actions.archive).toBe(true)
    expect(actions.fork).toBe(false)
    expect(actions.takeOver).toBe(false)
    expect(actions.shutDown).toBe(false)
    expect(actions.openInMonitor).toBe(false)
  })

  it('WATCHING session: Open in Monitor (fork/takeOver hidden)', () => {
    const actions = deriveDropdownActions(
      {
        isActive: true,
        liveData: liveAutonomous,
      },
      liveTmuxManaged.ownership,
    )
    expect(actions.openInMonitor).toBe(true)
    expect(actions.takeOver).toBe(false)
    expect(actions.fork).toBe(false)
    expect(actions.resume).toBe(false)
    expect(actions.shutDown).toBe(true) // tmux can be shut down
    expect(actions.archive).toBe(false)
  })

  it('OWN session: Shut Down + Open in Monitor (fork hidden)', () => {
    const actions = deriveDropdownActions(
      {
        isActive: true,
        liveData: liveSdkManaged,
      },
      liveSdkManaged.ownership,
    )
    expect(actions.shutDown).toBe(true)
    expect(actions.openInMonitor).toBe(true)
    expect(actions.fork).toBe(false)
    expect(actions.resume).toBe(false)
    expect(actions.takeOver).toBe(false)
    expect(actions.archive).toBe(false)
  })
})
