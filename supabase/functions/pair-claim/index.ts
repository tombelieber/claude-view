import { correlationIdFrom } from '../_shared/correlation.ts'
import { errorResponse, jsonResponse } from '../_shared/errors.ts'
import { extractCaller, serviceRoleClient } from '../_shared/auth.ts'

interface PairClaimRequest {
  token: string
  device_id: string
  ed25519_pubkey: string // base64
  x25519_pubkey: string // base64
  platform: 'ios' | 'android' | 'web'
  display_name: string
  app_version?: string
  os_version?: string
}

interface PairClaimResponse {
  device: {
    device_id: string
    platform: string
    display_name: string
    created_at: string
  }
  peer_devices: Array<{
    device_id: string
    platform: string
    display_name: string
    x25519_pubkey: string // base64
  }>
}

const DEVICE_ID_RE = /^(mac|ios|android|web)-[0-9a-f]{16}$/
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

function base64FromBytes(bytes: Uint8Array): string {
  let s = ''
  for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i])
  return btoa(s)
}

Deno.serve(async (req) => {
  const correlation_id = correlationIdFrom(req)
  if (req.method !== 'POST') {
    return errorResponse('METHOD_NOT_ALLOWED', correlation_id, { got: req.method })
  }

  const caller = extractCaller(req)
  if (!caller) return errorResponse('UNAUTHORIZED', correlation_id)

  let body: PairClaimRequest
  try {
    body = await req.json()
  } catch {
    return errorResponse('BAD_REQUEST', correlation_id, { reason: 'invalid_json' })
  }

  if (!body.device_id || !DEVICE_ID_RE.test(body.device_id)) {
    return errorResponse('INVALID_DEVICE_ID', correlation_id, { got: body.device_id })
  }
  if (!['ios', 'android', 'web'].includes(body.platform)) {
    return errorResponse('INVALID_DEVICE_ID', correlation_id, { reason: 'bad_platform' })
  }
  // Prevent platform/device_id prefix mismatch (e.g., device_id='mac-...' with
  // platform='ios'). Without this check, prefix-based classifiers downstream
  // could be fooled into treating an iPhone as a Mac.
  if (!body.device_id.startsWith(`${body.platform}-`)) {
    return errorResponse('INVALID_DEVICE_ID', correlation_id, {
      reason: 'platform_prefix_mismatch',
      device_id: body.device_id,
      platform: body.platform,
    })
  }

  const ed25519 = bytesFromBase64(body.ed25519_pubkey)
  const x25519 = bytesFromBase64(body.x25519_pubkey)
  if (!ed25519 || ed25519.length !== 32 || !x25519 || x25519.length !== 32) {
    return errorResponse('INVALID_DEVICE_ID', correlation_id, { reason: 'bad_pubkey_length' })
  }
  // Reject the all-zero key (Curve25519 identity / neutral element). This is a
  // known Curve25519 footgun — an attacker submitting 0x00*32 as their X25519
  // pubkey coerces peer ECDH to a known value, allowing passive decryption.
  // See RFC 7748 §5. Full curve-point validation (via libsodium
  // crypto_scalarmult_base) is deferred to Phase 2 where we have the
  // Mac-side Rust daemon to do it — for Phase 0 the zero-check kills the
  // most common malicious submission.
  if (ed25519.every((b) => b === 0) || x25519.every((b) => b === 0)) {
    return errorResponse('INVALID_DEVICE_ID', correlation_id, { reason: 'zero_pubkey' })
  }

  const sb = serviceRoleClient()

  // Look up the offer.
  const { data: offer, error: offerErr } = await sb
    .from('pairing_offers')
    .select('token,user_id,issuing_device_id,expires_at,consumed_at')
    .eq('token', body.token)
    .maybeSingle()
  if (offerErr) {
    return errorResponse('SUPABASE_UNREACHABLE', correlation_id, { pg: offerErr.message })
  }
  if (!offer) return errorResponse('TOKEN_NOT_FOUND', correlation_id)
  if (offer.consumed_at !== null) return errorResponse('TOKEN_NOT_FOUND', correlation_id)
  if (new Date(offer.expires_at).getTime() < Date.now()) {
    return errorResponse('TOKEN_EXPIRED', correlation_id)
  }
  if (offer.user_id !== caller.user_id) {
    return errorResponse('ACCOUNT_MISMATCH', correlation_id)
  }

  // Atomically: verify offer belongs to caller, check 10-device limit,
  // consume offer, insert device. ALL inside the claim_pairing stored
  // procedure (see migration 20260416120100_claim_pairing_fn.sql). The
  // RPC raises specific SQLSTATEs which we map to taxonomy codes below.
  //
  // No fallback path: if the RPC is missing, that is a deployment bug,
  // not a runtime branch. A silent fallback would bypass the RPC's
  // atomicity guarantees and create cross-tenant race conditions.
  const { error: rpcErr } = await sb.rpc('claim_pairing', {
    p_token: body.token,
    p_device_id: body.device_id,
    p_user_id: caller.user_id,
    p_ed25519_pubkey: base64FromBytes(ed25519),
    p_x25519_pubkey: base64FromBytes(x25519),
    p_platform: body.platform,
    p_display_name: body.display_name,
    p_app_version: body.app_version ?? null,
    p_os_version: body.os_version ?? null,
  })
  if (rpcErr) {
    // Map known SQLSTATEs to taxonomy codes. Unknown errors -> INTERNAL.
    const msg = rpcErr.message ?? ''
    if (msg.includes('TOKEN_NOT_FOUND')) {
      return errorResponse('TOKEN_NOT_FOUND', correlation_id)
    }
    if (msg.includes('ACCOUNT_MISMATCH')) {
      return errorResponse('ACCOUNT_MISMATCH', correlation_id)
    }
    if (msg.includes('DEVICE_LIMIT_REACHED')) {
      return errorResponse('DEVICE_LIMIT_REACHED', correlation_id, { limit: MAX_DEVICES_PER_USER })
    }
    // Postgres unique_violation on devices.device_id — same device_id already registered.
    if (rpcErr.code === '23505') {
      return errorResponse('INVALID_DEVICE_ID', correlation_id, {
        reason: 'device_id_already_registered',
      })
    }
    // Postgres foreign_key_violation — e.g., user_id doesn't exist in auth.users.
    if (rpcErr.code === '23503') {
      return errorResponse('UNAUTHORIZED', correlation_id, { reason: 'user_not_found' })
    }
    // PGRST202: RPC function missing from schema cache — deployment problem.
    if (rpcErr.code === 'PGRST202' || msg.includes('function public.claim_pairing')) {
      return errorResponse('INTERNAL', correlation_id, {
        reason: 'claim_pairing_rpc_missing',
        hint: 'run supabase db push to apply the claim_pairing migration',
      })
    }
    return errorResponse('SUPABASE_UNREACHABLE', correlation_id, { pg: msg, code: rpcErr.code })
  }

  // Return the new device row and the user's other devices (for peer discovery).
  const { data: newDevice, error: newDeviceErr } = await sb
    .from('devices')
    .select('device_id,platform,display_name,created_at')
    .eq('device_id', body.device_id)
    .single()
  if (newDeviceErr || !newDevice) {
    return errorResponse('SUPABASE_UNREACHABLE', correlation_id, { pg: newDeviceErr?.message })
  }

  const { data: peers, error: peerErr } = await sb
    .from('devices')
    .select('device_id,platform,display_name,x25519_pubkey')
    .eq('user_id', caller.user_id)
    .is('revoked_at', null)
    .neq('device_id', body.device_id)
  if (peerErr) {
    return errorResponse('SUPABASE_UNREACHABLE', correlation_id, { pg: peerErr.message })
  }

  const response: PairClaimResponse = {
    device: newDevice,
    peer_devices: (peers ?? []).map((p) => ({
      device_id: p.device_id,
      platform: p.platform,
      display_name: p.display_name,
      x25519_pubkey: typeof p.x25519_pubkey === 'string' ? p.x25519_pubkey : '',
    })),
  }
  return jsonResponse(response, correlation_id, 201)
})
