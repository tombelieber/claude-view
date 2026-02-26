import { useState, useEffect } from 'react'
import { getHighlighter, getHighlighterSync } from '../lib/shiki'

export function useShikiHighlighter() {
  const [ready, setReady] = useState(() => getHighlighterSync() !== null)

  useEffect(() => {
    if (!ready) {
      getHighlighter().then(() => setReady(true))
    }
  }, [ready])

  return getHighlighterSync()
}
