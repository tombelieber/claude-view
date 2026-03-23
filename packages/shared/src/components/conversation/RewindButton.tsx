import { useCallback, useState } from 'react'

interface RewindButtonProps {
  userMessageId: string
  rewindFiles: (userMessageId: string, opts?: { dryRun?: boolean }) => Promise<unknown>
}

export function RewindButton({ userMessageId, rewindFiles }: RewindButtonProps) {
  const [loading, setLoading] = useState(false)

  const handleRewind = useCallback(async () => {
    setLoading(true)
    try {
      await rewindFiles(userMessageId, { dryRun: true })
      const confirmed = window.confirm('Revert files changed by this message?')
      if (confirmed) {
        await rewindFiles(userMessageId)
      }
    } catch (err) {
      console.error('Rewind failed:', err)
    } finally {
      setLoading(false)
    }
  }, [userMessageId, rewindFiles])

  return (
    <button
      type="button"
      onClick={handleRewind}
      disabled={loading}
      className="opacity-0 group-hover:opacity-100 transition-opacity p-1 rounded hover:bg-bg-secondary"
      title="Undo file changes from this message"
    >
      {loading ? '...' : '\u21a9'}
    </button>
  )
}
