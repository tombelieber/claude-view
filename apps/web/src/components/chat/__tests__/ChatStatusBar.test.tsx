import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ChatStatusBar } from '../ChatStatusBar'

describe('ChatStatusBar', () => {
  const defaultProps = {
    model: 'claude-sonnet-4-20250514',
    contextTokens: 45231,
    contextLimit: 200000,
    contextPercent: 23,
    totalCost: 0.42,
  }

  it('displays model name from connection', () => {
    render(<ChatStatusBar {...defaultProps} />)
    expect(screen.getByText('claude-sonnet-4-20250514')).toBeDefined()
  })

  it('formats token count with commas (e.g. 45,231)', () => {
    render(<ChatStatusBar {...defaultProps} />)
    expect(screen.getByText('45,231 / 200,000 (23%)')).toBeDefined()
  })

  it('shows context percentage', () => {
    render(<ChatStatusBar {...defaultProps} contextPercent={87} />)
    expect(screen.getByText(/87%/)).toBeDefined()
  })

  it('shows cost when totalCost is not null', () => {
    render(<ChatStatusBar {...defaultProps} totalCost={1.5} />)
    expect(screen.getByText('$1.50 USD')).toBeDefined()
  })

  it('hides cost section when totalCost is null', () => {
    const { container } = render(<ChatStatusBar {...defaultProps} totalCost={null} />)
    expect(container.textContent).not.toContain('USD')
  })
})
