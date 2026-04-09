import { useEffect, useRef } from 'react'
import { useCliTerminal } from './useCliTerminal'
import '@xterm/xterm/css/xterm.css'

interface CliTerminalProps {
  tmuxSessionId: string | null
  className?: string
  onSendKeys?: (sendFn: (data: string) => void) => void
}

export function CliTerminal({ tmuxSessionId, className, onSendKeys }: CliTerminalProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const { isConnected, error, sendKeys, reconnect, focus } = useCliTerminal({
    tmuxSessionId,
    containerRef,
  })

  // Expose sendKeys to parent
  useEffect(() => {
    onSendKeys?.(sendKeys)
  }, [sendKeys, onSendKeys])

  return (
    <div className={`relative ${className ?? ''}`}>
      {/* Status bar */}
      <div className="absolute top-0 left-0 right-0 z-10 flex items-center gap-2 px-2 py-0.5 bg-gray-900/80 text-xs">
        <div
          className={`w-1.5 h-1.5 rounded-full ${isConnected ? 'bg-green-500' : 'bg-red-500'}`}
        />
        <span className="text-gray-400">
          {isConnected ? 'Connected' : (error ?? 'Connecting...')}
        </span>
      </div>
      {/* Terminal container — mousedown stops propagation to prevent dockview's
          focus management from stealing focus away from xterm's textarea */}
      <div
        ref={containerRef}
        className="w-full h-full pt-5"
        onMouseDown={(e) => {
          e.stopPropagation()
          requestAnimationFrame(focus)
        }}
      />
      {/* Error overlay with reconnect button */}
      {error && (
        <div className="absolute inset-0 flex items-center justify-center bg-gray-900/90">
          <div className="text-center space-y-2">
            <div className="text-sm text-gray-400">{error}</div>
            {!isConnected && (
              <button
                type="button"
                onClick={reconnect}
                className="px-3 py-1.5 text-xs font-medium text-white bg-emerald-600 rounded-md hover:bg-emerald-700 transition-colors"
              >
                Reconnect
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
