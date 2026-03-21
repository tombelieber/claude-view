import type { LiveSession } from '@claude-view/shared/types/generated'
import { describe, expect, it } from 'vitest'
import {
  deriveLiveStatus,
  derivePanelMode,
  modeToConnectionHealth,
  modeToInputBar,
} from './derive-panel-mode'
import type { LiveStatus, SessionState } from './derive-panel-mode'

/** Test helper: creates a minimal LiveSession stub with only the fields deriveLiveStatus reads */
function stubLive(partial: {
  status: LiveSession['status']
  control: { controlId: string } | null
}): LiveSession {
  return partial as unknown as LiveSession
}

describe('deriveLiveStatus', () => {
  it('returns inactive for null', () => {
    expect(deriveLiveStatus(null)).toBe('inactive')
  })

  it('returns inactive for undefined', () => {
    expect(deriveLiveStatus(undefined)).toBe('inactive')
  })

  it('returns inactive for done session without control', () => {
    expect(deriveLiveStatus(stubLive({ status: 'done', control: null }))).toBe('inactive')
  })

  it('returns cc_agent_sdk_owned for done + stale control (control!=null passes isActive)', () => {
    expect(deriveLiveStatus(stubLive({ status: 'done', control: { controlId: 'c1' } }))).toBe(
      'cc_agent_sdk_owned',
    )
  })

  it('returns cc_owned for working session without control', () => {
    expect(deriveLiveStatus(stubLive({ status: 'working', control: null }))).toBe('cc_owned')
  })

  it('returns cc_owned for paused session without control', () => {
    expect(deriveLiveStatus(stubLive({ status: 'paused', control: null }))).toBe('cc_owned')
  })

  it('returns cc_agent_sdk_owned for working session with control', () => {
    expect(deriveLiveStatus(stubLive({ status: 'working', control: { controlId: 'c1' } }))).toBe(
      'cc_agent_sdk_owned',
    )
  })

  it('returns cc_agent_sdk_owned for paused session with control', () => {
    expect(deriveLiveStatus(stubLive({ status: 'paused', control: { controlId: 'c1' } }))).toBe(
      'cc_agent_sdk_owned',
    )
  })
})

describe('derivePanelMode', () => {
  it('returns blank when no sessionId', () => {
    expect(derivePanelMode(undefined, 'inactive', 'idle')).toEqual({ mode: 'blank' })
  })

  it('returns watching when liveStatus is cc_owned', () => {
    expect(derivePanelMode('s1', 'cc_owned', 'idle')).toEqual({ mode: 'watching' })
  })

  it('returns watching regardless of sessionState when cc_owned', () => {
    expect(derivePanelMode('s1', 'cc_owned', 'active')).toEqual({ mode: 'watching' })
    expect(derivePanelMode('s1', 'cc_owned', 'error')).toEqual({ mode: 'watching' })
  })

  // --- SDK owned: connecting ---
  it('returns connecting(initial) for initializing', () => {
    expect(derivePanelMode('s1', 'inactive', 'initializing')).toEqual({
      mode: 'connecting',
      reason: 'initial',
    })
  })

  it('returns connecting(reconnecting) for reconnecting', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'reconnecting')).toEqual({
      mode: 'connecting',
      reason: 'reconnecting',
    })
  })

  // --- SDK owned: connected ---
  it('returns own(active) for waiting_input', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'waiting_input')).toEqual({
      mode: 'own',
      subState: 'active',
    })
  })

  it('returns own(streaming) for active', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'active')).toEqual({
      mode: 'own',
      subState: 'streaming',
    })
  })

  it('returns own(waiting_permission) for waiting_permission', () => {
    expect(derivePanelMode('s1', 'inactive', 'waiting_permission')).toEqual({
      mode: 'own',
      subState: 'waiting_permission',
    })
  })

  it('returns own(compacting) for compacting', () => {
    expect(derivePanelMode('s1', 'inactive', 'compacting')).toEqual({
      mode: 'own',
      subState: 'compacting',
    })
  })

  // --- SDK owned: failed ---
  it('returns error(fatal) for error', () => {
    expect(derivePanelMode('s1', 'inactive', 'error')).toEqual({
      mode: 'error',
      reason: 'fatal',
    })
  })

  it('returns error(replaced) for replaced', () => {
    expect(derivePanelMode('s1', 'inactive', 'replaced')).toEqual({
      mode: 'error',
      reason: 'replaced',
    })
  })

  // --- Inactive (history) ---
  it('returns history for idle', () => {
    expect(derivePanelMode('s1', 'inactive', 'idle')).toEqual({ mode: 'history' })
  })

  it('returns history for closed', () => {
    expect(derivePanelMode('s1', 'inactive', 'closed')).toEqual({ mode: 'history' })
  })

  // --- liveStatus does not override sessionState for SDK states ---
  it('SDK sessionStates work regardless of liveStatus (SSE can lag)', () => {
    // SSE says inactive but sidecar WS is connected
    expect(derivePanelMode('s1', 'inactive', 'waiting_input')).toEqual({
      mode: 'own',
      subState: 'active',
    })
    // SSE says sdk_owned and sessionState agrees
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'waiting_input')).toEqual({
      mode: 'own',
      subState: 'active',
    })
  })

  // --- Cross-product: cc_agent_sdk_owned with all SDK session states ---
  it('returns connecting(initial) when cc_agent_sdk_owned + initializing', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'initializing')).toEqual({
      mode: 'connecting',
      reason: 'initial',
    })
  })

  it('returns error(fatal) when cc_agent_sdk_owned + error', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'error')).toEqual({
      mode: 'error',
      reason: 'fatal',
    })
  })

  it('returns error(replaced) when cc_agent_sdk_owned + replaced', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'replaced')).toEqual({
      mode: 'error',
      reason: 'replaced',
    })
  })

  it('returns history when cc_agent_sdk_owned + idle (SSE/WS desync)', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'idle')).toEqual({ mode: 'history' })
  })

  it('returns history when cc_agent_sdk_owned + closed', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'closed')).toEqual({ mode: 'history' })
  })
})

