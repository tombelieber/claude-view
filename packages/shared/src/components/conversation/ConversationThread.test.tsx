import type { ConversationBlock } from '../../types/blocks'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ConversationThread, getBlockFineCategories } from './ConversationThread'
import type { BlockRenderers } from './types'

// Virtuoso doesn't render in jsdom — mock it as a plain list
vi.mock('react-virtuoso', () => ({
  Virtuoso: ({
    data,
    itemContent,
    computeItemKey,
  }: {
    data: unknown[]
    itemContent: (index: number, item: unknown) => React.ReactNode
    computeItemKey?: (index: number, item: unknown) => string | number
  }) => (
    <div data-testid="virtuoso-mock">
      {data.map((item, i) => (
        <div key={computeItemKey ? computeItemKey(i, item) : i}>{itemContent(i, item)}</div>
      ))}
    </div>
  ),
}))

const mockRenderers: BlockRenderers = {
  user: ({ block }) => <div data-testid={`block-${block.id}`}>user-{block.id}</div>,
  assistant: ({ block }) => <div data-testid={`block-${block.id}`}>assistant-{block.id}</div>,
  notice: ({ block }) => <div data-testid={`block-${block.id}`}>notice-{block.id}</div>,
}

const mockBlocks: ConversationBlock[] = [
  { type: 'user', id: 'u1', text: 'hello', timestamp: 1000 },
  { type: 'assistant', id: 'a1', segments: [], streaming: false },
  { type: 'user', id: 'u2', text: 'world', timestamp: 1001 },
  { type: 'notice', id: 'n1', variant: 'error', data: null },
]

describe('ConversationThread filterBar integration', () => {
  it('does NOT render filter bar when filterBar prop is omitted', () => {
    render(<ConversationThread blocks={mockBlocks} renderers={mockRenderers} />)
    expect(screen.queryByText(/All/)).not.toBeInTheDocument()
  })

  it('does NOT render filter bar when filterBar is false', () => {
    render(<ConversationThread blocks={mockBlocks} renderers={mockRenderers} filterBar={false} />)
    expect(screen.queryByText(/All/)).not.toBeInTheDocument()
  })

  it('renders filter bar when filterBar={true}', () => {
    render(<ConversationThread blocks={mockBlocks} renderers={mockRenderers} filterBar={true} />)
    expect(screen.getByText(/All/)).toBeInTheDocument()
    // Fine categories include User with count 2
    expect(screen.getByText('User')).toBeInTheDocument()
    expect(screen.getByText('2')).toBeInTheDocument()
  })

  it('filtering hides blocks of other types', () => {
    render(<ConversationThread blocks={mockBlocks} renderers={mockRenderers} filterBar={true} />)

    // All blocks visible initially
    expect(screen.getByTestId('block-u1')).toBeInTheDocument()
    expect(screen.getByTestId('block-a1')).toBeInTheDocument()
    expect(screen.getByTestId('block-n1')).toBeInTheDocument()

    // Click "User" chip to filter — shows only user blocks
    fireEvent.click(screen.getByText('User'))

    expect(screen.getByTestId('block-u1')).toBeInTheDocument()
    expect(screen.getByTestId('block-u2')).toBeInTheDocument()
    expect(screen.queryByTestId('block-a1')).not.toBeInTheDocument()
    expect(screen.queryByTestId('block-n1')).not.toBeInTheDocument()
  })

  it('renders all blocks via Virtuoso', () => {
    render(<ConversationThread blocks={mockBlocks} renderers={mockRenderers} />)
    expect(screen.getByTestId('virtuoso-mock')).toBeInTheDocument()
    expect(screen.getByTestId('block-u1')).toBeInTheDocument()
    expect(screen.getByTestId('block-a1')).toBeInTheDocument()
    expect(screen.getByTestId('block-u2')).toBeInTheDocument()
    expect(screen.getByTestId('block-n1')).toBeInTheDocument()
  })
})

