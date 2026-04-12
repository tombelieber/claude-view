import { useCallback, useEffect, useRef } from 'react'

interface UseFocusFollowsMouseOptions {
  enabled: boolean
  delayMs?: number
}

interface FocusHandlers {
  onPointerEnter: () => void
  onPointerLeave: () => void
}

const DEFAULT_DELAY = 150

export function useFocusFollowsMouse({ enabled, delayMs }: UseFocusFollowsMouseOptions) {
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined)
  const enabledRef = useRef(enabled)
  enabledRef.current = enabled
  const delayRef = useRef(delayMs)
  delayRef.current = delayMs

  // Cleanup on unmount
  useEffect(() => () => clearTimeout(timerRef.current), [])

  const getHandlers = useCallback((activate: () => void): FocusHandlers => {
    return {
      onPointerEnter: () => {
        if (!enabledRef.current) return
        clearTimeout(timerRef.current)
        timerRef.current = setTimeout(activate, delayRef.current ?? DEFAULT_DELAY)
      },
      onPointerLeave: () => {
        clearTimeout(timerRef.current)
      },
    }
  }, [])

  return { getHandlers }
}
