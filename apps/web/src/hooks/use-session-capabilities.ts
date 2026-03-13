import { useMemo } from 'react'
import type { PermissionMode } from '../types/control'

export interface SessionCapabilities {
  model: string
  permissionMode: PermissionMode
  slashCommands: string[]
  mcpServers: { name: string; status: string }[]
  fastModeState?: 'off' | 'cooldown' | 'on'
}

interface SessionInfoInput {
  model: string
  slashCommands: string[]
  mcpServers: { name: string; status: string }[]
  permissionMode: string
}

export function useSessionCapabilities(sessionInfo: SessionInfoInput): SessionCapabilities {
  return useMemo(
    () => ({
      model: sessionInfo.model,
      permissionMode: (sessionInfo.permissionMode || 'default') as PermissionMode,
      slashCommands: sessionInfo.slashCommands,
      mcpServers: sessionInfo.mcpServers,
    }),
    [
      sessionInfo.model,
      sessionInfo.permissionMode,
      sessionInfo.slashCommands,
      sessionInfo.mcpServers,
    ],
  )
}
