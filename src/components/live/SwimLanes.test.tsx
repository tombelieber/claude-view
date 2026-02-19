import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { SwimLanes } from './SwimLanes'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

// Helper to create minimal SubAgentInfo fixture
function makeSubAgent(overrides?: Partial<SubAgentInfo>): SubAgentInfo {
  return {
    toolUseId: 'toolu_123',
    agentType: 'test',
    description: 'Test agent',
    status: 'running',
    startedAt: Date.now(),
    completedAt: null,
    costUsd: null,
    durationMs: null,
    toolUseCount: null,
    agentId: null,
    ...overrides,
  }
}

describe('SwimLanes', () => {
  describe('empty state', () => {
    it('returns null when no sub-agents', () => {
      const { container } = render(
        <SwimLanes subAgents={[]} sessionActive={true} />
      )
      expect(container.firstChild).toBeNull()
    })
  })

  describe('rendering sub-agents', () => {
    it('renders a single running agent', () => {
      const agent = makeSubAgent({
        agentType: 'search',
        description: 'Searching for files',
      })
      render(<SwimLanes subAgents={[agent]} sessionActive={true} />)

      expect(screen.getByText('search')).toBeInTheDocument()
      expect(screen.getByText('Searching for files')).toBeInTheDocument()
    })

    it('renders multiple agents', () => {
      const agents = [
        makeSubAgent({ toolUseId: '1', agentType: 'search', description: 'Agent 1' }),
        makeSubAgent({ toolUseId: '2', agentType: 'edit', description: 'Agent 2' }),
      ]
      render(<SwimLanes subAgents={agents} sessionActive={true} />)

      expect(screen.getByText('search')).toBeInTheDocument()
      expect(screen.getByText('edit')).toBeInTheDocument()
      expect(screen.getByText('Agent 1')).toBeInTheDocument()
      expect(screen.getByText('Agent 2')).toBeInTheDocument()
    })

    it('shows ERR indicator for error status', () => {
      const agent = makeSubAgent({
        status: 'error',
        completedAt: Date.now(),
      })
      render(<SwimLanes subAgents={[agent]} sessionActive={false} />)

      expect(screen.getByText('ERR')).toBeInTheDocument()
    })

    it('shows metrics inline for completed agents', () => {
      const agent = makeSubAgent({
        status: 'complete',
        completedAt: Date.now(),
        costUsd: 0.0342,
        durationMs: 5000,
        toolUseCount: 3,
        agentId: 'agent-123',
      })
      render(<SwimLanes subAgents={[agent]} sessionActive={false} />)

      expect(screen.getByText('$0.03')).toBeInTheDocument()
      expect(screen.getByText('5s')).toBeInTheDocument()
      expect(screen.getByText('3 tool calls')).toBeInTheDocument()
    })

    it('handles singular tool call count', () => {
      const agent = makeSubAgent({
        status: 'complete',
        completedAt: Date.now(),
        toolUseCount: 1,
      })
      render(<SwimLanes subAgents={[agent]} sessionActive={false} />)

      expect(screen.getByText('1 tool call')).toBeInTheDocument()
    })
  })

  describe('drill-down click', () => {
    it('calls onDrillDown when clicking a row with agentId', async () => {
      const user = userEvent.setup()
      const onDrillDown = vi.fn()
      const agent = makeSubAgent({
        status: 'complete',
        completedAt: Date.now(),
        agentId: 'agent-abc',
        agentType: 'search',
        description: 'Find files',
      })
      render(<SwimLanes subAgents={[agent]} onDrillDown={onDrillDown} />)

      await user.click(screen.getByText('Find files'))
      expect(onDrillDown).toHaveBeenCalledWith('agent-abc', 'search', 'Find files')
    })

    it('does not call onDrillDown when agent has no agentId', async () => {
      const user = userEvent.setup()
      const onDrillDown = vi.fn()
      const agent = makeSubAgent({
        status: 'running',
        agentId: null,
        description: 'Still starting',
      })
      render(<SwimLanes subAgents={[agent]} onDrillDown={onDrillDown} />)

      await user.click(screen.getByText('Still starting'))
      expect(onDrillDown).not.toHaveBeenCalled()
    })

    it('calls onDrillDown for running agents with agentId', async () => {
      const user = userEvent.setup()
      const onDrillDown = vi.fn()
      const agent = makeSubAgent({
        status: 'running',
        agentId: 'agent-live',
        agentType: 'explore',
        description: 'Exploring codebase',
      })
      render(<SwimLanes subAgents={[agent]} onDrillDown={onDrillDown} />)

      await user.click(screen.getByText('Exploring codebase'))
      expect(onDrillDown).toHaveBeenCalledWith('agent-live', 'explore', 'Exploring codebase')
    })
  })

  describe('sorting', () => {
    it('sorts running agents before completed', () => {
      const agents = [
        makeSubAgent({ toolUseId: '1', status: 'complete', completedAt: Date.now(), description: 'Completed first' }),
        makeSubAgent({ toolUseId: '2', status: 'running', startedAt: Date.now(), description: 'Running second' }),
      ]
      render(<SwimLanes subAgents={agents} sessionActive={true} />)

      const descriptions = screen.getAllByText(/Completed first|Running second/)
      expect(descriptions[0]).toHaveTextContent('Running second')
      expect(descriptions[1]).toHaveTextContent('Completed first')
    })

    it('sorts completed agents by completedAt desc', () => {
      const now = Date.now()
      const agents = [
        makeSubAgent({ toolUseId: '1', status: 'complete', completedAt: now - 2000, description: 'Older' }),
        makeSubAgent({ toolUseId: '2', status: 'complete', completedAt: now, description: 'Newer' }),
      ]
      render(<SwimLanes subAgents={agents} sessionActive={false} />)

      const descriptions = screen.getAllByText(/Older|Newer/)
      expect(descriptions[0]).toHaveTextContent('Newer')
      expect(descriptions[1]).toHaveTextContent('Older')
    })

    it('sorts running agents by startedAt asc', () => {
      const now = Date.now()
      const agents = [
        makeSubAgent({ toolUseId: '1', status: 'running', startedAt: now, description: 'Started later' }),
        makeSubAgent({ toolUseId: '2', status: 'running', startedAt: now - 2000, description: 'Started earlier' }),
      ]
      render(<SwimLanes subAgents={agents} sessionActive={true} />)

      const descriptions = screen.getAllByText(/Started earlier|Started later/)
      expect(descriptions[0]).toHaveTextContent('Started earlier')
      expect(descriptions[1]).toHaveTextContent('Started later')
    })
  })

  describe('sessionActive prop', () => {
    it('accepts sessionActive=true with running agents', () => {
      const agent = makeSubAgent({ status: 'running' })
      render(<SwimLanes subAgents={[agent]} sessionActive={true} />)

      expect(screen.getByText('Test agent')).toBeInTheDocument()
    })

    it('accepts sessionActive=false with completed agents', () => {
      const agent = makeSubAgent({ status: 'complete', completedAt: Date.now() })
      render(<SwimLanes subAgents={[agent]} sessionActive={false} />)

      expect(screen.getByText('Test agent')).toBeInTheDocument()
    })

    it('accepts sessionActive=false with error agents', () => {
      const agent = makeSubAgent({ status: 'error', completedAt: Date.now() })
      render(<SwimLanes subAgents={[agent]} sessionActive={false} />)

      expect(screen.getByText('Test agent')).toBeInTheDocument()
      expect(screen.getByText('ERR')).toBeInTheDocument()
    })
  })

  describe('scrolling behavior', () => {
    it('enables scroll when more than 5 agents', () => {
      const agents = Array.from({ length: 6 }, (_, i) =>
        makeSubAgent({ toolUseId: `${i}`, description: `Agent ${i}` })
      )
      const { container } = render(
        <SwimLanes subAgents={agents} sessionActive={true} />
      )

      const wrapper = container.firstElementChild
      expect(wrapper?.className).toContain('overflow-y-auto')
    })

    it('does not enable scroll when 5 or fewer agents', () => {
      const agents = Array.from({ length: 5 }, (_, i) =>
        makeSubAgent({ toolUseId: `${i}`, description: `Agent ${i}` })
      )
      const { container } = render(
        <SwimLanes subAgents={agents} sessionActive={true} />
      )

      const wrapper = container.firstElementChild
      expect(wrapper?.className).not.toContain('overflow-y-auto')
    })
  })
})
