import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'
import { SubAgentPills } from './SubAgentPills'

describe('SubAgentPills', () => {
  const mockRunningAgent: SubAgentInfo = {
    toolUseId: 'tool_123',
    agentId: 'a123456',
    agentType: 'Explore',
    description: 'Searching for files',
    status: 'running',
    startedAt: 1708120000,
    completedAt: null,
    durationMs: null,
    toolUseCount: null,
    costUsd: null,
  }

  const mockCompleteAgent: SubAgentInfo = {
    toolUseId: 'tool_456',
    agentId: 'b789012',
    agentType: 'code-reviewer',
    description: 'Reviewing changes',
    status: 'complete',
    startedAt: 1708120000,
    completedAt: 1708120030,
    durationMs: 30000,
    toolUseCount: 15,
    costUsd: 0.02,
  }

  const mockErrorAgent: SubAgentInfo = {
    toolUseId: 'tool_789',
    agentId: 'c345678',
    agentType: 'search',
    description: 'Failed to find matches',
    status: 'error',
    startedAt: 1708120000,
    completedAt: 1708120010,
    durationMs: 10000,
    toolUseCount: 3,
    costUsd: 0.01,
  }

  it('returns null for empty array', () => {
    const { container } = render(<SubAgentPills subAgents={[]} />)
    expect(container.firstChild).toBeNull()
  })

  it('shows agent type as pill text', () => {
    render(<SubAgentPills subAgents={[mockRunningAgent]} />)
    expect(screen.getByText('Explore')).toBeInTheDocument()
  })

  it('shows multiple agent pills', () => {
    render(
      <SubAgentPills
        subAgents={[mockRunningAgent, { ...mockRunningAgent, toolUseId: 'tool_999' }]}
      />,
    )
    expect(screen.getAllByText('Explore')).toHaveLength(2)
  })

  it('shows agent types for mixed statuses', () => {
    render(<SubAgentPills subAgents={[mockRunningAgent, mockCompleteAgent]} />)
    expect(screen.getByText('Explore')).toBeInTheDocument()
    expect(screen.getByText('code-reviewer')).toBeInTheDocument()
  })

  it('shows all agent type labels when all done', () => {
    render(<SubAgentPills subAgents={[mockCompleteAgent, mockErrorAgent]} />)
    expect(screen.getByText('code-reviewer')).toBeInTheDocument()
    expect(screen.getByText('search')).toBeInTheDocument()
  })

  it('displays first 4 agents as pills', () => {
    const agents = [
      mockRunningAgent,
      mockCompleteAgent,
      mockErrorAgent,
      { ...mockRunningAgent, toolUseId: 'tool_4', agentType: 'edit-files', description: 'Editing' },
    ]
    render(<SubAgentPills subAgents={agents} />)

    expect(screen.getByText('Explore')).toBeInTheDocument()
    expect(screen.getByText('code-reviewer')).toBeInTheDocument()
    expect(screen.getByText('search')).toBeInTheDocument()
    expect(screen.getByText('edit-files')).toBeInTheDocument()
  })

  it('shows "+N more" pill when more than 4 agents', () => {
    const agents = [
      mockRunningAgent,
      mockCompleteAgent,
      mockErrorAgent,
      { ...mockRunningAgent, toolUseId: 'tool_4', agentType: 'edit-files', description: 'Editing' },
      { ...mockCompleteAgent, toolUseId: 'tool_5' },
    ]
    render(<SubAgentPills subAgents={agents} />)

    expect(screen.getByText('+1 more')).toBeInTheDocument()
  })

  it('calls onExpand when clicked', async () => {
    const user = userEvent.setup()
    const onExpand = vi.fn()

    render(<SubAgentPills subAgents={[mockRunningAgent]} onExpand={onExpand} />)

    const container = screen.getByRole('button')
    await user.click(container)

    expect(onExpand).toHaveBeenCalledTimes(1)
  })

  it('calls onExpand on Enter key', async () => {
    const user = userEvent.setup()
    const onExpand = vi.fn()

    render(<SubAgentPills subAgents={[mockRunningAgent]} onExpand={onExpand} />)

    const container = screen.getByRole('button')
    container.focus()
    await user.keyboard('{Enter}')

    expect(onExpand).toHaveBeenCalledTimes(1)
  })

  it('calls onExpand on Space key', async () => {
    const user = userEvent.setup()
    const onExpand = vi.fn()

    render(<SubAgentPills subAgents={[mockRunningAgent]} onExpand={onExpand} />)

    const container = screen.getByRole('button')
    container.focus()
    await user.keyboard(' ')

    expect(onExpand).toHaveBeenCalledTimes(1)
  })

  it('is not clickable when onExpand is not provided', () => {
    render(<SubAgentPills subAgents={[mockRunningAgent]} />)
    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })

  it('shows correct status styling for running agent (green bg)', () => {
    const { container } = render(<SubAgentPills subAgents={[mockRunningAgent]} />)
    const pill = container.querySelector('.bg-green-100')
    expect(pill).toBeInTheDocument()
  })

  it('shows correct status styling for complete agent (neutral bg)', () => {
    const { container } = render(<SubAgentPills subAgents={[mockCompleteAgent]} />)
    const pill = container.querySelector('.bg-zinc-100')
    expect(pill).toBeInTheDocument()
  })

  it('shows correct status styling for error agent (red bg)', () => {
    const { container } = render(<SubAgentPills subAgents={[mockErrorAgent]} />)
    const pill = container.querySelector('.bg-red-100')
    expect(pill).toBeInTheDocument()
  })

  it('shows agent type in pill', () => {
    render(<SubAgentPills subAgents={[mockRunningAgent]} />)
    expect(screen.getByText('Explore')).toBeInTheDocument()
  })

  it('shows aria-label with agent type and description', () => {
    const { container } = render(<SubAgentPills subAgents={[mockRunningAgent]} />)
    const pill = container.querySelector('[aria-label]')
    expect(pill).toHaveAttribute('aria-label', 'Explore: Searching for files')
  })

  it('handles edge case: agent type is empty string', () => {
    const emptyTypeAgent = { ...mockRunningAgent, agentType: '' }
    render(<SubAgentPills subAgents={[emptyTypeAgent]} />)
    // With empty agentType the description is still shown
    expect(screen.getByText('Searching for files')).toBeInTheDocument()
  })

  it('correctly renders pills for mix of statuses', () => {
    const agents = [
      mockRunningAgent,
      { ...mockRunningAgent, toolUseId: 'tool_2', status: 'running' as const },
      mockCompleteAgent,
      mockErrorAgent,
    ]
    render(<SubAgentPills subAgents={agents} />)
    // All 4 fit within display limit (4), so all should render as pills
    expect(screen.getAllByText('Explore')).toHaveLength(2)
    expect(screen.getByText('code-reviewer')).toBeInTheDocument()
    expect(screen.getByText('search')).toBeInTheDocument()
  })
})
