import { useCallback } from 'react'
import type { SessionOwnership } from '../types/generated/SessionOwnership'

export type InteractResult = { ok: true } | { ok: false; status: number; reason: string }

export type InteractRequest =
  | { variant: 'permission'; requestId: string; allowed: boolean; updatedPermissions?: unknown[] }
  | { variant: 'question'; requestId: string; answers: Record<string, string> }
  | {
      variant: 'plan'
      requestId: string
      approved: boolean
      feedback?: string
      bypassPermissions?: boolean
    }
  | { variant: 'elicitation'; requestId: string; response: string }

/**
 * Returns a callback that dispatches an interaction response to the backend,
 * or `undefined` when the session is not SDK-controlled. Only an SDK fork can
 * receive a decision — observed (read-only mirror) and tmux-owned-only CLI
 * sessions render the interaction read-only (wiring-up rule: never show a
 * control the backend can't honor). Take over a CLI session to drive it.
 */
export function useInteractionResponder(
  sessionId: string,
  ownership: SessionOwnership | null | undefined,
): ((request: InteractRequest) => Promise<InteractResult>) | undefined {
  // useCallback MUST be called unconditionally (React Rules of Hooks)
  const respond = useCallback(
    async (request: InteractRequest): Promise<InteractResult> => {
      try {
        const res = await fetch(`/api/sessions/${sessionId}/interact`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(request),
        })
        if (res.ok) return { ok: true }
        return { ok: false, status: res.status, reason: await res.text() }
      } catch (e) {
        return { ok: false, status: 0, reason: String(e) }
      }
    },
    [sessionId],
  )

  // Interactive only when SDK-controlled. tmux-owned-only / observed sessions
  // are read-only mirrors of the CLI — interaction must go through a fork.
  if (!ownership?.sdk) return undefined
  return respond
}
