import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TakeoverConfirmDialog } from '../TakeoverConfirmDialog'

beforeEach(() => {
  localStorage.clear()
})

afterEach(() => {
  localStorage.clear()
})

describe('TakeoverConfirmDialog', () => {
  it('renders confirmation title and description', () => {
    render(<TakeoverConfirmDialog open onConfirm={vi.fn()} onCancel={vi.fn()} />)

    expect(screen.getByText('Continue in Claude View?')).toBeInTheDocument()
    expect(screen.getByText(/started outside claude-view/i)).toBeInTheDocument()
  })

  it('dontRemind checkbox sets localStorage on confirm', () => {
    const onConfirm = vi.fn()
    render(<TakeoverConfirmDialog open onConfirm={onConfirm} onCancel={vi.fn()} />)

    // Check the "Don't remind me" checkbox
    const checkbox = screen.getByRole('checkbox')
    fireEvent.click(checkbox)

    // Click Take Control
    fireEvent.click(screen.getByRole('button', { name: /fork & continue/i }))

    expect(onConfirm).toHaveBeenCalled()
    expect(localStorage.getItem('claude-view:takeover-no-remind')).toBe('true')
  })

  it('does NOT set localStorage when checkbox unchecked', () => {
    const onConfirm = vi.fn()
    render(<TakeoverConfirmDialog open onConfirm={onConfirm} onCancel={vi.fn()} />)

    // Don't check the checkbox, just confirm
    fireEvent.click(screen.getByRole('button', { name: /fork & continue/i }))

    expect(onConfirm).toHaveBeenCalled()
    expect(localStorage.getItem('claude-view:takeover-no-remind')).toBeNull()
  })

  it('onConfirm callback called on Take Control click', () => {
    const onConfirm = vi.fn()
    render(<TakeoverConfirmDialog open onConfirm={onConfirm} onCancel={vi.fn()} />)

    fireEvent.click(screen.getByRole('button', { name: /fork & continue/i }))
    expect(onConfirm).toHaveBeenCalledTimes(1)
  })

  it('onCancel callback called on Cancel click', () => {
    const onCancel = vi.fn()
    render(<TakeoverConfirmDialog open onConfirm={vi.fn()} onCancel={onCancel} />)

    fireEvent.click(screen.getByRole('button', { name: /cancel/i }))
    expect(onCancel).toHaveBeenCalledTimes(1)
  })
})
