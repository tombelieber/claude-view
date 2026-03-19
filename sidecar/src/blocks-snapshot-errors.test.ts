// sidecar/src/blocks-snapshot-errors.test.ts
// NEG-01..NEG-05: Negative/error path tests for StreamAccumulator
import { describe, expect, it } from 'vitest'
import { StreamAccumulator } from './stream-accumulator.js'

// ── NEG-04: Accumulator receives malformed event (missing type) ───────
describe('NEG-04: malformed event does not crash accumulator', () => {
  it('push with missing type does not throw, getBlocks returns previous state', () => {
    const acc = new StreamAccumulator()

    // Initialize normally
    acc.push({
      type: 'session_init',
      seq: 0,
      sessionId: 'sess-neg4',
      tools: [],
      model: 'claude-sonnet-4-20250514',
      mcpServers: [],
      permissionMode: 'default',
      slashCommands: [],
      claudeCodeVersion: '1.0.0',
      cwd: '/tmp',
      agents: [],
      skills: [],
      outputStyle: 'text',
    })

    acc.push({
      type: 'assistant_text',
      seq: 1,
      text: 'valid text',
      messageId: 'msg-1',
      parentToolUseId: null,
    })

    const blocksBefore = acc.getBlocks()
    expect(blocksBefore.length).toBeGreaterThan(0)

    // Push malformed event (missing type) — should not throw
    // biome-ignore lint/suspicious/noExplicitAny: test for malformed input
    expect(() => acc.push({ seq: 2 } as any)).not.toThrow()

    // Blocks should be unchanged (malformed event silently ignored)
    const blocksAfter = acc.getBlocks()
    expect(blocksAfter.length).toBe(blocksBefore.length)
  })

  it('push with completely empty object does not throw', () => {
    const acc = new StreamAccumulator()

    // biome-ignore lint/suspicious/noExplicitAny: test for malformed input
    expect(() => acc.push({} as any)).not.toThrow()
    expect(acc.getBlocks()).toEqual([])
  })
})

// ── NEG-05: Large accumulator — 100 turns without error ───────────────
describe('NEG-05: 100 turns without error', () => {
  it('handles 100 complete turns (user + assistant + turn_complete) in <100ms', () => {
    const acc = new StreamAccumulator()
    let seq = 0

    acc.push({
      type: 'session_init',
      seq: seq++,
      sessionId: 'sess-neg5',
      tools: [],
      model: 'claude-sonnet-4-20250514',
      mcpServers: [],
      permissionMode: 'default',
      slashCommands: [],
      claudeCodeVersion: '1.0.0',
      cwd: '/tmp',
      agents: [],
      skills: [],
      outputStyle: 'text',
    })

    const start = performance.now()

    for (let turn = 0; turn < 100; turn++) {
      acc.push({
        type: 'user_message_echo',
        seq: seq++,
        content: `Question ${turn}`,
        timestamp: Date.now() / 1000,
      })

      acc.push({
        type: 'assistant_text',
        seq: seq++,
        text: `Answer to question ${turn}. `.repeat(10),
        messageId: `msg-${turn}`,
        parentToolUseId: null,
      })

      acc.push({
        type: 'turn_complete',
        seq: seq++,
        totalCostUsd: 0.001 * turn,
        numTurns: turn + 1,
        durationMs: 100,
        durationApiMs: 80,
        usage: {},
        modelUsage: {},
        permissionDenials: [],
        result: `done-${turn}`,
        stopReason: null,
      })
    }

    const elapsed = performance.now() - start

    const blocks = acc.getBlocks()
    // Each turn produces: user + assistant + turn_boundary = 3 blocks.
    // Plus 1 system block for session_init.
    // 100 turns * 3 + 1 = 301
    expect(blocks.length).toBe(301)
    expect(elapsed).toBeLessThan(100) // Must complete under 100ms
  })
})

// ── NEG-02 (sidecar side): empty blocks array produces empty state ────
describe('NEG-02 (sidecar): blocks_snapshot with empty blocks', () => {
  it('accumulator starts with empty blocks — no crash on getBlocks', () => {
    const acc = new StreamAccumulator()
    expect(acc.getBlocks()).toEqual([])
    // Finalize on empty state should also be safe
    expect(acc.finalize()).toEqual([])
  })
})

// ── NEG-03 (sidecar side): turn_complete does not wipe accumulated blocks ─
describe('NEG-03 (sidecar): turn_complete preserves accumulated blocks', () => {
  it('blocks persist after turn_complete', () => {
    const acc = new StreamAccumulator()
    let seq = 0

    acc.push({
      type: 'session_init',
      seq: seq++,
      sessionId: 'sess-neg3',
      tools: [],
      model: 'claude-sonnet-4-20250514',
      mcpServers: [],
      permissionMode: 'default',
      slashCommands: [],
      claudeCodeVersion: '1.0.0',
      cwd: '/tmp',
      agents: [],
      skills: [],
      outputStyle: 'text',
    })

    acc.push({
      type: 'assistant_text',
      seq: seq++,
      text: 'Hello',
      messageId: 'msg-1',
      parentToolUseId: null,
    })

    acc.push({
      type: 'assistant_text',
      seq: seq++,
      text: ' world',
      messageId: 'msg-1',
      parentToolUseId: null,
    })

    const blocksBefore = acc.getBlocks()
    expect(blocksBefore.length).toBe(2) // system + assistant(streaming)

    // Fire turn_complete
    acc.push({
      type: 'turn_complete',
      seq: seq++,
      totalCostUsd: 0.01,
      numTurns: 1,
      durationMs: 100,
      durationApiMs: 80,
      usage: {},
      modelUsage: {},
      permissionDenials: [],
      result: 'done',
      stopReason: null,
    })

    const blocksAfter = acc.getBlocks()
    // Should now have: system + assistant(finalized) + turn_boundary = 3
    expect(blocksAfter.length).toBe(3)
    // The assistant block should still exist (not cleared)
    const assistants = blocksAfter.filter((b) => b.type === 'assistant')
    expect(assistants.length).toBe(1)
  })
})
