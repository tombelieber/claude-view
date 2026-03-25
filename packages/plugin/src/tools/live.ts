import { z } from 'zod'
import type { ClaudeViewClient } from '../client.js'

interface ToolDef<TSchema extends z.ZodObject<z.ZodRawShape> = z.ZodObject<z.ZodRawShape>> {
  name: string
  description: string
  inputSchema: TSchema
  annotations: Record<string, boolean>
  handler: (client: ClaudeViewClient, args: z.output<TSchema>) => Promise<string>
}

export const liveTools: ToolDef[] = [
  {
    name: 'list_live_sessions',
    description:
      'List currently running Claude Code sessions with real-time agent state, model, token usage, cost, and activity.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any -- internal API, shape controlled by us
      const data: any = await client.get('/api/live/sessions')
      const sessions = (data.sessions ?? []).map((s: any) => ({
        id: s.id,
        project: s.projectDisplayName,
        agent_state: s.agentState?.label ?? s.agentState?.group,
        model: s.model,
        turn_count: s.turnCount,
        cost_usd: s.cost?.totalUsd,
        total_tokens: s.tokens?.totalTokens,
        started: s.startedAt ? new Date(s.startedAt * 1000).toISOString() : null,
        last_activity: s.lastActivityAt ? new Date(s.lastActivityAt * 1000).toISOString() : null,
        sub_agents: (s.subAgents ?? []).length || undefined,
      }))
      return JSON.stringify(
        { sessions, total: data.total, process_count: data.processCount },
        null,
        2,
      )
    },
  },
  {
    name: 'get_live_summary',
    description:
      'Get aggregate summary of all live Claude Code sessions: how many need attention, how many are autonomous, total cost today, total tokens today.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any -- internal API
      const data: any = await client.get('/api/live/summary')
      return JSON.stringify(
        {
          needs_attention: data.needsYouCount,
          autonomous: data.autonomousCount,
          total_cost_today_usd: data.totalCostTodayUsd,
          total_tokens_today: data.totalTokensToday,
          process_count: data.processCount,
        },
        null,
        2,
      )
    },
  },
]
