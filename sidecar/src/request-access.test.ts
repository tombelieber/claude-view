import { once } from 'node:events'
import { mkdtemp, rm } from 'node:fs/promises'
import { request } from 'node:http'
import { createRequire } from 'node:module'
import type { AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'
import WebSocket from 'ws'
import { isLocalRequest } from './request-access.js'
import type { ControlSession } from './session-registry.js'

// Exercise the actual listener without starting SDK subprocesses or model refreshes.
vi.mock('./model-cache.js', () => ({
  getCacheState: () => ({ models: [], updatedAt: null }),
  startModelCacheRefresh: vi.fn(),
  stopModelCacheRefresh: vi.fn(),
  updateModelCacheFromSession: vi.fn(),
}))
vi.mock('./ws-handler.js', () => ({
  handleWebSocket: (ws: WebSocket) => ws.close(),
}))

describe('sidecar local request boundary', () => {
  let sidecar: typeof import('./index.js')
  let port: number
  const signalListeners = new Map<NodeJS.Signals, NodeJS.SignalsListener[]>()

  beforeAll(async () => {
    const originalPort = process.env.SIDECAR_PORT
    const originalClaudeCode = process.env.CLAUDECODE
    const previousListeners = new Map(
      (['SIGINT', 'SIGTERM'] as const).map((signal) => [signal, process.listeners(signal)]),
    )
    process.env.SIDECAR_PORT = '0'
    try {
      sidecar = await import('./index.js')
      if (!sidecar.server.listening) await once(sidecar.server, 'listening')
      port = (sidecar.server.address() as AddressInfo).port
      vi.spyOn(sidecar.registry, 'getBySessionId').mockReturnValue({
        controlId: 'test-control',
      } as ControlSession)
    } finally {
      for (const [signal, previous] of previousListeners) {
        signalListeners.set(
          signal,
          process.listeners(signal).filter((listener) => !previous.includes(listener)),
        )
      }
      if (originalPort === undefined) Reflect.deleteProperty(process.env, 'SIDECAR_PORT')
      else process.env.SIDECAR_PORT = originalPort
      if (originalClaudeCode === undefined) Reflect.deleteProperty(process.env, 'CLAUDECODE')
      else process.env.CLAUDECODE = originalClaudeCode
    }
  })

  afterAll(async () => {
    vi.restoreAllMocks()
    if (sidecar) {
      await new Promise<void>((resolve, reject) => {
        sidecar.server.close((error) => (error ? reject(error) : resolve()))
      })
    }
    for (const [signal, owned] of signalListeners) {
      for (const listener of owned) process.removeListener(signal, listener)
    }
  })

  function httpStatus(headers: Record<string, string> = {}): Promise<number> {
    return new Promise((resolve, reject) => {
      const req = request(
        { host: '127.0.0.1', port, path: '/api/sidecar/sessions', headers },
        (res) => {
          res.resume()
          res.on('end', () => resolve(res.statusCode ?? 0))
        },
      )
      req.on('error', reject)
      req.setTimeout(2000, () => req.destroy(new Error('HTTP request timed out')))
      req.end()
    })
  }

  function websocketStatus(
    headers: Record<string, string> = {},
    targetPort = port,
  ): Promise<number> {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(`ws://127.0.0.1:${targetPort}/ws/chat/test-session`, {
        headers,
        handshakeTimeout: 2000,
      })
      ws.on('open', () => resolve(101))
      ws.on('unexpected-response', (_req, res) => {
        res.resume()
        resolve(res.statusCode ?? 0)
      })
      ws.on('error', reject)
    })
  }

  it('binds only the IPv4 loopback interface', () => {
    expect((sidecar.server.address() as AddressInfo).address).toBe('127.0.0.1')
  })

  it('fails closed for missing, ambiguous, or malformed authority headers', () => {
    expect(isLocalRequest({})).toBe(false)
    expect(isLocalRequest({ host: ['localhost', 'attacker.example'] })).toBe(false)
    expect(isLocalRequest({ host: 'localhost\n' })).toBe(false)
    expect(isLocalRequest({ host: 'localhost', origin: ['http://localhost'] })).toBe(false)
    expect(isLocalRequest({ host: 'localhost', origin: 'http://localhost\n' })).toBe(false)
  })

  it.each([undefined, 'http://localhost:5173', 'http://127.0.0.1:47892'])(
    'accepts HTTP from Rust or the local UI (%s)',
    async (origin) => {
      expect(await httpStatus(origin ? { Origin: origin } : {})).toBe(200)
    },
  )

  it.each(['http://evil.example', 'null', 'http://localhost.evil.example:5173'])(
    'rejects HTTP from an untrusted or opaque browser origin (%s)',
    async (origin) => {
      expect(await httpStatus({ Origin: origin })).toBe(403)
    },
  )

  it('rejects HTTP addressed to a DNS rebinding hostname', async () => {
    expect(await httpStatus({ Host: `attacker.example:${port}` })).toBe(403)
  })

  it.each(['localhost', '127.0.0.1:3001', '[::1]:3001'])(
    'accepts an exact loopback Host (%s)',
    async (host) => {
      expect(await httpStatus({ Host: host })).toBe(200)
    },
  )

  it.each(['localhost.evil.example', 'localhost:3001@evil.example', '127.0.0.1:65536'])(
    'rejects a deceptive or invalid Host (%s)',
    async (host) => {
      // The HTTP adapter rejects malformed authorities before the app guard.
      expect([400, 403]).toContain(await httpStatus({ Host: host }))
    },
  )

  it.each([undefined, 'http://localhost:5173', 'http://127.0.0.1:47892'])(
    'accepts a WebSocket from Rust or the local UI (%s)',
    async (origin) => {
      expect(await websocketStatus(origin ? { Origin: origin } : {})).toBe(101)
    },
  )

  it.each(['http://evil.example', 'null', 'http://localhost.evil.example:5173'])(
    'rejects a WebSocket from an untrusted or opaque browser origin (%s)',
    async (origin) => {
      expect(await websocketStatus({ Origin: origin })).toBe(403)
    },
  )

  it('rejects a WebSocket addressed to a DNS rebinding hostname', async () => {
    expect(await websocketStatus({ Host: `attacker.example:${port}` })).toBe(403)
  })

  describe('actual web Vite proxy', () => {
    let devServer: import('vite').ViteDevServer | undefined
    let cacheDir: string | undefined
    let proxyPort: number

    beforeAll(async () => {
      const webRoot = fileURLToPath(new URL('../../apps/web/', import.meta.url))
      const webRequire = createRequire(new URL('../../apps/web/package.json', import.meta.url))
      // Resolve from the web workspace, not the sidecar's transitive Vite 6.
      const vite = await import(pathToFileURL(webRequire.resolve('vite')).href)
      expect(vite.version).toMatch(/^7\./)
      const loaded = await vite.loadConfigFromFile(
        { command: 'serve', mode: 'test' },
        join(webRoot, 'vite.config.ts'),
      )
      if (!loaded) throw new Error('Web Vite config was not loaded')
      const chatProxy = loaded.config.server?.proxy?.['/ws/chat']
      if (!chatProxy || typeof chatProxy === 'string') {
        throw new Error('Web chat proxy is not configured')
      }
      cacheDir = await mkdtemp(join(tmpdir(), 'claude-view-vite-proxy-'))
      devServer = await vite.createServer({
        ...loaded.config,
        configFile: false,
        root: webRoot,
        cacheDir,
        server: {
          ...loaded.config.server,
          host: '127.0.0.1',
          port: 0,
          proxy: {
            ...loaded.config.server.proxy,
            '/ws/chat': { ...chatProxy, target: `ws://127.0.0.1:${port}` },
          },
        },
      })
      await devServer?.listen()
      proxyPort = (devServer?.httpServer?.address() as AddressInfo).port
    })

    afterAll(async () => {
      try {
        await devServer?.close()
      } finally {
        if (cacheDir) await rm(cacheDir, { recursive: true, force: true })
      }
    })

    it('preserves the local browser Origin through the dev chat proxy', async () => {
      expect(await websocketStatus({ Origin: `http://localhost:${proxyPort}` }, proxyPort)).toBe(
        101,
      )
    })

    it('preserves an untrusted browser Origin so the sidecar rejects it', async () => {
      expect(await websocketStatus({ Origin: 'https://attacker.example' }, proxyPort)).toBe(403)
    })
  })
})
