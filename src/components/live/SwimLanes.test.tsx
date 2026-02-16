import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
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

    it('shows ERROR indicator for error status', () => {
      const agent = makeSubAgent({
        status: 'error',
        completedAt: Date.now(),
      })
      render(<SwimLanes subAgents={[agent]} sessionActive={false} />)

      expect(screen.getByText('ERROR')).toBeInTheDocument()
    })

    it('shows metrics for completed agents', () => {
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
      expect(screen.getByText('id:agent-123')).toBeInTheDocument()
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

  describe('sorting', () => {
    it('sorts running agents before completed', () => {
      const agents = [
        makeSubAgent({ toolUseId: '1', status: 'complete', completedAt: Date.now(), description: 'Completed first' }),
        makeSubAgent({ toolUseId: '2', status: 'running', startedAt: Date.now(), description: 'Running second' }),
      ]
      const { container } = render(
        <SwimLanes subAgents={agents} sessionActive={true} />
      )

      const descriptions = Array.from(container.querySelectorAll('.text-sm.text-gray-300'))
        .map((el) => el.textContent)

      expect(descriptions[0]).toBe('Running second')
      expect(descriptions[1]).toBe('Completed first')
    })

    it('sorts completed agents by completedAt desc', () => {
      const now = Date.now()
      const agents = [
        makeSubAgent({ toolUseId: '1', status: 'complete', completedAt: now - 2000, description: 'Older' }),
        makeSubAgent({ toolUseId: '2', status: 'complete', completedAt: now, description: 'Newer' }),
      ]
      const { container } = render(
        <SwimLanes subAgents={agents} sessionActive={false} />
      )

      const descriptions = Array.from(container.querySelectorAll('.text-sm.text-gray-300'))
        .map((el) => el.textContent)

      expect(descriptions[0]).toBe('Newer')
      expect(descriptions[1]).toBe('Older')
    })

    it('sorts running agents by startedAt asc', () => {
      const now = Date.now()
      const agents = [
        makeSubAgent({ toolUseId: '1', status: 'running', startedAt: now, description: 'Started later' }),
        makeSubAgent({ toolUseId: '2', status: 'running', startedAt: now - 2000, description: 'Started earlier' }),
      ]
      const { container } = render(
        <SwimLanes subAgents={agents} sessionActive={true} />
      )

      const descriptions = Array.from(container.querySelectorAll('.text-sm.text-gray-300'))
        .map((el) => el.textContent)

      expect(descriptions[0]).toBe('Started earlier')
      expect(descriptions[1]).toBe('Started later')
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
      expect(screen.getByText('ERROR')).toBeInTheDocument()
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

      const wrapper = container.querySelector('.max-h-\\[280px\\].overflow-y-auto')
      expect(wrapper).toBeInTheDocument()
    })

    it('does not enable scroll when 5 or fewer agents', () => {
      const agents = Array.from({ length: 5 }, (_, i) =>
        makeSubAgent({ toolUseId: `${i}`, description: `Agent ${i}` })
      )
      const { container } = render(
        <SwimLanes subAgents={agents} sessionActive={true} />
      )

      const wrapper = container.querySelector('.max-h-\\[280px\\].overflow-y-auto')
      expect(wrapper).toBeNull()
    })
  })
})
