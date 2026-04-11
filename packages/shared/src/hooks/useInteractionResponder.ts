import { useCallback } from 'react'

export type InteractResult = { ok: true } | { ok: false; status: number; reason: string }

export type SessionOwnership =
  | { tier: 'sdk'; controlId: string; source: string | null; entrypoint: string | null }
  | { tier: 'tmux'; cliSessionId: string; source: string | null; entrypoint: string | null }
  | { tier: 'observed'; source: string | null; entrypoint: string | null }

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
 * or `undefined` when the session cannot be interacted with (null ownership
 * or observation-only tier).
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

  if (!ownership || ownership.tier === 'observed') return undefined
  return respond
}
