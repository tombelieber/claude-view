import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { CategoryFilterBar } from './CategoryFilterBar'

const mockBlocks: ConversationBlock[] = [
  { type: 'user', id: 'u1', text: 'hello', timestamp: 1000 },
  { type: 'assistant', id: 'a1', segments: [], streaming: false },
  { type: 'user', id: 'u2', text: 'world', timestamp: 1001 },
  { type: 'notice', id: 'n1', variant: 'error', data: null },
]

describe('CategoryFilterBar', () => {
  it('renders chips for each block type present', () => {
    render(<CategoryFilterBar blocks={mockBlocks} activeFilter={null} onFilterChange={() => {}} />)
    expect(screen.getByText(/All/)).toBeInTheDocument()
    expect(screen.getByText(/User/)).toBeInTheDocument()
    expect(screen.getByText(/Assistant/)).toBeInTheDocument()
    expect(screen.getByText(/Notice/)).toBeInTheDocument()
  })

  it('shows correct counts per type', () => {
    render(<CategoryFilterBar blocks={mockBlocks} activeFilter={null} onFilterChange={() => {}} />)
    expect(screen.getByText('User (2)')).toBeInTheDocument()
    expect(screen.getByText('Assistant (1)')).toBeInTheDocument()
    expect(screen.getByText('Notice (1)')).toBeInTheDocument()
  })

  it('clicking a chip calls onFilterChange with the category', () => {
    const onChange = vi.fn()
    render(<CategoryFilterBar blocks={mockBlocks} activeFilter={null} onFilterChange={onChange} />)
    fireEvent.click(screen.getByText('User (2)'))
    expect(onChange).toHaveBeenCalledWith('user')
  })

  it('active chip has distinct bg-blue-500 styling', () => {
    render(<CategoryFilterBar blocks={mockBlocks} activeFilter="user" onFilterChange={() => {}} />)
    const userChip = screen.getByText('User (2)')
    expect(userChip.className).toContain('bg-blue-500')
    expect(userChip.className).toContain('text-white')
  })

  it('"All" chip clears filter by calling onFilterChange(null)', () => {
    const onChange = vi.fn()
    render(<CategoryFilterBar blocks={mockBlocks} activeFilter="user" onFilterChange={onChange} />)
    fireEvent.click(screen.getByText(/All/))
    expect(onChange).toHaveBeenCalledWith(null)
  })
})
