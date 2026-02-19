import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SubAgentPills } from './SubAgentPills'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

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

  it('shows single agent with correct summary text', () => {
    render(<SubAgentPills subAgents={[mockRunningAgent]} />)
    expect(screen.getByText('1 agent (1 active)')).toBeInTheDocument()
  })

  it('shows multiple agents with correct summary text (all active)', () => {
    render(<SubAgentPills subAgents={[mockRunningAgent, { ...mockRunningAgent, toolUseId: 'tool_999' }]} />)
    expect(screen.getByText('2 agents (2 active)')).toBeInTheDocument()
  })

  it('shows correct summary text when some completed (uses plural)', () => {
    render(<SubAgentPills subAgents={[mockRunningAgent, mockCompleteAgent]} />)
    expect(screen.getByText('2 agents (1 active)')).toBeInTheDocument()
  })

  it('shows "all done" when no agents are running', () => {
    render(<SubAgentPills subAgents={[mockCompleteAgent, mockErrorAgent]} />)
    expect(screen.getByText('2 agents (all done)')).toBeInTheDocument()
  })

  it('displays first 3 agents as pills', () => {
    const agents = [
      mockRunningAgent,
      mockCompleteAgent,
      mockErrorAgent,
    ]
    render(<SubAgentPills subAgents={agents} />)

    // Check that pills are rendered (by their initials)
    expect(screen.getByText('E')).toBeInTheDocument() // Explore
    expect(screen.getByText('C')).toBeInTheDocument() // code-reviewer
    expect(screen.getByText('S')).toBeInTheDocument() // search
  })

  it('shows "+N more" pill when more than 3 agents', () => {
    const agents = [
      mockRunningAgent,
      mockCompleteAgent,
      mockErrorAgent,
      { ...mockRunningAgent, toolUseId: 'tool_4' },
      { ...mockCompleteAgent, toolUseId: 'tool_5' },
    ]
    render(<SubAgentPills subAgents={agents} />)

    expect(screen.getByText('+2 more')).toBeInTheDocument()
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

  it('shows correct status styling for running agent (green)', () => {
    const { container } = render(<SubAgentPills subAgents={[mockRunningAgent]} />)
    const pill = container.querySelector('.border-green-500')
    expect(pill).toBeInTheDocument()
  })

  it('shows correct status styling for complete agent (neutral)', () => {
    const { container } = render(<SubAgentPills subAgents={[mockCompleteAgent]} />)
    const pill = container.querySelector('.border-zinc-300')
    expect(pill).toBeInTheDocument()
  })

  it('shows correct status styling for error agent (red)', () => {
    const { container } = render(<SubAgentPills subAgents={[mockErrorAgent]} />)
    const pill = container.querySelector('.border-red-500')
    expect(pill).toBeInTheDocument()
  })

  it('shows agent type initial in pill', () => {
    render(<SubAgentPills subAgents={[mockRunningAgent]} />)
    expect(screen.getByText('E')).toBeInTheDocument() // First letter of "Explore"
  })

  it('shows tooltip with agent type and description', () => {
    const { container } = render(<SubAgentPills subAgents={[mockRunningAgent]} />)
    const pill = container.querySelector('[title]')
    expect(pill).toHaveAttribute('title', 'Explore: Searching for files')
  })

  it('handles edge case: agent type is empty string', () => {
    const emptyTypeAgent = { ...mockRunningAgent, agentType: '' }
    render(<SubAgentPills subAgents={[emptyTypeAgent]} />)
    expect(screen.getByText('T')).toBeInTheDocument() // Falls back to 'T'
  })

  it('correctly counts active agents when mix of statuses', () => {
    const agents = [
      mockRunningAgent,
      { ...mockRunningAgent, toolUseId: 'tool_2', status: 'running' as const },
      mockCompleteAgent,
      mockErrorAgent,
    ]
    render(<SubAgentPills subAgents={agents} />)
    expect(screen.getByText('4 agents (2 active)')).toBeInTheDocument()
  })
})
