import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { RewindButton } from './RewindButton'

describe('RewindButton', () => {
  let mockRewindFiles: ReturnType<typeof vi.fn> &
    ((userMessageId: string, opts?: { dryRun?: boolean }) => Promise<unknown>)

  beforeEach(() => {
    vi.restoreAllMocks()
    mockRewindFiles = vi.fn() as typeof mockRewindFiles
  })

  it('calls rewindFiles with dryRun on first click', async () => {
    mockRewindFiles.mockResolvedValue({ files: ['a.ts'] })
    window.confirm = vi.fn().mockReturnValue(false)

    render(<RewindButton userMessageId="msg-1" rewindFiles={mockRewindFiles} />)
    fireEvent.click(screen.getByTitle('Undo file changes from this message'))

    await waitFor(() => {
      expect(mockRewindFiles).toHaveBeenCalledWith('msg-1', { dryRun: true })
    })
  })

  it('calls rewindFiles without dryRun when confirmed', async () => {
    mockRewindFiles.mockResolvedValue({ files: ['a.ts'] })
    window.confirm = vi.fn().mockReturnValue(true)

    render(<RewindButton userMessageId="msg-1" rewindFiles={mockRewindFiles} />)
    fireEvent.click(screen.getByTitle('Undo file changes from this message'))

    await waitFor(() => {
      expect(mockRewindFiles).toHaveBeenCalledTimes(2)
      expect(mockRewindFiles).toHaveBeenLastCalledWith('msg-1')
    })
  })

  it('shows loading state while processing', async () => {
    let resolvePromise!: () => void
    mockRewindFiles.mockImplementation(
      () =>
        new Promise((r) => {
          resolvePromise = r as () => void
        }),
    )

    render(<RewindButton userMessageId="msg-1" rewindFiles={mockRewindFiles} />)
    fireEvent.click(screen.getByTitle('Undo file changes from this message'))

    expect(screen.getByRole('button')).toBeDisabled()
    resolvePromise()
  })

  it('handles error without crashing', async () => {
    mockRewindFiles.mockRejectedValue(new Error('server error'))
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {})

    render(<RewindButton userMessageId="msg-1" rewindFiles={mockRewindFiles} />)
    fireEvent.click(screen.getByTitle('Undo file changes from this message'))

    await waitFor(() => {
      expect(consoleSpy).toHaveBeenCalled()
    })
    expect(screen.getByRole('button')).not.toBeDisabled()
  })
})
