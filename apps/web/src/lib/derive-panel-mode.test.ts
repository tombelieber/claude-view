import { describe, expect, it } from 'vitest'
import { derivePanelMode, modeToInputBar } from './derive-panel-mode'

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

  it('returns history for unknown sessionState', () => {
    expect(derivePanelMode('s1', 'inactive', 'something_unknown')).toEqual({ mode: 'history' })
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

  it('watching → controlled_elsewhere', () => {
    expect(modeToInputBar({ mode: 'watching' })).toBe('controlled_elsewhere')
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

// Inline copy of the OLD function (snapshot for comparison)
function oldDeriveInputBarState(
  sessionState: string,
  isLive: boolean,
  canResumeLazy?: boolean,
): string {
  if (sessionState === 'replaced') return 'completed'
  if (!isLive) return canResumeLazy ? 'active' : 'dormant'
  switch (sessionState) {
    case 'waiting_input':
      return 'active'
    case 'active':
      return 'streaming'
    case 'waiting_permission':
      return 'waiting_permission'
    case 'compacting':
      return 'streaming'
    case 'initializing':
    case 'connecting':
      return 'connecting'
    case 'reconnecting':
      return 'reconnecting'
    case 'closed':
    case 'completed':
    case 'error':
    case 'fatal':
    case 'failed':
      return 'completed'
    default:
      return 'dormant'
  }
}

describe('behavioral preservation: modeToInputBar(derivePanelMode()) ≡ oldDeriveInputBarState()', () => {
  const realStates = [
    'waiting_input',
    'active',
    'waiting_permission',
    'compacting',
    'initializing',
    'reconnecting',
    'error',
    'replaced',
    'closed',
    'idle',
  ]

  describe('when isLive=true (active session, canResumeLazy=true)', () => {
    for (const state of realStates) {
      it(`sessionState='${state}' produces same InputBarState`, () => {
        // Skip states that can't be isLive=true (closed, idle always have isLive=false)
        if (['closed', 'idle'].includes(state)) return

        const oldResult = oldDeriveInputBarState(state, true, true)
        const panelMode = derivePanelMode('s1', 'cc_agent_sdk_owned', state)
        const newResult = modeToInputBar(panelMode)
        expect(newResult).toBe(oldResult)
      })
    }
  })

  describe('when isLive=false, canResumeLazy=true (dormant resumable)', () => {
    for (const state of ['idle', 'closed']) {
      it(`sessionState='${state}' produces same InputBarState`, () => {
        const oldResult = oldDeriveInputBarState(state, false, true)
        const panelMode = derivePanelMode('s1', 'inactive', state)
        const newResult = modeToInputBar(panelMode)
        expect(newResult).toBe(oldResult) // both return 'active'
      })
    }
  })

  // INTENTIONAL behavioral change documented
  describe('INTENTIONAL change: canResumeLazy=false no longer produces dormant', () => {
    it('OLD: idle + !isLive + !canResumeLazy → dormant', () => {
      expect(oldDeriveInputBarState('idle', false, false)).toBe('dormant')
    })
    it('NEW: idle + inactive → HISTORY → active (auto-resumable)', () => {
      const panelMode = derivePanelMode('s1', 'inactive', 'idle')
      expect(modeToInputBar(panelMode)).toBe('active')
    })
  })

  // replaced handled identically regardless of isLive
  it('replaced state handled identically regardless of isLive', () => {
    expect(oldDeriveInputBarState('replaced', true, true)).toBe('completed')
    expect(oldDeriveInputBarState('replaced', false, true)).toBe('completed')
    expect(modeToInputBar(derivePanelMode('s1', 'inactive', 'replaced'))).toBe('completed')
    expect(modeToInputBar(derivePanelMode('s1', 'cc_agent_sdk_owned', 'replaced'))).toBe(
      'completed',
    )
  })
})

describe('integration: derivePanelMode → modeToInputBar pipeline', () => {
  const scenarios = [
    {
      desc: 'no session',
      sessionId: undefined as string | undefined,
      liveStatus: 'inactive' as const,
      sessionState: 'idle',
      expectCanSend: false,
    },
    {
      desc: 'history session',
      sessionId: 's1',
      liveStatus: 'inactive' as const,
      sessionState: 'idle',
      expectCanSend: true,
    },
    {
      desc: 'watching session',
      sessionId: 's1',
      liveStatus: 'cc_owned' as const,
      sessionState: 'idle',
      expectCanSend: false,
    },
    {
      desc: 'active own session',
      sessionId: 's1',
      liveStatus: 'cc_agent_sdk_owned' as const,
      sessionState: 'waiting_input',
      expectCanSend: true,
    },
    {
      desc: 'streaming session',
      sessionId: 's1',
      liveStatus: 'cc_agent_sdk_owned' as const,
      sessionState: 'active',
      expectCanSend: false,
    },
    {
      desc: 'connecting session',
      sessionId: 's1',
      liveStatus: 'inactive' as const,
      sessionState: 'initializing',
      expectCanSend: false,
    },
    {
      desc: 'error session',
      sessionId: 's1',
      liveStatus: 'inactive' as const,
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
