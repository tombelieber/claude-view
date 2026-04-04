import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { TelemetrySection } from './TelemetrySection'

describe('TelemetrySection regression invariants', () => {
  it('invariant: self-hosted builds show privacy message with no toggle', () => {
    render(
      <TelemetrySection
        telemetryStatus="disabled"
        hasPosHogKey={false}
        onEnable={vi.fn()}
        onDisable={vi.fn()}
      />,
    )
    // Self-hosted renders a "Fully private" section with no toggle control
    expect(screen.queryByRole('switch')).not.toBeInTheDocument()
    expect(screen.getByText(/no data leaves your machine/i)).toBeInTheDocument()
  })
})
