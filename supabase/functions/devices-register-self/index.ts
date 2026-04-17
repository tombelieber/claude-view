import { extractCaller, serviceRoleClient } from '../_shared/auth.ts'
import { correlationIdFrom } from '../_shared/correlation.ts'
import { errorResponse, jsonResponse } from '../_shared/errors.ts'

// -----------------------------------------------------------------------------
// devices-register-self
//
// The caller (an authenticated user with a valid Supabase JWT) registers
// their OWN device into public.devices. This closes the chicken-and-egg
// gap in the pairing model: pair-offer requires the issuing device to
// already exist, but a freshly-signed-in Mac / web client has no row.
//
// Contract:
//   POST /functions/v1/devices-register-self
//   Body: {
//     device_id, ed25519_pubkey (b64), x25519_pubkey (b64),
//     platform, display_name, app_version?, os_version?
//   }
//   Response (200): { device: { device_id, platform, display_name, created_at, last_seen_at } }
//
// Idempotency: calling this repeatedly for the same (user_id, device_id)
// refreshes last_seen_at / pubkeys / display_name and clears any prior
// revocation. This supports "fresh install on same hardware" without the
// client having to distinguish first-register from re-register.
//
// Error codes (from the shared taxonomy):
//   METHOD_NOT_ALLOWED (non-POST)
//   UNAUTHORIZED (missing / malformed JWT)
//   BAD_REQUEST (invalid JSON body)
//   INVALID_DEVICE_ID (bad id format, bad platform, pubkey wrong length,
//                      zero pubkey, or device_id already belongs to a different user)
//   DEVICE_LIMIT_REACHED (user has 10 active devices)
//   SUPABASE_UNREACHABLE (DB error)
//   INTERNAL (missing RPC = deployment problem)
// -----------------------------------------------------------------------------

interface RegisterSelfRequest {
  device_id: string
  ed25519_pubkey: string // base64
  x25519_pubkey: string // base64
  platform: 'mac' | 'ios' | 'android' | 'web'
  display_name: string
  app_version?: string
  os_version?: string
}

interface RegisterSelfResponse {
  device: {
    device_id: string
    platform: string
    display_name: string
    created_at: string
    last_seen_at: string
  }
}

const DEVICE_ID_RE = /^(mac|ios|android|web)-[0-9a-f]{16}$/
const VALID_PLATFORMS = ['mac', 'ios', 'android', 'web'] as const
const MAX_DEVICES_PER_USER = 10

function bytesFromBase64(b64: string): Uint8Array | null {
  try {
    const bin = atob(b64)
    const out = new Uint8Array(bin.length)
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i)
    return out
  } catch {
    return null
  }
}

Deno.serve(async (req) => {
  const correlation_id = correlationIdFrom(req)
  if (req.method !== 'POST') {
    return errorResponse('METHOD_NOT_ALLOWED', correlation_id, { got: req.method })
  }

  const caller = extractCaller(req)
  if (!caller) return errorResponse('UNAUTHORIZED', correlation_id)

  let body: RegisterSelfRequest
  try {
    body = await req.json()
  } catch {
    return errorResponse('BAD_REQUEST', correlation_id, { reason: 'invalid_json' })
  }

  // -- Validate identifiers ---------------------------------------------------

  if (!body.device_id || !DEVICE_ID_RE.test(body.device_id)) {
    return errorResponse('INVALID_DEVICE_ID', correlation_id, { got: body.device_id })
  }
  if (!VALID_PLATFORMS.includes(body.platform)) {
    return errorResponse('INVALID_DEVICE_ID', correlation_id, { reason: 'bad_platform' })
  }
  // Prevent platform/device_id prefix mismatch. Without this, a Mac could
  // self-register with platform='ios' and confuse downstream classifiers.
  if (!body.device_id.startsWith(`${body.platform}-`)) {
    return errorResponse('INVALID_DEVICE_ID', correlation_id, {
      reason: 'platform_prefix_mismatch',
      device_id: body.device_id,
      platform: body.platform,
    })
  }

  if (!body.display_name || body.display_name.length > 64) {
    return errorResponse('BAD_REQUEST', correlation_id, { reason: 'bad_display_name' })
  }

  // -- Validate pubkeys -------------------------------------------------------

  const ed25519 = bytesFromBase64(body.ed25519_pubkey)
  const x25519 = bytesFromBase64(body.x25519_pubkey)
  if (!ed25519 || ed25519.length !== 32 || !x25519 || x25519.length !== 32) {
    return errorResponse('INVALID_DEVICE_ID', correlation_id, { reason: 'bad_pubkey_length' })
  }
  // Reject the all-zero Curve25519 identity key. Same rationale as
  // pair-claim: an attacker submitting 0x00*32 coerces peer ECDH to a
  // known value (RFC 7748 §5). Full curve-point validation via
  // crypto_scalarmult_base stays a Mac-daemon responsibility (Phase 2).
  if (ed25519.every((b) => b === 0) || x25519.every((b) => b === 0)) {
    return errorResponse('INVALID_DEVICE_ID', correlation_id, { reason: 'zero_pubkey' })
  }

  // -- Atomic upsert via RPC --------------------------------------------------

  const sb = serviceRoleClient()
  const { error: rpcErr } = await sb.rpc('register_device_self', {
    p_device_id: body.device_id,
    p_user_id: caller.user_id,
    p_ed25519_pubkey: body.ed25519_pubkey,
    p_x25519_pubkey: body.x25519_pubkey,
    p_platform: body.platform,
    p_display_name: body.display_name,
    p_app_version: body.app_version ?? null,
    p_os_version: body.os_version ?? null,
  })
  if (rpcErr) {
    const msg = rpcErr.message ?? ''
    if (msg.includes('INVALID_DEVICE_ID')) {
      return errorResponse('INVALID_DEVICE_ID', correlation_id, {
        reason: 'device_id_taken_by_other_user',
      })
    }
    if (msg.includes('DEVICE_LIMIT_REACHED')) {
      return errorResponse('DEVICE_LIMIT_REACHED', correlation_id, { limit: MAX_DEVICES_PER_USER })
    }
    if (rpcErr.code === 'PGRST202' || msg.includes('function public.register_device_self')) {
      return errorResponse('INTERNAL', correlation_id, {
        reason: 'register_device_self_rpc_missing',
        hint: 'apply migration 20260417150000_register_device_self_fn.sql',
      })
    }
    return errorResponse('SUPABASE_UNREACHABLE', correlation_id, { pg: msg, code: rpcErr.code })
  }

  // -- Return the (possibly-refreshed) device row ----------------------------

  const { data: device, error: readErr } = await sb
    .from('devices')
    .select('device_id,platform,display_name,created_at,last_seen_at')
    .eq('device_id', body.device_id)
    .single()
  if (readErr || !device) {
    return errorResponse('SUPABASE_UNREACHABLE', correlation_id, { pg: readErr?.message })
  }

  const response: RegisterSelfResponse = { device }
  return jsonResponse(response, correlation_id)
})
