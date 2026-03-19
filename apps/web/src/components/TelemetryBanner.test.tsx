import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { TelemetryBanner } from './TelemetryBanner'

describe('TelemetryBanner', () => {
  it('renders when status is undecided', () => {
    render(<TelemetryBanner onEnable={vi.fn()} onDisable={vi.fn()} />)
    expect(screen.getByText(/help improve claude-view/i)).toBeInTheDocument()
  })

  it('calls onEnable when Enable clicked', () => {
    const onEnable = vi.fn()
    render(<TelemetryBanner onEnable={onEnable} onDisable={vi.fn()} />)
    fireEvent.click(screen.getByText(/enable/i))
    expect(onEnable).toHaveBeenCalledOnce()
  })

  it('calls onDisable when No thanks clicked', () => {
    const onDisable = vi.fn()
    render(<TelemetryBanner onEnable={vi.fn()} onDisable={onDisable} />)
    fireEvent.click(screen.getByText(/no thanks/i))
    expect(onDisable).toHaveBeenCalledOnce()
  })

  it('has no dismiss/X button', () => {
    render(<TelemetryBanner onEnable={vi.fn()} onDisable={vi.fn()} />)
    expect(screen.queryByRole('button', { name: /close|dismiss|x/i })).not.toBeInTheDocument()
  })

  it('has a Learn more link', () => {
    render(<TelemetryBanner onEnable={vi.fn()} onDisable={vi.fn()} />)
    const link = screen.getByText(/learn more/i)
    expect(link).toBeInTheDocument()
    expect(link.closest('a')).toHaveAttribute('href', expect.stringContaining('telemetry'))
  })
})
