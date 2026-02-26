import { useEffect } from 'react'
import { useAppStore } from '../store/app-store'
import type { Theme } from '../store/app-store'

function getSystemPreference(): 'light' | 'dark' {
  if (typeof window === 'undefined') return 'light'
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

export function useTheme() {
  const theme = useAppStore((s) => s.theme)
  const setTheme = useAppStore((s) => s.setTheme)
  const cycleTheme = useAppStore((s) => s.cycleTheme)

  const resolvedTheme: 'light' | 'dark' =
    theme === 'system' ? getSystemPreference() : theme

  // Apply dark class to <html> and listen for OS changes in system mode
  useEffect(() => {
    const root = document.documentElement

    function apply(resolved: 'light' | 'dark') {
      if (resolved === 'dark') {
        root.classList.add('dark')
      } else {
        root.classList.remove('dark')
      }
    }

    apply(resolvedTheme)

    if (theme === 'system') {
      const mql = window.matchMedia('(prefers-color-scheme: dark)')
      const onChange = (e: MediaQueryListEvent) => apply(e.matches ? 'dark' : 'light')
      mql.addEventListener('change', onChange)
      return () => mql.removeEventListener('change', onChange)
    }
  }, [theme, resolvedTheme])

  return { theme, resolvedTheme, setTheme, cycleTheme }
}
