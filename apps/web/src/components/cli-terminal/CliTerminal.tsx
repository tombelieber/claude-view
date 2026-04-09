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
  const { isConnected, error, sendKeys } = useCliTerminal({ tmuxSessionId, containerRef })

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
      {/* Terminal container */}
      <div ref={containerRef} className="w-full h-full pt-5" />
      {/* Error overlay */}
      {error && (
        <div className="absolute inset-0 flex items-center justify-center bg-gray-900/90">
          <div className="text-center">
            <div className="text-sm text-gray-400">{error}</div>
          </div>
        </div>
      )}
    </div>
  )
}
