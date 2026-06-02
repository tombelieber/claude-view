import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { WatchingBanner } from '../WatchingBanner'

describe('WatchingBanner', () => {
  it('renders the watching copy and a Take over button', () => {
    render(<WatchingBanner onTakeover={vi.fn()} />)
    expect(screen.getByText(/watching live/i)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /take over/i })).toBeInTheDocument()
  })

  it('calls onTakeover when the button is clicked', () => {
    const onTakeover = vi.fn()
    render(<WatchingBanner onTakeover={onTakeover} />)
    fireEvent.click(screen.getByRole('button', { name: /take over/i }))
    expect(onTakeover).toHaveBeenCalledTimes(1)
  })
})
