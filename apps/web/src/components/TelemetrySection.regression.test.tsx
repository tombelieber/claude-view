import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { TelemetrySection } from './TelemetrySection'

describe('TelemetrySection regression invariants', () => {
  it('invariant: self-hosted builds show disabled toggle with explanation', () => {
    render(
      <TelemetrySection
        telemetryStatus="disabled"
        hasPosHogKey={false}
        onEnable={vi.fn()}
        onDisable={vi.fn()}
      />,
    )
    expect(screen.getByRole('checkbox')).toBeDisabled()
    expect(screen.getByText(/not available/i)).toBeInTheDocument()
  })
})
