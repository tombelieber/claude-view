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

  it('places user after completed single turn (boundary present)', () => {
    const u = user('hi')
    const a = assistant('a1')
    const b = boundary('b1')

    const result = interleaveUserBlocks([u], [a, b])
    // Turn is complete (has boundary), user is new → goes at end
    expect(result.map((r) => r.id)).toEqual(['a1', 'b1', 'u-hi'])
  })

  it('places user before assistant in active turn (no boundary)', () => {
    const u = user('hi')
    const a = assistant('a1')

    const result = interleaveUserBlocks([u], [a])
    // No boundary → active turn → user placed before assistant
    expect(result.map((r) => r.id)).toEqual(['u-hi', 'a1'])
  })

  it('places user after system blocks but before first assistant (no boundary)', () => {
    const sys = systemBlock('sys')
    const u = user('hi')
    const a = assistant('a1')

    const result = interleaveUserBlocks([u], [sys, a])
    expect(result.map((r) => r.id)).toEqual(['sys', 'u-hi', 'a1'])
  })

  // ── Multiple turns ─────────────────────────────────────────────────────

  it('places optimistic users AFTER all completed turns (replay-safe)', () => {
    // On replay, stream has 3 complete turns. Optimistic messages are NEW — they belong
    // after the last boundary, not interleaved with replayed turns.
    const users = [user('new-q')]
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
    // User message goes at the end (after all completed turns)
    expect(result.map((r) => r.id)).toEqual(['sys', 'a1', 'b1', 'a2', 'b2', 'a3', 'b3', 'u-new-q'])
  })

  it('places user before assistant in latest turn after completed turns', () => {
    // Live session: 2 completed turns + new turn starting
    const users = [user('q3')]
    const stream: ConversationBlock[] = [
      assistant('a1'),
      boundary('b1'),
      assistant('a2'),
      boundary('b2'),
      assistant('a3'), // new turn starting
    ]

    const result = interleaveUserBlocks(users, stream)
    expect(result.map((r) => r.id)).toEqual(['a1', 'b1', 'a2', 'b2', 'u-q3', 'a3'])
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

  it('treats interaction as a turn start (completed turn → user at end)', () => {
    const u = user('do something')
    const stream: ConversationBlock[] = [interaction('perm1'), assistant('a1'), boundary('b1')]

    const result = interleaveUserBlocks([u], stream)
    // b1 is last boundary, so user goes at end (completed turn)
    expect(result.map((r) => r.id)).toEqual(['perm1', 'a1', 'b1', 'u-do something'])
  })

  it('treats interaction as a turn start in active turn (no boundary)', () => {
    const u = user('do something')
    const stream: ConversationBlock[] = [interaction('perm1'), assistant('a1')]

    const result = interleaveUserBlocks([u], stream)
    // No boundary → lastBoundaryIdx=-1 → user placed before first assistant/interaction
    expect(result.map((r) => r.id)).toEqual(['u-do something', 'perm1', 'a1'])
  })

  it('does not insert extra users for interaction mid-turn', () => {
    const users = [user('q1')]
    // Completed turn with interaction mid-turn
    const stream: ConversationBlock[] = [
      assistant('a1'),
      interaction('perm1'),
      assistant('a1-cont'),
      boundary('b1'),
    ]

    const result = interleaveUserBlocks(users, stream)
    // All completed (b1 is last boundary), user goes at end
    expect(result.map((r) => r.id)).toEqual(['a1', 'perm1', 'a1-cont', 'b1', 'u-q1'])
  })

  // ── Edge: more turns than users ────────────────────────────────────────

  it('handles more turns than user messages gracefully', () => {
    const users = [user('q1')]
    const stream: ConversationBlock[] = [
      assistant('a1'),
      boundary('b1'),
      assistant('a2'),
      boundary('b2'),
    ]

    const result = interleaveUserBlocks(users, stream)
    // All turns are completed (last boundary = b2), user goes at end
    expect(result.map((r) => r.id)).toEqual(['a1', 'b1', 'a2', 'b2', 'u-q1'])
  })

  // ── Edge: notice blocks between turns ──────────────────────────────────

  it('preserves notice blocks between turns, users only in latest turn', () => {
    // 1 completed turn + 1 new turn with notice between
    const users = [user('q2')]
    const stream: ConversationBlock[] = [
      assistant('a1'),
      boundary('b1'),
      notice('rate'), // notice between turns
      assistant('a2'),
      boundary('b2'),
    ]

    const result = interleaveUserBlocks(users, stream)
    // User placed after last boundary (b2 is last), so appended at end
    expect(result.map((r) => r.id)).toEqual(['a1', 'b1', 'rate', 'a2', 'b2', 'u-q2'])
  })

  it('preserves notice blocks with user in active latest turn', () => {
    const users = [user('q2')]
    const stream: ConversationBlock[] = [
      assistant('a1'),
      boundary('b1'),
      notice('rate'),
      assistant('a2'), // latest turn (no boundary after)
    ]

    const result = interleaveUserBlocks(users, stream)
    expect(result.map((r) => r.id)).toEqual(['a1', 'b1', 'rate', 'u-q2', 'a2'])
  })

  // ── Edge: only system/notice blocks, no assistant ──────────────────────

  it('appends users at end when stream has no assistant blocks', () => {
    const users = [user('q1')]
    const stream: ConversationBlock[] = [systemBlock('sys'), notice('n1')]

    const result = interleaveUserBlocks(users, stream)
    expect(result.map((r) => r.id)).toEqual(['sys', 'n1', 'u-q1'])
  })
})
