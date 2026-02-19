import { useEffect, useRef, useCallback, useState } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebglAddon } from '@xterm/addon-webgl'
import { Loader2, WifiOff, ArrowDown } from 'lucide-react'
import '@xterm/xterm/css/xterm.css'
import { useTerminalSocket, type ConnectionState } from '../../../hooks/use-terminal-socket'

export interface TerminalPaneProps {
  sessionId: string
  mode: 'raw' | 'rich'
  scrollback?: number
  isVisible: boolean
  onConnectionChange?: (state: ConnectionState) => void
}

/**
 * xterm.js wrapper that renders a live terminal view of a Claude Code session.
 *
 * Lifecycle:
 * 1. On mount: creates Terminal + FitAddon, loads WebglAddon (canvas fallback)
 * 2. On isVisible change: connects/disconnects WebSocket via useTerminalSocket
 * 3. On container resize: fitAddon.fit() via ResizeObserver
 * 4. On unmount: disposes terminal, cleans up WebSocket
 *
 * Writes are throttled at 60fps via requestAnimationFrame batching.
 *
 * Scroll behavior:
 * - Auto-scrolls to bottom when viewport is already at bottom (xterm.js native)
 * - When user scrolls up: stops auto-scrolling, shows "scroll to bottom" button
 * - When initial buffer finishes loading: forces scroll to bottom
 * - After fitAddon.fit(): re-scrolls to bottom if was at bottom
 */
