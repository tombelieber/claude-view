import { extractCaller, serviceRoleClient } from '../_shared/auth.ts'
import { correlationIdFrom } from '../_shared/correlation.ts'
import { errorResponse, jsonResponse } from '../_shared/errors.ts'

interface TerminateRequest {
  calling_device_id: string
}

Deno.serve(async (req) => {
  const correlation_id = correlationIdFrom(req)
  if (req.method !== 'POST') {
    return errorResponse('METHOD_NOT_ALLOWED', correlation_id, { got: req.method })
  }

  const caller = extractCaller(req)
  if (!caller) return errorResponse('UNAUTHORIZED', correlation_id)

  let body: TerminateRequest
  try {
    body = await req.json()
  } catch {
    return errorResponse('BAD_REQUEST', correlation_id, { reason: 'invalid_json' })
  }
  if (!body.calling_device_id) return errorResponse('INVALID_DEVICE_ID', correlation_id)

  const sb = serviceRoleClient()

  // Atomic bulk update: mark every non-calling device of this user as revoked.
  const { data: revoked, error: updErr } = await sb
    .from('devices')
    .update({ revoked_at: new Date().toISOString(), revoked_reason: 'bulk_terminate' })
    .eq('user_id', caller.user_id)
    .is('revoked_at', null)
    .neq('device_id', body.calling_device_id)
    .select('device_id')
  if (updErr) return errorResponse('SUPABASE_UNREACHABLE', correlation_id, { pg: updErr.message })

  return jsonResponse({ revoked_count: revoked?.length ?? 0 }, correlation_id)
})
