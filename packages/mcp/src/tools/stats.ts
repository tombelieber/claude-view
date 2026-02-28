import { z } from 'zod'
import type { ClaudeViewClient } from '../client.js'

interface ToolDef<TSchema extends z.ZodObject<any> = z.ZodObject<any>> {
  name: string
  description: string
  inputSchema: TSchema
  annotations: Record<string, boolean>
  handler: (client: ClaudeViewClient, args: z.output<TSchema>) => Promise<string>
}

const getStatsSchema = z.object({
  project: z.string().optional().describe('Filter by project name'),
  branch: z.string().optional().describe('Filter by git branch'),
  from: z.string().optional().describe('Start date (ISO 8601 or YYYY-MM-DD)'),
  to: z.string().optional().describe('End date (ISO 8601 or YYYY-MM-DD)'),
})

export const statsTools: ToolDef[] = [
  {
    name: 'get_stats',
    description:
      'Get dashboard overview statistics: total sessions, projects, top skills, tool usage totals, current week metrics, and week-over-week trends. Optionally filter by project, branch, or date range.',
    inputSchema: getStatsSchema,
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const data = await client.get<any>('/api/stats/dashboard', {
        project: args.project,
        branch: args.branch,
        from: args.from,
        to: args.to,
      })
      return JSON.stringify(
        {
          total_sessions: data.totalSessions,
          total_projects: data.totalProjects,
          top_projects: (data.topProjects ?? []).slice(0, 5).map((p: any) => ({
            name: p.displayName || p.name,
            sessions: p.sessionCount,
          })),
          top_skills: (data.topSkills ?? []).slice(0, 5),
          tool_totals: data.toolTotals,
          current_week: data.currentWeek,
          trends: data.trends,
        },
        null,
        2,
      )
    },
  },
  {
    name: 'get_fluency_score',
    description:
      'Get the AI Fluency Score (0-100 composite) measuring coding effectiveness with Claude. Includes achievement rate, friction rate, cost efficiency, satisfaction trend, consistency.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client) => {
      const data = await client.get<any>('/api/score')
      return JSON.stringify(
        {
          score: data.score,
          achievementRate: data.achievementRate,
          frictionRate: data.frictionRate,
          costEfficiency: data.costEfficiency,
          satisfactionTrend: data.satisfactionTrend,
          consistency: data.consistency,
          sessionsAnalyzed: data.sessionsAnalyzed,
        },
        null,
        2,
      )
    },
  },
  {
    name: 'get_token_stats',
    description:
      "Get token usage statistics: total input/output/cache tokens, cache hit ratio, session and turn counts. Note: no USD cost fields — use live summary for today's cost.",
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client) => {
      const data = await client.get<any>('/api/stats/tokens')
      return JSON.stringify(data, null, 2)
    },
  },
]
