import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { LayoutModeToggle } from '../LayoutModeToggle'

describe('LayoutModeToggle', () => {
  it('renders Auto and Custom buttons', () => {
    render(<LayoutModeToggle mode="auto-grid" onToggle={() => {}} />)
    expect(screen.getByText('Auto')).toBeDefined()
    expect(screen.getByText('Custom')).toBeDefined()
  })

  it('highlights Auto when mode is auto-grid', () => {
    render(<LayoutModeToggle mode="auto-grid" onToggle={() => {}} />)
    const autoBtn = screen.getByTitle('Auto: responsive grid')
    expect(autoBtn.getAttribute('aria-pressed')).toBe('true')
    const customBtn = screen.getByTitle('Custom: drag to arrange')
    expect(customBtn.getAttribute('aria-pressed')).toBe('false')
  })

  it('highlights Custom when mode is custom', () => {
    render(<LayoutModeToggle mode="custom" onToggle={() => {}} />)
    const autoBtn = screen.getByTitle('Auto: responsive grid')
    expect(autoBtn.getAttribute('aria-pressed')).toBe('false')
    const customBtn = screen.getByTitle('Custom: drag to arrange')
    expect(customBtn.getAttribute('aria-pressed')).toBe('true')
  })

  it('calls onToggle when either button is clicked', () => {
    const onToggle = vi.fn()
    render(<LayoutModeToggle mode="auto-grid" onToggle={onToggle} />)
    fireEvent.click(screen.getByText('Auto'))
    expect(onToggle).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByText('Custom'))
    expect(onToggle).toHaveBeenCalledTimes(2)
  })
})
