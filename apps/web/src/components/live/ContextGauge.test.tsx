import { act, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { ContextGauge } from './ContextGauge'

const baseProps = {
  contextWindowTokens: 80_000,
  model: 'claude-sonnet-4',
  group: 'autonomous' as const,
  tokens: {
    inputTokens: 80_000,
    outputTokens: 10_000,
    cacheReadTokens: 0,
    cacheCreationTokens: 0,
    totalTokens: 90_000,
  },
  turnCount: 20,
  statuslineContextWindowSize: 200_000,
  statuslineUsedPct: 40,
}

describe('ContextGauge fallback (no statusline)', () => {
  it('computes from contextWindowTokens when statusline props are null', () => {
    render(
      <ContextGauge {...baseProps} statuslineUsedPct={null} statuslineContextWindowSize={null} />,
    )
    // 80K / 200K default = 40%
    expect(screen.queryByText('\u2014')).not.toBeInTheDocument()
  })

  it('uses statuslineUsedPct even when statuslineContextWindowSize is null', () => {
    render(
      <ContextGauge {...baseProps} statuslineUsedPct={50} statuslineContextWindowSize={null} />,
    )
    expect(screen.queryByText('\u2014')).not.toBeInTheDocument()
  })

  it('shows dash when contextWindowTokens is 0 and no statusline', () => {
    render(<ContextGauge contextWindowTokens={0} model="claude-sonnet-4" group="autonomous" />)
    expect(screen.getByText('\u2014')).toBeInTheDocument()
  })
})

describe('ContextGauge statusline props', () => {
  it('uses statuslineUsedPct directly instead of computing from tokens', () => {
    render(
      <ContextGauge
        {...baseProps}
        contextWindowTokens={80_000}
        statuslineUsedPct={55.3}
        statuslineContextWindowSize={200_000}
        expanded
      />,
    )
    expect(screen.getByText(/55\.3% used/)).toBeInTheDocument()
  })

  it('shows /1.0M denominator when statuslineContextWindowSize=1_000_000', () => {
    render(
      <ContextGauge
        {...baseProps}
        contextWindowTokens={150_000}
        statuslineContextWindowSize={1_000_000}
        statuslineUsedPct={15}
      />,
    )
    expect(screen.getByText(/150\.0k\/1\.0M tokens/)).toBeInTheDocument()
  })

  it('shows /200.0k denominator when statuslineContextWindowSize=200_000', () => {
    render(
      <ContextGauge
        {...baseProps}
        contextWindowTokens={80_000}
        statuslineContextWindowSize={200_000}
        statuslineUsedPct={40}
      />,
    )
    expect(screen.getByText(/80\.0k\/200\.0k tokens/)).toBeInTheDocument()
  })

  it('caps statuslineUsedPct at 100%', () => {
    render(
      <ContextGauge
        {...baseProps}
        contextWindowTokens={80_000}
        statuslineUsedPct={105.0}
        statuslineContextWindowSize={200_000}
        expanded
      />,
    )
    // Math.min(105, 100) = 100
    expect(screen.getByText(/100\.0% used/)).toBeInTheDocument()
    expect(screen.queryByText(/105/)).not.toBeInTheDocument()
  })
})

describe('ContextGauge compacting overlay', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  it('shows compacting label when agentStateKey is "compacting"', () => {
    render(
      <ContextGauge
        {...baseProps}
        agentStateKey="compacting"
        agentLabel="Auto-compacting context..."
      />,
    )
    expect(screen.getByText(/compacting/i)).toBeInTheDocument()
  })

  it('does not show compacting label during normal thinking', () => {
    render(<ContextGauge {...baseProps} agentStateKey="thinking" agentLabel="Thinking..." />)
    expect(screen.queryByText(/compacting/i)).not.toBeInTheDocument()
  })

  it('does not show compacting when label contains "compacting" but state is not compacting', () => {
    render(
      <ContextGauge {...baseProps} agentStateKey="acting" agentLabel="Searching: compacting" />,
    )
    expect(screen.queryByText(/compacting\.\.\./i)).not.toBeInTheDocument()
  })

  it('shows compacted label briefly after compacting ends', () => {
    vi.useFakeTimers()
    const { rerender } = render(
      <ContextGauge {...baseProps} agentStateKey="compacting" agentLabel="Compacting context..." />,
    )

    // State transitions away from compacting
    rerender(<ContextGauge {...baseProps} agentStateKey="acting" agentLabel="Using tools..." />)
    expect(screen.getByText(/compacted/i)).toBeInTheDocument()

    // After 5 seconds, the label should disappear
    act(() => {
      vi.advanceTimersByTime(5_000)
    })
    expect(screen.queryByText(/compacted/i)).not.toBeInTheDocument()
  })

  it('shows compacting in expanded mode when state is compacting', () => {
    render(
      <ContextGauge
        {...baseProps}
        agentStateKey="compacting"
        agentLabel="Auto-compacting context..."
        expanded
      />,
    )
    expect(screen.getByText(/compacting/i)).toBeInTheDocument()
  })
})
