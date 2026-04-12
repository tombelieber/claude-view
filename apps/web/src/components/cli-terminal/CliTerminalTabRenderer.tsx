import type { IDockviewPanelHeaderProps } from 'dockview-react'
import { TabContent } from '../dockview/TabContent'

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
    if (e.button === 1) handleClose(e)
  }

  return (
    <TabContent
      title={api.title ?? ''}
      status={null}
      agentStateGroup={null}
      isTmux={true}
      onClose={handleClose}
      onMiddleClick={handleMiddleClick}
      dotColorOverride="bg-emerald-500"
    />
  )
}
