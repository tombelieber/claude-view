import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest'
import type { WebSocket } from 'ws'

// ── Mock node-pty before importing the module ───────────────────────

const mockPty = {
  onData: vi.fn(),
  onExit: vi.fn(),
  resize: vi.fn(),
  kill: vi.fn(),
  pause: vi.fn(),
  resume: vi.fn(),
  write: vi.fn(),
  pid: 1234,
  cols: 120,
  rows: 40,
  process: 'tmux',
  handleFlowControl: false,
  clear: vi.fn(),
}

vi.mock('node-pty', () => ({
  spawn: vi.fn(() => mockPty),
}))

// Import after mock setup
import { spawn } from 'node-pty'
import {
  parseClientMessage,
  hasBackpressure,
  canResume,
  handleTerminalWebSocket,
  closeAllTerminals,
  activeTerminalCount,
} from '../terminal-relay.js'

// ── Helpers ─────────────────────────────────────────────────────────

function createMockWs(overrides: Partial<WebSocket> = {}): WebSocket {
  return {
    readyState: 1, // OPEN
    bufferedAmount: 0,
    send: vi.fn(),
    close: vi.fn(),
    on: vi.fn(),
    ...overrides,
  } as unknown as WebSocket
}

// ── Tests ───────────────────────────────────────────────────────────

describe('parseClientMessage', () => {
  it('parses a valid resize message', () => {
    const result = parseClientMessage('{"type":"resize","cols":80,"rows":24}')
    expect(result).toEqual({ type: 'resize', cols: 80, rows: 24 })
  })

  it('returns null for invalid JSON', () => {
    expect(parseClientMessage('not json')).toBeNull()
  })

  it('returns null for unknown message type', () => {
    expect(parseClientMessage('{"type":"unknown"}')).toBeNull()
  })

  it('returns null for resize with zero cols', () => {
    expect(parseClientMessage('{"type":"resize","cols":0,"rows":24}')).toBeNull()
  })

  it('returns null for resize with negative rows', () => {
    expect(parseClientMessage('{"type":"resize","cols":80,"rows":-1}')).toBeNull()
  })

  it('returns null for resize with excessively large cols', () => {
    expect(parseClientMessage('{"type":"resize","cols":501,"rows":24}')).toBeNull()
  })

  it('returns null for resize with excessively large rows', () => {
    expect(parseClientMessage('{"type":"resize","cols":80,"rows":201}')).toBeNull()
  })

  it('returns null for resize with non-numeric cols', () => {
    expect(parseClientMessage('{"type":"resize","cols":"abc","rows":24}')).toBeNull()
  })

  it('accepts boundary values (1x1 and 500x200)', () => {
    expect(parseClientMessage('{"type":"resize","cols":1,"rows":1}')).toEqual({
      type: 'resize',
      cols: 1,
      rows: 1,
    })
    expect(parseClientMessage('{"type":"resize","cols":500,"rows":200}')).toEqual({
      type: 'resize',
      cols: 500,
      rows: 200,
    })
  })
})

describe('hasBackpressure', () => {
  it('returns false when all clients are below threshold', () => {
    const clients = new Set([
      createMockWs({ bufferedAmount: 0 } as Partial<WebSocket>),
      createMockWs({ bufferedAmount: 1000 } as Partial<WebSocket>),
    ])
    expect(hasBackpressure(clients)).toBe(false)
  })

  it('returns true when any client exceeds threshold', () => {
    const clients = new Set([
      createMockWs({ bufferedAmount: 0 } as Partial<WebSocket>),
      createMockWs({
        bufferedAmount: 128 * 1024 + 1,
      } as Partial<WebSocket>),
    ])
    expect(hasBackpressure(clients)).toBe(true)
  })

  it('returns false for empty client set', () => {
    expect(hasBackpressure(new Set())).toBe(false)
  })
})

describe('canResume', () => {
  it('returns true when all clients are below low watermark', () => {
    const clients = new Set([
      createMockWs({ bufferedAmount: 0 } as Partial<WebSocket>),
      createMockWs({ bufferedAmount: 100 } as Partial<WebSocket>),
    ])
    expect(canResume(clients)).toBe(true)
  })

  it('returns false when any client is above low watermark', () => {
    const clients = new Set([
      createMockWs({ bufferedAmount: 0 } as Partial<WebSocket>),
      createMockWs({
        bufferedAmount: 16 * 1024 + 1,
      } as Partial<WebSocket>),
    ])
    expect(canResume(clients)).toBe(false)
  })

  it('returns true for empty client set', () => {
    expect(canResume(new Set())).toBe(true)
  })
})

