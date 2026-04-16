// sidecar/src/memory-caps.test.ts
// Regression tests for the per-session memory caps added to StreamAccumulator
// in response to #54. Without these, long-running sessions silently drift past
// the thresholds and the next OOM wave hits.
import { describe, expect, it } from 'vitest'
import { StreamAccumulator } from './stream-accumulator.js'

function pushInit(acc: StreamAccumulator): void {
  acc.push({
    type: 'session_init',
    seq: 0,
    sessionId: 'sess-1',
    tools: ['Read'],
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
}

function pushToolStart(acc: StreamAccumulator, toolUseId: string): void {
  acc.push({
    type: 'tool_use_start',
    seq: 1,
    toolName: 'Bash',
    toolInput: { command: 'cat big.log' },
    toolUseId,
    messageId: 'msg-1',
    parentToolUseId: null,
  })
}

describe('StreamAccumulator memory caps', () => {
  it('truncates oversized tool output with a byte marker', () => {
    const acc = new StreamAccumulator()
    pushInit(acc)
    pushToolStart(acc, 'tool-huge')

    const huge = 'x'.repeat(500_000) // 500 KB — 5x over the 100 KB cap
    acc.push({
      type: 'tool_use_result',
      seq: 2,
      toolUseId: 'tool-huge',
      output: huge,
      isError: false,
      isReplay: false,
    })

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant')
    if (assistant?.type !== 'assistant') throw new Error('expected assistant block')
    const tool = assistant.segments.find((s) => s.kind === 'tool')
    if (tool?.kind !== 'tool') throw new Error('expected tool segment')

    const output = tool.execution.result?.output ?? ''
    expect(output.length).toBeLessThan(huge.length)
    expect(output.length).toBeLessThanOrEqual(100_000)
    expect(output).toContain('[... truncated, original 500000 bytes ...]')
  })

  it('does not truncate tool output that fits within the cap', () => {
    const acc = new StreamAccumulator()
    pushInit(acc)
    pushToolStart(acc, 'tool-small')

    const small = 'normal bash output\n'
    acc.push({
      type: 'tool_use_result',
      seq: 2,
      toolUseId: 'tool-small',
      output: small,
      isError: false,
      isReplay: false,
    })

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant')
    if (assistant?.type !== 'assistant') throw new Error('expected assistant block')
    const tool = assistant.segments.find((s) => s.kind === 'tool')
    if (tool?.kind !== 'tool') throw new Error('expected tool segment')
    expect(tool.execution.result?.output).toBe(small)
  })

  it('truncates oversized tool summary', () => {
    const acc = new StreamAccumulator()
    pushInit(acc)
    pushToolStart(acc, 'tool-a')

    const hugeSummary = 's'.repeat(50_000) // 5x over the 10 KB summary cap
    acc.push({
      type: 'tool_summary',
      seq: 2,
      summary: hugeSummary,
      precedingToolUseIds: ['tool-a'],
    })

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant')
    if (assistant?.type !== 'assistant') throw new Error('expected assistant block')
    const tool = assistant.segments.find((s) => s.kind === 'tool')
    if (tool?.kind !== 'tool') throw new Error('expected tool segment')
    const summary = tool.execution.summary ?? ''
    expect(summary.length).toBeLessThan(hugeSummary.length)
    expect(summary.length).toBeLessThanOrEqual(10_000)
    expect(summary).toContain('[... truncated')
  })

  it('caps pre-init buffer at MAX_PRE_INIT_BUFFER and drops oldest', () => {
    const acc = new StreamAccumulator()
    // Do NOT send session_init — forces buffer accumulation.
    // 1500 events overflow the 1000-entry cap by 500.
    for (let i = 0; i < 1500; i++) {
      acc.push({
        type: 'assistant_text',
        seq: i,
        text: `chunk-${i}`,
        messageId: 'msg-1',
        parentToolUseId: null,
      })
    }
    // Now init — should flush buffered events.
    pushInit(acc)

    // The oldest 500 events were dropped; the remaining 1000 should have flushed
    // into a single assistant block. Concatenated text starts from chunk-500.
    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant')
    if (assistant?.type !== 'assistant') throw new Error('expected assistant block')
    const firstText = assistant.segments[0]
    if (firstText.kind !== 'text') throw new Error('expected text segment')
    expect(firstText.text.startsWith('chunk-500')).toBe(true)
    expect(firstText.text.includes('chunk-1499')).toBe(true)
    expect(firstText.text.includes('chunk-499')).toBe(false)
  })

  it('evicts oldest non-turn_boundary blocks beyond MAX_BLOCKS, keeping turn anchors', () => {
    const acc = new StreamAccumulator()
    pushInit(acc)

    // Emit a single turn_boundary early so we can verify it survives eviction.
    acc.push({
      type: 'turn_complete',
      seq: 1,
      durationMs: 100,
      durationApiMs: 80,
      numTurns: 1,
      totalCostUsd: 0.01,
      modelUsage: {},
      usage: { input_tokens: 10, output_tokens: 5 },
      stopReason: 'end_turn',
      permissionDenials: [],
    })

    // Pump 11_000 system blocks — 1000 past the 10_000 cap.
    for (let i = 0; i < 11_000; i++) {
      acc.push({
        type: 'hook_event',
        seq: i + 2,
        event: 'PreToolUse',
        name: `h-${i}`,
        tool: 'Bash',
      } as never)
    }

    const blocks = acc.getBlocks()
    // Cap is 10_000, so after eviction we should have at most that many blocks.
    expect(blocks.length).toBeLessThanOrEqual(10_000)
    // The turn_boundary must still be present — it's the scroll anchor.
    expect(blocks.some((b) => b.type === 'turn_boundary')).toBe(true)
  })
})
