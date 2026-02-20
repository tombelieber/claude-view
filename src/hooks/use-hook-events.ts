import { useState, useEffect } from 'react'
import type { HookEventItem } from '../components/live/action-log/types'

/**
 * Fetch stored hook events for a historical session.
 * Returns empty array for sessions with no stored events (old sessions).
 */
export function useHookEvents(sessionId: string, enabled: boolean): HookEventItem[] {
  const [events, setEvents] = useState<HookEventItem[]>([])

  useEffect(() => {
    if (!enabled) {
      setEvents([])
      return
    }
    let cancelled = false

    fetch(`/api/sessions/${encodeURIComponent(sessionId)}/hook-events`)
      .then((r) => r.json())
      .then((data) => {
        if (cancelled) return
        const items: HookEventItem[] = (data.hookEvents ?? []).map((e: Record<string, unknown>, i: number) => ({
          id: `hook-${i}`,
          type: 'hook_event' as const,
          timestamp: e.timestamp as number,
          eventName: e.eventName as string,
          toolName: e.toolName as string | undefined,
          label: e.label as string,
          group: e.group as HookEventItem['group'],
          context: e.context as string | undefined,
        }))
        setEvents(items)
      })
      .catch(() => {
        if (!cancelled) setEvents([])
      })

    return () => { cancelled = true }
  }, [sessionId, enabled])

  return events
}
