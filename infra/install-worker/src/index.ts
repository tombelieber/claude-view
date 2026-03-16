/**
 * Install script proxy with download tracking.
 *
 * CF Workers dashboard automatically tracks request count, bandwidth,
 * geographic distribution, and status codes — zero extra code needed.
 *
 * Routes:
 *   GET /install.sh  → proxy install script from GitHub (cached 5 min at edge)
 *   GET /            → redirect to repo
 */

interface Env {
  GITHUB_RAW_BASE: string
}

const CACHE_TTL_SECONDS = 300 // 5 minutes — fresh enough for updates, light on GitHub

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url)

    if (url.pathname === '/' || url.pathname === '') {
      return Response.redirect('https://github.com/tombelieber/claude-view', 302)
    }

    if (url.pathname === '/install.sh') {
      return handleInstallScript(request, env)
    }

    return new Response('Not Found', { status: 404 })
  },
} satisfies ExportedHandler<Env>

async function handleInstallScript(request: Request, env: Env): Promise<Response> {
  // Check CF edge cache first
  const cache = caches.default
  const cacheKey = new Request(request.url, request)
  const cached = await cache.match(cacheKey)
  if (cached) return cached

  // Fetch from GitHub
  const githubUrl = `${env.GITHUB_RAW_BASE}/install.sh`
  const upstream = await fetch(githubUrl, {
    headers: { 'User-Agent': 'claude-view-install-worker' },
  })

  if (!upstream.ok) {
    return new Response('Failed to fetch install script', {
      status: 502,
    })
  }

  const script = await upstream.text()
  const response = new Response(script, {
    headers: {
      'Content-Type': 'text/plain; charset=utf-8',
      'Cache-Control': `public, max-age=${CACHE_TTL_SECONDS}`,
      'X-Content-Type-Options': 'nosniff',
    },
  })

  // Store in edge cache (non-blocking)
  request.cf && cache.put(cacheKey, response.clone())

  return response
}
