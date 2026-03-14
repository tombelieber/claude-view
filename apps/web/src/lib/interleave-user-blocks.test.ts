import type {
  AssistantBlock,
  ConversationBlock,
  InteractionBlock,
  NoticeBlock,
  SystemBlock,
  TurnBoundaryBlock,
  UserBlock,
} from '@claude-view/shared/types/blocks'
import { describe, expect, it } from 'vitest'
import { interleaveUserBlocks } from './interleave-user-blocks'

// ── Helpers ──────────────────────────────────────────────────────────────────

function user(text: string, ts = 0): UserBlock {
  return { type: 'user', id: `u-${text}`, text, timestamp: ts, status: 'sent' }
}

function assistant(id: string, text = 'response'): AssistantBlock {
  return {
    type: 'assistant',
    id,
    segments: [{ kind: 'text', text, parentToolUseId: null }],
    streaming: false,
    timestamp: Date.now() / 1000,
  }
}

function streamingAssistant(id: string, text = ''): AssistantBlock {
  return {
    type: 'assistant',
    id,
    segments: text ? [{ kind: 'text', text, parentToolUseId: null }] : [],
    streaming: true,
    timestamp: Date.now() / 1000,
  }
}

function boundary(id: string): TurnBoundaryBlock {
  return {
    type: 'turn_boundary',
    id,
    success: true,
    totalCostUsd: 0.01,
    numTurns: 1,
    durationMs: 1000,
    usage: {},
    modelUsage: {},
    permissionDenials: [],
    stopReason: 'end_turn',
  }
}

function systemBlock(id: string, variant = 'session_init' as const): SystemBlock {
  return {
    type: 'system',
    id,
    variant,
    data: {} as SystemBlock['data'],
  }
}

function interaction(id: string): InteractionBlock {
  return {
    type: 'interaction',
    id,
    variant: 'permission',
    requestId: id,
    resolved: false,
    data: {} as InteractionBlock['data'],
  }
}

function notice(id: string): NoticeBlock {
  return { type: 'notice', id, variant: 'rate_limit', data: null }
}

// ── Tests ────────────────────────────────────────────────────────────────────

describe('interleaveUserBlocks', () => {
  // ── Empty / identity cases ─────────────────────────────────────────────

  it('returns stream blocks unchanged when no user blocks', () => {
    const stream = [systemBlock('s1'), assistant('a1'), boundary('b1')]
    expect(interleaveUserBlocks([], stream)).toBe(stream) // same ref
  })

  it('returns user blocks when stream is empty', () => {
    const users = [user('hello'), user('world')]
    const result = interleaveUserBlocks(users, [])
    expect(result).toEqual(users)
  })

  it('returns empty array when both inputs empty', () => {
    expect(interleaveUserBlocks([], [])).toEqual([])
  })

  // ── Single turn ────────────────────────────────────────────────────────

  it('places user before assistant in a single turn', () => {
    const u = user('hi')
    const a = assistant('a1')
    const b = boundary('b1')

    const result = interleaveUserBlocks([u], [a, b])
    expect(result.map((r) => r.id)).toEqual(['u-hi', 'a1', 'b1'])
  })

  it('places user after system blocks but before first assistant', () => {
    const sys = systemBlock('sys')
    const u = user('hi')
    const a = assistant('a1')
    const b = boundary('b1')

    const result = interleaveUserBlocks([u], [sys, a, b])
    expect(result.map((r) => r.id)).toEqual(['sys', 'u-hi', 'a1', 'b1'])
  })

  // ── Multiple turns ─────────────────────────────────────────────────────

  it('interleaves 3 users with 3 complete turns', () => {
    const users = [user('q1'), user('q2'), user('q3')]
    const stream: ConversationBlock[] = [
      systemBlock('sys'),
      assistant('a1'),
      boundary('b1'),
      assistant('a2'),
      boundary('b2'),
      assistant('a3'),
      boundary('b3'),
    ]

    const result = interleaveUserBlocks(users, stream)
    expect(result.map((r) => r.id)).toEqual([
      'sys',
      'u-q1',
      'a1',
      'b1',
      'u-q2',
      'a2',
      'b2',
      'u-q3',
      'a3',
      'b3',
    ])
  })

  // ── Streaming (incomplete turn) ────────────────────────────────────────

  it('places user before a currently streaming assistant', () => {
    const u = user('q1')
    const stream: ConversationBlock[] = [systemBlock('sys'), streamingAssistant('a1', 'partial')]

    const result = interleaveUserBlocks([u], stream)
    expect(result.map((r) => r.id)).toEqual(['sys', 'u-q1', 'a1'])
  })

  it('places remaining users at end when stream still processing first turn', () => {
    const users = [user('q1'), user('q2'), user('q3')]
    const stream: ConversationBlock[] = [systemBlock('sys'), streamingAssistant('a1')]

    const result = interleaveUserBlocks(users, stream)
    expect(result.map((r) => r.id)).toEqual(['sys', 'u-q1', 'a1', 'u-q2', 'u-q3'])
  })

  // ── Interaction blocks (permission requests) ───────────────────────────

  it('treats interaction as a turn start', () => {
    const u = user('do something')
    const stream: ConversationBlock[] = [interaction('perm1'), assistant('a1'), boundary('b1')]

    const result = interleaveUserBlocks([u], stream)
    expect(result.map((r) => r.id)).toEqual(['u-do something', 'perm1', 'a1', 'b1'])
  })

  it('does not insert extra users for interaction mid-turn', () => {
    const users = [user('q1')]
    // Assistant starts, then requests permission mid-turn, then continues
    const stream: ConversationBlock[] = [
      assistant('a1'),
      interaction('perm1'),
      assistant('a1-cont'),
      boundary('b1'),
    ]

    const result = interleaveUserBlocks(users, stream)
    // user inserted before first assistant (turn start), interaction/continuation are mid-turn
    expect(result.map((r) => r.id)).toEqual(['u-q1', 'a1', 'perm1', 'a1-cont', 'b1'])
  })

  // ── Edge: more turns than users ────────────────────────────────────────

  it('handles more turns than user messages gracefully', () => {
    const users = [user('q1')]
    const stream: ConversationBlock[] = [
      assistant('a1'),
      boundary('b1'),
      assistant('a2'), // no matching user
      boundary('b2'),
    ]

    const result = interleaveUserBlocks(users, stream)
    expect(result.map((r) => r.id)).toEqual(['u-q1', 'a1', 'b1', 'a2', 'b2'])
  })

  // ── Edge: notice blocks between turns ──────────────────────────────────

  it('preserves notice blocks between turns', () => {
    const users = [user('q1'), user('q2')]
    const stream: ConversationBlock[] = [
      assistant('a1'),
      boundary('b1'),
      notice('rate'), // notice between turns
      assistant('a2'),
      boundary('b2'),
    ]

    const result = interleaveUserBlocks(users, stream)
    expect(result.map((r) => r.id)).toEqual(['u-q1', 'a1', 'b1', 'rate', 'u-q2', 'a2', 'b2'])
  })

  // ── Edge: only system/notice blocks, no assistant ──────────────────────

  it('appends users at end when stream has no assistant blocks', () => {
    const users = [user('q1')]
    const stream: ConversationBlock[] = [systemBlock('sys'), notice('n1')]

    const result = interleaveUserBlocks(users, stream)
    expect(result.map((r) => r.id)).toEqual(['sys', 'n1', 'u-q1'])
  })
})
