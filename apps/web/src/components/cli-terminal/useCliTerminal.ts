import { useCallback, useEffect, useRef, useState } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebglAddon } from '@xterm/addon-webgl'

interface UseCliTerminalOptions {
  tmuxSessionId: string | null
  containerRef: React.RefObject<HTMLDivElement | null>
}

interface UseCliTerminalResult {
  isConnected: boolean
  error: string | null
  /** Send raw key data to the terminal (used by card delegation) */
  sendKeys: (data: string) => void
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

export function useCliTerminal({
  tmuxSessionId,
  containerRef,
}: UseCliTerminalOptions): UseCliTerminalResult {
  const [isConnected, setIsConnected] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const terminalRef = useRef<Terminal | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const resizeObserverRef = useRef<ResizeObserver | null>(null)

  const sendKeys = useCallback((data: string) => {
    const ws = wsRef.current
    if (ws?.readyState === WebSocket.OPEN) {
      ws.send(data)
    }
  }, [])

  useEffect(() => {
    if (!tmuxSessionId || !containerRef.current) return

    const container = containerRef.current
    setError(null)
    setIsConnected(false)

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
      // Canvas fallback — xterm uses it by default
    }

    // Initial fit
    requestAnimationFrame(() => fitAddon.fit())

    // --- WebSocket setup ---
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const wsUrl = `${protocol}//${window.location.host}/ws/terminal/${tmuxSessionId}`
    const ws = new WebSocket(wsUrl)
    ws.binaryType = 'arraybuffer'
    wsRef.current = ws

    ws.addEventListener('open', () => {
      setIsConnected(true)
      setError(null)
      // Send initial resize
      const { cols, rows } = terminal
      ws.send(JSON.stringify({ type: 'resize', cols, rows }))
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
            setError(`Process exited (code ${msg.code ?? '?'})`)
            setIsConnected(false)
          } else if (msg.type === 'error') {
            setError(msg.message ?? 'Terminal error')
            setIsConnected(false)
          }
        } catch {
          // Not JSON — write as plain text
          terminal.write(event.data)
        }
      }
    })

    ws.addEventListener('close', () => {
      setIsConnected(false)
    })

    ws.addEventListener('error', () => {
      setError('WebSocket connection failed')
      setIsConnected(false)
    })

    // Terminal → WS: keystrokes
    const dataDisposable = terminal.onData((data) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(data)
      }
    })

    // --- Resize observer ---
    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        fitAddon.fit()
        if (ws.readyState === WebSocket.OPEN) {
          const { cols, rows } = terminal
          ws.send(JSON.stringify({ type: 'resize', cols, rows }))
        }
      })
    })
    resizeObserver.observe(container)
    resizeObserverRef.current = resizeObserver

    // --- Cleanup ---
    return () => {
      dataDisposable.dispose()
      resizeObserver.disconnect()
      resizeObserverRef.current = null

      if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
        ws.close()
      }
      wsRef.current = null

      terminal.dispose()
      terminalRef.current = null
      fitAddonRef.current = null
    }
  }, [tmuxSessionId, containerRef])

  return { isConnected, error, sendKeys }
}
