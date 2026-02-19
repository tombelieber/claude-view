import { describe, expect, it, vi, afterEach } from 'vitest'
import { render, screen, act } from '@testing-library/react'
import { ContextGauge } from './ContextGauge'

const baseProps = {
  contextWindowTokens: 80_000,
  model: 'claude-sonnet-4',
  group: 'autonomous' as const,
  tokens: { inputTokens: 80_000, outputTokens: 10_000, cacheReadTokens: 0, cacheCreationTokens: 0, totalTokens: 90_000 },
  turnCount: 20,
}

describe('ContextGauge compacting overlay', () => {
  afterEach(() => { vi.useRealTimers() })

  it('shows compacting label when agentLabel contains "compacting"', () => {
    render(<ContextGauge {...baseProps} agentLabel="Auto-compacting context..." />)
    expect(screen.getByText(/compacting/i)).toBeInTheDocument()
  })

  it('does not show compacting label during normal thinking', () => {
    render(<ContextGauge {...baseProps} agentLabel="Thinking..." />)
    expect(screen.queryByText(/compacting/i)).not.toBeInTheDocument()
  })

  it('shows compacted label briefly after compacting ends', () => {
    vi.useFakeTimers()
    const { rerender } = render(<ContextGauge {...baseProps} agentLabel="Compacting context..." />)

    // State transitions away from compacting
    rerender(<ContextGauge {...baseProps} agentLabel="Using tools..." />)
    expect(screen.getByText(/compacted/i)).toBeInTheDocument()

    // After 5 seconds, the label should disappear
    act(() => { vi.advanceTimersByTime(5_000) })
    expect(screen.queryByText(/compacted/i)).not.toBeInTheDocument()
  })

  it('does not show compacting in expanded mode either', () => {
    render(<ContextGauge {...baseProps} agentLabel="Auto-compacting context..." expanded />)
    expect(screen.getByText(/compacting/i)).toBeInTheDocument()
  })
})
