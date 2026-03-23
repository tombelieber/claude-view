import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

const mockSetDisplayMode = vi.fn()
vi.mock('../../store/monitor-store', () => ({
  useMonitorStore: vi.fn((sel: any) =>
    sel({ displayMode: 'chat', setDisplayMode: mockSetDisplayMode }),
  ),
}))

import { DisplayModeToggle } from './DisplayModeToggle'

describe('DisplayModeToggle', () => {
  it('renders Chat and Developer buttons', () => {
    render(<DisplayModeToggle />)
    expect(screen.getByText('Chat')).toBeInTheDocument()
    expect(screen.getByText('Developer')).toBeInTheDocument()
  })

  it('calls setDisplayMode on click', () => {
    render(<DisplayModeToggle />)
    fireEvent.click(screen.getByText('Developer'))
    expect(mockSetDisplayMode).toHaveBeenCalledWith('developer')
  })
})
