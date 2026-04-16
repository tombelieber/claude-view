import { extractCaller, serviceRoleClient } from '../_shared/auth.ts'
import { correlationIdFrom } from '../_shared/correlation.ts'
import { errorResponse, jsonResponse } from '../_shared/errors.ts'

interface RevokeRequest {
  device_id: string
  reason?: 'user_action' | 'bulk_terminate'
}

Deno.serve(async (req) => {
  const correlation_id = correlationIdFrom(req)
  if (req.method !== 'POST') {
    return errorResponse('METHOD_NOT_ALLOWED', correlation_id, { got: req.method })
  }

  const caller = extractCaller(req)
  if (!caller) return errorResponse('UNAUTHORIZED', correlation_id)

  let body: RevokeRequest
  try {
    body = await req.json()
  } catch {
    return errorResponse('BAD_REQUEST', correlation_id, { reason: 'invalid_json' })
  }
  if (!body.device_id) return errorResponse('INVALID_DEVICE_ID', correlation_id)

  const sb = serviceRoleClient()
  const { data: device, error: findErr } = await sb
    .from('devices')
    .select('device_id,user_id,revoked_at')
    .eq('device_id', body.device_id)
    .maybeSingle()
  if (findErr) return errorResponse('SUPABASE_UNREACHABLE', correlation_id, { pg: findErr.message })
  if (!device) return errorResponse('DEVICE_NOT_FOUND', correlation_id)
  if (device.user_id !== caller.user_id) return errorResponse('DEVICE_NOT_FOUND', correlation_id) // leak-free: same message
  if (device.revoked_at !== null) return errorResponse('ALREADY_REVOKED', correlation_id)

  const { data: updated, error: updErr } = await sb
    .from('devices')
    .update({ revoked_at: new Date().toISOString(), revoked_reason: body.reason ?? 'user_action' })
    .eq('device_id', body.device_id)
    .select('device_id,platform,display_name,revoked_at,revoked_reason')
    .single()
  if (updErr || !updated) {
    return errorResponse('SUPABASE_UNREACHABLE', correlation_id, { pg: updErr?.message })
  }

  return jsonResponse({ device: updated }, correlation_id)
})
