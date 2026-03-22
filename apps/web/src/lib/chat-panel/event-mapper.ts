import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import type { RawEvent } from './types'

/**
 * Maps raw WebSocket JSON messages (parsed objects) into typed RawEvent.
 * Returns null for infrastructure messages that don't need FSM handling.
 */
export function mapWsEvent(raw: Record<string, unknown>): RawEvent | null {
  const type = raw.type as string

  switch (type) {
    case 'session_init':
      return {
        type: 'SESSION_INIT',
        model: raw.model as string,
        permissionMode: raw.permissionMode as string,
        slashCommands: raw.slashCommands as string[],
        mcpServers: raw.mcpServers as { name: string; status: string }[],
        skills: raw.skills as string[],
        agents: raw.agents as string[],
        capabilities: raw.capabilities as string[],
      }

    case 'blocks_snapshot':
      return {
        type: 'BLOCKS_SNAPSHOT',
        blocks: (raw.blocks as ConversationBlock[] | undefined) ?? [],
      }

    case 'blocks_update':
      return {
        type: 'BLOCKS_UPDATE',
        blocks: (raw.blocks as ConversationBlock[] | undefined) ?? [],
      }

    case 'stream_delta':
      // Only emit STREAM_DELTA for text deltas — content_block_start/stop
      // and input_json_delta have no textDelta (undefined → "undefinedundefined..." bug)
      if (raw.textDelta != null) {
        return { type: 'STREAM_DELTA', text: raw.textDelta as string }
      }
      return null

    case 'turn_complete':
      return {
        type: 'TURN_COMPLETE',
        blocks: (raw.blocks as ConversationBlock[] | undefined) ?? [],
        totalInputTokens: (raw.totalInputTokens as number) ?? 0,
        contextWindowSize: (raw.contextWindowSize as number) ?? 0,
      }

    case 'turn_error':
      return {
        type: 'TURN_ERROR',
        blocks: (raw.blocks as ConversationBlock[] | undefined) ?? [],
        totalInputTokens: (raw.totalInputTokens as number) ?? 0,
        contextWindowSize: (raw.contextWindowSize as number) ?? 0,
      }

    case 'session_status':
      if (raw.status === 'compacting') return { type: 'SESSION_COMPACTING' }
      if (raw.status === null) return { type: 'COMPACT_DONE' }
      return null

    case 'permission_request':
      return { type: 'PERMISSION_REQUEST', kind: 'permission', requestId: raw.requestId as string }

    case 'ask_question':
      return { type: 'PERMISSION_REQUEST', kind: 'question', requestId: raw.requestId as string }

    case 'plan_approval':
      return { type: 'PERMISSION_REQUEST', kind: 'plan', requestId: raw.requestId as string }

    case 'elicitation':
      return { type: 'PERMISSION_REQUEST', kind: 'elicitation', requestId: raw.requestId as string }

    case 'session_closed':
      return { type: 'SESSION_CLOSED' }

    case 'mode_changed':
      return { type: 'SERVER_MODE_CONFIRMED', mode: raw.mode as string }

    case 'mode_rejected':
      return {
        type: 'SERVER_MODE_REJECTED',
        mode: raw.mode as string,
        reason: raw.reason as string | undefined,
      }

    case 'query_result': {
      const queryType = raw.queryType as string
      if (queryType === 'commands') {
        return { type: 'COMMANDS_UPDATED', commands: raw.data as string[] }
      }
      if (queryType === 'agents') {
        return { type: 'AGENTS_UPDATED', agents: raw.data as string[] }
      }
      return null
    }

    // Infrastructure — handled by executor, not FSM
    case 'heartbeat_config':
    case 'pong':
      return null

    case 'error':
      // replay_buffer_exhausted is handled by binary source switch, not FSM
      return null

    default:
      return null
  }
}
