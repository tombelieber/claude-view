import { extractCaller, serviceRoleClient } from '../_shared/auth.ts'
import { correlationIdFrom } from '../_shared/correlation.ts'
import { errorResponse, jsonResponse } from '../_shared/errors.ts'

interface PairOfferRequest {
  issuing_device_id: string
}

interface PairOfferResponse {
  token: string
  relay_ws_url: string
  expires_at: string
}

const DEVICE_ID_RE = /^(mac|ios|android|web)-[0-9a-f]{16}$/
const RELAY_WS_URL = Deno.env.get('RELAY_WS_URL') ?? 'wss://claude-view-relay.fly.dev/ws'

Deno.serve(async (req) => {
  const correlation_id = correlationIdFrom(req)
  if (req.method !== 'POST') {
    return errorResponse('METHOD_NOT_ALLOWED', correlation_id, { got: req.method })
  }

  const caller = extractCaller(req)
  if (!caller) return errorResponse('UNAUTHORIZED', correlation_id)

  let body: PairOfferRequest
  try {
    body = await req.json()
  } catch {
    return errorResponse('BAD_REQUEST', correlation_id, { reason: 'invalid_json' })
  }

  if (!body.issuing_device_id || !DEVICE_ID_RE.test(body.issuing_device_id)) {
    return errorResponse('INVALID_DEVICE_ID', correlation_id, { got: body.issuing_device_id })
  }

  const sb = serviceRoleClient()

  // Verify the issuing device exists and belongs to this user.
  const { data: issuingDevice, error: deviceErr } = await sb
    .from('devices')
    .select('device_id,user_id,revoked_at')
    .eq('device_id', body.issuing_device_id)
    .maybeSingle()
  if (deviceErr) {
    return errorResponse('SUPABASE_UNREACHABLE', correlation_id, { pg: deviceErr.message })
  }
  if (
    !issuingDevice ||
    issuingDevice.user_id !== caller.user_id ||
    issuingDevice.revoked_at !== null
  ) {
    return errorResponse('ISSUING_DEVICE_NOT_FOUND', correlation_id)
  }

  // Generate 32-byte random token → base64url (43 chars, no padding).
  const tokenBytes = crypto.getRandomValues(new Uint8Array(32))
  const token = btoa(String.fromCharCode(...tokenBytes))
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replaceAll('=', '')

  const expires_at = new Date(Date.now() + 5 * 60 * 1000).toISOString()

  const { error: insertErr } = await sb.from('pairing_offers').insert({
    token,
    user_id: caller.user_id,
    issuing_device_id: body.issuing_device_id,
    expires_at,
  })
  if (insertErr) {
    return errorResponse('SUPABASE_UNREACHABLE', correlation_id, { pg: insertErr.message })
  }

  const response: PairOfferResponse = { token, relay_ws_url: RELAY_WS_URL, expires_at }
  return jsonResponse(response, correlation_id)
})
