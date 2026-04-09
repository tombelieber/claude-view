import type { IDockviewPanelProps } from 'dockview-react'
import { CliTerminal } from './CliTerminal'

export interface CliTerminalPanelParams {
  tmuxSessionId: string
}

export function CliTerminalPanel({ params }: IDockviewPanelProps<CliTerminalPanelParams>) {
  return <CliTerminal tmuxSessionId={params.tmuxSessionId} className="h-full" />
}
