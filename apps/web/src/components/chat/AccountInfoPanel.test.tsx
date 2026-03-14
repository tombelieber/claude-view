import { render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { AccountInfoPanel } from './AccountInfoPanel'

describe('AccountInfoPanel', () => {
  it('shows loading state while fetching', () => {
    const queryAccountInfo = vi.fn().mockReturnValue(new Promise(() => {}))
    render(<AccountInfoPanel queryAccountInfo={queryAccountInfo} />)
    expect(screen.getByText('Loading...')).toBeInTheDocument()
  })

  it('shows error state on failure', async () => {
    const queryAccountInfo = vi.fn().mockRejectedValue(new Error('fail'))
    render(<AccountInfoPanel queryAccountInfo={queryAccountInfo} />)
    await waitFor(() => {
      expect(screen.getByText('Failed to load account info')).toBeInTheDocument()
    })
  })

  it('renders data on success', async () => {
    const queryAccountInfo = vi.fn().mockResolvedValue({ plan: 'pro', usage: 42 })
    render(<AccountInfoPanel queryAccountInfo={queryAccountInfo} />)
    await waitFor(() => {
      expect(screen.getByText(/pro/)).toBeInTheDocument()
    })
  })
})
