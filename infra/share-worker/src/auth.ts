// src/auth.ts
import { type JWTPayload, createRemoteJWKSet, jwtVerify } from 'jose'

export interface AuthUser {
  userId: string
  email: string | undefined
}

let cachedJWKS: ReturnType<typeof createRemoteJWKSet> | null = null

function getJWKS(supabaseUrl: string): ReturnType<typeof createRemoteJWKSet> {
  if (!cachedJWKS) {
    const jwksUrl = new URL(`${supabaseUrl}/auth/v1/.well-known/jwks.json`)
    cachedJWKS = createRemoteJWKSet(jwksUrl)
  }
  return cachedJWKS
}

/**
 * Validate a Supabase JWT from the Authorization: Bearer header.
 * Returns the authenticated user or throws an error.
 */
export async function requireAuth(request: Request, supabaseUrl: string): Promise<AuthUser> {
  const authHeader = request.headers.get('Authorization')
  if (!authHeader?.startsWith('Bearer ')) {
    throw new AuthError('Missing Authorization header', 401)
  }

  const token = authHeader.slice(7)
  const JWKS = getJWKS(supabaseUrl)

  let payload: JWTPayload
  try {
    const result = await jwtVerify(token, JWKS, {
      issuer: `${supabaseUrl}/auth/v1`,
    })
    payload = result.payload
  } catch (err) {
    throw new AuthError(`Invalid token: ${String(err)}`, 401)
  }

  const userId = payload.sub
  if (!userId) throw new AuthError('Token missing sub claim', 401)

  return {
    userId,
    email: typeof payload['email'] === 'string' ? payload['email'] : undefined,
  }
}

export class AuthError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message)
  }
}
