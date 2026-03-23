import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ThinkingBudgetControl } from './ThinkingBudgetControl'

describe('ThinkingBudgetControl', () => {
  it('displays Max when value is null', () => {
    render(<ThinkingBudgetControl value={null} onChange={vi.fn()} />)
    expect(screen.getByTitle('Thinking budget')).toBeInTheDocument()
    expect(screen.getByText('(Max)')).toBeInTheDocument()
  })

  it('displays correct level for token value', () => {
    render(<ThinkingBudgetControl value={4096} onChange={vi.fn()} />)
    expect(screen.getByText('(Medium)')).toBeInTheDocument()
  })

  it('fires onChange with tokens when dot clicked', () => {
    const onChange = vi.fn()
    render(<ThinkingBudgetControl value={0} onChange={onChange} />)
    fireEvent.click(screen.getByTitle('Low'))
    expect(onChange).toHaveBeenCalledWith(1024)
  })

  it('fires onChange(0) when Max clicked', () => {
    const onChange = vi.fn()
    render(<ThinkingBudgetControl value={4096} onChange={onChange} />)
    fireEvent.click(screen.getByTitle('Max'))
    expect(onChange).toHaveBeenCalledWith(0)
  })

  it('disables dots when disabled prop is true', () => {
    render(<ThinkingBudgetControl value={null} onChange={vi.fn()} disabled />)
    const buttons = screen.getAllByRole('button')
    for (const btn of buttons) {
      expect(btn).toBeDisabled()
    }
  })
})
