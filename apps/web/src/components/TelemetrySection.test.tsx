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

  it('checkbox is checked when telemetry is enabled', () => {
    render(<TelemetrySection {...defaultProps} telemetryStatus="enabled" />)
    const checkbox = screen.getByRole('checkbox')
    expect(checkbox).toBeChecked()
  })

  it('checkbox is unchecked when telemetry is disabled', () => {
    render(<TelemetrySection {...defaultProps} telemetryStatus="disabled" />)
    const checkbox = screen.getByRole('checkbox')
    expect(checkbox).not.toBeChecked()
  })

  it('calls onEnable when toggled on', () => {
    const onEnable = vi.fn()
    render(<TelemetrySection {...defaultProps} telemetryStatus="disabled" onEnable={onEnable} />)
    fireEvent.click(screen.getByRole('checkbox'))
    expect(onEnable).toHaveBeenCalledOnce()
  })

  it('calls onDisable when toggled off', () => {
    const onDisable = vi.fn()
    render(<TelemetrySection {...defaultProps} telemetryStatus="enabled" onDisable={onDisable} />)
    fireEvent.click(screen.getByRole('checkbox'))
    expect(onDisable).toHaveBeenCalledOnce()
  })

  it('checkbox is disabled when no posthog key (self-hosted)', () => {
    render(<TelemetrySection {...defaultProps} telemetryStatus="disabled" hasPosHogKey={false} />)
    const checkbox = screen.getByRole('checkbox')
    expect(checkbox).toBeDisabled()
  })

  it('shows "not available" text when self-hosted', () => {
    render(<TelemetrySection {...defaultProps} telemetryStatus="disabled" hasPosHogKey={false} />)
    expect(screen.getByText(/not available/i)).toBeInTheDocument()
  })
})
