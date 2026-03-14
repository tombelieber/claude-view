import { useMemo } from 'react'
import type { SessionChannel } from '../lib/session-channel'

export interface SessionActions {
  // Existing
  sendMessage: (text: string) => void
  respondPermission: (requestId: string, allowed: boolean, updatedPermissions?: unknown[]) => void
  answerQuestion: (requestId: string, answers: Record<string, string>) => void
  approvePlan: (requestId: string, approved: boolean, feedback?: string) => void
  submitElicitation: (requestId: string, response: string) => void
  setPermissionMode: (mode: string) => void
  // Fire-and-forget
  interrupt: () => void
  setModel: (model: string) => void
  setMaxThinkingTokens: (tokens: number | null) => void
  stopTask: (taskId: string) => void
  reconnectMcp: (serverName: string) => void
  toggleMcp: (serverName: string, enabled: boolean) => void
  // Request/response
  queryModels: () => Promise<unknown>
  queryCommands: () => Promise<unknown>
  queryAgents: () => Promise<unknown>
  queryMcpStatus: () => Promise<unknown>
  queryAccountInfo: () => Promise<unknown>
  setMcpServers: (servers: Record<string, unknown>) => Promise<unknown>
  rewindFiles: (userMessageId: string, opts?: { dryRun?: boolean }) => Promise<unknown>
}

const noSession = () => Promise.reject(new Error('No session'))

export const NOOP_ACTIONS: SessionActions = {
  sendMessage: () => {},
  respondPermission: () => {},
  answerQuestion: () => {},
  approvePlan: () => {},
  submitElicitation: () => {},
  setPermissionMode: () => {},
  interrupt: () => {},
  setModel: () => {},
  setMaxThinkingTokens: () => {},
  stopTask: () => {},
  reconnectMcp: () => {},
  toggleMcp: () => {},
  queryModels: noSession,
  queryCommands: noSession,
  queryAgents: noSession,
  queryMcpStatus: noSession,
  queryAccountInfo: noSession,
  setMcpServers: noSession,
  rewindFiles: noSession,
}

export function useSessionActions(
  send: ((msg: Record<string, unknown>) => void) | null,
  channel: SessionChannel | null,
): SessionActions {
  return useMemo(() => {
    if (!send) return NOOP_ACTIONS

    return {
      sendMessage: (text: string) => {
        send({ type: 'user_message', content: text })
      },
      respondPermission: (requestId: string, allowed: boolean, updatedPermissions?: unknown[]) => {
        send({ type: 'permission_response', requestId, allowed, updatedPermissions })
      },
      answerQuestion: (requestId: string, answers: Record<string, string>) => {
        send({ type: 'question_response', requestId, answers })
      },
      approvePlan: (requestId: string, approved: boolean, feedback?: string) => {
        send({ type: 'plan_response', requestId, approved, feedback })
      },
      submitElicitation: (requestId: string, response: string) => {
        send({ type: 'elicitation_response', requestId, response })
      },
      setPermissionMode: (mode: string) => {
        send({ type: 'set_mode', mode })
      },
      // Fire-and-forget
      interrupt: () => send({ type: 'interrupt' }),
      setModel: (model: string) => send({ type: 'set_model', model }),
      setMaxThinkingTokens: (tokens: number | null) =>
        send({ type: 'set_max_thinking_tokens', maxThinkingTokens: tokens }),
      stopTask: (taskId: string) => send({ type: 'stop_task', taskId }),
      reconnectMcp: (serverName: string) => send({ type: 'reconnect_mcp', serverName }),
      toggleMcp: (serverName: string, enabled: boolean) =>
        send({ type: 'toggle_mcp', serverName, enabled }),
      // Request/response
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
  }, [send, channel])
}
