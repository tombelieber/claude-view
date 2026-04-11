import type { IDockviewPanelHeaderProps } from 'dockview-react'

export function CliTerminalTabRenderer({ api, params }: IDockviewPanelHeaderProps) {
  const handleClose = (e: React.MouseEvent) => {
    e.stopPropagation()
    e.preventDefault()
    const sessionId = (params as Record<string, unknown>).tmuxSessionId as string | undefined
    if (sessionId) {
      fetch(`/api/cli-sessions/${sessionId}`, { method: 'DELETE' }).catch(() => {})
    }
    api.close()
  }

  const handleMiddleClick = (e: React.MouseEvent) => {
    if (e.button === 1) {
      handleClose(e)
    }
  }

  return (
    <div
      className="group flex items-center gap-1.5 px-3 h-full text-xs"
      onMouseDown={handleMiddleClick}
    >
      <div className="w-1.5 h-1.5 rounded-full bg-emerald-500 flex-shrink-0" />
      <span className="truncate">{api.title}</span>
      <button
        type="button"
        onClick={handleClose}
        className="ml-1 shrink-0 flex items-center justify-center w-5 h-5 rounded text-gray-400 dark:text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-600/50 transition-colors"
        title="Kill CLI session"
      >
        <svg
          width="10"
          height="10"
          viewBox="0 0 10 10"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.25"
          strokeLinecap="round"
        >
          <path d="M2 2l6 6M8 2l-6 6" />
        </svg>
      </button>
    </div>
  )
}
