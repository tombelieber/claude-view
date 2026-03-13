import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { SessionRollupBar } from '../SessionRollupBar'

describe('SessionRollupBar', () => {
  it('renders label and formatted percentage', () => {
    render(<SessionRollupBar label="CPU" value={28} max={1000} suffix="of 10 cores" />)
    expect(screen.getByText('CPU')).toBeInTheDocument()
    // 28/1000 = 2.8%
    expect(screen.getByText('2.8%')).toBeInTheDocument()
    expect(screen.getByText('of 10 cores')).toBeInTheDocument()
  })

  it('renders green bar when below 70%', () => {
    const { container } = render(<SessionRollupBar label="CPU" value={50} max={1000} />)
    const fill = container.querySelector('[data-testid="rollup-fill"]')
    expect(fill?.className).toContain('bg-green-500')
  })

  it('renders amber bar at 70-89%', () => {
    const { container } = render(<SessionRollupBar label="CPU" value={750} max={1000} />)
    const fill = container.querySelector('[data-testid="rollup-fill"]')
    expect(fill?.className).toContain('bg-amber-500')
  })

  it('renders red bar at 90%+', () => {
    const { container } = render(<SessionRollupBar label="CPU" value={950} max={1000} />)
    const fill = container.querySelector('[data-testid="rollup-fill"]')
    expect(fill?.className).toContain('bg-red-500')
  })

  it('caps bar width at 100% when value exceeds max', () => {
    const { container } = render(<SessionRollupBar label="CPU" value={1200} max={1000} />)
    const fill = container.querySelector('[data-testid="rollup-fill"]') as HTMLElement
    expect(fill?.style.width).toBe('100%')
  })

  it('handles zero max gracefully', () => {
    render(<SessionRollupBar label="CPU" value={0} max={0} />)
    expect(screen.getByText('0.0%')).toBeInTheDocument()
  })

  it('has correct accessibility attributes', () => {
    render(<SessionRollupBar label="RAM" value={4200000000} max={32000000000} suffix="of 32 GB" />)
    const bar = screen.getByRole('progressbar')
    expect(bar).toHaveAttribute('aria-valuenow')
    expect(bar).toHaveAttribute('aria-valuemax', '100')
  })

  it('renders custom formatValue when provided', () => {
    render(
      <SessionRollupBar
        label="RAM"
        value={4200000000}
        max={32000000000}
        formatValue={(v) => `${(v / 1e9).toFixed(1)} GB`}
      />,
    )
    expect(screen.getByText('4.2 GB')).toBeInTheDocument()
  })
})
