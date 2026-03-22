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
    expect(
      getStatusDotColor({ liveData: liveSidecarManaged, liveStatus: 'cc_agent_sdk_owned' }),
    ).toBe('bg-green-500')
  })

  it('gray when no live data', () => {
    expect(getStatusDotColor({})).toBe('bg-gray-300 dark:bg-gray-600')
  })
})

// ---------------------------------------------------------------------------
// getSessionSource — terminal vs sdk based on liveStatus
// ---------------------------------------------------------------------------

describe('getSessionSource', () => {
  it('sdk when cc_agent_sdk_owned', () => {
    expect(getSessionSource({ liveStatus: 'cc_agent_sdk_owned' })).toBe('sdk')
  })

  it('terminal when cc_owned', () => {
    expect(getSessionSource({ liveStatus: 'cc_owned' })).toBe('terminal')
  })

  it('terminal when inactive (default)', () => {
    expect(getSessionSource({ liveStatus: 'inactive' })).toBe('terminal')
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
      { liveData: liveNeedsYou, liveStatus: 'cc_owned' as const },
      { liveData: liveAutonomous, liveStatus: 'cc_owned' as const },
      { liveData: liveSidecarManaged, liveStatus: 'cc_agent_sdk_owned' as const },
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
  it('HISTORY session: Resume + Fork + Archive', () => {
    const actions = deriveDropdownActions({
      liveStatus: 'inactive',
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
      liveStatus: 'cc_owned',
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
      liveStatus: 'cc_agent_sdk_owned',
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
