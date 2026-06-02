// sidecar/src/interaction-resolver.ts
// Single resolution primitive for frontend interaction decisions.
//
// Both transports converge here:
//   - the control WebSocket (`ws-handler.ts`, used by the in-app chat panel), and
//   - the REST delivery bridge (`POST /api/sidecar/sessions/:id/interact`, used by
//     the Rust server for monitor/mobile surfaces that have no live control WS).
//
// Returning one `{ ok }` shape from one function keeps the two paths from drifting
// (one-architecture-per-concern) and gives the Rust server an explicit delivery ack
// so it only clears pending state once a decision was actually applied.
import type { PermissionUpdate } from '@anthropic-ai/claude-agent-sdk'
import type {
  ElicitationResponse,
  PermissionResponse,
  PlanResponseMsg,
  QuestionResponse,
} from './protocol.js'
import type { ControlSession } from './session-registry.js'

/** A user's decision on a pending interaction, delivered over WS or REST. */
export type InteractionResponse =
  | PermissionResponse
  | QuestionResponse
  | PlanResponseMsg
  | ElicitationResponse

/** Outcome of applying a decision. `ok:false` means the requestId was unknown
 *  (already resolved, aborted, or stale) — the caller must NOT report success. */
export interface ResolveResult {
  ok: boolean
  reason?: string
}

/**
 * Apply a frontend decision to the session's matching pending request.
 *
 * Never throws: an unknown `requestId` returns `{ ok: false, reason }` so the
 * caller can surface "already resolved" rather than fabricate an outcome.
 */
export function resolveInteraction(
  session: ControlSession,
  msg: InteractionResponse,
): ResolveResult {
  const { permissions } = session
  switch (msg.type) {
    case 'permission_response':
      return permissions.resolvePermission(
        msg.requestId,
        msg.allowed,
        msg.updatedPermissions as PermissionUpdate[] | undefined,
      )
        ? { ok: true }
        : { ok: false, reason: 'Unknown permission requestId' }
    case 'question_response':
      return permissions.resolveQuestion(msg.requestId, msg.answers)
        ? { ok: true }
        : { ok: false, reason: 'Unknown question requestId' }
    case 'plan_response':
      return permissions.resolvePlan(
        msg.requestId,
        msg.approved,
        msg.feedback,
        msg.bypassPermissions,
      )
        ? { ok: true }
        : { ok: false, reason: 'Unknown plan requestId' }
    case 'elicitation_response':
      return permissions.resolveElicitation(msg.requestId, msg.response)
        ? { ok: true }
        : { ok: false, reason: 'Unknown elicitation requestId' }
    default:
      return {
        ok: false,
        reason: `Unsupported interaction type: ${(msg as { type?: string }).type}`,
      }
  }
}
