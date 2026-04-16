// Error taxonomy from design spec §6. Every edge function uses these helpers
// to return errors in a consistent shape — clients parse error.code and
// display error.message directly.

/** Stable error codes, mirror-matched against spec §6.
 *
 * Some codes are declared here for Phase 0 completeness but are only consumed
 * by later phases (Phase 2 Mac daemon, Phase 4 mobile). They are listed so the
 * taxonomy is a single source of truth across all layers. A linter may flag
 * them as unused within Phase 0's edge functions — that is expected.
 *
 * Phase 0 consumers:   UNAUTHORIZED, ACCOUNT_MISMATCH, TOKEN_NOT_FOUND,
 *                      TOKEN_EXPIRED, DEVICE_LIMIT_REACHED, DEVICE_NOT_FOUND,
 *                      SUPABASE_UNREACHABLE, ISSUING_DEVICE_NOT_FOUND,
 *                      INVALID_DEVICE_ID, ALREADY_REVOKED, METHOD_NOT_ALLOWED,
 *                      BAD_REQUEST, INTERNAL
 * Phase 2+ consumers:  SESSION_EXPIRED, JWT_REFRESH_FAILED, KEYCHAIN_ERROR,
 *                      BIOMETRIC_CANCELLED, OFFER_ALREADY_PENDING, RATE_LIMITED
 */
export type ErrorCode =
  | 'UNAUTHORIZED'
  | 'SESSION_EXPIRED' // Phase 2+
  | 'ACCOUNT_MISMATCH'
  | 'TOKEN_NOT_FOUND'
  | 'TOKEN_EXPIRED'
  | 'DEVICE_LIMIT_REACHED'
  | 'DEVICE_NOT_FOUND'
  | 'BIOMETRIC_CANCELLED' // Phase 4 mobile
  | 'KEYCHAIN_ERROR' // Phase 2+
  | 'JWT_REFRESH_FAILED' // Phase 2+
  | 'SUPABASE_UNREACHABLE'
  | 'RATE_LIMITED' // Phase 2+ (rate limiter config)
  | 'ISSUING_DEVICE_NOT_FOUND'
  | 'OFFER_ALREADY_PENDING' // Phase 2+ (pair-offer duplicate check)
  | 'ALREADY_REVOKED'
  | 'INVALID_DEVICE_ID'
  | 'METHOD_NOT_ALLOWED'
  | 'BAD_REQUEST'
  | 'INTERNAL'

export interface ApiError {
  code: ErrorCode
  message: string
  correlation_id: string
}

const HTTP_STATUS: Record<ErrorCode, number> = {
  UNAUTHORIZED: 401,
  SESSION_EXPIRED: 401,
  JWT_REFRESH_FAILED: 401,
  ACCOUNT_MISMATCH: 403,
  ISSUING_DEVICE_NOT_FOUND: 403,
  BAD_REQUEST: 400,
  INVALID_DEVICE_ID: 400,
  METHOD_NOT_ALLOWED: 405,
  TOKEN_NOT_FOUND: 404,
  DEVICE_NOT_FOUND: 404,
  TOKEN_EXPIRED: 410,
  DEVICE_LIMIT_REACHED: 409,
  OFFER_ALREADY_PENDING: 409,
  ALREADY_REVOKED: 409,
  RATE_LIMITED: 429,
  SUPABASE_UNREACHABLE: 503,
  BIOMETRIC_CANCELLED: 400,
  KEYCHAIN_ERROR: 500,
  INTERNAL: 500,
}

const USER_MESSAGE: Record<ErrorCode, string> = {
  UNAUTHORIZED: 'Please sign in to continue',
  SESSION_EXPIRED: 'Your session expired. Sign in again to continue.',
  JWT_REFRESH_FAILED: "Couldn't refresh your session. Sign in again.",
  ACCOUNT_MISMATCH: 'This pairing code belongs to a different account.',
  ISSUING_DEVICE_NOT_FOUND: 'Issuing device is not registered.',
  BAD_REQUEST: 'The request was malformed. Please try again.',
  INVALID_DEVICE_ID: 'Invalid device identifier.',
  METHOD_NOT_ALLOWED: 'This endpoint does not accept that HTTP method.',
  TOKEN_NOT_FOUND: 'This pairing code is invalid. Ask for a new one.',
  DEVICE_NOT_FOUND: "This device isn't in your list anymore.",
  TOKEN_EXPIRED: 'This pairing code expired. Ask for a new one.',
  DEVICE_LIMIT_REACHED: 'You already have 10 devices linked. Unpair one to add a new device.',
  OFFER_ALREADY_PENDING: 'An active pairing offer already exists for this device.',
  ALREADY_REVOKED: 'This device is already revoked.',
  RATE_LIMITED: "You're doing that too quickly. Wait a moment.",
  SUPABASE_UNREACHABLE: "Can't reach the account service. Check your internet.",
  BIOMETRIC_CANCELLED: 'Pairing cancelled.',
  KEYCHAIN_ERROR: 'Failed to save secure data. Please restart the app.',
  INTERNAL: 'Something went wrong on our end. Please try again.',
}

/** Build an ApiError JSON response with the standard shape and HTTP status. */
export function errorResponse(
  code: ErrorCode,
  correlation_id: string,
  extraContext?: Record<string, unknown>,
): Response {
  const body: ApiError & { context?: Record<string, unknown> } = {
    code,
    message: USER_MESSAGE[code],
    correlation_id,
  }
  if (extraContext) body.context = extraContext
  return new Response(JSON.stringify({ error: body }), {
    status: HTTP_STATUS[code],
    headers: {
      'content-type': 'application/json',
      'x-correlation-id': correlation_id,
    },
  })
}

/** Build a successful JSON response with correlation ID header. */
export function jsonResponse<T>(payload: T, correlation_id: string, status = 200): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: {
      'content-type': 'application/json',
      'x-correlation-id': correlation_id,
    },
  })
}