describe('getBlockFineCategories', () => {
  it('maps user block to "user"', () => {
    const block: ConversationBlock = { type: 'user', id: 'u1', text: 'hello', timestamp: 1000 }
    expect(getBlockFineCategories(block)).toEqual(['user'])
  })

  it('maps text-only assistant to "assistant"', () => {
    const block: ConversationBlock = {
      type: 'assistant',
      id: 'a1',
      segments: [{ kind: 'text', text: 'hello' }],
      streaming: false,
    }
    expect(getBlockFineCategories(block)).toEqual(['assistant'])
  })

  it('maps assistant with MCP tool to ["mcp", "assistant"]', () => {
    const block: ConversationBlock = {
      type: 'assistant',
      id: 'a1',
      segments: [
        { kind: 'text', text: 'Let me search...' },
        {
          kind: 'tool',
          execution: {
            toolName: 'mcp__server__search',
            toolInput: {},
            toolUseId: 't1',
            status: 'complete',
            category: 'mcp',
          },
        },
      ],
      streaming: false,
    }
    expect(getBlockFineCategories(block)).toEqual(['mcp', 'assistant'])
  })

  it('maps assistant with only tools (no text) to tool categories only', () => {
    const block: ConversationBlock = {
      type: 'assistant',
      id: 'a1',
      segments: [
        {
          kind: 'tool',
          execution: {
            toolName: 'Read',
            toolInput: {},
            toolUseId: 't1',
            status: 'complete',
            category: 'builtin',
          },
        },
      ],
      streaming: false,
    }
    expect(getBlockFineCategories(block)).toEqual(['builtin'])
  })

  it('maps notice to "error"', () => {
    const block: ConversationBlock = { type: 'notice', id: 'n1', variant: 'rate_limit', data: null }
    expect(getBlockFineCategories(block)).toEqual(['error'])
  })

  it('maps system hook_event to "hook"', () => {
    const block: ConversationBlock = {
      type: 'system',
      id: 's1',
      variant: 'hook_event',
      data: {} as any,
    }
    expect(getBlockFineCategories(block)).toEqual(['hook'])
  })

  it('maps system queue_operation to "queue"', () => {
    const block: ConversationBlock = {
      type: 'system',
      id: 's1',
      variant: 'queue_operation',
      data: {} as any,
    }
    expect(getBlockFineCategories(block)).toEqual(['queue'])
  })

  it('maps interaction to "prompt"', () => {
    const block: ConversationBlock = {
      type: 'interaction',
      id: 'i1',
      variant: 'permission',
      requestId: 'r1',
      resolved: false,
      data: {} as any,
    }
    expect(getBlockFineCategories(block)).toEqual(['prompt'])
  })

  it('maps turn_boundary to "turn"', () => {
    const block: ConversationBlock = {
      type: 'turn_boundary',
      id: 'tb1',
      success: true,
      totalCostUsd: 0,
      numTurns: 1,
      durationMs: 1000,
      usage: {},
      modelUsage: {},
      permissionDenials: [],
      stopReason: null,
    }
    expect(getBlockFineCategories(block)).toEqual(['turn'])
  })

  it('maps progress with agent category to "agent"', () => {
    const block: ConversationBlock = {
      type: 'progress',
      id: 'p1',
      variant: 'agent',
      category: 'agent',
      data: {} as any,
      ts: 1000,
    }
    expect(getBlockFineCategories(block)).toEqual(['agent'])
  })

  it('maps progress with hook variant to "hook"', () => {
    const block: ConversationBlock = {
      type: 'progress',
      id: 'p1',
      variant: 'hook',
      category: 'hook',
      data: {} as any,
      ts: 1000,
    }
    expect(getBlockFineCategories(block)).toEqual(['hook'])
  })

  it('deduplicates categories for assistant with multiple tools of same category', () => {
    const block: ConversationBlock = {
      type: 'assistant',
      id: 'a1',
      segments: [
        {
          kind: 'tool',
          execution: {
            toolName: 'Read',
            toolInput: {},
            toolUseId: 't1',
            status: 'complete',
            category: 'builtin',
          },
        },
        {
          kind: 'tool',
          execution: {
            toolName: 'Edit',
            toolInput: {},
            toolUseId: 't2',
            status: 'complete',
            category: 'builtin',
          },
        },
      ],
      streaming: false,
    }
    expect(getBlockFineCategories(block)).toEqual(['builtin'])
  })
})
