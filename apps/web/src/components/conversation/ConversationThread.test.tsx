import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ConversationThread } from './ConversationThread'
import type { BlockRenderers } from './types'

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
    expect(screen.getByText('User (2)')).toBeInTheDocument()
  })

  it('filtering hides blocks of other types', () => {
    render(<ConversationThread blocks={mockBlocks} renderers={mockRenderers} filterBar={true} />)

    // All blocks visible initially
    expect(screen.getByTestId('block-u1')).toBeInTheDocument()
    expect(screen.getByTestId('block-a1')).toBeInTheDocument()
    expect(screen.getByTestId('block-n1')).toBeInTheDocument()

    // Click "User (2)" to filter
    fireEvent.click(screen.getByText('User (2)'))

    // Only user blocks visible
    expect(screen.getByTestId('block-u1')).toBeInTheDocument()
    expect(screen.getByTestId('block-u2')).toBeInTheDocument()
    expect(screen.queryByTestId('block-a1')).not.toBeInTheDocument()
    expect(screen.queryByTestId('block-n1')).not.toBeInTheDocument()
  })
})
