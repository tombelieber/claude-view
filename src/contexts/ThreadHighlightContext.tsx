import { createContext, useContext, useState, useCallback, type ReactNode } from 'react'

interface ThreadHighlightState {
  highlightedUuids: Set<string>
  setHighlightedUuids: (uuids: Set<string>) => void
  clearHighlight: () => void
}

const EMPTY_SET = new Set<string>()

const NOOP_STATE: ThreadHighlightState = {
  highlightedUuids: EMPTY_SET,
  setHighlightedUuids: () => {},
  clearHighlight: () => {},
}

const ThreadHighlightContext = createContext<ThreadHighlightState | null>(null)

export function ThreadHighlightProvider({ children }: { children: ReactNode }) {
  const [highlightedUuids, setHighlightedUuids] = useState<Set<string>>(EMPTY_SET)
  const clearHighlight = useCallback(() => setHighlightedUuids(EMPTY_SET), [])

  return (
    <ThreadHighlightContext.Provider value={{ highlightedUuids, setHighlightedUuids, clearHighlight }}>
      {children}
    </ThreadHighlightContext.Provider>
  )
}

export function useThreadHighlight(): ThreadHighlightState {
  const ctx = useContext(ThreadHighlightContext)
  return ctx ?? NOOP_STATE
}