describe('modeToInputBar', () => {
  it('blank → dormant', () => {
    expect(modeToInputBar({ mode: 'blank' })).toBe('dormant')
  })

  it('history → active', () => {
    expect(modeToInputBar({ mode: 'history' })).toBe('active')
  })

  it('watching → active (user can resume by sending a message)', () => {
    expect(modeToInputBar({ mode: 'watching' })).toBe('active')
  })

  it('connecting(initial) → connecting', () => {
    expect(modeToInputBar({ mode: 'connecting', reason: 'initial' })).toBe('connecting')
  })

  it('connecting(reconnecting) → reconnecting', () => {
    expect(modeToInputBar({ mode: 'connecting', reason: 'reconnecting' })).toBe('reconnecting')
  })

  it('own(active) → active', () => {
    expect(modeToInputBar({ mode: 'own', subState: 'active' })).toBe('active')
  })

  it('own(streaming) → streaming', () => {
    expect(modeToInputBar({ mode: 'own', subState: 'streaming' })).toBe('streaming')
  })

  it('own(waiting_permission) → waiting_permission', () => {
    expect(modeToInputBar({ mode: 'own', subState: 'waiting_permission' })).toBe(
      'waiting_permission',
    )
  })

  it('own(compacting) → streaming', () => {
    expect(modeToInputBar({ mode: 'own', subState: 'compacting' })).toBe('streaming')
  })

  it('error(fatal) → completed', () => {
    expect(modeToInputBar({ mode: 'error', reason: 'fatal' })).toBe('completed')
  })

  it('error(replaced) → completed', () => {
    expect(modeToInputBar({ mode: 'error', reason: 'replaced' })).toBe('completed')
  })
})

describe('integration: derivePanelMode → modeToInputBar pipeline', () => {
  const scenarios: {
    desc: string
    sessionId: string | undefined
    liveStatus: LiveStatus
    sessionState: SessionState
    expectCanSend: boolean
  }[] = [
    {
      desc: 'no session',
      sessionId: undefined,
      liveStatus: 'inactive',
      sessionState: 'idle',
      expectCanSend: false,
    },
    {
      desc: 'history session',
      sessionId: 's1',
      liveStatus: 'inactive',
      sessionState: 'idle',
      expectCanSend: true,
    },
    {
      desc: 'watching session',
      sessionId: 's1',
      liveStatus: 'cc_owned',
      sessionState: 'idle',
      expectCanSend: false,
    },
    {
      desc: 'active own session',
      sessionId: 's1',
      liveStatus: 'cc_agent_sdk_owned',
      sessionState: 'waiting_input',
      expectCanSend: true,
    },
    {
      desc: 'streaming session',
      sessionId: 's1',
      liveStatus: 'cc_agent_sdk_owned',
      sessionState: 'active',
      expectCanSend: false,
    },
    {
      desc: 'connecting session',
      sessionId: 's1',
      liveStatus: 'inactive',
      sessionState: 'initializing',
      expectCanSend: false,
    },
    {
      desc: 'error session',
      sessionId: 's1',
      liveStatus: 'inactive',
      sessionState: 'error',
      expectCanSend: false,
    },
  ]

  for (const s of scenarios) {
    it(`${s.desc}: produces valid InputBarState`, () => {
      const mode = derivePanelMode(s.sessionId, s.liveStatus, s.sessionState)
      const inputBar = modeToInputBar(mode)
      expect([
        'dormant',
        'active',
        'streaming',
        'waiting_permission',
        'completed',
        'controlled_elsewhere',
        'connecting',
        'reconnecting',
      ]).toContain(inputBar)
    })
  }
})

