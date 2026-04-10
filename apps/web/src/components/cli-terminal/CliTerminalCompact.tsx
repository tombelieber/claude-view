import { CliTerminal } from './CliTerminal'

interface CliTerminalCompactProps {
  tmuxSessionId: string
  onExpand?: () => void
}

export function CliTerminalCompact({ tmuxSessionId, onExpand }: CliTerminalCompactProps) {
  return (
    <div className="relative">
      <CliTerminal tmuxSessionId={tmuxSessionId} className="h-48" />
      {onExpand && (
        <button
          type="button"
          onClick={onExpand}
          className="absolute bottom-1 right-1 px-2 py-0.5 text-xs bg-gray-800/80 text-gray-300 rounded hover:bg-gray-700/80 transition-colors"
        >
          Expand
        </button>
      )}
    </div>
  )
}
