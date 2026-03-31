/**
 * Install script proxy with download tracking.
 *
 * CF Workers dashboard automatically tracks request count, bandwidth,
 * geographic distribution, and status codes — zero extra code needed.
 *
 * Routes:
 *   GET /install.sh      → proxy install script from GitHub (cached 5 min at edge)
 *   GET /ping?source=X   → install source beacon (plugin, npx, install_sh)
 *   GET /                 → redirect to repo
 */

interface Env {
  GITHUB_RAW_BASE: string
}

const CACHE_TTL_SECONDS = 300 // 5 minutes — fresh enough for updates, light on GitHub

const VALID_SOURCES = new Set(['plugin', 'npx', 'install_sh'])

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url)

    if (url.pathname === '/' || url.pathname === '') {
      return Response.redirect('https://github.com/tombelieber/claude-view', 302)
    }

    if (url.pathname === '/install.sh') {
      return handleInstallScript(request, env)
    }

    if (url.pathname === '/ping') {
      return handlePing(url)
    }

    return new Response('Not Found', { status: 404 })
  },
} satisfies ExportedHandler<Env>

/**
 * Lightweight beacon — the server pings this on first startup so all install
 * sources (plugin, npx, install.sh) appear in one CF Workers dashboard.
 *
 * CF analytics auto-tracks: request count, geo, status code per unique URL.
 * The `source` query param appears in the pathname+query breakdown.
 */
function handlePing(url: URL): Response {
  const source = url.searchParams.get('source') ?? 'unknown'
  const version = url.searchParams.get('v') ?? 'unknown'

  if (!VALID_SOURCES.has(source)) {
    return new Response('ok', { status: 200 })
  }

  // 1x1 transparent pixel response — minimal bandwidth.
  // The real value is the CF analytics entry, not the response body.
  return new Response('ok', {
    status: 200,
    headers: {
      'Cache-Control': 'no-store',
      'X-Install-Source': source,
      'X-Version': version,
    },
  })
}

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
