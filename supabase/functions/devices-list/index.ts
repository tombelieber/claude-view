import { extractCaller, serviceRoleClient } from '../_shared/auth.ts'
import { correlationIdFrom } from '../_shared/correlation.ts'
import { errorResponse, jsonResponse } from '../_shared/errors.ts'

Deno.serve(async (req) => {
  const correlation_id = correlationIdFrom(req)
  if (req.method !== 'GET') {
    return errorResponse('METHOD_NOT_ALLOWED', correlation_id, { got: req.method })
  }

  const caller = extractCaller(req)
  if (!caller) return errorResponse('UNAUTHORIZED', correlation_id)

  const sb = serviceRoleClient()
  const { data, error } = await sb
    .from('devices')
    .select(
      'device_id,platform,display_name,app_version,os_version,created_at,last_seen_at,revoked_at,revoked_reason',
    )
    .eq('user_id', caller.user_id)
    .order('created_at', { ascending: false })
  if (error) return errorResponse('SUPABASE_UNREACHABLE', correlation_id, { pg: error.message })

  return jsonResponse({ devices: data ?? [] }, correlation_id)
})
