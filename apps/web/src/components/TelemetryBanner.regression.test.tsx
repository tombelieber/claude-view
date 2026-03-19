import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { TelemetryBanner } from './TelemetryBanner'

describe('TelemetryBanner regression invariants', () => {
  it('invariant: banner has no dismiss/close button (must force Yes/No choice)', () => {
    render(<TelemetryBanner onEnable={vi.fn()} onDisable={vi.fn()} />)
    expect(screen.queryByRole('button', { name: /close|dismiss|x/i })).not.toBeInTheDocument()
  })

  it('invariant: banner links to telemetry transparency page', () => {
    render(<TelemetryBanner onEnable={vi.fn()} onDisable={vi.fn()} />)
    const link = screen.getByText(/learn more/i)
    expect(link.closest('a')).toHaveAttribute('href', expect.stringContaining('telemetry'))
  })
})
