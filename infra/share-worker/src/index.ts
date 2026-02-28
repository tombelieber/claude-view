import { withSentry } from '@sentry/cloudflare'
import { AuthError, requireAuth } from './auth'
import { getCorsHeaders, getPublicCorsHeaders } from './cors'
import { checkRateLimit, cleanupExpiredWindows } from './rate-limit'
import { generateToken } from './token'

export interface Env {
  SHARE_BUCKET: R2Bucket
  DB: D1Database
  ENVIRONMENT: string
  SUPABASE_URL: string
  POSTHOG_API_KEY: string
  SENTRY_DSN?: string
}

const MAX_BLOB_BYTES = 50 * 1024 * 1024 // 50MB
const MAX_JSON_BODY_BYTES = 1024 // 1KB for JSON endpoints

const RATE_LIMITS = {
  create: { limit: 10, windowSecs: 3600 },
  read: { limit: 60, windowSecs: 60 },
  delete: { limit: 20, windowSecs: 3600 },
  list: { limit: 30, windowSecs: 60 },
} as const

export default withSentry(
  (env: Env) => ({
    dsn: env.SENTRY_DSN,
    environment: env.ENVIRONMENT,
    tracesSampleRate: 0.1,
  }),
  {
    async fetch(request: Request, env: Env, _ctx: ExecutionContext): Promise<Response> {
      const url = new URL(request.url)
      const corsHeaders = getCorsHeaders(request, env)

      if (request.method === 'OPTIONS') {
        return new Response(null, { status: 204, headers: corsHeaders })
      }

      try {
        const response = await route(url, request, env)
        for (const [k, v] of Object.entries(corsHeaders)) {
          response.headers.set(k, v)
        }
        return response
      } catch (err) {
        if (err instanceof AuthError) {
          return jsonResponse({ error: err.message }, err.status, corsHeaders)
        }
        console.error('Unhandled error:', err)
        return jsonResponse({ error: 'Internal server error' }, 500, corsHeaders)
      }
    },

    async scheduled(
      _controller: ScheduledController,
      env: Env,
      _ctx: ExecutionContext,
    ): Promise<void> {
      const cutoff = Math.floor(Date.now() / 1000) - 3600
      const { results } = await env.DB.prepare(
        "SELECT token FROM shares WHERE status = 'pending' AND created_at < ?",
      )
        .bind(cutoff)
        .all<{ token: string }>()

      for (const row of results) {
        await env.SHARE_BUCKET.delete(`shares/${row.token}`)
        await env.DB.prepare('DELETE FROM shares WHERE token = ?').bind(row.token).run()
      }

      await cleanupExpiredWindows(env.DB)
    },
  },
)

async function route(url: URL, request: Request, env: Env): Promise<Response> {
  const path = url.pathname
  const method = request.method

  if (path === '/api/share' && method === 'POST') {
    return handleCreateShare(request, env)
  }

  const blobMatch = path.match(/^\/api\/share\/([\w]+)\/blob$/)
  if (blobMatch && method === 'PUT') {
    return handleUploadBlob(blobMatch[1], request, env)
  }

  const shareMatch = path.match(/^\/api\/share\/([\w]+)$/)
  if (shareMatch && method === 'GET') {
    return handleGetShare(shareMatch[1], request, env)
  }

  if (shareMatch && method === 'DELETE') {
    return handleDeleteShare(shareMatch[1], request, env)
  }

  const sessionDeleteMatch = path.match(/^\/api\/shares\/by-session\/([\w-]+)$/)
  if (sessionDeleteMatch && method === 'DELETE') {
    return handleDeleteShareBySession(sessionDeleteMatch[1], request, env)
  }

  if (path === '/api/shares' && method === 'GET') {
    return handleListShares(request, env)
  }

  return jsonResponse({ error: 'Not found' }, 404)
}

// ---- Handlers ----