export function TerminalPane({
  sessionId,
  mode,
  scrollback = 100_000,
  isVisible,
  onConnectionChange,
}: TerminalPaneProps) {
  // All hooks at top — no hooks after early returns (CLAUDE.md rule)
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected')
  const [showScrollBtn, setShowScrollBtn] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const terminalRef = useRef<Terminal | null>(null)
  const fitAddonRef = useRef<FitAddon | null>(null)
  const resizeObserverRef = useRef<ResizeObserver | null>(null)

  // Write-throttling state (not in React state — mutable refs for RAF performance)
  const writeBufferRef = useRef<string[]>([])
  const rafHandleRef = useRef<number | null>(null)

  // Scroll tracking: ref for hot-path checks, state only for button visibility.
  // bufferLoadedRef is false during initial scrollback load — forces scroll to
  // bottom on every write so intermediate onScroll events can't break it.
  const isAtBottomRef = useRef(true)
  const bufferLoadedRef = useRef(false)

  const flushWrites = useCallback(() => {
    const terminal = terminalRef.current
    const buffer = writeBufferRef.current
    if (terminal && buffer.length > 0) {
      const data = buffer.join('')
      buffer.length = 0
      // Use write callback so scrollToBottom runs AFTER data is processed.
      // terminal.write() is async (queues data for the parser).
      terminal.write(data, () => {
        if (!terminalRef.current) return // terminal disposed during write
        // After buffer loaded: only scroll to bottom if user hasn't scrolled up.
        // During initial buffer load: don't force scroll — we'll scroll to top
        // after the buffer finishes loading so the user sees content from the start.
        if (bufferLoadedRef.current && isAtBottomRef.current) {
          terminal.scrollToBottom()
        }
      })
    }
    rafHandleRef.current = null
  }, [])

  const enqueueWrite = useCallback((data: string) => {
    writeBufferRef.current.push(data)
    if (rafHandleRef.current === null) {
      rafHandleRef.current = requestAnimationFrame(flushWrites)
    }
  }, [flushWrites])

  // Scroll to bottom — used by button and programmatic scrolls
  const scrollToBottom = useCallback(() => {
    const terminal = terminalRef.current
    if (terminal) {
      terminal.scrollToBottom()
      isAtBottomRef.current = true
      setShowScrollBtn(false)
    }
  }, [])

  // Message handler for incoming WebSocket data
  const handleMessage = useCallback((data: string) => {
    let parsed: { type: string; data?: string; message?: string }
    try {
      parsed = JSON.parse(data)
    } catch {
      // Non-JSON message — write raw to terminal
      enqueueWrite(data)
      return
    }

    switch (parsed.type) {
      case 'line':
        if (parsed.data != null) {
          enqueueWrite(parsed.data + '\r\n')
        }
        break

      case 'buffer_end':
        // Connection state handled by useTerminalSocket hook — no visual action
        break

      case 'error':
        enqueueWrite(`\x1b[31m[Error] ${parsed.message ?? 'Unknown error'}\x1b[0m\r\n`)
        break

      case 'reset': {
        const terminal = terminalRef.current
        if (terminal) {
          terminal.clear()
          enqueueWrite('\x1b[33m[Session reset]\x1b[0m\r\n')
        }
        break
      }

      default:
        // Unknown message type — write raw data if present
        if (parsed.data != null) {
          enqueueWrite(parsed.data)
        }
        break
    }
  }, [enqueueWrite])

  // Connection change handler: update local state + forward to parent
  const handleConnectionChange = useCallback((state: ConnectionState) => {
    setConnectionState(state)
    onConnectionChange?.(state)
  }, [onConnectionChange])

  // WebSocket connection — controlled by isVisible
  const { reconnect } = useTerminalSocket({
    sessionId,
    mode,
    scrollback: scrollback,
    enabled: isVisible,
    onMessage: handleMessage,
    onConnectionChange: handleConnectionChange,
  })

  // Terminal lifecycle: create on mount, dispose on unmount
  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const terminal = new Terminal({
      fontSize: 12,
      fontFamily: 'JetBrains Mono, Menlo, monospace',
      theme: {
        // Base
        background: '#0D1117',
        foreground: '#C9D1D9',
        cursor: '#79C0FF',
        cursorAccent: '#0D1117',
        selectionBackground: '#1F6FEB44',
        selectionInactiveBackground: '#1F6FEB22',
        // ANSI normal (0–7)
        black: '#484F58',
        red: '#FF7B72',
        green: '#7EE787',
        yellow: '#FFA657',
        blue: '#79C0FF',
        magenta: '#D2A8FF',
        cyan: '#56D4DD',
        white: '#B1BAC4',
        // ANSI bright (8–15)
        brightBlack: '#6E7681',
        brightRed: '#FFA198',
        brightGreen: '#56D364',
        brightYellow: '#E3B341',
        brightBlue: '#A5D6FF',
        brightMagenta: '#E2B5FF',
        brightCyan: '#76E4F7',
        brightWhite: '#F0F6FC',
      },
      scrollback: scrollback,
      cursorBlink: false,
      disableStdin: true,
      convertEol: true,
      allowProposedApi: true,
    })

    const fitAddon = new FitAddon()
    terminal.loadAddon(fitAddon)

    terminal.open(container)

    // Try WebGL renderer for performance, fall back to canvas
    try {
      terminal.loadAddon(new WebglAddon())
    } catch {
      // WebGL not available — xterm falls back to canvas automatically
    }

    // Initial fit
    fitAddon.fit()

    terminalRef.current = terminal
    fitAddonRef.current = fitAddon

    // Track scroll position via xterm.js onScroll event.
    // Only update React state when the button visibility actually changes
    // to avoid re-renders on every scroll tick.
    const scrollDisposable = terminal.onScroll(() => {
      const buf = terminal.buffer.active
      const atBottom = buf.viewportY >= buf.baseY
      isAtBottomRef.current = atBottom
      setShowScrollBtn((prev) => {
        const shouldShow = !atBottom
        return prev === shouldShow ? prev : shouldShow
      })
    })

    // ResizeObserver for responsive fit
    const observer = new ResizeObserver(() => {
      // Guard: only fit if terminal is still alive and container has dimensions
      if (terminalRef.current && container.offsetWidth > 0 && container.offsetHeight > 0) {
        fitAddonRef.current?.fit()
        // After resize, re-scroll to bottom if viewport was at bottom.
        // fit() changes row count which can shift the viewport position.
        if (isAtBottomRef.current) {
          terminalRef.current.scrollToBottom()
        }
      }
    })
    observer.observe(container)
    resizeObserverRef.current = observer

    // Capture mutable ref values for cleanup
    const writeBuffer = writeBufferRef.current

    return () => {
      // Cancel any pending RAF
      if (rafHandleRef.current !== null) {
        cancelAnimationFrame(rafHandleRef.current)
        rafHandleRef.current = null
      }
      writeBuffer.length = 0

      scrollDisposable.dispose()
      observer.disconnect()
      resizeObserverRef.current = null

      terminal.dispose()
      terminalRef.current = null
      fitAddonRef.current = null
    }
  }, [scrollback])

  // Scroll to top when initial buffer finishes loading so the user sees
  // content from the start of the session, not the end. The 'connected'
  // state fires after buffer_end is received, meaning all scrollback lines
  // have been written. A short delay lets the last write batch and
  // fitAddon.fit() settle before scrolling.
  useEffect(() => {
    if (connectionState === 'connected') {
      const timer = setTimeout(() => {
        if (terminalRef.current) {
          terminalRef.current.scrollToTop()
          isAtBottomRef.current = false
          setShowScrollBtn(true)
        }
        // Transition to normal scroll mode — from now on, respect user scroll position
        bufferLoadedRef.current = true
      }, 100)
      return () => clearTimeout(timer)
    }
    // Reset when disconnecting (re-connect will re-load buffer)
    if (connectionState === 'connecting') {
      bufferLoadedRef.current = false
    }
  }, [connectionState])

  // Re-fit when visibility changes (container may have been resized while hidden)
  useEffect(() => {
    if (isVisible && fitAddonRef.current && containerRef.current) {
      // Small delay to let layout settle after visibility change
      const timer = setTimeout(() => {
        if (containerRef.current && containerRef.current.offsetWidth > 0) {
          fitAddonRef.current?.fit()
          // Re-scroll after fit if was at bottom
          if (isAtBottomRef.current && terminalRef.current) {
            terminalRef.current.scrollToBottom()
          }
        }
      }, 50)
      return () => clearTimeout(timer)
    }
  }, [isVisible])

  const showOverlay = connectionState === 'connecting' || connectionState === 'disconnected' || connectionState === 'error'

  return (
    <div className="relative h-full w-full">
      <div
        ref={containerRef}
        className="h-full w-full"
        style={{ backgroundColor: '#0D1117' }}
      />

      {/* Scroll to bottom button — visible when user has scrolled up */}
      {showScrollBtn && !showOverlay && (
        <button
          onClick={scrollToBottom}
          className="absolute bottom-3 right-3 z-10 flex items-center gap-1.5 rounded-md bg-[#1F2937]/95 border border-[#374151] px-2.5 py-1.5 text-[11px] font-medium text-[#A5D6FF] hover:bg-[#374151] hover:text-[#F0F6FC] transition-all duration-150 shadow-lg shadow-black/40 backdrop-blur-sm"
          title="Scroll to bottom"
        >
          <ArrowDown className="w-3 h-3" />
          Bottom
        </button>
      )}

      {showOverlay && (
        <div
          className={`absolute inset-0 flex items-center justify-center bg-[#0D1117]/80 backdrop-blur-[2px] transition-opacity duration-200 ${
            connectionState === 'error' ? 'cursor-pointer' : ''
          }`}
          onClick={connectionState === 'error' ? reconnect : undefined}
        >
          <div className="flex flex-col items-center gap-2.5 text-sm">
            {connectionState === 'connecting' && (
              <>
                <Loader2 className="h-5 w-5 animate-spin text-[#79C0FF]" />
                <span className="text-[#8B949E]">Connecting...</span>
              </>
            )}
            {connectionState === 'disconnected' && (
              <>
                <Loader2 className="h-5 w-5 animate-spin text-[#FFA657]" />
                <span className="text-[#8B949E]">Reconnecting...</span>
              </>
            )}
            {connectionState === 'error' && (
              <>
                <WifiOff className="h-5 w-5 text-[#FF7B72]" />
                <span className="text-[#8B949E]">Connection failed</span>
                <span className="text-[11px] text-[#6E7681]">Click to retry</span>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
