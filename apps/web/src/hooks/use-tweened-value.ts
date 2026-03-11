import { useEffect, useRef, useState } from 'react'

const DURATION_MS = 200

function easeOut(t: number): number {
  return 1 - (1 - t) * (1 - t)
}

export function useTweenedValue(target: number): number {
  const [value, setValue] = useState(target)
  const rafRef = useRef<number | null>(null)
  const startRef = useRef({ value: target, time: 0 })

  useEffect(() => {
    const prefersReduced =
      typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches

    if (prefersReduced) {
      setValue(target)
      return
    }

    const from = value
    if (from === target) return

    startRef.current = { value: from, time: performance.now() }

    const animate = (now: number) => {
      const elapsed = now - startRef.current.time
      const progress = Math.min(elapsed / DURATION_MS, 1)
      const eased = easeOut(progress)
      const current = from + (target - from) * eased

      setValue(current)

      if (progress < 1) {
        rafRef.current = requestAnimationFrame(animate)
      } else {
        setValue(target)
        rafRef.current = null
      }
    }

    rafRef.current = requestAnimationFrame(animate)

    return () => {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current)
        rafRef.current = null
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [target])

  return value
}