async function handleCreateShare(request: Request, env: Env): Promise<Response> {
  const user = await requireAuth(request, env.SUPABASE_URL)

  const rl = await checkRateLimit(
    env.DB,
    `${user.userId}:create`,
    RATE_LIMITS.create.limit,
    RATE_LIMITS.create.windowSecs,
  )
  if (!rl.allowed) {
    return jsonResponse(
      {
        error: 'Rate limit exceeded',
        retry_after: rl.resetAt - Math.floor(Date.now() / 1000),
      },
      429,
      {
        'Retry-After': String(rl.resetAt - Math.floor(Date.now() / 1000)),
      },
    )
  }

  const contentLength = Number.parseInt(request.headers.get('Content-Length') || '0')
  if (contentLength > MAX_JSON_BODY_BYTES) {
    return jsonResponse({ error: 'Request body too large' }, 413)
  }

  const body = (await request.json()) as {
    session_id?: string
    title?: string
    size_bytes?: number
  }

  if (!body.session_id) {
    return jsonResponse({ error: 'session_id required' }, 400)
  }

  const token = generateToken()
  const now = Math.floor(Date.now() / 1000)

  await env.DB.prepare(
    `INSERT INTO shares (token, user_id, session_id, title, size_bytes, status, created_at)
     VALUES (?, ?, ?, ?, ?, 'pending', ?)`,
  )
    .bind(token, user.userId, body.session_id, body.title ?? null, body.size_bytes ?? 0, now)
    .run()

  void trackEvent(env, 'share_created', user.userId, {
    size_bytes: body.size_bytes ?? 0,
  })

  return jsonResponse({ token })
}

async function handleUploadBlob(token: string, request: Request, env: Env): Promise<Response> {
  const contentLength = Number.parseInt(request.headers.get('Content-Length') || '0')
  if (contentLength > MAX_BLOB_BYTES) {
    return jsonResponse({ error: 'Blob too large (max 50MB)' }, 413)
  }

  const row = await env.DB.prepare('SELECT status FROM shares WHERE token = ?')
    .bind(token)
    .first<{ status: string }>()

  if (!row) return jsonResponse({ error: 'Token not found' }, 404)
  if (row.status !== 'pending') return jsonResponse({ error: 'Share already uploaded' }, 409)

  const body = await request.arrayBuffer()
  if (body.byteLength > MAX_BLOB_BYTES) {
    return jsonResponse({ error: 'Blob too large (max 50MB)' }, 413)
  }

  await env.SHARE_BUCKET.put(`shares/${token}`, body, {
    httpMetadata: { contentType: 'application/octet-stream' },
  })

  await env.DB.prepare("UPDATE shares SET status = 'ready', size_bytes = ? WHERE token = ?")
    .bind(body.byteLength, token)
    .run()

  return jsonResponse({ status: 'ready', size_bytes: body.byteLength })
}

async function handleGetShare(token: string, request: Request, env: Env): Promise<Response> {
  const ip = request.headers.get('CF-Connecting-IP') || 'unknown'
  const rl = await checkRateLimit(
    env.DB,
    `${ip}:read`,
    RATE_LIMITS.read.limit,
    RATE_LIMITS.read.windowSecs,
  )
  if (!rl.allowed) {
    return jsonResponse({ error: 'Rate limit exceeded' }, 429)
  }

  const row = await env.DB.prepare(
    `SELECT token, session_id, title, created_at, view_count
     FROM shares WHERE token = ? AND status = 'ready'`,
  )
    .bind(token)
    .first<{
      token: string
      session_id: string
      title: string
      created_at: number
      view_count: number
    }>()

  if (!row)
    return new Response('Not found', {
      status: 404,
      headers: getPublicCorsHeaders(),
    })

  const obj = await env.SHARE_BUCKET.get(`shares/${token}`)
  if (!obj)
    return new Response('Blob not found', {
      status: 404,
      headers: getPublicCorsHeaders(),
    })

  void env.DB.prepare('UPDATE shares SET view_count = view_count + 1 WHERE token = ?')
    .bind(token)
    .run()

  const tokenHash = await sha256hex(token)
  void trackEventAnon(env, 'share_viewed', {
    token_hash: tokenHash.slice(0, 16),
  })

  return new Response(obj.body, {
    headers: {
      ...getPublicCorsHeaders(),
      'Content-Type': 'application/octet-stream',
      'Cache-Control': 'public, max-age=300',
    },
  })
}

