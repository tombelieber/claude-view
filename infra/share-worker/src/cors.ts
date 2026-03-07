const ALLOWED_ORIGINS = [
  'https://share.claudeview.ai',
  'https://claudeview.ai',
  'https://share.claudeview.com',
  'https://claudeview.com',
]

const DEV_ORIGIN_PATTERN = /^http:\/\/localhost(:\d+)?$/

export function getCorsHeaders(
  request: Request,
  env: { ENVIRONMENT: string },
): Record<string, string> {
  const origin = request.headers.get('Origin') || ''
  const isDev = env.ENVIRONMENT === 'development'

  const isAllowed = ALLOWED_ORIGINS.includes(origin) || (isDev && DEV_ORIGIN_PATTERN.test(origin))

  // Only reflect the origin if it's in the allowlist — reject unknown origins
  // by omitting Access-Control-Allow-Origin entirely
  if (!isAllowed) {
    return { Vary: 'Origin' }
  }

  return {
    'Access-Control-Allow-Origin': origin,
    'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type, Authorization',
    Vary: 'Origin',
  }
}

/** GET /api/share/:token is public — allow all origins (Slack previews etc) */
export function getPublicCorsHeaders(): Record<string, string> {
  return {
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type',
  }
}
