import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { SessionStatusDot } from '../SessionStatusDot'

describe('SessionStatusDot', () => {
  it('renders green pulse for active status', () => {
    const { container } = render(<SessionStatusDot status="active" />)
    const dot = container.querySelector('span')
    expect(dot?.className).toContain('bg-green-500')
    expect(dot?.className).toContain('animate-pulse')
  })

  it('renders green solid for idle status', () => {
    const { container } = render(<SessionStatusDot status="idle" />)
    const dot = container.querySelector('span')
    expect(dot?.className).toContain('bg-green-500')
    expect(dot?.className).not.toContain('animate-pulse')
  })

  it('renders gray ring for watching status', () => {
    const { container } = render(<SessionStatusDot status="watching" />)
    const dot = container.querySelector('span')
    expect(dot?.className).toContain('bg-slate-400')
    expect(dot?.className).toContain('ring-1')
  })

  it('renders red for error status', () => {
    const { container } = render(<SessionStatusDot status="error" />)
    const dot = container.querySelector('span')
    expect(dot?.className).toContain('bg-red-500')
  })

  it('renders gray for ended status', () => {
    const { container } = render(<SessionStatusDot status="ended" />)
    const dot = container.querySelector('span')
    expect(dot?.className).toContain('bg-slate-500')
  })

  it('renders amber animate-ping for permissionPending=true', () => {
    const { container } = render(<SessionStatusDot status="active" permissionPending />)
    // Should render the ping variant with two spans inside
    const spans = container.querySelectorAll('span')
    // Outer wrapper + ping span + solid span = 3
    expect(spans.length).toBe(3)
    // The ping span should have animate-ping and amber color
    const pingSpan = spans[1]
    expect(pingSpan?.className).toContain('animate-ping')
    expect(pingSpan?.className).toContain('bg-amber-500')
    // The solid inner span
    const solidSpan = spans[2]
    expect(solidSpan?.className).toContain('bg-amber-500')
  })
})
