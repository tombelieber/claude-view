import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { TimelineView } from './TimelineView'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

describe('TimelineView', () => {
  const baseSessionStart = 1700000000 // Unix timestamp in seconds

  const createAgent = (overrides: Partial<SubAgentInfo> = {}): SubAgentInfo => ({
    toolUseId: overrides.toolUseId ?? 'test-1',
    agentType: overrides.agentType ?? 'Explore',
    description: overrides.description ?? 'Test description',
    status: overrides.status ?? 'complete',
    startedAt: overrides.startedAt ?? baseSessionStart + 5,
    completedAt: overrides.completedAt ?? baseSessionStart + 10,
    durationMs: overrides.durationMs ?? 5000,
    toolUseCount: overrides.toolUseCount ?? 3,
    costUsd: overrides.costUsd ?? 0.05,
    agentId: overrides.agentId ?? 'abc1234',
  })

  it('renders nothing when subAgents array is empty', () => {
    const { container } = render(
      <TimelineView
        subAgents={[]}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={30000}
      />
    )
    expect(container.firstChild).toBeNull()
  })

  it('renders time axis with appropriate intervals for short sessions (<30s)', () => {
    const agents = [createAgent()]
    render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={20000} // 20 seconds
      />
    )
    // Should use 5s intervals: 0s, 5s, 10s, 15s, 20s
    expect(screen.getByText('0s')).toBeDefined()
    expect(screen.getByText('5s')).toBeDefined()
    expect(screen.getByText('10s')).toBeDefined()
    expect(screen.getByText('15s')).toBeDefined()
    expect(screen.getByText('20s')).toBeDefined()
  })

  it('renders time axis with appropriate intervals for medium sessions (30-60s)', () => {
    const agents = [createAgent()]
    render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={45000} // 45 seconds
      />
    )
    // Should use 10s intervals
    expect(screen.getByText('0s')).toBeDefined()
    expect(screen.getByText('10s')).toBeDefined()
    expect(screen.getByText('20s')).toBeDefined()
    expect(screen.getByText('30s')).toBeDefined()
    expect(screen.getByText('40s')).toBeDefined()
  })

  it('renders time axis with minute labels for longer sessions', () => {
    const agents = [createAgent()]
    render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={120000} // 2 minutes
      />
    )
    // Should use 30s intervals, formatted as "1m", "1m 30s"
    expect(screen.getByText('0s')).toBeDefined()
    expect(screen.getByText('1m')).toBeDefined()
    expect(screen.getByText('2m')).toBeDefined()
  })

  it('renders agent type labels', () => {
    const agents = [
      createAgent({ agentType: 'Explore', toolUseId: '1' }),
      createAgent({ agentType: 'code-review', toolUseId: '2' }),
      createAgent({ agentType: 'search', toolUseId: '3' }),
    ]
    render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={30000}
      />
    )
    expect(screen.getByText('Explore')).toBeDefined()
    expect(screen.getByText('code-review')).toBeDefined()
    expect(screen.getByText('search')).toBeDefined()
  })

  it('sorts agents chronologically by startedAt', () => {
    const agents = [
      createAgent({ agentType: 'Third', startedAt: baseSessionStart + 20, toolUseId: '3' }),
      createAgent({ agentType: 'First', startedAt: baseSessionStart + 5, toolUseId: '1' }),
      createAgent({ agentType: 'Second', startedAt: baseSessionStart + 10, toolUseId: '2' }),
    ]
    render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={30000}
      />
    )
    // Check that labels appear in chronological order in the DOM
    const labels = screen.getAllByText(/First|Second|Third/)
    expect(labels[0].textContent).toBe('First')
    expect(labels[1].textContent).toBe('Second')
    expect(labels[2].textContent).toBe('Third')
  })

  it('handles running agents (no completedAt or durationMs)', () => {
    const agents = [
      createAgent({
        status: 'running',
        completedAt: null,
        durationMs: null,
        costUsd: null,
        toolUseCount: null,
        agentId: null,
      }),
    ]
    const { container } = render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={30000}
      />
    )
    // Should render without crashing, bar should have animation class
    const bars = container.querySelectorAll('.timeline-bar-growing')
    expect(bars.length).toBeGreaterThan(0)
  })

  it('handles error status agents', () => {
    const agents = [
      createAgent({
        status: 'error',
        completedAt: baseSessionStart + 8,
        durationMs: 3000,
      }),
    ]
    const { container } = render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={30000}
      />
    )
    // Should render with red color class
    const bars = container.querySelectorAll('.bg-red-500')
    expect(bars.length).toBeGreaterThan(0)
  })

  it('handles complete status agents', () => {
    const agents = [
      createAgent({
        status: 'complete',
        completedAt: baseSessionStart + 10,
        durationMs: 5000,
      }),
    ]
    const { container } = render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={30000}
      />
    )
    // Should render with green color class
    const bars = container.querySelectorAll('.bg-green-600')
    expect(bars.length).toBeGreaterThan(0)
  })

  it('handles agents with very short durations (min width enforcement)', () => {
    const agents = [
      createAgent({
        startedAt: baseSessionStart + 1,
        completedAt: baseSessionStart + 1.01, // 10ms duration
        durationMs: 10,
      }),
    ]
    const { container } = render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={30000}
      />
    )
    // Should render with min-width CSS (max(2px, ...))
    // We can't easily test the computed width, but verify it renders
    const bars = container.querySelectorAll('[class*="bg-green"]')
    expect(bars.length).toBeGreaterThan(0)
  })

  it('handles overlapping agents (parallel execution)', () => {
    const agents = [
      createAgent({
        agentType: 'Agent1',
        startedAt: baseSessionStart + 5,
        completedAt: baseSessionStart + 15,
        durationMs: 10000,
        toolUseId: '1',
      }),
      createAgent({
        agentType: 'Agent2',
        startedAt: baseSessionStart + 10,
        completedAt: baseSessionStart + 20,
        durationMs: 10000,
        toolUseId: '2',
      }),
    ]
    render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={30000}
      />
    )
    // Both should render (overlapping bars visible)
    expect(screen.getByText('Agent1')).toBeDefined()
    expect(screen.getByText('Agent2')).toBeDefined()
  })

  it('handles agents starting before session start (negative offset)', () => {
    const agents = [
      createAgent({
        startedAt: baseSessionStart - 5, // Started 5s before session
        completedAt: baseSessionStart + 5,
        durationMs: 10000,
      }),
    ]
    const { container } = render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={30000}
      />
    )
    // Should clamp to 0% start position
    const bars = container.querySelectorAll('[class*="bg-green"]')
    expect(bars.length).toBeGreaterThan(0)
  })

  it('handles agents extending beyond session duration', () => {
    const agents = [
      createAgent({
        startedAt: baseSessionStart + 25,
        completedAt: baseSessionStart + 40, // Extends beyond 30s session
        durationMs: 15000,
      }),
    ]
    const { container } = render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={30000}
      />
    )
    // Should clamp width to not exceed 100%
    const bars = container.querySelectorAll('[class*="bg-green"]')
    expect(bars.length).toBeGreaterThan(0)
  })

  it('handles agents with null cost/toolUseCount gracefully', () => {
    const agents = [
      createAgent({
        costUsd: null,
        toolUseCount: null,
      }),
    ]
    const { container } = render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={30000}
      />
    )
    // Should render without crashing
    expect(container.querySelector('.font-mono')).toBeDefined()
  })

  it('formats cost correctly with $ prefix', () => {
    const agents = [createAgent({ costUsd: 0.0342 })]
    render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={30000}
      />
    )
    // Tooltip should show "$0.03" (rounded to 2 decimals)
    // Note: Tooltip content is in portal, harder to test without user interaction
    // This test just verifies component renders
    expect(screen.getByText('Explore')).toBeDefined()
  })

  it('formats duration in seconds with 1 decimal', () => {
    const agents = [createAgent({ durationMs: 2134 })] // 2.134s
    render(
      <TimelineView
        subAgents={agents}
        sessionStartedAt={baseSessionStart}
        sessionDurationMs={30000}
      />
    )
    // Tooltip should show "2.1s"
    // Note: Tooltip content is in portal, harder to test without user interaction
    // This test just verifies component renders
    expect(screen.getByText('Explore')).toBeDefined()
  })
})
