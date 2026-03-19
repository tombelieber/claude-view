// apps/web/src/hooks/use-available-sessions.ts
import { useCallback, useEffect, useState } from 'react'

// AvailableSession shape matches sidecar/src/protocol.ts AvailableSession.
// Defined inline here because the sidecar is a separate TS project (not a shared
// workspace package). If a shared types package is added later, import from there.
export interface AvailableSession {
  sessionId: string
  summary: string
  lastModified: number
  fileSize: number
  customTitle?: string
  firstPrompt?: string
  gitBranch?: string
  cwd?: string
}

export function useAvailableSessions() {
  const [sessions, setSessions] = useState<AvailableSession[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const res = await fetch('/api/sessions')
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = await res.json()
      setSessions(data)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    refresh()
  }, [refresh])

  return { sessions, loading, error, refresh }
}