async function handleDeleteShare(token: string, request: Request, env: Env): Promise<Response> {
  const user = await requireAuth(request, env.SUPABASE_URL)

  const rl = await checkRateLimit(
    env.DB,
    `${user.userId}:delete`,
    RATE_LIMITS.delete.limit,
    RATE_LIMITS.delete.windowSecs,
  )
  if (!rl.allowed) return jsonResponse({ error: 'Rate limit exceeded' }, 429)

  const row = await env.DB.prepare(
    "SELECT user_id FROM shares WHERE token = ? AND status = 'ready'",
  )
    .bind(token)
    .first<{ user_id: string }>()

  if (!row) return jsonResponse({ error: 'Share not found' }, 404)
  if (row.user_id !== user.userId) return jsonResponse({ error: 'Forbidden' }, 403)

  await env.SHARE_BUCKET.delete(`shares/${token}`)
  await env.DB.prepare("UPDATE shares SET status = 'deleted' WHERE token = ?").bind(token).run()

  void trackEvent(env, 'share_revoked', user.userId, {})

  return jsonResponse({ status: 'deleted' })
}

async function handleDeleteShareBySession(
  sessionId: string,
  request: Request,
  env: Env,
): Promise<Response> {
  const user = await requireAuth(request, env.SUPABASE_URL)

  const rl = await checkRateLimit(
    env.DB,
    `${user.userId}:delete`,
    RATE_LIMITS.delete.limit,
    RATE_LIMITS.delete.windowSecs,
  )
  if (!rl.allowed) return jsonResponse({ error: 'Rate limit exceeded' }, 429)

  const row = await env.DB.prepare(
    "SELECT token, user_id FROM shares WHERE session_id = ? AND user_id = ? AND status = 'ready'",
  )
    .bind(sessionId, user.userId)
    .first<{ token: string; user_id: string }>()

  if (!row) return jsonResponse({ error: 'Share not found' }, 404)

  await env.SHARE_BUCKET.delete(`shares/${row.token}`)
  await env.DB.prepare("UPDATE shares SET status = 'deleted' WHERE token = ?").bind(row.token).run()

  void trackEvent(env, 'share_revoked', user.userId, {})
  return jsonResponse({ status: 'deleted' })
}

async function handleListShares(request: Request, env: Env): Promise<Response> {
  const user = await requireAuth(request, env.SUPABASE_URL)

  const rl = await checkRateLimit(
    env.DB,
    `${user.userId}:list`,
    RATE_LIMITS.list.limit,
    RATE_LIMITS.list.windowSecs,
  )
  if (!rl.allowed) return jsonResponse({ error: 'Rate limit exceeded' }, 429)

  const { results } = await env.DB.prepare(
    `SELECT token, session_id, title, size_bytes, created_at, view_count
     FROM shares WHERE user_id = ? AND status = 'ready'
     ORDER BY created_at DESC LIMIT 100`,
  )
    .bind(user.userId)
    .all()

  return jsonResponse({ shares: results ?? [] })
}

// ---- Helpers ----

function jsonResponse(
  data: unknown,
  status = 200,
  extraHeaders: Record<string, string> = {},
): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'Content-Type': 'application/json', ...extraHeaders },
  })
}

async function trackEvent(
  env: Env,
  event: string,
  userId: string,
  props: Record<string, unknown>,
): Promise<void> {
  if (!env.POSTHOG_API_KEY) return
  await fetch('https://us.i.posthog.com/capture/', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      api_key: env.POSTHOG_API_KEY,
      event,
      distinct_id: userId,
      properties: { ...props, $lib: 'cloudflare-worker' },
    }),
  })
}

async function trackEventAnon(
  env: Env,
  event: string,
  props: Record<string, unknown>,
): Promise<void> {
  if (!env.POSTHOG_API_KEY) return
  await fetch('https://us.i.posthog.com/capture/', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      api_key: env.POSTHOG_API_KEY,
      event,
      distinct_id: 'anonymous',
      properties: { ...props, $lib: 'cloudflare-worker' },
    }),
  })
}

async function sha256hex(input: string): Promise<string> {
  const data = new TextEncoder().encode(input)
  const hash = await crypto.subtle.digest('SHA-256', data)
  return Array.from(new Uint8Array(hash))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
}
