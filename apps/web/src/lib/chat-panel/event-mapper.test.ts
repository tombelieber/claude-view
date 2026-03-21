import { describe, expect, test } from 'vitest'
import { mapWsEvent } from './event-mapper'

describe('mapWsEvent', () => {
  // ─── Protocol events → typed RawEvent ─────────────────────

  test('session_init → SESSION_INIT', () => {
    const raw = {
      type: 'session_init',
      model: 'opus',
      permissionMode: 'default',
      slashCommands: [],
      mcpServers: [],
      skills: [],
      agents: [],
      capabilities: [],
    }
    expect(mapWsEvent(raw)).toEqual({
      type: 'SESSION_INIT',
      model: 'opus',
      permissionMode: 'default',
      slashCommands: [],
      mcpServers: [],
      skills: [],
      agents: [],
      capabilities: [],
    })
  })

  test('blocks_snapshot → BLOCKS_SNAPSHOT', () => {
    const blocks = [{ id: 'b1', type: 'text' }]
    expect(mapWsEvent({ type: 'blocks_snapshot', blocks })).toEqual({
      type: 'BLOCKS_SNAPSHOT',
      blocks,
    })
  })

  test('blocks_update → BLOCKS_UPDATE', () => {
    const blocks = [{ id: 'b2', type: 'tool_use' }]
    expect(mapWsEvent({ type: 'blocks_update', blocks })).toEqual({
      type: 'BLOCKS_UPDATE',
      blocks,
    })
  })

  test('stream_delta → STREAM_DELTA', () => {
    expect(mapWsEvent({ type: 'stream_delta', textDelta: 'hello' })).toEqual({
      type: 'STREAM_DELTA',
      text: 'hello',
    })
  })

  test('turn_complete → TURN_COMPLETE with token extraction', () => {
    const raw = {
      type: 'turn_complete',
      blocks: [],
      totalInputTokens: 1000,
      contextWindowSize: 200000,
    }
    expect(mapWsEvent(raw)).toEqual({
      type: 'TURN_COMPLETE',
      blocks: [],
      totalInputTokens: 1000,
      contextWindowSize: 200000,
    })
  })

  test('turn_error → TURN_ERROR', () => {
    const raw = {
      type: 'turn_error',
      blocks: [{ id: 'b3' }],
      totalInputTokens: 500,
      contextWindowSize: 200000,
    }
    expect(mapWsEvent(raw)).toEqual({
      type: 'TURN_ERROR',
      blocks: [{ id: 'b3' }],
      totalInputTokens: 500,
      contextWindowSize: 200000,
    })
  })

  // ─── session_status variants ──────────────────────────────

  test('session_status compacting → SESSION_COMPACTING', () => {
    expect(mapWsEvent({ type: 'session_status', status: 'compacting' })).toEqual({
      type: 'SESSION_COMPACTING',
    })
  })

  test('session_status null → COMPACT_DONE', () => {
    expect(mapWsEvent({ type: 'session_status', status: null })).toEqual({
      type: 'COMPACT_DONE',
    })
  })

  // ─── Permission-like events ───────────────────────────────

  test('permission_request → PERMISSION_REQUEST', () => {
    expect(mapWsEvent({ type: 'permission_request', requestId: 'r1' })).toEqual({
      type: 'PERMISSION_REQUEST',
      kind: 'permission',
      requestId: 'r1',
    })
  })

  test('ask_question → PERMISSION_REQUEST(question)', () => {
    expect(mapWsEvent({ type: 'ask_question', requestId: 'r2' })).toEqual({
      type: 'PERMISSION_REQUEST',
      kind: 'question',
      requestId: 'r2',
    })
  })

  test('plan_approval → PERMISSION_REQUEST(plan)', () => {
    expect(mapWsEvent({ type: 'plan_approval', requestId: 'r3' })).toEqual({
      type: 'PERMISSION_REQUEST',
      kind: 'plan',
      requestId: 'r3',
    })
  })

  test('elicitation → PERMISSION_REQUEST(elicitation)', () => {
    expect(mapWsEvent({ type: 'elicitation', requestId: 'r4' })).toEqual({
      type: 'PERMISSION_REQUEST',
      kind: 'elicitation',
      requestId: 'r4',
    })
  })

  // ─── Session closed ───────────────────────────────────────

  test('session_closed → SESSION_CLOSED', () => {
    expect(mapWsEvent({ type: 'session_closed' })).toEqual({
      type: 'SESSION_CLOSED',
    })
  })

  // ─── E-M5: mode events ───────────────────────────────────

  test('mode_changed → SERVER_MODE_CONFIRMED', () => {
    expect(mapWsEvent({ type: 'mode_changed', mode: 'plan' })).toEqual({
      type: 'SERVER_MODE_CONFIRMED',
      mode: 'plan',
    })
  })

  test('mode_rejected → SERVER_MODE_REJECTED', () => {
    expect(
      mapWsEvent({ type: 'mode_rejected', mode: 'bypassPermissions', reason: 'not allowed' }),
    ).toEqual({
      type: 'SERVER_MODE_REJECTED',
      mode: 'bypassPermissions',
      reason: 'not allowed',
    })
  })

  // ─── query_result variants ────────────────────────────────

  test('query_result commands → COMMANDS_UPDATED', () => {
    expect(
      mapWsEvent({ type: 'query_result', queryType: 'commands', data: ['help', 'clear'] }),
    ).toEqual({
      type: 'COMMANDS_UPDATED',
      commands: ['help', 'clear'],
    })
  })

  test('query_result agents → AGENTS_UPDATED', () => {
    expect(mapWsEvent({ type: 'query_result', queryType: 'agents', data: ['researcher'] })).toEqual(
      {
        type: 'AGENTS_UPDATED',
        agents: ['researcher'],
      },
    )
  })

  // ─── Infrastructure → null ────────────────────────────────

  test('heartbeat_config → null', () => {
    expect(mapWsEvent({ type: 'heartbeat_config' })).toBeNull()
  })

  test('pong → null', () => {
    expect(mapWsEvent({ type: 'pong' })).toBeNull()
  })

  test('error replay_buffer_exhausted → null', () => {
    expect(mapWsEvent({ type: 'error', code: 'replay_buffer_exhausted' })).toBeNull()
  })

  test('unknown type → null', () => {
    expect(mapWsEvent({ type: 'some_unknown_type' })).toBeNull()
  })

  test('query_result other → null', () => {
    expect(mapWsEvent({ type: 'query_result', queryType: 'models', data: [] })).toBeNull()
  })
})
