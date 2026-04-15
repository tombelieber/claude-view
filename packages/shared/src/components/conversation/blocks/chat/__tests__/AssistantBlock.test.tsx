import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { AssistantBlock, ToolExecution } from '../../../../../types/blocks'
import { ChatAssistantBlock, isChatAssistantBlockEmpty } from '../AssistantBlock'

// ── Test fixtures ────────────────────────────────────────────────────────────

function makeAssistant(overrides: Partial<AssistantBlock> = {}): AssistantBlock {
  return {
    type: 'assistant',
    id: 'a-1',
    segments: [],
    streaming: false,
    timestamp: 1_700_000_000,
    ...overrides,
  }
}

function textSeg(text: string): AssistantBlock['segments'][number] {
  return { kind: 'text', text }
}

function toolSeg(toolName = 'Bash'): AssistantBlock['segments'][number] {
  const execution: ToolExecution = {
    toolName,
    toolInput: { command: 'echo hi' },
    toolUseId: 'tu-1',
    status: 'complete',
    result: { output: 'hi', isError: false, isReplay: false },
  }
  return { kind: 'tool', execution }
}

// ── isChatAssistantBlockEmpty predicate ──────────────────────────────────────
//
// The predicate encodes the judgment call of "what counts as empty content in
// the chat view". Keep these tests pure / render-free — they run in < 1ms and
// pin the intended contract. Integration-level rendering tests live below.

describe('isChatAssistantBlockEmpty', () => {
  it('returns true for no segments, no thinking, not streaming', () => {
    expect(isChatAssistantBlockEmpty(makeAssistant())).toBe(true)
  })

  it('returns true for whitespace-only text segment', () => {
    expect(isChatAssistantBlockEmpty(makeAssistant({ segments: [textSeg('   \n\t')] }))).toBe(true)
  })

  it('returns true for segments containing only empty text placeholders', () => {
    // Produced by `content_block_start` before the first delta arrives.
    expect(isChatAssistantBlockEmpty(makeAssistant({ segments: [textSeg('')] }))).toBe(true)
  })

  it('returns true when agentId is set but no content exists', () => {
    // A bare agent badge does not justify a row.
    expect(isChatAssistantBlockEmpty(makeAssistant({ agentId: 'research-agent' }))).toBe(true)
  })

  it('returns true when isSidechain is true but no content exists', () => {
    expect(isChatAssistantBlockEmpty(makeAssistant({ isSidechain: true }))).toBe(true)
  })

  it('returns false when currently streaming (live cursor is meaningful)', () => {
    expect(isChatAssistantBlockEmpty(makeAssistant({ streaming: true }))).toBe(false)
  })

  it('returns false when thinking has content', () => {
    expect(
      isChatAssistantBlockEmpty(makeAssistant({ thinking: 'Let me reason about this…' })),
    ).toBe(false)
  })

  it('returns false when any text segment is non-empty', () => {
    expect(isChatAssistantBlockEmpty(makeAssistant({ segments: [textSeg('Hello!')] }))).toBe(false)
  })

  it('returns false when any tool segment exists (even with other empty text)', () => {
    expect(
      isChatAssistantBlockEmpty(makeAssistant({ segments: [textSeg(''), toolSeg('Read')] })),
    ).toBe(false)
  })
})

// ── ChatAssistantBlock rendering ─────────────────────────────────────────────

describe('ChatAssistantBlock — empty-block suppression', () => {
  it('renders null for an empty block (no timestamp row appears)', () => {
    const { container } = render(<ChatAssistantBlock block={makeAssistant()} />)
    expect(container.firstChild).toBeNull()
  })

  it('renders null for a whitespace-only text segment', () => {
    const { container } = render(
      <ChatAssistantBlock block={makeAssistant({ segments: [textSeg('  ')] })} />,
    )
    expect(container.firstChild).toBeNull()
  })

  it('renders a normal block with text (regression guard)', () => {
    render(<ChatAssistantBlock block={makeAssistant({ segments: [textSeg('Hi there')] })} />)
    expect(screen.getByText('Hi there')).toBeInTheDocument()
  })

  it('renders a streaming block even with no segments (cursor is live signal)', () => {
    const { container } = render(<ChatAssistantBlock block={makeAssistant({ streaming: true })} />)
    expect(container.firstChild).not.toBeNull()
  })

  it('renders a block with tool-only content', () => {
    const { container } = render(
      <ChatAssistantBlock block={makeAssistant({ segments: [toolSeg('Bash')] })} />,
    )
    expect(container.firstChild).not.toBeNull()
  })
})
