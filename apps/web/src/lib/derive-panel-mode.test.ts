import { describe, expect, it } from 'vitest'
import {
  derivePanelMode,
  isWatchable,
  modeToConnectionHealth,
  modeToInputBar,
} from './derive-panel-mode'
import type { SessionState } from './derive-panel-mode'
import type { SessionOwnership } from '@claude-view/shared/types/generated/SessionOwnership'

// Helper ownership objects
const tmuxOwnership: SessionOwnership = { tmux: { cliSessionId: 'cv-1' } }
const sdkOwnership: SessionOwnership = { sdk: { controlId: 'ctl-1' } }
const observedOwnership: SessionOwnership = {} // no bindings = observed

describe('isWatchable', () => {
  it('true for tmux ownership', () => expect(isWatchable(tmuxOwnership)).toBe(true))
  it('true for observed ownership (no bindings)', () =>
    expect(isWatchable(observedOwnership)).toBe(true))
  it('false for sdk ownership', () => expect(isWatchable(sdkOwnership)).toBe(false))
  it('false for null', () => expect(isWatchable(null)).toBe(false))
})

describe('derivePanelMode', () => {
  it('returns blank when no sessionId', () => {
    expect(derivePanelMode(undefined, null, 'idle')).toEqual({ mode: 'blank' })
  })

  it('returns watching for tmux ownership', () => {
    expect(derivePanelMode('s1', tmuxOwnership, 'idle')).toEqual({ mode: 'watching' })
  })

  it('returns watching for observed ownership', () => {
    expect(derivePanelMode('s1', observedOwnership, 'idle')).toEqual({ mode: 'watching' })
  })

  it('returns watching regardless of sessionState when tmux', () => {
    expect(derivePanelMode('s1', tmuxOwnership, 'active')).toEqual({ mode: 'watching' })
    expect(derivePanelMode('s1', tmuxOwnership, 'error')).toEqual({ mode: 'watching' })
  })

  it('does not return watching for sdk ownership', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'idle')).toEqual({ mode: 'history' })
  })

  // --- SDK owned: connecting ---
  it('returns connecting(initial) for initializing', () => {
    expect(derivePanelMode('s1', null, 'initializing')).toEqual({
      mode: 'connecting',
      reason: 'initial',
    })
  })

  it('returns connecting(reconnecting) for reconnecting', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'reconnecting')).toEqual({
      mode: 'connecting',
      reason: 'reconnecting',
    })
  })

  // --- SDK owned: connected ---
  it('returns own(active) for waiting_input', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'waiting_input')).toEqual({
      mode: 'own',
      subState: 'active',
    })
  })

  it('returns own(streaming) for active', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'active')).toEqual({
      mode: 'own',
      subState: 'streaming',
    })
  })

  it('returns own(waiting_permission) for waiting_permission', () => {
    expect(derivePanelMode('s1', null, 'waiting_permission')).toEqual({
      mode: 'own',
      subState: 'waiting_permission',
    })
  })

  it('returns own(compacting) for compacting', () => {
    expect(derivePanelMode('s1', null, 'compacting')).toEqual({
      mode: 'own',
      subState: 'compacting',
    })
  })

  // --- SDK owned: failed ---
  it('returns error(fatal) for error', () => {
    expect(derivePanelMode('s1', null, 'error')).toEqual({
      mode: 'error',
      reason: 'fatal',
    })
  })

  it('returns error(replaced) for replaced', () => {
    expect(derivePanelMode('s1', null, 'replaced')).toEqual({
      mode: 'error',
      reason: 'replaced',
    })
  })

  // --- Null ownership (history) ---
  it('returns history for null ownership + idle', () => {
    expect(derivePanelMode('s1', null, 'idle')).toEqual({ mode: 'history' })
  })

  it('returns history for closed', () => {
    expect(derivePanelMode('s1', null, 'closed')).toEqual({ mode: 'history' })
  })

  // --- ownership does not override sessionState for SDK states ---
  it('SDK sessionStates work regardless of ownership (SSE can lag)', () => {
    // SSE says null but sidecar WS is connected
    expect(derivePanelMode('s1', null, 'waiting_input')).toEqual({
      mode: 'own',
      subState: 'active',
    })
    // SSE says sdk and sessionState agrees
    expect(derivePanelMode('s1', sdkOwnership, 'waiting_input')).toEqual({
      mode: 'own',
      subState: 'active',
    })
  })

  // --- Cross-product: sdk ownership with all SDK session states ---
  it('returns connecting(initial) when sdk + initializing', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'initializing')).toEqual({
      mode: 'connecting',
      reason: 'initial',
    })
  })

  it('returns error(fatal) when sdk + error', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'error')).toEqual({
      mode: 'error',
      reason: 'fatal',
    })
  })

  it('returns error(replaced) when sdk + replaced', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'replaced')).toEqual({
      mode: 'error',
      reason: 'replaced',
    })
  })

  it('returns history when sdk + idle (SSE/WS desync)', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'idle')).toEqual({ mode: 'history' })
  })

  it('returns history when sdk + closed', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'closed')).toEqual({ mode: 'history' })
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
    ownership: SessionOwnership | null
    sessionState: SessionState
    expectCanSend: boolean
  }[] = [
    {
      desc: 'no session',
      sessionId: undefined,
      ownership: null,
      sessionState: 'idle',
      expectCanSend: false,
    },
    {
      desc: 'history session',
      sessionId: 's1',
      ownership: null,
      sessionState: 'idle',
      expectCanSend: true,
    },
    {
      desc: 'watching session',
      sessionId: 's1',
      ownership: tmuxOwnership,
      sessionState: 'idle',
      expectCanSend: false,
    },
    {
      desc: 'active own session',
      sessionId: 's1',
      ownership: sdkOwnership,
      sessionState: 'waiting_input',
      expectCanSend: true,
    },
    {
      desc: 'streaming session',
      sessionId: 's1',
      ownership: sdkOwnership,
      sessionState: 'active',
      expectCanSend: false,
    },
    {
      desc: 'connecting session',
      sessionId: 's1',
      ownership: null,
      sessionState: 'initializing',
      expectCanSend: false,
    },
    {
      desc: 'error session',
      sessionId: 's1',
      ownership: null,
      sessionState: 'error',
      expectCanSend: false,
    },
  ]

  for (const s of scenarios) {
    it(`${s.desc}: produces valid InputBarState`, () => {
      const mode = derivePanelMode(s.sessionId, s.ownership, s.sessionState)
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
    expect(derivePanelMode(undefined, null, 'idle')).toEqual({ mode: 'blank' })
    expect(derivePanelMode('s1', null, 'initializing')).toEqual({
      mode: 'connecting',
      reason: 'initial',
    })
  })

  it('HISTORY → CONNECTING when user types + send', () => {
    expect(derivePanelMode('s1', null, 'idle')).toEqual({ mode: 'history' })
    expect(derivePanelMode('s1', null, 'initializing')).toEqual({
      mode: 'connecting',
      reason: 'initial',
    })
  })

  it('CONNECTING → OWN when session_init received', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'initializing')).toEqual({
      mode: 'connecting',
      reason: 'initial',
    })
    expect(derivePanelMode('s1', sdkOwnership, 'waiting_input')).toEqual({
      mode: 'own',
      subState: 'active',
    })
  })

  it('OWN(active) → OWN(streaming) when assistant starts', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'waiting_input')).toEqual({
      mode: 'own',
      subState: 'active',
    })
    expect(derivePanelMode('s1', sdkOwnership, 'active')).toEqual({
      mode: 'own',
      subState: 'streaming',
    })
  })

  it('OWN(streaming) → OWN(waiting_permission) when permission requested', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'active')).toEqual({
      mode: 'own',
      subState: 'streaming',
    })
    expect(derivePanelMode('s1', sdkOwnership, 'waiting_permission')).toEqual({
      mode: 'own',
      subState: 'waiting_permission',
    })
  })

  it('OWN(*) → HISTORY when session closes', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'active')).toEqual({
      mode: 'own',
      subState: 'streaming',
    })
    expect(derivePanelMode('s1', null, 'closed')).toEqual({ mode: 'history' })
  })

  it('OWN(*) → CONNECTING(reconnecting) when WS drops', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'active')).toEqual({
      mode: 'own',
      subState: 'streaming',
    })
    expect(derivePanelMode('s1', sdkOwnership, 'reconnecting')).toEqual({
      mode: 'connecting',
      reason: 'reconnecting',
    })
  })

  it('OWN(*) → ERROR(replaced) when WS close 4001', () => {
    expect(derivePanelMode('s1', sdkOwnership, 'active')).toEqual({
      mode: 'own',
      subState: 'streaming',
    })
    expect(derivePanelMode('s1', null, 'replaced')).toEqual({
      mode: 'error',
      reason: 'replaced',
    })
  })

  it('WATCHING → HISTORY when SSE shows session ended', () => {
    expect(derivePanelMode('s1', tmuxOwnership, 'idle')).toEqual({ mode: 'watching' })
    expect(derivePanelMode('s1', null, 'idle')).toEqual({ mode: 'history' })
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
