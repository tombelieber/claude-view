import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { TelemetrySection } from './TelemetrySection'

describe('TelemetrySection', () => {
  const defaultProps = {
    telemetryStatus: 'enabled' as const,
    hasPosHogKey: true,
    onEnable: vi.fn(),
    onDisable: vi.fn(),
  }

  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders the telemetry toggle with correct label', () => {
    render(<TelemetrySection {...defaultProps} />)
    expect(screen.getByText('Anonymous Usage Analytics')).toBeInTheDocument()
  })

  it('switch is checked when telemetry is enabled', () => {
    render(<TelemetrySection {...defaultProps} telemetryStatus="enabled" />)
    const toggle = screen.getByRole('switch')
    expect(toggle).toHaveAttribute('aria-checked', 'true')
  })

  it('switch is unchecked when telemetry is disabled', () => {
    render(<TelemetrySection {...defaultProps} telemetryStatus="disabled" />)
    const toggle = screen.getByRole('switch')
    expect(toggle).toHaveAttribute('aria-checked', 'false')
  })

  it('calls onEnable when toggled on', () => {
    const onEnable = vi.fn()
    render(<TelemetrySection {...defaultProps} telemetryStatus="disabled" onEnable={onEnable} />)
    fireEvent.click(screen.getByRole('switch'))
    expect(onEnable).toHaveBeenCalledOnce()
  })

  it('calls onDisable when toggled off', () => {
    const onDisable = vi.fn()
    render(<TelemetrySection {...defaultProps} telemetryStatus="enabled" onDisable={onDisable} />)
    fireEvent.click(screen.getByRole('switch'))
    expect(onDisable).toHaveBeenCalledOnce()
  })

  it('shows private message when no posthog key (self-hosted)', () => {
    render(<TelemetrySection {...defaultProps} telemetryStatus="disabled" hasPosHogKey={false} />)
    // Self-hosted path renders a privacy message with no toggle at all
    expect(screen.getByText(/no data leaves your machine/i)).toBeInTheDocument()
    expect(screen.queryByRole('switch')).not.toBeInTheDocument()
  })

  it('shows fully private explanation when self-hosted', () => {
    render(<TelemetrySection {...defaultProps} telemetryStatus="disabled" hasPosHogKey={false} />)
    expect(screen.getByText(/local build with no analytics endpoint/i)).toBeInTheDocument()
  })
})
