// Backpressure tests for ws-handler: a slow/stalled client must not be able to
// make the sidecar buffer unbounded full-conversation snapshots in its ws send
// queue (a real OOM vector for long live-chat sessions). The happy path (empty
// buffer, or a mock ws with no bufferedAmount) MUST behave exactly as before.
import { describe, expect, it, vi } from 'vitest'
import { handleWebSocket } from './ws-handler.js'

// Build a mock ws + session and capture the server→client `onMessage` callback
// that ws-handler registers on session.emitter. Connect happens with an empty
// buffer; the test sets ws.bufferedAmount afterward to simulate a slow client.
function setup() {
  const sent: string[] = []
  let onMessage: ((msg: unknown) => void) | undefined
  // biome-ignore lint/suspicious/noExplicitAny: test mock
  const ws: any = {
    send: vi.fn((s: string) => sent.push(s)),
    close: vi.fn(),
    readyState: 1,
    OPEN: 1,
    bufferedAmount: 0,
    on: vi.fn(),
  }
  // biome-ignore lint/suspicious/noExplicitAny: test mock
  const session: any = {
    wsClients: new Set(),
    lastSessionInit: null,
    state: 'active',
    emitter: {
      on: vi.fn((evt: string, cb: (msg: unknown) => void) => {
        if (evt === 'message') onMessage = cb
      }),
      removeListener: vi.fn(),
    },
    permissions: { drainInteractive: vi.fn() },
    accumulator: {
      getBlocks: vi.fn().mockReturnValue([{ type: 'user', id: 'u1', text: 'hi', timestamp: 0 }]),
    },
  }
  // biome-ignore lint/suspicious/noExplicitAny: test mock
  const registry: any = { get: vi.fn().mockReturnValue(session), emitSequenced: vi.fn() }
  handleWebSocket(ws, 'ctrl-1', registry)
  if (!onMessage) throw new Error('onMessage was not registered')
  return {
    ws,
    sent,
    emit: (m: unknown) => (onMessage as (msg: unknown) => void)(m),
    resetSent: () => {
      sent.length = 0
    },
  }
}

const typesOf = (sent: string[]) => sent.map((s) => JSON.parse(s).type)

describe('ws-handler backpressure', () => {
  it('sends blocks_update for a block-producing event when the buffer is empty (happy path)', () => {
    const h = setup()
    h.ws.bufferedAmount = 0
    h.resetSent()
    h.emit({ type: 'assistant_text', text: 'hello', messageId: 'm1', parentToolUseId: null })
    const types = typesOf(h.sent)
    expect(types).toContain('assistant_text')
    expect(types).toContain('blocks_update')
    expect(h.ws.close).not.toHaveBeenCalled()
  })

  it('treats a missing bufferedAmount as zero backpressure (mock-safe — preserves existing tests)', () => {
    const h = setup()
    h.ws.bufferedAmount = undefined
    h.resetSent()
    h.emit({
      type: 'tool_use_start',
      toolName: 'Bash',
      toolInput: {},
      toolUseId: 't1',
      messageId: 'm1',
    })
    expect(typesOf(h.sent)).toContain('blocks_update')
    expect(h.ws.close).not.toHaveBeenCalled()
  })

  it('skips the redundant full-snapshot blocks_update when the buffer is over the soft limit', () => {
    const h = setup()
    h.ws.bufferedAmount = 8 * 1024 * 1024 // 8 MB — over the soft limit
    h.resetSent()
    h.emit({ type: 'assistant_text', text: 'hello', messageId: 'm1', parentToolUseId: null })
    const types = typesOf(h.sent)
    // The small raw event still flows (it's the streaming delta the client needs)...
    expect(types).toContain('assistant_text')
    // ...but we do NOT pile another full conversation snapshot onto a backed-up
    // socket. The next under-limit event or turn_complete carries the latest state.
    expect(types).not.toContain('blocks_update')
    expect(h.ws.close).not.toHaveBeenCalled()
  })

  it('closes a client whose send buffer exceeds the hard limit (dead/too-slow client)', () => {
    const h = setup()
    h.ws.bufferedAmount = 64 * 1024 * 1024 // 64 MB — client is not draining; protect the server
    h.resetSent()
    h.emit({ type: 'assistant_text', text: 'hello', messageId: 'm1', parentToolUseId: null })
    expect(h.ws.close).toHaveBeenCalled()
  })
})
