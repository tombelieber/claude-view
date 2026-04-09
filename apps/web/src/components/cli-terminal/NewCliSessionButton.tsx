import { Terminal } from 'lucide-react'
import { useCallback, useState } from 'react'

interface NewCliSessionButtonProps {
  onSessionCreated?: (sessionId: string) => void
  projectDir?: string
}

export function NewCliSessionButton({ onSessionCreated, projectDir }: NewCliSessionButtonProps) {
  const [isCreating, setIsCreating] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleCreate = useCallback(async () => {
    setIsCreating(true)
    setError(null)
    try {
      const resp = await fetch('/api/cli-sessions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ projectDir }),
      })
      if (!resp.ok) {
        const data = await resp.json().catch(() => ({ error: 'Unknown error' }))
        throw new Error(data.details ?? data.error ?? `HTTP ${resp.status}`)
      }
      const { session } = await resp.json()
      onSessionCreated?.(session.id)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create session')
    } finally {
      setIsCreating(false)
    }
  }, [projectDir, onSessionCreated])

  return (
    <div className="inline-flex items-center gap-2">
      <button
        type="button"
        onClick={handleCreate}
        disabled={isCreating}
        className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md transition-colors bg-emerald-600 text-white hover:bg-emerald-700 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {isCreating ? (
          <span className="w-3 h-3 border-2 border-white/30 border-t-white rounded-full animate-spin" />
        ) : (
          <Terminal className="w-3.5 h-3.5" />
        )}
        New CLI Session
      </button>
      {error && <span className="text-xs text-red-500">{error}</span>}
    </div>
  )
}
