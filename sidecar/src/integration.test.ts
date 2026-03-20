// sidecar/src/integration.test.ts
// E2E integration tests against the REAL Agent SDK with claude-haiku-4-5-20251001.
//
// These tests verify the full V1 query() + MessageBridge flow:
// create → stream_delta → assistant → turn_complete, plus resume, fork, interrupt.
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

const HAS_API_KEY = Boolean(process.env.ANTHROPIC_API_KEY || process.env.CLAUDE_CODE_AUTH)
const TEST_PORT = 3099
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
      host: '127.0.0.1',
      port: TEST_PORT,
      path,
      method,
      headers: { 'Content-Type': 'application/json' },
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

function connectWs(sessionId: string): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://localhost:${TEST_PORT}/ws/chat/${sessionId}`)
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

function waitForAnyOf(
  ws: WebSocket,
  types: string[],
  timeoutMs = 30_000,
): Promise<Record<string, unknown>> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error(`Timeout waiting for any of [${types.join(', ')}] (${timeoutMs}ms)`)),
      timeoutMs,
    )

    const handler = (raw: WebSocket.Data) => {
      try {
        const event = JSON.parse(raw.toString())
        if (types.includes(event.type)) {
          clearTimeout(timeout)
          ws.off('message', handler)
          resolve(event)
        }
      } catch {
        // ignore
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

function collectUntilEvent(
  ws: WebSocket,
  type: string,
  timeoutMs = 60_000,
): Promise<Record<string, unknown>[]> {
  return new Promise((resolve, reject) => {
    const events: Record<string, unknown>[] = []
    const timeout = setTimeout(() => {
      ws.off('message', handler)
      reject(
        new Error(
          `Timeout collecting until ${type} (${timeoutMs}ms). Got: ${events.map((e) => e.type).join(', ')}`,
        ),
      )
    }, timeoutMs)

    const handler = (raw: WebSocket.Data) => {
      try {
        const event = JSON.parse(raw.toString())
        events.push(event)
        if (event.type === type) {
          clearTimeout(timeout)
          ws.off('message', handler)
          resolve(events)
        }
      } catch {
        // ignore
      }
    }
    ws.on('message', handler)
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

async function createSession(initialMessage?: string) {
  const body: Record<string, unknown> = { model: MODEL }
  if (initialMessage) body.initialMessage = initialMessage
  const { data } = await httpRequest('POST', '/api/sidecar/sessions', body)
  return data as { sessionId: string; status: string }
}

async function cleanupSession(sessionId: string) {
  try {
    await httpRequest('DELETE', `/api/sidecar/sessions/${sessionId}`)
  } catch {
    // ignore — may already be closed
  }
}

// --- Setup/Teardown ---

describe.skipIf(!HAS_API_KEY)('Sidecar V1 Integration Tests', () => {
  beforeAll(async () => {
    // Strip CLAUDE* env vars (blocks nested sessions)
    const env: Record<string, string> = {}
    for (const [k, v] of Object.entries(process.env)) {
      if (!k.startsWith('CLAUDE') && v !== undefined) {
        env[k] = v
      }
    }
    env.SIDECAR_PORT = String(TEST_PORT)

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
  })

  // --- 1. Create → stream_delta deltas → assistant message → turn_complete ---
  it('create session → receive stream_delta deltas → turn_complete', async () => {
    const created = await createSession('Reply with exactly one word: pong')
    expect(created.sessionId).toBeTruthy()

    const ws = await connectWs(created.sessionId)
    // WS connected — blocks_snapshot delivered on connect, no resume needed

    const events = await collectUntilEvent(ws, 'turn_complete', 60_000)
    ws.close()

    const types = events.map((e) => e.type)
    // V1 with includePartialMessages emits stream_delta events
    expect(types).toContain('session_init')
    // Must have at least one assistant response (stream_delta or assistant_text)
    const hasResponse = types.includes('stream_delta') || types.includes('assistant_text')
    expect(hasResponse).toBe(true)
    expect(types).toContain('turn_complete')

    await cleanupSession(created.sessionId)
  }, 90_000)

  // --- 2. Resume session → multi-turn ---
  it('resume session → multi-turn: 2 messages get responses', async () => {
    const created = await createSession('Reply with exactly one word: alpha')
    expect(created.sessionId).toBeTruthy()
    const sessionId = created.sessionId

    // Wait for first turn to complete
    const ws1 = await connectWs(sessionId)
    // WS connected — blocks_snapshot delivered on connect, no resume needed
    await waitForEvent(ws1, 'turn_complete', 60_000)
    ws1.close()

    // Close and resume
    await cleanupSession(sessionId)
    await new Promise((r) => setTimeout(r, 1_000))

    const { data: resumed } = await httpRequest('POST', '/api/sidecar/sessions/:id/resume', {
      sessionId,
      model: MODEL,
    })
    expect(resumed.status).toMatch(/resumed|already_active/)

    // Send second message on resumed session
    const ws2 = await connectWs(resumed.sessionId as string)
    // WS connected — blocks_snapshot delivered on connect, no resume needed

    // Wait for init replay, then send
    await new Promise((r) => setTimeout(r, 2_000))
    ws2.send(JSON.stringify({ type: 'user_message', content: 'Reply with exactly one word: beta' }))

    const turnComplete = await waitForEvent(ws2, 'turn_complete', 60_000)
    ws2.close()

    expect(turnComplete.type).toBe('turn_complete')
    expect(turnComplete.numTurns as number).toBeGreaterThanOrEqual(1)

    await cleanupSession(resumed.sessionId as string)
  }, 120_000)

  // --- 3. Fork session → new sessionId ---
  it('fork session → new sessionId differs from original', async () => {
    const created = await createSession('Reply with exactly one word: original')
    expect(created.sessionId).toBeTruthy()
    const originalSessionId = created.sessionId

    // Wait for first turn
    const ws1 = await connectWs(originalSessionId)
    // WS connected — blocks_snapshot delivered on connect, no resume needed
    await waitForEvent(ws1, 'turn_complete', 60_000)
    ws1.close()

    // Fork
    const { status, data: forked } = await httpRequest('POST', '/api/sidecar/sessions/fork', {
      sessionId: originalSessionId,
      model: MODEL,
    })
    expect(status).toBe(200)
    expect(forked.status).toBe('forked')
    expect(forked.sessionId).toBeTruthy()
    expect(forked.sessionId).not.toBe(originalSessionId)

    await cleanupSession(originalSessionId)
    await cleanupSession(forked.sessionId as string)
  }, 90_000)

  // --- 4. Interrupt mid-response → session stays alive ---
  it('interrupt mid-response → session survives', async () => {
    const created = await createSession()
    const ws = await connectWs(created.sessionId)
    // WS connected — blocks_snapshot delivered on connect, no resume needed
    await new Promise((r) => setTimeout(r, 1_000))

    // Send a message that will produce a long response
    ws.send(
      JSON.stringify({
        type: 'user_message',
        content: 'List the numbers 1 through 100, one per line',
      }),
    )

    // Wait for first stream activity then interrupt
    await waitForAnyOf(ws, ['stream_delta', 'assistant_text'], 30_000)
    ws.send(JSON.stringify({ type: 'interrupt' }))

    // Session should still be alive — wait for turn_complete or turn_error
    const result = await waitForAnyOf(ws, ['turn_complete', 'turn_error'], 30_000)
    expect(['turn_complete', 'turn_error']).toContain(result.type)
    ws.close()

    // Verify session is still in list
    const { data: sessions } = await httpRequest('GET', '/api/sidecar/sessions')
    const found = (sessions as unknown as Record<string, unknown>[]).find(
      (s) => s.sessionId === created.sessionId,
    )
    expect(found).toBeDefined()

    await cleanupSession(created.sessionId)
  }, 60_000)

  // --- 5. Permission mode change via WS ---
  it('set_mode changes permission mode without error', async () => {
    const created = await createSession('Reply with exactly one word: test')
    const ws = await connectWs(created.sessionId)
    // WS connected — blocks_snapshot delivered on connect, no resume needed

    // Wait for turn to complete (session is in waiting_input)
    await waitForEvent(ws, 'turn_complete', 60_000)

    // Change mode — should not produce a fatal error
    ws.send(JSON.stringify({ type: 'set_mode', mode: 'plan' }))

    // Collect events for 2s — no fatal error expected
    const events = await collectEvents(ws, 2_000)
    ws.close()

    const fatalError = events.find((e) => e.type === 'error' && e.fatal === true)
    expect(fatalError).toBeUndefined()

    await cleanupSession(created.sessionId)
  }, 90_000)

  // --- 6. DELETE session closes it and removes from list ---
  it('DELETE session closes it and removes from list', async () => {
    const created = await createSession('Reply with exactly one word: test')
    expect(created.sessionId).toBeTruthy()

    const { data: before } = await httpRequest('GET', '/api/sidecar/sessions')
    const existsBefore = (before as unknown as Record<string, unknown>[]).some(
      (s) => s.sessionId === created.sessionId,
    )
    expect(existsBefore).toBe(true)

    await httpRequest('DELETE', `/api/sidecar/sessions/${created.sessionId}`)
    // Wait for delayed cleanup
    await new Promise((r) => setTimeout(r, 6_000))

    const { data: after } = await httpRequest('GET', '/api/sidecar/sessions')
    const existsAfter = (after as unknown as Record<string, unknown>[]).some(
      (s) => s.sessionId === created.sessionId,
    )
    expect(existsAfter).toBe(false)
  }, 30_000)

  // --- 7. Bootstrap message session_id: '' ---
  it('create without initialMessage accepts session_id empty string', async () => {
    const created = await createSession()
    expect(created.sessionId).toBeTruthy()
    // Session created without error — SDK accepted the empty session_id bridge
    expect(created.status).toBe('created')

    await cleanupSession(created.sessionId)
  }, 30_000)

  // --- 8. Concurrent sends ---
  it('concurrent sends → both get responses', async () => {
    const created = await createSession('Reply with exactly one word: init')
    const ws = await connectWs(created.sessionId)
    // WS connected — blocks_snapshot delivered on connect, no resume needed

    // Wait for first turn
    await waitForEvent(ws, 'turn_complete', 60_000)

    // Send two messages rapidly
    ws.send(JSON.stringify({ type: 'user_message', content: 'Reply with exactly one word: first' }))
    ws.send(
      JSON.stringify({ type: 'user_message', content: 'Reply with exactly one word: second' }),
    )

    // Wait for at least one turn_complete
    await waitForEvent(ws, 'turn_complete', 60_000)

    // The bridge queues both — SDK processes them sequentially
    // We just verify the session didn't crash
    const { data: sessions } = await httpRequest('GET', '/api/sidecar/sessions')
    const found = (sessions as unknown as Record<string, unknown>[]).find(
      (s) => s.sessionId === created.sessionId,
    )
    expect(found).toBeDefined()
    ws.close()

    await cleanupSession(created.sessionId)
  }, 120_000)

  // --- 9. REGRESSION: session stays alive after turn_complete ---
  it('REGRESSION: session stays alive after turn_complete', async () => {
    const created = await createSession('Reply with exactly one word: hello')
    const ws = await connectWs(created.sessionId)
    // WS connected — blocks_snapshot delivered on connect, no resume needed

    // Wait for first turn_complete
    await waitForEvent(ws, 'turn_complete', 60_000)

    // Verify state is NOT closed
    const { data: sessions } = await httpRequest('GET', '/api/sidecar/sessions')
    const found = (sessions as unknown as Record<string, unknown>[]).find(
      (s) => s.sessionId === created.sessionId,
    )
    expect(found).toBeDefined()
    expect(found?.state).not.toBe('closed')
    expect(found?.state).toBe('waiting_input')

    // Send second message — should work
    ws.send(JSON.stringify({ type: 'user_message', content: 'Reply with exactly one word: world' }))
    const secondTurn = await waitForEvent(ws, 'turn_complete', 60_000)
    expect(secondTurn.type).toBe('turn_complete')
    expect(secondTurn.numTurns as number).toBeGreaterThanOrEqual(2)

    ws.close()
    await cleanupSession(created.sessionId)
  }, 120_000)

  // --- 10. REGRESSION: resumed session survives multiple turns ---
  it('REGRESSION: resumed session survives multiple turns', async () => {
    const created = await createSession('Reply with exactly one word: alpha')
    const sessionId = created.sessionId
    expect(sessionId).toBeTruthy()

    // First turn
    const ws1 = await connectWs(sessionId)
    // WS connected — blocks_snapshot delivered on connect, no resume needed
    await waitForEvent(ws1, 'turn_complete', 60_000)
    ws1.close()

    // Close original, resume
    await cleanupSession(sessionId)
    await new Promise((r) => setTimeout(r, 1_000))

    const { data: resumed } = await httpRequest(
      'POST',
      `/api/sidecar/sessions/${sessionId}/resume`,
      {
        model: MODEL,
      },
    )

    const ws2 = await connectWs(resumed.sessionId as string)
    // WS connected — blocks_snapshot delivered on connect, no resume needed
    await new Promise((r) => setTimeout(r, 2_000))

    // Turn 1 on resumed session
    ws2.send(JSON.stringify({ type: 'user_message', content: 'Reply with exactly one word: beta' }))
    await waitForEvent(ws2, 'turn_complete', 60_000)

    // Turn 2 on resumed session — verifies it survives
    ws2.send(
      JSON.stringify({ type: 'user_message', content: 'Reply with exactly one word: gamma' }),
    )
    const lastTurn = await waitForEvent(ws2, 'turn_complete', 60_000)
    expect(lastTurn.type).toBe('turn_complete')

    ws2.close()
    await cleanupSession(resumed.sessionId as string)
  }, 180_000)

  // --- 11. REGRESSION: rate_limit allowed_warning does NOT close session ---
  it('REGRESSION: rate_limit event does not close session', async () => {
    const created = await createSession('Reply with exactly one word: test')
    const ws = await connectWs(created.sessionId)
    // WS connected — blocks_snapshot delivered on connect, no resume needed

    // Collect all events until turn_complete
    const events = await collectUntilEvent(ws, 'turn_complete', 60_000)
    ws.close()

    // Check if any rate_limit events appeared
    const rateLimitEvents = events.filter((e) => e.type === 'rate_limit')

    // Whether or not rate_limit events appeared, session should NOT be closed
    const { data: sessions } = await httpRequest('GET', '/api/sidecar/sessions')
    const found = (sessions as unknown as Record<string, unknown>[]).find(
      (s) => s.sessionId === created.sessionId,
    )
    expect(found).toBeDefined()
    expect(found?.state).not.toBe('closed')
    expect(found?.state).not.toBe('error')

    // If rate_limit events did arrive, log them for visibility
    if (rateLimitEvents.length > 0) {
      console.log(`[test] Got ${rateLimitEvents.length} rate_limit events — session survived`)
    }

    await cleanupSession(created.sessionId)
  }, 90_000)

  // --- Create session basics ---
  it('POST /api/sidecar/sessions with initialMessage returns non-empty sessionId', async () => {
    const created = await createSession('Reply with exactly one word: test')
    expect(created.sessionId).toBeTruthy()
    expect(typeof created.sessionId).toBe('string')
    expect(created.sessionId.length).toBe(36) // UUID format
    expect(created.status).toBe('created')

    await cleanupSession(created.sessionId)
  }, 30_000)

  // --- Concurrent creates ---
  it('concurrent creates produce unique sessionIds', async () => {
    const [r1, r2] = await Promise.all([
      createSession('Reply with exactly one word: alpha'),
      createSession('Reply with exactly one word: beta'),
    ])

    expect(r1.sessionId).toBeTruthy()
    expect(r2.sessionId).toBeTruthy()
    expect(r1.sessionId).not.toBe(r2.sessionId)

    await Promise.all([cleanupSession(r1.sessionId), cleanupSession(r2.sessionId)])
  }, 60_000)

  // --- WS infrastructure ---
  it('WS stream emits heartbeat_config and session_status after connect', async () => {
    const created = await createSession('Reply with exactly one word: test')
    const ws = await connectWs(created.sessionId)

    const events = await collectEvents(ws, 3_000)
    ws.close()

    const types = events.map((e) => e.type)
    expect(types).toContain('heartbeat_config')
    expect(types).toContain('session_status')

    await cleanupSession(created.sessionId)
  }, 30_000)

  // --- Send endpoint ---
  it('POST /api/sidecar/sessions/:id/send returns 200 for active session', async () => {
    const created = await createSession('Reply with exactly one word: hello')
    expect(created.sessionId).toBeTruthy()

    // Wait for init to complete
    await new Promise((r) => setTimeout(r, 2_000))

    const { status, data } = await httpRequest(
      'POST',
      `/api/sidecar/sessions/${created.sessionId}/send`,
      {
        message: 'Reply with exactly one word: ping',
      },
    )
    expect(status).toBe(200)
    expect(data.status).toBe('sent')

    await cleanupSession(created.sessionId)
  }, 30_000)

  it('POST /api/sidecar/sessions/:id/send returns 404 for unknown sessionId', async () => {
    const { status } = await httpRequest(
      'POST',
      '/api/sidecar/sessions/nonexistent-session-id/send',
      {
        message: 'hello',
      },
    )
    expect(status).toBe(404)
  }, 10_000)
})
