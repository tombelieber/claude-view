import { useMemo } from 'react'
import type { SessionChannel } from '../lib/session-channel'

export interface SessionActions {
  // Message — can trigger session resume on dormant sessions
  sendMessage: (text: string) => void
  // Interactive responses — only meaningful during live session
  respondPermission: (requestId: string, allowed: boolean, updatedPermissions?: unknown[]) => void
  answerQuestion: (requestId: string, answers: Record<string, string>) => void
  approvePlan: (requestId: string, approved: boolean, feedback?: string) => void
  submitElicitation: (requestId: string, response: string) => void
  // Control commands — live-only, never trigger resume
  setPermissionMode: (mode: string) => void
  interrupt: () => void
  setModel: (model: string) => void
  setMaxThinkingTokens: (tokens: number | null) => void
  stopTask: (taskId: string) => void
  reconnectMcp: (serverName: string) => void
  toggleMcp: (serverName: string, enabled: boolean) => void
  // Request/response — already live-only via channel
  queryModels: () => Promise<unknown>
  queryCommands: () => Promise<unknown>
  queryAgents: () => Promise<unknown>
  queryMcpStatus: () => Promise<unknown>
  queryAccountInfo: () => Promise<unknown>
  setMcpServers: (servers: Record<string, unknown>) => Promise<unknown>
  rewindFiles: (userMessageId: string, opts?: { dryRun?: boolean }) => Promise<unknown>
}

const noSession = () => Promise.reject(new Error('No session'))
const noop = () => {}

export const NOOP_ACTIONS: SessionActions = {
  sendMessage: noop,
  respondPermission: noop,
  answerQuestion: noop,
  approvePlan: noop,
  submitElicitation: noop,
  setPermissionMode: noop,
  interrupt: noop,
  setModel: noop,
  setMaxThinkingTokens: noop,
  stopTask: noop,
  reconnectMcp: noop,
  toggleMcp: noop,
  queryModels: noSession,
  queryCommands: noSession,
  queryAgents: noSession,
  queryMcpStatus: noSession,
  queryAccountInfo: noSession,
  setMcpServers: noSession,
  rewindFiles: noSession,
}

/**
 * Build session actions from two send pipes:
 * - `send`: May trigger session resume (for user_message only)
 * - `sendIfLive`: Live-only, never triggers resume (for all control commands)
 */
export function useSessionActions(
  send: ((msg: Record<string, unknown>) => void) | null,
  sendIfLive: ((msg: Record<string, unknown>) => void) | null,
  channel: SessionChannel | null,
): SessionActions {
  return useMemo(() => {
    if (!send) return NOOP_ACTIONS

    // Control commands: use sendIfLive when WS is open, otherwise silent no-op.
    // NEVER use `send` for control commands — it triggers session resume on dormant sessions,
    // which replays events, corrupts the display, and spikes context to 100%.
    const ctrl = sendIfLive ?? noop

    return {
      sendMessage: (text: string) => {
        send({ type: 'user_message', content: text })
      },
      // Interactive responses — session is live when these fire
      respondPermission: (requestId: string, allowed: boolean, updatedPermissions?: unknown[]) => {
        ctrl({ type: 'permission_response', requestId, allowed, updatedPermissions })
      },
      answerQuestion: (requestId: string, answers: Record<string, string>) => {
        ctrl({ type: 'question_response', requestId, answers })
      },
      approvePlan: (requestId: string, approved: boolean, feedback?: string) => {
        ctrl({ type: 'plan_response', requestId, approved, feedback })
      },
      submitElicitation: (requestId: string, response: string) => {
        ctrl({ type: 'elicitation_response', requestId, response })
      },
      // Control commands — live-only
      setPermissionMode: (mode: string) => ctrl({ type: 'set_mode', mode }),
      interrupt: () => ctrl({ type: 'interrupt' }),
      setModel: (model: string) => ctrl({ type: 'set_model', model }),
      setMaxThinkingTokens: (tokens: number | null) =>
        ctrl({ type: 'set_max_thinking_tokens', maxThinkingTokens: tokens }),
      stopTask: (taskId: string) => ctrl({ type: 'stop_task', taskId }),
      reconnectMcp: (serverName: string) => ctrl({ type: 'reconnect_mcp', serverName }),
      toggleMcp: (serverName: string, enabled: boolean) =>
        ctrl({ type: 'toggle_mcp', serverName, enabled }),
      // Request/response — already live-only via channel
      queryModels: () =>
        channel?.request({ type: 'query_models' }) ?? Promise.reject(new Error('No session')),
      queryCommands: () =>
        channel?.request({ type: 'query_commands' }) ?? Promise.reject(new Error('No session')),
      queryAgents: () =>
        channel?.request({ type: 'query_agents' }) ?? Promise.reject(new Error('No session')),
      queryMcpStatus: () =>
        channel?.request({ type: 'query_mcp_status' }) ?? Promise.reject(new Error('No session')),
      queryAccountInfo: () =>
        channel?.request({ type: 'query_account_info' }) ?? Promise.reject(new Error('No session')),
      setMcpServers: (servers: Record<string, unknown>) =>
        channel?.request({ type: 'set_mcp_servers', servers }) ??
        Promise.reject(new Error('No session')),
      rewindFiles: (userMessageId: string, opts?: { dryRun?: boolean }) =>
        channel?.request({ type: 'rewind_files', userMessageId, dryRun: opts?.dryRun }) ??
        Promise.reject(new Error('No session')),
    }
  }, [send, sendIfLive, channel])
}
