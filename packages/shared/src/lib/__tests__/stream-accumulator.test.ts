import { describe, expect, it } from 'vitest'
import { StreamAccumulator } from '../stream-accumulator'

// Helper to create a SequencedEvent with seq number
function makeEvent(seq: number, event: Record<string, unknown>) {
  return { seq, ...event } as any
}

describe('StreamAccumulator — ProgressBlock emission', () => {
  it('tool_progress emits a progress block with bash variant', () => {
    const acc = new StreamAccumulator()
    acc.push(
      makeEvent(1, {
        type: 'session_init',
        model: 'test',
        tools: [],
        permissionMode: 'default',
        mcpServers: [],
        slashCommands: [],
        claudeCodeVersion: '1.0',
        cwd: '/tmp',
        agents: [],
        skills: [],
        outputStyle: 'text',
      }),
    )
    acc.push(
      makeEvent(2, {
        type: 'tool_use_start',
        messageId: 'msg-1',
        toolName: 'Bash',
        toolInput: { command: 'ls' },
        toolUseId: 'tu-1',
        parentToolUseId: null,
      }),
    )
    acc.push(
      makeEvent(3, {
        type: 'tool_progress',
        toolUseId: 'tu-1',
        toolName: 'Bash',
        elapsedSeconds: 2.5,
        parentToolUseId: null,
      }),
    )

    const blocks = acc.getBlocks()
    const progressBlocks = blocks.filter((b) => b.type === 'progress')
    expect(progressBlocks).toHaveLength(1)
    expect(progressBlocks[0]).toMatchObject({
      type: 'progress',
      variant: 'bash',
      category: 'builtin',
      parentToolUseId: 'tu-1',
    })
    // Verify the data shape
    const data = (progressBlocks[0] as any).data
    expect(data.type).toBe('bash')
    expect(data.elapsedTimeSeconds).toBe(2.5)
  })

  it('task_progress emits an agent progress block', () => {
    const acc = new StreamAccumulator()
    acc.push(
      makeEvent(1, {
        type: 'session_init',
        model: 'test',
        tools: [],
        permissionMode: 'default',
        mcpServers: [],
        slashCommands: [],
        claudeCodeVersion: '1.0',
        cwd: '/tmp',
        agents: [],
        skills: [],
        outputStyle: 'text',
      }),
    )
    acc.push(
      makeEvent(2, {
        type: 'task_progress',
        taskId: 'task-1',
        toolUseId: 'tu-agent-1',
        description: 'Working on something...',
        summary: 'Step 3 of 5',
        usage: { totalTokens: 100, toolUses: 5, durationMs: 3000 },
      }),
    )

    const blocks = acc.getBlocks()
    const progressBlocks = blocks.filter((b) => b.type === 'progress')
    expect(progressBlocks).toHaveLength(1)
    expect(progressBlocks[0]).toMatchObject({
      type: 'progress',
      variant: 'agent',
      category: 'agent',
    })
    const data = (progressBlocks[0] as any).data
    expect(data.type).toBe('agent')
    expect(data.agentId).toBe('task-1')
  })

  it('blocks_snapshot is handled explicitly (no unknown SystemBlock)', () => {
    const acc = new StreamAccumulator()
    acc.push(
      makeEvent(1, {
        type: 'session_init',
        model: 'test',
        tools: [],
        permissionMode: 'default',
        mcpServers: [],
        slashCommands: [],
        claudeCodeVersion: '1.0',
        cwd: '/tmp',
        agents: [],
        skills: [],
        outputStyle: 'text',
      }),
    )
    // Should not throw or create an unknown SystemBlock
    acc.push(makeEvent(2, { type: 'blocks_snapshot', blocks: [], lastSeq: 0 }))

    const blocks = acc.getBlocks()
    const unknownBlocks = blocks.filter(
      (b) => b.type === 'system' && (b as any).variant === 'unknown',
    )
    expect(unknownBlocks).toHaveLength(0)
  })

  it('blocks_update is handled explicitly (no unknown SystemBlock)', () => {
    const acc = new StreamAccumulator()
    acc.push(
      makeEvent(1, {
        type: 'session_init',
        model: 'test',
        tools: [],
        permissionMode: 'default',
        mcpServers: [],
        slashCommands: [],
        claudeCodeVersion: '1.0',
        cwd: '/tmp',
        agents: [],
        skills: [],
        outputStyle: 'text',
      }),
    )
    acc.push(makeEvent(2, { type: 'blocks_update', blocks: [] }))

    const blocks = acc.getBlocks()
    const unknownBlocks = blocks.filter(
      (b) => b.type === 'system' && (b as any).variant === 'unknown',
    )
    expect(unknownBlocks).toHaveLength(0)
  })
})
