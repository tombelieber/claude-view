import { useMemo } from 'react'
import type { PermissionMode } from '../types/control'

export interface SessionCapabilities {
  model: string
  permissionMode: PermissionMode
  slashCommands: string[]
  mcpServers: { name: string; status: string }[]
  skills: string[]
  agents: string[]
  fastModeState?: 'off' | 'cooldown' | 'on'
}

interface SessionInfoInput {
  model: string
  slashCommands: string[]
  mcpServers: { name: string; status: string }[]
  permissionMode: string
  skills: string[]
  agents: string[]
}

export function useSessionCapabilities(sessionInfo: SessionInfoInput): SessionCapabilities {
  return useMemo(
    () => ({
      model: sessionInfo.model,
      permissionMode: (sessionInfo.permissionMode || 'default') as PermissionMode,
      slashCommands: sessionInfo.slashCommands,
      mcpServers: sessionInfo.mcpServers,
      skills: sessionInfo.skills,
      agents: sessionInfo.agents,
    }),
    [
      sessionInfo.model,
      sessionInfo.permissionMode,
      sessionInfo.slashCommands,
      sessionInfo.mcpServers,
      sessionInfo.skills,
      sessionInfo.agents,
    ],
  )
}
