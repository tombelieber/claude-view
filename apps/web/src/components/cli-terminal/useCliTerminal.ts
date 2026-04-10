import { useCallback, useEffect, useRef, useState } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebglAddon } from '@xterm/addon-webgl'

interface UseCliTerminalOptions {
  tmuxSessionId: string | null
  containerRef: React.RefObject<HTMLDivElement | null>
}

export type TerminalStatus =
  | { state: 'connecting' }
  | { state: 'connected' }
  | { state: 'exited'; code: number | null }
  | { state: 'disconnected'; reason: string }

interface UseCliTerminalResult {
  status: TerminalStatus
  /** Send raw key data to the terminal (used by card delegation) */
  sendKeys: (data: string) => void
  /** Manually retry connection after exhausting auto-reconnect attempts. */
  reconnect: () => void
  /** Focus the terminal so it receives keyboard input. */
  focus: () => void
}

const TERMINAL_THEME = {
  background: '#1a1b26',
  foreground: '#c0caf5',
  cursor: '#c0caf5',
  selectionBackground: '#33467c',
  black: '#15161e',
  red: '#f7768e',
  green: '#9ece6a',
  yellow: '#e0af68',
  blue: '#7aa2f7',
  magenta: '#bb9af7',
  cyan: '#7dcfff',
  white: '#a9b1d6',
}

const MAX_RECONNECT_ATTEMPTS = 3
const RECONNECT_DELAY_MS = 2000

export function useCliTerminal({
  tmuxSessionId,
  containerRef,
}: UseCliTerminalOptions): UseCliTerminalResult {
  const [status, setStatus] = useState<TerminalStatus>({ state: 'connecting' })

  const terminalRef = useRef<Terminal | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const resizeObserverRef = useRef<ResizeObserver | null>(null)
  const reconnectAttempts = useRef(0)
  const intentionalClose = useRef(false)
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const sendKeys = useCallback((data: string) => {
    const ws = wsRef.current
    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(data)
    }
  }, [])

  const connectWs = useCallback(
    (terminal: Terminal) => {
      if (!tmuxSessionId) return

      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
      const wsUrl = `${protocol}//${window.location.host}/ws/terminal/${tmuxSessionId}`
      const ws = new WebSocket(wsUrl)
      ws.binaryType = 'arraybuffer'
      wsRef.current = ws

      ws.addEventListener('open', () => {
        setStatus({ state: 'connected' })
        reconnectAttempts.current = 0
        const { cols, rows } = terminal
        ws.send(JSON.stringify({ type: 'resize', cols, rows }))
        // Focus terminal so it receives keystrokes immediately
        terminal.focus()
      })

      ws.addEventListener('message', (event) => {
        if (event.data instanceof ArrayBuffer) {
          terminal.write(new Uint8Array(event.data))
        } else if (typeof event.data === 'string') {
          try {
            const msg = JSON.parse(event.data) as {
              type: string
              code?: number
              message?: string
            }
            if (msg.type === 'exit') {
              setStatus({ state: 'exited', code: msg.code ?? null })
            } else if (msg.type === 'error') {
              setStatus({ state: 'disconnected', reason: msg.message ?? 'Terminal error' })
            }
          } catch {
            terminal.write(event.data)
          }
        }
      })

      ws.addEventListener('close', () => {
        setStatus((prev) => {
          // Don't overwrite 'exited' — process exit already set the terminal state
          if (prev.state === 'exited') return prev
          if (!intentionalClose.current && reconnectAttempts.current < MAX_RECONNECT_ATTEMPTS) {
            reconnectAttempts.current++
            reconnectTimer.current = setTimeout(() => connectWs(terminal), RECONNECT_DELAY_MS)
            return { state: 'connecting' }
          }
          if (!intentionalClose.current) {
            return { state: 'disconnected', reason: 'Connection lost' }
          }
          return prev
        })
      })

      ws.addEventListener('error', () => {
        // Let the 'close' handler decide the final state
      })

      const dataDisposable = terminal.onData((data) => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(data)
        }
      })

      // Store disposable for cleanup
      ;(ws as unknown as { _dataDisposable: { dispose: () => void } })._dataDisposable =
        dataDisposable
    },
    [tmuxSessionId],
  )

  // Manual reconnect — only meaningful when disconnected (not exited)
  const reconnect = useCallback(() => {
    if (!terminalRef.current) return
    reconnectAttempts.current = 0
    setStatus({ state: 'connecting' })
    const oldWs = wsRef.current
    if (
      oldWs &&
      (oldWs.readyState === WebSocket.OPEN || oldWs.readyState === WebSocket.CONNECTING)
    ) {
      intentionalClose.current = true
      oldWs.close()
      intentionalClose.current = false
    }
    connectWs(terminalRef.current)
  }, [connectWs])

  useEffect(() => {
    if (!tmuxSessionId || !containerRef.current) return

    const container = containerRef.current
    setStatus({ state: 'connecting' })
    intentionalClose.current = false
    reconnectAttempts.current = 0

    // --- Terminal setup ---
    const terminal = new Terminal({
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      fontSize: 13,
      theme: TERMINAL_THEME,
      cursorBlink: true,
      allowProposedApi: true,
    })
    terminalRef.current = terminal

    const fitAddon = new FitAddon()
    fitAddonRef.current = fitAddon
    terminal.loadAddon(fitAddon)

    terminal.open(container)

    // Try WebGL renderer, fall back to canvas
    try {
      const webglAddon = new WebglAddon()
      webglAddon.onContextLoss(() => webglAddon.dispose())
      terminal.loadAddon(webglAddon)
    } catch {
      // Canvas fallback
    }

    requestAnimationFrame(() => fitAddon.fit())

    // --- WebSocket setup ---
    connectWs(terminal)

    // --- Resize observer ---
    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        fitAddon.fit()
        const ws = wsRef.current
        if (ws?.readyState === WebSocket.OPEN) {
          const { cols, rows } = terminal
          ws.send(JSON.stringify({ type: 'resize', cols, rows }))
        }
      })
    })
    resizeObserver.observe(container)
    resizeObserverRef.current = resizeObserver

    // --- Cleanup ---
    return () => {
      intentionalClose.current = true

      if (reconnectTimer.current) {
        clearTimeout(reconnectTimer.current)
        reconnectTimer.current = null
      }

      const ws = wsRef.current
      if (ws) {
        const d = (ws as unknown as { _dataDisposable?: { dispose: () => void } })._dataDisposable
        d?.dispose()
        if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
          ws.close()
        }
        wsRef.current = null
      }

      resizeObserver.disconnect()
      resizeObserverRef.current = null

      terminal.dispose()
      terminalRef.current = null
      fitAddonRef.current = null
    }
  }, [tmuxSessionId, containerRef, connectWs])

  const focus = useCallback(() => {
    terminalRef.current?.focus()
  }, [])

  return { status, sendKeys, reconnect, focus }
}
