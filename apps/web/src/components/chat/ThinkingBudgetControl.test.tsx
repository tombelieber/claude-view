import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ThinkingBudgetControl } from './ThinkingBudgetControl'

describe('ThinkingBudgetControl', () => {
  it('renders with null default (shows "Default")', () => {
    render(<ThinkingBudgetControl value={null} onChange={vi.fn()} />)
    expect(screen.getByTitle('Thinking budget')).toHaveValue('null')
  })

  it('onChange fires null when "Default" selected', () => {
    const onChange = vi.fn()
    render(<ThinkingBudgetControl value={4096} onChange={onChange} />)
    fireEvent.change(screen.getByTitle('Thinking budget'), { target: { value: 'null' } })
    expect(onChange).toHaveBeenCalledWith(null)
  })

  it('onChange fires numeric value when preset selected', () => {
    const onChange = vi.fn()
    render(<ThinkingBudgetControl value={null} onChange={onChange} />)
    fireEvent.change(screen.getByTitle('Thinking budget'), { target: { value: '4096' } })
    expect(onChange).toHaveBeenCalledWith(4096)
  })

  it('disables select when disabled prop is true', () => {
    render(<ThinkingBudgetControl value={null} onChange={vi.fn()} disabled />)
    expect(screen.getByTitle('Thinking budget')).toBeDisabled()
  })
})
