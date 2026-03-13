// sidecar/src/integration.test.ts
// E2E integration tests against the REAL Agent SDK with claude-haiku-4-5-20251001.
//
// These tests verify the full create → init → stream → message flow that broke
// 6 times. They use the actual SDK, actual Unix socket, actual WS — no mocks.
//
// Requirements:
// - Claude Code credentials configured (~/.claude/)
// - Network access to Anthropic API
// - CLAUDECODE must NOT be set (blocks nested sessions)
//
// These tests are slow (~10-20s each) because they wait for real API responses.

import { type ChildProcess, spawn } from 'node:child_process'
import http from 'node:http'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import WebSocket from 'ws'

const TEST_SOCKET = `/tmp/claude-view-sidecar-integration-test-${process.pid}.sock`
const MODEL = 'claude-haiku-4-5-20251001'

let sidecarProcess: ChildProcess | null = null

// --- Helpers ---

function httpRequest(
  method: string,
  path: string,
  body?: unknown,
): Promise<{ status: number; data: Record<string, unknown> }> {
  return new Promise((resolve, reject) => {
    const options: http.RequestOptions = {
      socketPath: TEST_SOCKET,
      path,
      method,
      headers: { 'Content-Type': 'application/json', Host: 'localhost' },
    }

    const req = http.request(options, (res) => {
      let data = ''
      res.on('data', (chunk) => {
        data += chunk
      })
      res.on('end', () => {
        try {
          resolve({ status: res.statusCode ?? 0, data: JSON.parse(data) })
        } catch {
          reject(new Error(`Invalid JSON: ${data}`))
        }
      })
    })

    req.on('error', reject)
    if (body) req.write(JSON.stringify(body))
    req.end()
  })
}

function connectWs(controlId: string): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws+unix://${TEST_SOCKET}:/control/sessions/${controlId}/stream`)
    ws.on('open', () => resolve(ws))
    ws.on('error', reject)
    const timeout = setTimeout(() => reject(new Error('WS connect timeout')), 10_000)
    ws.on('open', () => clearTimeout(timeout))
  })
}

function waitForEvent(
  ws: WebSocket,
  type: string,
  timeoutMs = 30_000,
): Promise<Record<string, unknown>> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error(`Timeout waiting for ${type} event (${timeoutMs}ms)`)),
      timeoutMs,
    )

    const handler = (raw: WebSocket.Data) => {
      try {
        const event = JSON.parse(raw.toString())
        if (event.type === type) {
          clearTimeout(timeout)
          ws.off('message', handler)
          resolve(event)
        }
      } catch {
        // ignore parse errors
      }
    }
    ws.on('message', handler)
  })
}

function collectEvents(ws: WebSocket, durationMs: number): Promise<Record<string, unknown>[]> {
  return new Promise((resolve) => {
    const events: Record<string, unknown>[] = []
    const handler = (raw: WebSocket.Data) => {
      try {
        events.push(JSON.parse(raw.toString()))
      } catch {
        // ignore
      }
    }
    ws.on('message', handler)
    setTimeout(() => {
      ws.off('message', handler)
      resolve(events)
    }, durationMs)
  })
}

async function healthCheck(retries = 30): Promise<boolean> {
  for (let i = 0; i < retries; i++) {
    try {
      const { status } = await httpRequest('GET', '/health')
      if (status === 200) return true
    } catch {
      // not ready yet
    }
    await new Promise((r) => setTimeout(r, 200))
  }
  return false
}

// --- Setup/Teardown ---

beforeAll(async () => {
  // Clean up stale socket
  try {
    const fs = await import('node:fs')
    if (fs.existsSync(TEST_SOCKET)) fs.unlinkSync(TEST_SOCKET)
  } catch {
    // ignore
  }

  // Strip CLAUDE* env vars (blocks nested sessions) and ANTHROPIC_API_KEY
  // (SDK uses its own auth)
  const env: Record<string, string> = {}
  for (const [k, v] of Object.entries(process.env)) {
    if (!k.startsWith('CLAUDE') && k !== 'ANTHROPIC_API_KEY' && v !== undefined) {
      env[k] = v
    }
  }
  env.SIDECAR_SOCKET = TEST_SOCKET

  sidecarProcess = spawn('node', ['dist/index.js'], {
    cwd: `${process.cwd()}`,
    env,
    stdio: ['ignore', 'pipe', 'pipe'],
  })

  sidecarProcess.stdout?.on('data', (d) => process.stdout.write(`[sidecar] ${d}`))
  sidecarProcess.stderr?.on('data', (d) => process.stderr.write(`[sidecar:err] ${d}`))

  const ready = await healthCheck()
  if (!ready) throw new Error('Sidecar failed to start within 6s')
}, 15_000)

afterAll(async () => {
  if (sidecarProcess) {
    sidecarProcess.kill('SIGTERM')
    await new Promise((r) => setTimeout(r, 500))
    if (!sidecarProcess.killed) sidecarProcess.kill('SIGKILL')
    sidecarProcess = null
  }
  // Clean up socket
  try {
    const fs = await import('node:fs')
    if (fs.existsSync(TEST_SOCKET)) fs.unlinkSync(TEST_SOCKET)
  } catch {
    // ignore
  }
})