describe('handleTerminalWebSocket', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    // Reset active sessions via closeAllTerminals
    closeAllTerminals()
    // Re-clear mocks after closeAllTerminals may have called kill/close
    vi.clearAllMocks()
  })

  it('spawns a pty with correct args for a new session', () => {
    const ws = createMockWs()
    handleTerminalWebSocket(ws, 'cv-a1b2c3d4')

    expect(spawn).toHaveBeenCalledWith(
      'tmux',
      ['attach-session', '-t', 'cv-a1b2c3d4'],
      expect.objectContaining({
        cols: 120,
        rows: 40,
        name: 'xterm-256color',
      }),
    )
    expect(activeTerminalCount()).toBe(1)
  })

  it('reuses existing pty for same session ID', () => {
    const ws1 = createMockWs()
    const ws2 = createMockWs()

    handleTerminalWebSocket(ws1, 'cv-e5f6a7b8')
    handleTerminalWebSocket(ws2, 'cv-e5f6a7b8')

    // spawn called only once
    expect(spawn).toHaveBeenCalledTimes(1)
    expect(activeTerminalCount()).toBe(1)
  })

  it('registers message and close handlers on the ws', () => {
    const ws = createMockWs()
    handleTerminalWebSocket(ws, 'cv-11111111')

    const onCalls = (ws.on as ReturnType<typeof vi.fn>).mock.calls
    const events = onCalls.map((call: unknown[]) => call[0])
    expect(events).toContain('message')
    expect(events).toContain('close')
    expect(events).toContain('error')
  })

  it('forwards binary messages to pty.write', () => {
    const ws = createMockWs()
    handleTerminalWebSocket(ws, 'cv-22222222')

    // Get the 'message' handler
    const onCalls = (ws.on as ReturnType<typeof vi.fn>).mock.calls
    const messageHandler = onCalls.find((call: unknown[]) => call[0] === 'message')?.[1] as (
      data: Buffer | string,
      isBinary: boolean,
    ) => void

    expect(messageHandler).toBeDefined()

    // Simulate binary message
    const keystroke = Buffer.from('ls\n')
    messageHandler(keystroke, true)

    expect(mockPty.write).toHaveBeenCalledWith('ls\n')
  })

  it('handles resize JSON messages', () => {
    const ws = createMockWs()
    handleTerminalWebSocket(ws, 'cv-33333333')

    const onCalls = (ws.on as ReturnType<typeof vi.fn>).mock.calls
    const messageHandler = onCalls.find((call: unknown[]) => call[0] === 'message')?.[1] as (
      data: Buffer | string,
      isBinary: boolean,
    ) => void

    // Simulate text message with resize
    messageHandler('{"type":"resize","cols":80,"rows":24}', false)

    expect(mockPty.resize).toHaveBeenCalledWith(80, 24)
  })

  it('cleans up session when last client disconnects', () => {
    const ws = createMockWs()
    handleTerminalWebSocket(ws, 'cv-44444444')

    expect(activeTerminalCount()).toBe(1)

    // Simulate close
    const onCalls = (ws.on as ReturnType<typeof vi.fn>).mock.calls
    const closeHandler = onCalls.find((call: unknown[]) => call[0] === 'close')?.[1] as () => void

    closeHandler()

    expect(mockPty.kill).toHaveBeenCalled()
    expect(activeTerminalCount()).toBe(0)
  })

  it('keeps session alive when one of multiple clients disconnects', () => {
    const ws1 = createMockWs()
    const ws2 = createMockWs()

    handleTerminalWebSocket(ws1, 'cv-55555555')
    handleTerminalWebSocket(ws2, 'cv-55555555')

    expect(activeTerminalCount()).toBe(1)

    // Disconnect first client
    const ws1OnCalls = (ws1.on as ReturnType<typeof vi.fn>).mock.calls
    const closeHandler = ws1OnCalls.find(
      (call: unknown[]) => call[0] === 'close',
    )?.[1] as () => void

    closeHandler()

    // Session still alive (ws2 still connected)
    expect(activeTerminalCount()).toBe(1)
    expect(mockPty.kill).not.toHaveBeenCalled()
  })
})

describe('closeAllTerminals', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    closeAllTerminals()
    vi.clearAllMocks()
  })

  it('kills all ptys and closes all clients', () => {
    const ws1 = createMockWs()
    const ws2 = createMockWs()

    handleTerminalWebSocket(ws1, 'cv-66666666')
    handleTerminalWebSocket(ws2, 'cv-77777777')

    expect(activeTerminalCount()).toBe(2)

    closeAllTerminals()

    expect(mockPty.kill).toHaveBeenCalled()
    expect(ws1.close).toHaveBeenCalled()
    expect(ws2.close).toHaveBeenCalled()
    expect(activeTerminalCount()).toBe(0)
  })

  it('is safe to call when no sessions exist', () => {
    expect(activeTerminalCount()).toBe(0)
    closeAllTerminals() // should not throw
    expect(activeTerminalCount()).toBe(0)
  })
})
