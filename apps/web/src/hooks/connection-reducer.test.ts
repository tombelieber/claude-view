import { describe, expect, it } from 'vitest'
import { type ConnectionState, connectionReducer } from './connection-reducer'

const idle: ConnectionState = { phase: 'idle' }

describe('connectionReducer', () => {
  it('idle + connect → connecting', () => {
    const next = connectionReducer(idle, { type: 'connect', sessionId: 's1' })
    expect(next).toEqual({ phase: 'connecting', sessionId: 's1' })
  })

  it('connecting + ws_open → active', () => {
    const state: ConnectionState = { phase: 'connecting', sessionId: 's1' }
    const next = connectionReducer(state, { type: 'ws_open' })
    expect(next).toEqual({ phase: 'active', sessionId: 's1', lastSeq: -1 })
  })

  it('connecting + ws_close → failed (no retry on first connect)', () => {
    const state: ConnectionState = { phase: 'connecting', sessionId: 's1' }
    const next = connectionReducer(state, { type: 'ws_close', code: 1006, reason: '' })
    expect(next.phase).toBe('failed')
  })

  it('active + fatal error → fatal', () => {
    const state: ConnectionState = { phase: 'active', sessionId: 's1', lastSeq: 5 }
    const next = connectionReducer(state, {
      type: 'ws_message',
      msg: { type: 'error', message: 'Session not found', fatal: true },
      seq: 6,
    })
    expect(next.phase).toBe('fatal')
    expect((next as { reason: string }).reason).toBe('Session not found')
  })

  it('fatal + ws_close → stays fatal (no override)', () => {
    const state: ConnectionState = { phase: 'fatal', sessionId: 's1', reason: 'dead' }
    const next = connectionReducer(state, { type: 'ws_close', code: 1000, reason: '' })
    expect(next.phase).toBe('fatal')
    expect(next).toBe(state) // same reference — no state change
  })

  it('active + recoverable close → reconnecting', () => {
    const state: ConnectionState = { phase: 'active', sessionId: 's1', lastSeq: 10 }
    const next = connectionReducer(state, { type: 'ws_close', code: 1006, reason: '' })
    expect(next).toEqual({
      phase: 'reconnecting',
      sessionId: 's1',
      attempt: 1,
      lastSeq: 10,
    })
  })

  it('active + non-recoverable close → fatal', () => {
    const state: ConnectionState = { phase: 'active', sessionId: 's1', lastSeq: 10 }
    const next = connectionReducer(state, {
      type: 'ws_close',
      code: 4004,
      reason: 'session_not_found',
    })
    expect(next.phase).toBe('fatal')
  })

  it('reconnecting increments attempt on close', () => {
    const state: ConnectionState = {
      phase: 'reconnecting',
      sessionId: 's1',
      attempt: 3,
      lastSeq: 10,
    }
    const next = connectionReducer(state, { type: 'ws_close', code: 1006, reason: '' })
    expect(next).toEqual({
      phase: 'reconnecting',
      sessionId: 's1',
      attempt: 4,
      lastSeq: 10,
    })
  })

  it('reconnecting → failed after max attempts', () => {
    const state: ConnectionState = {
      phase: 'reconnecting',
      sessionId: 's1',
      attempt: 10,
      lastSeq: 10,
    }
    const next = connectionReducer(state, { type: 'ws_close', code: 1006, reason: '' })
    expect(next.phase).toBe('failed')
  })

  it('reconnecting + ws_open → active (preserves lastSeq)', () => {
    const state: ConnectionState = {
      phase: 'reconnecting',
      sessionId: 's1',
      attempt: 2,
      lastSeq: 42,
    }
    const next = connectionReducer(state, { type: 'ws_open' })
    expect(next).toEqual({ phase: 'active', sessionId: 's1', lastSeq: 42 })
  })

  it('active + ws_message updates lastSeq', () => {
    const state: ConnectionState = { phase: 'active', sessionId: 's1', lastSeq: 5 }
    const next = connectionReducer(state, {
      type: 'ws_message',
      msg: { type: 'pong' },
      seq: 6,
    })
    expect((next as { lastSeq: number }).lastSeq).toBe(6)
  })

  it('active + session_closed → completed', () => {
    const state: ConnectionState = { phase: 'active', sessionId: 's1', lastSeq: 5 }
    const next = connectionReducer(state, {
      type: 'ws_message',
      msg: { type: 'session_closed', reason: 'done' },
      seq: 6,
    })
    expect(next.phase).toBe('completed')
  })

  it('failed + connect → connecting (user retry)', () => {
    const state: ConnectionState = { phase: 'failed', sessionId: 's1', reason: 'timeout' }
    const next = connectionReducer(state, { type: 'connect', sessionId: 's1' })
    expect(next.phase).toBe('connecting')
  })

  it('reset → idle', () => {
    const state: ConnectionState = { phase: 'active', sessionId: 's1', lastSeq: 99 }
    const next = connectionReducer(state, { type: 'reset' })
    expect(next).toEqual({ phase: 'idle' })
  })

  it('active + ws_error → stays active (informational, ws_close follows)', () => {
    const state: ConnectionState = { phase: 'active', sessionId: 's1', lastSeq: 5 }
    const next = connectionReducer(state, { type: 'ws_error', error: 'network error' })
    expect(next).toBe(state) // same reference — no state change
  })

  it('active + connect → stays active (no-op)', () => {
    const state: ConnectionState = { phase: 'active', sessionId: 's1', lastSeq: 5 }
    const next = connectionReducer(state, { type: 'connect', sessionId: 's2' })
    expect(next).toBe(state) // already connected, ignore
  })

  it('completed + connect → connecting (start new session)', () => {
    const state: ConnectionState = { phase: 'completed', sessionId: 's1' }
    const next = connectionReducer(state, { type: 'connect', sessionId: 's1' })
    expect(next).toEqual({ phase: 'connecting', sessionId: 's1' })
  })

  it('connecting + non-recoverable close → fatal (not just failed)', () => {
    const state: ConnectionState = { phase: 'connecting', sessionId: 's1' }
    const next = connectionReducer(state, {
      type: 'ws_close',
      code: 4004,
      reason: 'session_not_found',
    })
    expect(next.phase).toBe('fatal')
    expect((next as { code: number }).code).toBe(4004)
  })

  it('active + heartbeat timeout (4200) → reconnecting', () => {
    const state: ConnectionState = { phase: 'active', sessionId: 's1', lastSeq: 15 }
    const next = connectionReducer(state, {
      type: 'ws_close',
      code: 4200,
      reason: 'heartbeat_timeout',
    })
    expect(next).toEqual({
      phase: 'reconnecting',
      sessionId: 's1',
      attempt: 1,
      lastSeq: 15,
    })
  })
})
