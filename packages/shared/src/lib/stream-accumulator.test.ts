import { describe, expect, it } from 'vitest'
import type { AssistantBlock, ToolExecution } from '../types/blocks'
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

describe('StreamAccumulator', () => {
  // ── Basic text accumulation ─────────────────────────────────────────────

  it('accumulates assistant_text into one text segment', () => {
    const acc = makeAcc()
    acc.push(ev('assistant_text', { text: 'Hello ', messageId: 'a1', parentToolUseId: null }, 1))
    acc.push(ev('assistant_text', { text: 'world', messageId: 'a1', parentToolUseId: null }, 2))

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock | undefined
    expect(assistant).toBeDefined()
    expect(assistant!.segments).toHaveLength(1)
    expect(assistant!.segments[0]).toEqual({
      kind: 'text',
      text: 'Hello world',
      parentToolUseId: null,
    })
    expect(assistant!.streaming).toBe(true)
  })

  it('creates separate text segments when parentToolUseId changes', () => {
    const acc = makeAcc()
    acc.push(ev('assistant_text', { text: 'top-level', messageId: 'a1', parentToolUseId: null }, 1))
    acc.push(
      ev('assistant_text', { text: 'nested', messageId: 'a1', parentToolUseId: 'tool-1' }, 2),
    )

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock
    expect(assistant.segments).toHaveLength(2)
    expect(assistant.segments[0]).toEqual({
      kind: 'text',
      text: 'top-level',
      parentToolUseId: null,
    })
    expect(assistant.segments[1]).toEqual({
      kind: 'text',
      text: 'nested',
      parentToolUseId: 'tool-1',
    })
  })

  it('accumulates thinking into AssistantBlock.thinking', () => {
    const acc = makeAcc()
    acc.push(
      ev(
        'assistant_thinking',
        { thinking: 'Let me think...', messageId: 'a1', parentToolUseId: null },
        1,
      ),
    )
    acc.push(
      ev(
        'assistant_thinking',
        { thinking: ' More thoughts.', messageId: 'a1', parentToolUseId: null },
        2,
      ),
    )

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock
    expect(assistant.thinking).toBe('Let me think... More thoughts.')
  })

  // ── Tool interleaving ────────────────────────────────────────────────────

  it('interleaves text→tool→text as 3 segments', () => {
    const acc = makeAcc()
    acc.push(ev('assistant_text', { text: 'Before', messageId: 'a1', parentToolUseId: null }, 1))
    acc.push(
      ev(
        'tool_use_start',
        {
          toolName: 'Read',
          toolInput: { file: 'x.ts' },
          toolUseId: 't1',
          messageId: 'a1',
          parentToolUseId: null,
        },
        2,
      ),
    )
    acc.push(ev('assistant_text', { text: 'After', messageId: 'a1', parentToolUseId: null }, 3))

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock
    expect(assistant.segments).toHaveLength(3)
    expect(assistant.segments[0]).toEqual({ kind: 'text', text: 'Before', parentToolUseId: null })
    expect(assistant.segments[1].kind).toBe('tool')
    expect(assistant.segments[2]).toEqual({ kind: 'text', text: 'After', parentToolUseId: null })
  })

  it('tool_use_start creates a running ToolExecution', () => {
    const acc = makeAcc()
    acc.push(
      ev(
        'tool_use_start',
        {
          toolName: 'Bash',
          toolInput: { command: 'ls' },
          toolUseId: 't1',
          messageId: 'a1',
          parentToolUseId: null,
        },
        1,
      ),
    )

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock
    expect(assistant.segments).toHaveLength(1)
    const seg = assistant.segments[0]
    expect(seg.kind).toBe('tool')
    if (seg.kind === 'tool') {
      expect(seg.execution.toolName).toBe('Bash')
      expect(seg.execution.status).toBe('running')
      expect(seg.execution.toolUseId).toBe('t1')
    }
  })

  // ── Tool result pairing ──────────────────────────────────────────────────

  it('tool_use_result attaches to existing ToolExecution by toolUseId', () => {
    const acc = makeAcc()
    acc.push(
      ev(
        'tool_use_start',
        {
          toolName: 'Read',
          toolInput: {},
          toolUseId: 't1',
          messageId: 'a1',
          parentToolUseId: null,
        },
        1,
      ),
    )
    acc.push(
      ev(
        'tool_use_result',
        { toolUseId: 't1', output: 'file contents', isError: false, isReplay: false },
        2,
      ),
    )

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock
    const seg = assistant.segments[0]
    if (seg.kind === 'tool') {
      expect(seg.execution.result).toEqual({
        output: 'file contents',
        isError: false,
        isReplay: false,
      })
      expect(seg.execution.status).toBe('complete')
    }
  })

  it('tool_use_result with isError marks status as error', () => {
    const acc = makeAcc()
    acc.push(
      ev(
        'tool_use_start',
        {
          toolName: 'Bash',
          toolInput: {},
          toolUseId: 't1',
          messageId: 'a1',
          parentToolUseId: null,
        },
        1,
      ),
    )
    acc.push(
      ev(
        'tool_use_result',
        { toolUseId: 't1', output: 'Error!', isError: true, isReplay: false },
        2,
      ),
    )

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock
    const seg = assistant.segments[0]
    if (seg.kind === 'tool') {
      expect(seg.execution.status).toBe('error')
      expect(seg.execution.result?.isError).toBe(true)
    }
  })

  it('tool_use_result with isReplay passes flag through', () => {
    const acc = makeAcc()
    acc.push(
      ev(
        'tool_use_start',
        {
          toolName: 'Read',
          toolInput: {},
          toolUseId: 't1',
          messageId: 'a1',
          parentToolUseId: null,
        },
        1,
      ),
    )
    acc.push(
      ev(
        'tool_use_result',
        { toolUseId: 't1', output: 'cached', isError: false, isReplay: true },
        2,
      ),
    )

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock
    const seg = assistant.segments[0]
    if (seg.kind === 'tool') {
      expect(seg.execution.result?.isReplay).toBe(true)
    }
  })

  it('tool_progress updates elapsedSeconds on existing ToolExecution', () => {
    const acc = makeAcc()
    acc.push(
      ev(
        'tool_use_start',
        {
          toolName: 'Bash',
          toolInput: {},
          toolUseId: 't1',
          messageId: 'a1',
          parentToolUseId: null,
        },
        1,
      ),
    )
    acc.push(
      ev(
        'tool_progress',
        { toolUseId: 't1', toolName: 'Bash', elapsedSeconds: 5, parentToolUseId: null },
        2,
      ),
    )

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock
    const seg = assistant.segments[0]
    if (seg.kind === 'tool') {
      expect(seg.execution.progress).toEqual({ elapsedSeconds: 5 })
    }
  })

  it('tool_summary attaches to all referenced precedingToolUseIds', () => {
    const acc = makeAcc()
    acc.push(
      ev(
        'tool_use_start',
        {
          toolName: 'Read',
          toolInput: {},
          toolUseId: 't1',
          messageId: 'a1',
          parentToolUseId: null,
        },
        1,
      ),
    )
    acc.push(
      ev(
        'tool_use_start',
        {
          toolName: 'Write',
          toolInput: {},
          toolUseId: 't2',
          messageId: 'a1',
          parentToolUseId: null,
        },
        2,
      ),
    )
    acc.push(ev('tool_summary', { summary: 'Did stuff', precedingToolUseIds: ['t1', 't2'] }, 3))

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock
    const tools = assistant.segments
      .filter((s) => s.kind === 'tool')
      .map((s) => (s as { kind: 'tool'; execution: ToolExecution }).execution)
    expect(tools[0].summary).toBe('Did stuff')
    expect(tools[1].summary).toBe('Did stuff')
  })

  // ── Turn lifecycle ────────────────────────────────────────────────────────

  it('turn_complete finalizes AssistantBlock and pushes TurnBoundaryBlock', () => {
    const acc = makeAcc()
    acc.push(ev('assistant_text', { text: 'Done', messageId: 'a1', parentToolUseId: null }, 1))
    acc.push(
      ev(
        'turn_complete',
        {
          totalCostUsd: 0.01,
          numTurns: 1,
          durationMs: 500,
          durationApiMs: 400,
          usage: { input_tokens: 100 },
          modelUsage: {},
          permissionDenials: [],
          result: 'stop',
          stopReason: 'end_turn',
          fastModeState: 'off',
        },
        2,
      ),
    )

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock
    expect(assistant.streaming).toBe(false)

    const boundary = blocks.find((b) => b.type === 'turn_boundary')
    expect(boundary).toBeDefined()
    expect(boundary!.type).toBe('turn_boundary')
    if (boundary?.type === 'turn_boundary') {
      expect(boundary.success).toBe(true)
      expect(boundary.totalCostUsd).toBe(0.01)
      expect(boundary.durationMs).toBe(500)
      expect(boundary.durationApiMs).toBe(400)
    }
  })

  it('turn_error finalizes AssistantBlock and pushes TurnBoundaryBlock with error info', () => {
    const acc = makeAcc()
    acc.push(
      ev('assistant_text', { text: 'In progress', messageId: 'a1', parentToolUseId: null }, 1),
    )
    acc.push(
      ev(
        'turn_error',
        {
          subtype: 'error_max_turns',
          errors: ['Max turns exceeded'],
          permissionDenials: [],
          totalCostUsd: 0.005,
          numTurns: 5,
          durationMs: 2000,
          usage: {},
          modelUsage: {},
          stopReason: null,
        },
        2,
      ),
    )

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock
    expect(assistant.streaming).toBe(false)

    const boundary = blocks.find((b) => b.type === 'turn_boundary')
    expect(boundary?.type).toBe('turn_boundary')
    if (boundary?.type === 'turn_boundary') {
      expect(boundary.success).toBe(false)
      expect(boundary.error?.subtype).toBe('error_max_turns')
      expect(boundary.error?.messages).toEqual(['Max turns exceeded'])
      expect(boundary.durationApiMs).toBeUndefined()
    }
  })

  // ── InteractionBlocks ────────────────────────────────────────────────────

  it('permission_request creates InteractionBlock with variant=permission', () => {
    const acc = makeAcc()
    acc.push(
      ev(
        'permission_request',
        {
          requestId: 'r1',
          toolName: 'Bash',
          toolInput: {},
          toolUseID: 'tu1',
          timeoutMs: 60000,
        },
        1,
      ),
    )

    const blocks = acc.getBlocks()
    const interaction = blocks.find((b) => b.type === 'interaction')
    expect(interaction?.type).toBe('interaction')
    if (interaction?.type === 'interaction') {
      expect(interaction.variant).toBe('permission')
      expect(interaction.requestId).toBe('r1')
      expect(interaction.resolved).toBe(false)
    }
  })

  it('ask_question creates InteractionBlock with variant=question', () => {
    const acc = makeAcc()
    acc.push(ev('ask_question', { requestId: 'r2', questions: [] }, 1))

    const blocks = acc.getBlocks()
    const interaction = blocks.find((b) => b.type === 'interaction')
    expect(interaction?.type).toBe('interaction')
    if (interaction?.type === 'interaction') {
      expect(interaction.variant).toBe('question')
    }
  })

  it('plan_approval creates InteractionBlock with variant=plan', () => {
    const acc = makeAcc()
    acc.push(ev('plan_approval', { requestId: 'r3', planData: {} }, 1))

    const blocks = acc.getBlocks()
    const interaction = blocks.find((b) => b.type === 'interaction')
    expect(interaction?.type === 'interaction' && interaction.variant === 'plan').toBe(true)
  })

  it('elicitation creates InteractionBlock with variant=elicitation', () => {
    const acc = makeAcc()
    acc.push(
      ev(
        'elicitation',
        { requestId: 'r4', toolName: 'mcp', toolInput: {}, prompt: 'Enter value' },
        1,
      ),
    )

    const blocks = acc.getBlocks()
    const interaction = blocks.find((b) => b.type === 'interaction')
    expect(interaction?.type === 'interaction' && interaction.variant === 'elicitation').toBe(true)
  })

  // ── NoticeBlocks ──────────────────────────────────────────────────────────

  it('assistant_error finalizes current AssistantBlock and pushes NoticeBlock', () => {
    const acc = makeAcc()
    acc.push(ev('assistant_text', { text: 'hello', messageId: 'a1', parentToolUseId: null }, 1))
    acc.push(ev('assistant_error', { error: 'rate_limit', messageId: 'a1' }, 2))

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock
    expect(assistant.streaming).toBe(false)

    const notice = blocks.find((b) => b.type === 'notice')
    expect(notice?.type).toBe('notice')
    if (notice?.type === 'notice') {
      expect(notice.variant).toBe('assistant_error')
    }
  })

  it('rate_limit creates NoticeBlock', () => {
    const acc = makeAcc()
    acc.push(ev('rate_limit', { status: 'allowed_warning', utilization: 0.8 }, 1))

    const blocks = acc.getBlocks()
    const notice = blocks.find((b) => b.type === 'notice')
    expect(notice?.type === 'notice' && notice.variant === 'rate_limit').toBe(true)
  })

  it('context_compacted creates NoticeBlock', () => {
    const acc = makeAcc()
    acc.push(ev('context_compacted', { trigger: 'auto', preTokens: 50000 }, 1))

    const notice = acc.getBlocks().find((b) => b.type === 'notice')
    expect(notice?.type === 'notice' && notice.variant === 'context_compacted').toBe(true)
  })

  it('auth_status creates NoticeBlock', () => {
    const acc = makeAcc()
    acc.push(ev('auth_status', { isAuthenticating: true, output: [] }, 1))

    const notice = acc.getBlocks().find((b) => b.type === 'notice')
    expect(notice?.type === 'notice' && notice.variant === 'auth_status').toBe(true)
  })

  it('session_closed creates NoticeBlock', () => {
    const acc = makeAcc()
    acc.push(ev('session_closed', { reason: 'user_exit' }, 1))

    const notice = acc.getBlocks().find((b) => b.type === 'notice')
    expect(notice?.type === 'notice' && notice.variant === 'session_closed').toBe(true)
  })

  it('error event creates NoticeBlock', () => {
    const acc = makeAcc()
    acc.push(ev('error', { message: 'Something went wrong', fatal: false }, 1))

    const notice = acc.getBlocks().find((b) => b.type === 'notice')
    expect(notice?.type === 'notice' && notice.variant === 'error').toBe(true)
  })

  it('prompt_suggestion creates NoticeBlock', () => {
    const acc = makeAcc()
    acc.push(ev('prompt_suggestion', { suggestion: 'Try this prompt' }, 1))

    const notice = acc.getBlocks().find((b) => b.type === 'notice')
    expect(notice?.type === 'notice' && notice.variant === 'prompt_suggestion').toBe(true)
  })

  // ── SystemBlocks ──────────────────────────────────────────────────────────

  it('elicitation_complete creates SystemBlock', () => {
    const acc = makeAcc()
    acc.push(ev('elicitation_complete', { mcpServerName: 'my-mcp', elicitationId: 'e1' }, 1))

    const sys = acc.getBlocks().findLast((b) => b.type === 'system')
    expect(sys?.type === 'system' && sys.variant === 'elicitation_complete').toBe(true)
  })

  it('hook_event creates SystemBlock', () => {
    const acc = makeAcc()
    acc.push(
      ev(
        'hook_event',
        { phase: 'started', hookId: 'h1', hookName: 'pre-push', hookEventName: 'pre-push' },
        1,
      ),
    )

    const sys = acc.getBlocks().findLast((b) => b.type === 'system')
    expect(sys?.type === 'system' && sys.variant === 'hook_event').toBe(true)
  })

  it('task_started creates SystemBlock', () => {
    const acc = makeAcc()
    acc.push(ev('task_started', { taskId: 'task1', description: 'Running analysis' }, 1))

    const sys = acc.getBlocks().findLast((b) => b.type === 'system')
    expect(sys?.type === 'system' && sys.variant === 'task_started').toBe(true)
  })

  it('task_progress creates SystemBlock', () => {
    const acc = makeAcc()
    acc.push(
      ev(
        'task_progress',
        {
          taskId: 'task1',
          description: 'In progress',
          usage: { totalTokens: 100, toolUses: 2, durationMs: 500 },
        },
        1,
      ),
    )

    const sys = acc.getBlocks().findLast((b) => b.type === 'system')
    expect(sys?.type === 'system' && sys.variant === 'task_progress').toBe(true)
  })

  it('task_notification creates SystemBlock', () => {
    const acc = makeAcc()
    acc.push(
      ev(
        'task_notification',
        { taskId: 'task1', status: 'completed', outputFile: 'out.md', summary: 'Done' },
        1,
      ),
    )

    const sys = acc.getBlocks().findLast((b) => b.type === 'system')
    expect(sys?.type === 'system' && sys.variant === 'task_notification').toBe(true)
  })

  it('files_saved creates SystemBlock', () => {
    const acc = makeAcc()
    acc.push(ev('files_saved', { files: [], failed: [], processedAt: '2026-01-01' }, 1))

    const sys = acc.getBlocks().findLast((b) => b.type === 'system')
    expect(sys?.type === 'system' && sys.variant === 'files_saved').toBe(true)
  })

  it('command_output creates SystemBlock', () => {
    const acc = makeAcc()
    acc.push(ev('command_output', { content: 'ls output' }, 1))

    const sys = acc.getBlocks().findLast((b) => b.type === 'system')
    expect(sys?.type === 'system' && sys.variant === 'command_output').toBe(true)
  })

  it('stream_delta creates SystemBlock', () => {
    const acc = makeAcc()
    acc.push(ev('stream_delta', { event: {}, messageId: 'm1' }, 1))

    const sys = acc.getBlocks().findLast((b) => b.type === 'system')
    expect(sys?.type === 'system' && sys.variant === 'stream_delta').toBe(true)
  })

  it('unknown_sdk_event creates SystemBlock with variant=unknown', () => {
    const acc = makeAcc()
    acc.push(ev('unknown_sdk_event', { sdkType: 'weird_event', raw: {} }, 1))

    const sys = acc.getBlocks().findLast((b) => b.type === 'system')
    expect(sys?.type === 'system' && sys.variant === 'unknown').toBe(true)
  })

  it('session_status creates SystemBlock', () => {
    const acc = makeAcc()
    acc.push(ev('session_status', { status: 'compacting' }, 1))

    const sys = acc.getBlocks().findLast((b) => b.type === 'system')
    expect(sys?.type === 'system' && sys.variant === 'session_status').toBe(true)
  })

  // ── Edge cases ────────────────────────────────────────────────────────────

  it('drops duplicate events with seq <= lastProcessedSeq (reconnect dedup)', () => {
    const acc = makeAcc() // seq=0
    acc.push(ev('assistant_text', { text: 'Hello', messageId: 'a1', parentToolUseId: null }, 1))
    // Simulate reconnect replay — seq 1 again, should be dropped
    acc.push(ev('assistant_text', { text: 'Hello', messageId: 'a1', parentToolUseId: null }, 1))
    acc.push(ev('assistant_text', { text: ' World', messageId: 'a1', parentToolUseId: null }, 2))

    const blocks = acc.getBlocks()
    const assistant = blocks.find((b) => b.type === 'assistant') as AssistantBlock
    // Should be 'Hello World' not 'HelloHello World'
    expect(assistant.segments[0]).toEqual({
      kind: 'text',
      text: 'Hello World',
      parentToolUseId: null,
    })
  })

  it('finalize() closes in-progress AssistantBlock with streaming=false', () => {
    const acc = makeAcc()
    acc.push(
      ev('assistant_text', { text: 'Incomplete', messageId: 'a1', parentToolUseId: null }, 1),
    )
    expect((acc.getBlocks().find((b) => b.type === 'assistant') as AssistantBlock).streaming).toBe(
      true,
    )

    const finalBlocks = acc.finalize()
    const assistant = finalBlocks.find((b) => b.type === 'assistant') as AssistantBlock
    expect(assistant.streaming).toBe(false)
  })

  it('events before session_init are buffered and flushed after init', () => {
    const acc = new StreamAccumulator()
    // Push events BEFORE session_init
    acc.push(ev('assistant_text', { text: 'Early', messageId: 'a1', parentToolUseId: null }, 1))
    acc.push(ev('assistant_text', { text: ' message', messageId: 'a1', parentToolUseId: null }, 2))

    // No blocks yet — not initialized
    expect(acc.getBlocks().find((b) => b.type === 'assistant')).toBeUndefined()

    // Now init arrives
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

    // Now buffered events are flushed
    const assistant = acc.getBlocks().find((b) => b.type === 'assistant') as AssistantBlock
    expect(assistant).toBeDefined()
    expect(assistant.segments[0]).toEqual({
      kind: 'text',
      text: 'Early message',
      parentToolUseId: null,
    })
  })

  it('pong events are silently ignored (no block created)', () => {
    const acc = makeAcc()
    acc.push(ev('pong', {}, 1))

    const blocks = acc.getBlocks()
    // Only system block from session_init
    expect(blocks.find((b) => b.type !== 'system')).toBeUndefined()
  })
})