describe('state transitions (spec compliance)', () => {
  it('BLANK → CONNECTING when user sends message (sessionState changes)', () => {
    expect(derivePanelMode(undefined, 'inactive', 'idle')).toEqual({ mode: 'blank' })
    expect(derivePanelMode('s1', 'inactive', 'initializing')).toEqual({
      mode: 'connecting',
      reason: 'initial',
    })
  })

  it('HISTORY → CONNECTING when user types + send', () => {
    expect(derivePanelMode('s1', 'inactive', 'idle')).toEqual({ mode: 'history' })
    expect(derivePanelMode('s1', 'inactive', 'initializing')).toEqual({
      mode: 'connecting',
      reason: 'initial',
    })
  })

  it('CONNECTING → OWN when session_init received', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'initializing')).toEqual({
      mode: 'connecting',
      reason: 'initial',
    })
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'waiting_input')).toEqual({
      mode: 'own',
      subState: 'active',
    })
  })

  it('OWN(active) → OWN(streaming) when assistant starts', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'waiting_input')).toEqual({
      mode: 'own',
      subState: 'active',
    })
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'active')).toEqual({
      mode: 'own',
      subState: 'streaming',
    })
  })

  it('OWN(streaming) → OWN(waiting_permission) when permission requested', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'active')).toEqual({
      mode: 'own',
      subState: 'streaming',
    })
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'waiting_permission')).toEqual({
      mode: 'own',
      subState: 'waiting_permission',
    })
  })

  it('OWN(*) → HISTORY when session closes', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'active')).toEqual({
      mode: 'own',
      subState: 'streaming',
    })
    expect(derivePanelMode('s1', 'inactive', 'closed')).toEqual({ mode: 'history' })
  })

  it('OWN(*) → CONNECTING(reconnecting) when WS drops', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'active')).toEqual({
      mode: 'own',
      subState: 'streaming',
    })
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'reconnecting')).toEqual({
      mode: 'connecting',
      reason: 'reconnecting',
    })
  })

  it('OWN(*) → ERROR(replaced) when WS close 4001', () => {
    expect(derivePanelMode('s1', 'cc_agent_sdk_owned', 'active')).toEqual({
      mode: 'own',
      subState: 'streaming',
    })
    expect(derivePanelMode('s1', 'inactive', 'replaced')).toEqual({
      mode: 'error',
      reason: 'replaced',
    })
  })

  it('WATCHING → HISTORY when SSE shows session ended', () => {
    expect(derivePanelMode('s1', 'cc_owned', 'idle')).toEqual({ mode: 'watching' })
    expect(derivePanelMode('s1', 'inactive', 'idle')).toEqual({ mode: 'history' })
  })
})

describe('modeToConnectionHealth', () => {
  it('ok for blank', () => {
    expect(modeToConnectionHealth({ mode: 'blank' })).toBe('ok')
  })
  it('ok for history', () => {
    expect(modeToConnectionHealth({ mode: 'history' })).toBe('ok')
  })
  it('ok for own(active)', () => {
    expect(modeToConnectionHealth({ mode: 'own', subState: 'active' })).toBe('ok')
  })
  it('degraded for connecting(reconnecting)', () => {
    expect(modeToConnectionHealth({ mode: 'connecting', reason: 'reconnecting' })).toBe('degraded')
  })
  it('ok for connecting(initial)', () => {
    expect(modeToConnectionHealth({ mode: 'connecting', reason: 'initial' })).toBe('ok')
  })
  it('lost for error(fatal)', () => {
    expect(modeToConnectionHealth({ mode: 'error', reason: 'fatal' })).toBe('lost')
  })
  it('lost for error(replaced)', () => {
    expect(modeToConnectionHealth({ mode: 'error', reason: 'replaced' })).toBe('lost')
  })
})
