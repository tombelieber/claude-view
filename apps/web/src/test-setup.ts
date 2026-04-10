import '@testing-library/jest-dom'
import { afterEach } from 'vitest'

if (document.doctype == null) {
  const doctype = document.implementation.createDocumentType('html', '', '')
  document.insertBefore(doctype, document.documentElement)
}

const originalConsoleWarn = console.warn.bind(console)
const originalConsoleError = console.error.bind(console)

function isKatexQuirksModeMessage(value: unknown): boolean {
  return typeof value === 'string' && value.includes("KaTeX doesn't work in quirks mode")
}

function jsonResponse(data: unknown, init: ResponseInit = {}): Response {
  return new Response(JSON.stringify(data), {
    status: 200,
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...(init.headers ?? {}),
    },
  })
}

const defaultFetch: typeof fetch = async (input) => {
  const url =
    typeof input === 'string' ? input : input instanceof Request ? input.url : String(input)
  const { pathname } = new URL(url, 'http://localhost:3000')

  switch (pathname) {
    case '/api/config':
      return jsonResponse({
        auth: false,
        sharing: false,
        version: '',
        telemetry: 'disabled',
        posthogKey: null,
        anonymousId: null,
      })
    case '/api/health':
      return jsonResponse({ status: 'ok', version: 'test' })
    case '/api/status':
      return jsonResponse({
        lastIndexedAt: null,
        lastIndexDurationMs: null,
        sessionsIndexed: 0,
        projectsIndexed: 0,
        lastGitSyncAt: null,
        commitsFound: 0,
        linksCreated: 0,
        updatedAt: 0,
      })
    case '/api/sidecar/sessions/models':
      return jsonResponse({ models: [], updatedAt: null })
    case '/api/plugins/marketplaces':
      return jsonResponse([])
    case '/api/mcp-servers':
      return jsonResponse({ servers: [], rawFileCount: 0 })
    case '/api/teams':
      return jsonResponse([])
    case '/api/local-llm/status':
      return jsonResponse({
        enabled: false,
        provider: null,
        active_model: null,
      })
    default:
      if (pathname.startsWith('/api/')) {
        return new Response('Not Found', { status: 404 })
      }
      return new Response(null, { status: 204 })
  }
}

globalThis.fetch = defaultFetch

afterEach(() => {
  globalThis.fetch = defaultFetch
})

console.warn = (...args: unknown[]) => {
  const [firstArg] = args
  if (isKatexQuirksModeMessage(firstArg)) return
  originalConsoleWarn(...args)
}

console.error = (...args: unknown[]) => {
  const [firstArg] = args
  if (isKatexQuirksModeMessage(firstArg)) return
  originalConsoleError(...args)
}

function isHappyDomFetchAbort(reason: unknown): boolean {
  return (
    reason instanceof DOMException &&
    reason.name === 'AbortError' &&
    typeof reason.stack === 'string' &&
    reason.stack.includes('happy-dom/lib/fetch/Fetch.js')
  )
}

function isLocalhostConnectionRefused(reason: unknown): boolean {
  if (!(reason instanceof AggregateError)) return false
  const errors = (reason as AggregateError & { errors?: unknown[] }).errors
  return (
    Array.isArray(errors) &&
    errors.length > 0 &&
    errors.every((error) => {
      if (!(error instanceof Error)) return false
      const err = error as Error & { code?: string; port?: number }
      return err.code === 'ECONNREFUSED' && err.port === 3000
    })
  )
}

window.addEventListener('unhandledrejection', (event) => {
  if (isHappyDomFetchAbort(event.reason) || isLocalhostConnectionRefused(event.reason)) {
    event.preventDefault()
  }
})

const unhandledRejectionHandlerKey = Symbol.for('claude-view.web.test.unhandledRejectionHandler')
const handlerState = globalThis as typeof globalThis & {
  [unhandledRejectionHandlerKey]?: boolean
}

if (!handlerState[unhandledRejectionHandlerKey]) {
  process.on('unhandledRejection', (reason) => {
    if (isHappyDomFetchAbort(reason) || isLocalhostConnectionRefused(reason)) {
      return
    }
    throw reason
  })
  handlerState[unhandledRejectionHandlerKey] = true
}
