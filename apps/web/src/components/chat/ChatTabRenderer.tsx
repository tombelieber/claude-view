import type { IDockviewPanelHeaderProps } from 'dockview-react'
import { TabContent } from '../dockview/TabContent'
import { TabContextMenu } from '../dockview/TabContextMenu'

export function ChatTabRenderer({ api, params, containerApi }: IDockviewPanelHeaderProps) {
  const status = (params.status as string | null) ?? null
  const agentStateGroup = (params.agentStateGroup as string | null) ?? null
  const tmuxSessionId = (params.tmuxSessionId as string | undefined) ?? undefined
  const isTmux = !!tmuxSessionId

  const handleClose = (e: React.MouseEvent) => {
    e.stopPropagation()
    if (tmuxSessionId) {
      fetch(`/api/cli-sessions/${tmuxSessionId}`, { method: 'DELETE' }).catch(() => {})
    }
    api.close()
  }

  const handleMiddleClick = (e: React.MouseEvent) => {
    if (e.button === 1) handleClose(e)
  }

  const panel = containerApi.panels.find((p) => p.id === api.id)

  const tab = (
    <TabContent
      title={api.title ?? ''}
      status={status}
      agentStateGroup={agentStateGroup}
      isTmux={isTmux}
      onClose={handleClose}
      onMiddleClick={handleMiddleClick}
    />
  )

  if (panel) {
    return (
      <TabContextMenu panel={panel} api={containerApi} splitComponent="chat">
        {tab}
      </TabContextMenu>
    )
  }

  return tab
}
