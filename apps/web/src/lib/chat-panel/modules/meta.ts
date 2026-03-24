import type { SessionMeta } from '../types'

export type MetaEvent =
  | {
      type: 'SESSION_INIT'
      model: string
      permissionMode: string
      slashCommands: string[]
      mcpServers: { name: string; status: string }[]
      skills: string[]
      agents: string[]
      capabilities: string[]
    }
  | { type: 'SERVER_MODE_CONFIRMED'; mode: string }
  | { type: 'COMMANDS_UPDATED'; commands: string[] }
  | { type: 'AGENTS_UPDATED'; agents: string[] }
  | { type: 'TURN_USAGE'; totalInputTokens: number; contextWindowSize: number }

export function metaTransition(meta: SessionMeta | null, event: MetaEvent): SessionMeta | null {
  if (event.type === 'SESSION_INIT') {
    return {
      model: event.model,
      permissionMode: event.permissionMode,
      slashCommands: event.slashCommands,
      mcpServers: event.mcpServers,
      skills: event.skills,
      agents: event.agents,
      capabilities: event.capabilities,
      totalInputTokens: 0,
      contextWindowSize: 0,
    }
  }

  if (meta === null) return null

  switch (event.type) {
    case 'SERVER_MODE_CONFIRMED':
      return { ...meta, permissionMode: event.mode }
    case 'COMMANDS_UPDATED':
      return { ...meta, slashCommands: event.commands }
    case 'AGENTS_UPDATED':
      return { ...meta, agents: event.agents }
    case 'TURN_USAGE':
      return {
        ...meta,
        totalInputTokens: event.totalInputTokens,
        contextWindowSize: event.contextWindowSize,
      }
  }
}
