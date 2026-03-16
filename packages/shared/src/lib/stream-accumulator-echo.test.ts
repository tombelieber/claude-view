import { describe, expect, it } from 'vitest'
import type { UserBlock } from '../types/blocks'
import type { SequencedEvent } from '../types/sidecar-protocol'
import { StreamAccumulator } from './stream-accumulator'

// Helper to build typed sequenced events concisely
function ev<T extends object>(type: string, fields: T, seq: number): SequencedEvent {
  return { type, ...fields, seq } as unknown as SequencedEvent
}

function makeAcc() {
  const acc = new StreamAccumulator()
  acc.push(
    ev(
      'session_init',
      {
        tools: [],
        model: 'claude-sonnet-4-20250514',
        mcpServers: [],
        permissionMode: 'default',
        slashCommands: [],
        claudeCodeVersion: '1.0.0',
        cwd: '/',
        agents: [],
        skills: [],
        outputStyle: 'default',
      },
      0,
    ),
  )
  return acc
}

describe('StreamAccumulator — user_message_echo', () => {
  it('produces UserBlock from echo event', () => {
    const acc = makeAcc()
    acc.push(ev('user_message_echo', { content: 'Hello Claude', timestamp: 1710000000 }, 1))

    const blocks = acc.getBlocks()
    const user = blocks.find((b) => b.type === 'user') as UserBlock | undefined
    expect(user).toBeDefined()
    expect(user!.type).toBe('user')
    expect(user!.text).toBe('Hello Claude')
    expect(user!.timestamp).toBe(1710000000)
    expect(user!.id).toBe('user-1')
  })

  it('bypasses init gate — echo at seq 0 before session_init renders immediately', () => {
    const acc = new StreamAccumulator()
    // Push echo BEFORE session_init
    acc.push(ev('user_message_echo', { content: 'Early message', timestamp: 1710000000 }, 0))

    // Should render immediately without waiting for session_init
    const blocks = acc.getBlocks()
    const user = blocks.find((b) => b.type === 'user') as UserBlock | undefined
    expect(user).toBeDefined()
    expect(user!.text).toBe('Early message')
  })

  it('deduplicates on reconnect replay (same seq ignored)', () => {
    const acc = makeAcc()
    acc.push(ev('user_message_echo', { content: 'Hello', timestamp: 1710000000 }, 1))
    // Simulate reconnect replay — same seq
    acc.push(ev('user_message_echo', { content: 'Hello', timestamp: 1710000000 }, 1))

    const blocks = acc.getBlocks()
    const userBlocks = blocks.filter((b) => b.type === 'user')
    expect(userBlocks).toHaveLength(1)
  })

  it('echo after session_init produces UserBlock in correct position', () => {
    const acc = makeAcc()
    acc.push(
      ev('assistant_text', { text: 'Response 1', messageId: 'a1', parentToolUseId: null }, 1),
    )
    acc.push(
      ev(
        'turn_complete',
        {
          totalCostUsd: 0.01,
          numTurns: 1,
          durationMs: 500,
          durationApiMs: 400,
          usage: {},
          modelUsage: {},
          permissionDenials: [],
          result: 'stop',
          stopReason: 'end_turn',
        },
        2,
      ),
    )
    acc.push(ev('user_message_echo', { content: 'Follow-up', timestamp: 1710000001 }, 3))

    const blocks = acc.getBlocks()
    // UserBlock should be after the turn_boundary
    const userIdx = blocks.findIndex((b) => b.type === 'user')
    const boundaryIdx = blocks.findIndex((b) => b.type === 'turn_boundary')
    expect(userIdx).toBeGreaterThan(boundaryIdx)
    expect((blocks[userIdx] as UserBlock).text).toBe('Follow-up')
  })

  it('multiple echoes in multi-turn appear in order', () => {
    const acc = makeAcc()

    // Turn 1: user echo -> assistant -> turn complete
    acc.push(ev('user_message_echo', { content: 'First question', timestamp: 1710000000 }, 1))
    acc.push(ev('assistant_text', { text: 'Answer 1', messageId: 'a1', parentToolUseId: null }, 2))
    acc.push(
      ev(
        'turn_complete',
        {
          totalCostUsd: 0.01,
          numTurns: 1,
          durationMs: 500,
          durationApiMs: 400,
          usage: {},
          modelUsage: {},
          permissionDenials: [],
          result: 'stop',
          stopReason: 'end_turn',
        },
        3,
      ),
    )

    // Turn 2: user echo -> assistant -> turn complete
    acc.push(ev('user_message_echo', { content: 'Second question', timestamp: 1710000001 }, 4))
    acc.push(ev('assistant_text', { text: 'Answer 2', messageId: 'a2', parentToolUseId: null }, 5))
    acc.push(
      ev(
        'turn_complete',
        {
          totalCostUsd: 0.02,
          numTurns: 2,
          durationMs: 1000,
          durationApiMs: 800,
          usage: {},
          modelUsage: {},
          permissionDenials: [],
          result: 'stop',
          stopReason: 'end_turn',
        },
        6,
      ),
    )

    const blocks = acc.getBlocks()
    const userBlocks = blocks.filter((b) => b.type === 'user') as UserBlock[]
    expect(userBlocks).toHaveLength(2)
    expect(userBlocks[0].text).toBe('First question')
    expect(userBlocks[1].text).toBe('Second question')

    // Verify order: user1, assistant1, boundary1, user2, assistant2, boundary2
    const types = blocks.map((b) => b.type)
    const user1Idx = types.indexOf('user')
    const assistant1Idx = types.indexOf('assistant')
    const user2Idx = types.lastIndexOf('user')
    const assistant2Idx = types.lastIndexOf('assistant')
    expect(user1Idx).toBeLessThan(assistant1Idx)
    expect(user2Idx).toBeLessThan(assistant2Idx)
    expect(user2Idx).toBeGreaterThan(assistant1Idx)
  })
})
