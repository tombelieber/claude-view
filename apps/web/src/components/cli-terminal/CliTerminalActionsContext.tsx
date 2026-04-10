import { createContext, useContext } from 'react'

export interface CliTerminalActions {
  /** Send raw key data to the terminal */
  sendKeys: (data: string) => void
  /** Whether the terminal is connected */
  isConnected: boolean
  /** The tmux session ID (null if no CLI session) */
  tmuxSessionId: string | null
}

const CliTerminalActionsContext = createContext<CliTerminalActions | null>(null)

export const CliTerminalActionsProvider = CliTerminalActionsContext.Provider

export function useCliTerminalActions(): CliTerminalActions | null {
  return useContext(CliTerminalActionsContext)
}