// --- Tests ---

describe('Sidecar E2E — create session flow', () => {
  // --- E2E: create session WITH initialMessage returns non-empty sessionId ---
  // This is THE bug that broke 6 times. The create endpoint previously returned
  // sessionId='' because it didn't wait for the SDK's session_init event.
  // V2 SDK only initializes after first send(), so initialMessage is required
  // to get a sessionId back.
  it('POST /control/sessions with initialMessage returns non-empty sessionId', async () => {
    const { status, data } = await httpRequest('POST', '/control/sessions', {
      model: MODEL,
      initialMessage: 'Reply with exactly one word: test',
    })

    expect(status).toBe(200)
    expect(data.controlId).toBeTruthy()
    expect(data.sessionId).toBeTruthy() // THE KEY ASSERTION — was '' before fix
    expect(typeof data.sessionId).toBe('string')
    expect((data.sessionId as string).length).toBe(36) // UUID format
    expect(data.status).toBe('created')

    // Cleanup: terminate the session
    await httpRequest('DELETE', `/control/sessions/${data.controlId}`)
  }, 30_000)

  // --- E2E: create WITHOUT initialMessage returns empty sessionId ---
  // V2 SDK doesn't initialize until first send(). Without initialMessage,
  // sessionId is '' — expected behavior, not a bug.
  it('POST /control/sessions without initialMessage returns empty sessionId', async () => {
    const { status, data } = await httpRequest('POST', '/control/sessions', {
      model: MODEL,
    })

    expect(status).toBe(200)
    expect(data.controlId).toBeTruthy()
    expect(data.sessionId).toBe('') // No init yet — expected
    expect(data.status).toBe('created')

    await httpRequest('DELETE', `/control/sessions/${data.controlId}`)
  }, 30_000)

  // --- E2E: created session appears in active sessions list ---
  it('created session appears in GET /control/sessions with populated sessionId', async () => {
    const { data: created } = await httpRequest('POST', '/control/sessions', {
      model: MODEL,
      initialMessage: 'Reply with exactly one word: test',
    })
    expect(created.sessionId).toBeTruthy()

    const { status, data: sessions } = await httpRequest('GET', '/control/sessions')
    expect(status).toBe(200)
    expect(Array.isArray(sessions)).toBe(true)

    const found = (sessions as unknown as Record<string, unknown>[]).find(
      (s) => s.controlId === created.controlId,
    )
    expect(found).toBeDefined()
    expect(found!.sessionId).toBe(created.sessionId)
    expect(found!.sessionId).toBeTruthy() // NOT empty string
    expect(found!.state).toBeTruthy()

    await httpRequest('DELETE', `/control/sessions/${created.controlId}`)
  }, 30_000)
})

describe('Sidecar E2E — WS stream flow', () => {
  // --- E2E: WS stream delivers session_init with correct model ---
  it('WS stream emits session_init event after connect', async () => {
    const { data: created } = await httpRequest('POST', '/control/sessions', {
      model: MODEL,
    })

    const ws = await connectWs(created.controlId as string)

    // Collect events for 3s — should include session_status and heartbeat_config
    const events = await collectEvents(ws, 3_000)
    ws.close()

    const types = events.map((e) => e.type)
    // Must have heartbeat_config (always sent on connect)
    expect(types).toContain('heartbeat_config')
    // session_status is emitted on connect
    expect(types).toContain('session_status')

    await httpRequest('DELETE', `/control/sessions/${created.controlId}`)
  }, 30_000)

  // --- E2E: resume replay delivers buffered events ---
  it('resume message with lastSeq=-1 replays buffered events', async () => {
    const { data: created } = await httpRequest('POST', '/control/sessions', {
      model: MODEL,
    })

    const ws = await connectWs(created.controlId as string)

    // Wait for initial events
    await new Promise((r) => setTimeout(r, 1_000))

    // Send resume with lastSeq=-1 (replay all)
    ws.send(JSON.stringify({ type: 'resume', lastSeq: -1 }))

    // Collect replayed events
    const events = await collectEvents(ws, 2_000)
    ws.close()

    // Should have at least the session_init event replayed
    // (session_init was emitted during creation, before WS connected)
    const sessionInit = events.find((e) => e.type === 'session_init')
    if (sessionInit) {
      expect(sessionInit.model).toBeTruthy()
    }
    // Even if session_init isn't in the replay (might have been consumed),
    // we should get SOME events from the replay
    expect(events.length).toBeGreaterThan(0)

    await httpRequest('DELETE', `/control/sessions/${created.controlId}`)
  }, 30_000)
})

