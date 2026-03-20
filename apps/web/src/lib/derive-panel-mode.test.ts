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
