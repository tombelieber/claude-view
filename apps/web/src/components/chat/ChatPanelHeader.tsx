import type { PermissionMode } from '../../types/control'
import type { ChatSessionStatus } from './SessionStatusDot'
import { SessionStatusDot } from './SessionStatusDot'

type DisplayMode = 'chat' | 'developer'

interface ChatPanelHeaderProps {
  /** Current session status for the status indicator */
  status: ChatSessionStatus
  /** Whether a permission request is pending */
  permissionPending?: boolean
  /** Whether the WS connection is live */
  isLive: boolean
  /** Current display mode (chat vs developer view) */
  displayMode: DisplayMode
  /** Callback when display mode changes */
  onDisplayModeChange: (mode: DisplayMode) => void
  /** Current permission mode */
  permissionMode: PermissionMode
  /** Callback when permission mode changes */
  onPermissionModeChange: (mode: PermissionMode) => void
  /** Whether MCP panel toggle is available */
  hasMcp?: boolean
  /** Callback when MCP button is clicked */
  onMcpToggle?: () => void
  /** Whether thinking budget control is available */
  hasThinkingBudget?: boolean
  /** Current thinking budget value */
  thinkingBudget?: number | null
  /** Callback when thinking budget changes */
  onThinkingBudgetChange?: (tokens: number | null) => void
}

function ModeToggle({ mode, onChange }: { mode: DisplayMode; onChange: (m: DisplayMode) => void }) {
  return (
    <div className="flex items-center gap-1 p-0.5 rounded-md bg-gray-100 dark:bg-gray-800 text-sm">
      {(['chat', 'developer'] as const).map((m) => (
        <button
          type="button"
          key={m}
          onClick={() => onChange(m)}
          className={[
            'px-2.5 py-1 rounded transition-colors capitalize',
            mode === m
              ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
              : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300',
          ].join(' ')}
        >
          {m}
        </button>
      ))}
    </div>
  )
}

export function ChatPanelHeader({
  status,
  permissionPending,
  isLive,
  displayMode,
  onDisplayModeChange,
  hasMcp,
  onMcpToggle,
}: ChatPanelHeaderProps) {
  return (
    <div className="flex items-center justify-between px-3 py-1.5 border-b border-gray-200 dark:border-[#30363D] bg-[#f6f8fa] dark:bg-[#161B22] flex-shrink-0">
      <div className="flex items-center gap-2">
        <SessionStatusDot status={status} permissionPending={permissionPending} />
        <span className="text-xs text-gray-500 dark:text-[#8B949E] capitalize">
          {isLive ? status : 'History'}
        </span>
      </div>
      <div className="flex items-center gap-2">
        {hasMcp && (
          <button
            type="button"
            onClick={onMcpToggle}
            className="text-xs px-2 py-0.5 rounded border border-gray-200 dark:border-[#30363D] hover:bg-gray-100 dark:hover:bg-[#21262D] text-gray-600 dark:text-[#8B949E]"
          >
            MCP
          </button>
        )}
        <ModeToggle mode={displayMode} onChange={onDisplayModeChange} />
      </div>
    </div>
  )
}