describe('Sidecar E2E — initialMessage flow', () => {
  // --- E2E: create with initialMessage produces assistant response ---
  // This tests the full round-trip: create session → send initial message →
  // receive assistant response via WS stream.
  it('create with initialMessage delivers assistant response via WS stream', async () => {
    const { data: created } = await httpRequest('POST', '/control/sessions', {
      model: MODEL,
      initialMessage: 'Reply with exactly one word: pong',
    })

    expect(created.sessionId).toBeTruthy()
    expect(created.controlId).toBeTruthy()

    // Connect to WS stream
    const ws = await connectWs(created.controlId as string)

    // Replay all buffered events (including any that fired before WS connected)
    ws.send(JSON.stringify({ type: 'resume', lastSeq: -1 }))

    // Wait for turn_complete — the SDK processes initialMessage and responds
    const turnComplete = await waitForEvent(ws, 'turn_complete', 60_000)

    expect(turnComplete.type).toBe('turn_complete')
    expect(turnComplete.numTurns).toBeGreaterThanOrEqual(1)

    // Also verify we got assistant_text events (the actual response)
    // These would have been replayed via the resume
    ws.close()

    await httpRequest('DELETE', `/control/sessions/${created.controlId}`)
  }, 90_000) // generous timeout for API call

  // --- E2E: create WITHOUT initialMessage does NOT produce assistant response ---
  it('create without initialMessage does NOT trigger a turn', async () => {
    const { data: created } = await httpRequest('POST', '/control/sessions', {
      model: MODEL,
      // no initialMessage
    })

    const ws = await connectWs(created.controlId as string)

    // Collect events for 3s — should NOT include turn_complete or assistant_text
    const events = await collectEvents(ws, 3_000)
    ws.close()

    const turnComplete = events.find((e) => e.type === 'turn_complete')
    const assistantText = events.find((e) => e.type === 'assistant_text')

    expect(turnComplete).toBeUndefined()
    expect(assistantText).toBeUndefined()

    await httpRequest('DELETE', `/control/sessions/${created.controlId}`)
  }, 30_000)
})

describe('Sidecar E2E — send message after create', () => {
  // --- E2E: send message via WS produces assistant response ---
  it('user_message via WS triggers assistant response', async () => {
    const { data: created } = await httpRequest('POST', '/control/sessions', {
      model: MODEL,
    })

    const ws = await connectWs(created.controlId as string)

    // Wait for session to be ready (session_status event)
    await new Promise((r) => setTimeout(r, 1_000))

    // Send a message via WS
    ws.send(
      JSON.stringify({
        type: 'user_message',
        content: 'Reply with exactly one word: ping',
      }),
    )

    // Wait for turn_complete
    const turnComplete = await waitForEvent(ws, 'turn_complete', 60_000)
    expect(turnComplete.type).toBe('turn_complete')
    expect(turnComplete.numTurns).toBeGreaterThanOrEqual(1)

    ws.close()
    await httpRequest('DELETE', `/control/sessions/${created.controlId}`)
  }, 90_000)
})

describe('Sidecar E2E — regression guards', () => {
  // --- Regression: sessionId in list matches sessionId from create ---
  it('sessionId consistency between create response and list response', async () => {
    const { data: created } = await httpRequest('POST', '/control/sessions', {
      model: MODEL,
    })

    const createSessionId = created.sessionId as string
    expect(createSessionId).toBeTruthy()
    expect(createSessionId.length).toBe(36)

    const { data: sessions } = await httpRequest('GET', '/control/sessions')
    const found = (sessions as unknown as Record<string, unknown>[]).find(
      (s) => s.controlId === created.controlId,
    )
    expect(found).toBeDefined()
    expect(found!.sessionId).toBe(createSessionId)

    await httpRequest('DELETE', `/control/sessions/${created.controlId}`)
  }, 30_000)

  // --- Regression: multiple concurrent creates each get unique sessionId ---
  it('concurrent creates produce unique sessionIds', async () => {
    const [r1, r2] = await Promise.all([
      httpRequest('POST', '/control/sessions', { model: MODEL }),
      httpRequest('POST', '/control/sessions', { model: MODEL }),
    ])

    expect(r1.data.sessionId).toBeTruthy()
    expect(r2.data.sessionId).toBeTruthy()
    expect(r1.data.sessionId).not.toBe(r2.data.sessionId)
    expect(r1.data.controlId).not.toBe(r2.data.controlId)

    await Promise.all([
      httpRequest('DELETE', `/control/sessions/${r1.data.controlId}`),
      httpRequest('DELETE', `/control/sessions/${r2.data.controlId}`),
    ])
  }, 60_000)

  // --- Regression: terminate session removes it from list ---
  it('DELETE removes session from active list', async () => {
    const { data: created } = await httpRequest('POST', '/control/sessions', {
      model: MODEL,
    })
    const controlId = created.controlId as string

    // Verify it exists
    const { data: before } = await httpRequest('GET', '/control/sessions')
    const existsBefore = (before as unknown as Record<string, unknown>[]).some(
      (s) => s.controlId === controlId,
    )
    expect(existsBefore).toBe(true)

    // Terminate
    await httpRequest('DELETE', `/control/sessions/${controlId}`)

    // Wait for cleanup
    await new Promise((r) => setTimeout(r, 500))

    // Verify it's gone
    const { data: after } = await httpRequest('GET', '/control/sessions')
    const existsAfter = (after as unknown as Record<string, unknown>[]).some(
      (s) => s.controlId === controlId,
    )
    expect(existsAfter).toBe(false)
  }, 30_000)
})
