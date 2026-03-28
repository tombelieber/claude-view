import { z } from 'zod'
import type { ToolDef } from './types.js'

const getStatsSchema = z.object({
  project: z.string().optional().describe('Filter by project name'),
  branch: z.string().optional().describe('Filter by git branch'),
  from: z
    .union([z.number(), z.string()])
    .optional()
    .describe('Start time (Unix seconds, or ISO 8601 / YYYY-MM-DD date string)'),
  to: z
    .union([z.number(), z.string()])
    .optional()
    .describe('End time (Unix seconds, or ISO 8601 / YYYY-MM-DD date string)'),
})

function parseUnixSeconds(
  value: string | number | undefined,
  field: 'from' | 'to',
): number | undefined {
  if (value === undefined) {
    return undefined
  }

  if (typeof value === 'number') {
    if (!Number.isFinite(value)) {
      throw new Error(`Invalid ${field} value: must be a finite number`)
    }
    return Math.trunc(value)
  }

  const trimmed = value.trim()
  if (trimmed === '') {
    return undefined
  }

  const numeric = Number(trimmed)
  if (Number.isFinite(numeric) && /^\d+$/.test(trimmed)) {
    return Math.trunc(numeric)
  }

  const parsedMs = Date.parse(trimmed)
  if (!Number.isNaN(parsedMs)) {
    return Math.trunc(parsedMs / 1000)
  }

  throw new Error(`Invalid ${field} value: expected Unix seconds or parseable date string`)
}

export const statsTools: ToolDef[] = [
  {
    name: 'get_stats',
    description:
      'Get dashboard overview statistics: total sessions, projects, top skills, tool usage totals, current week metrics, and week-over-week trends. Optionally filter by project, branch, or date range.',
    inputSchema: getStatsSchema,
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const from = parseUnixSeconds(args.from, 'from')
      const to = parseUnixSeconds(args.to, 'to')
      const data = await client.get<any>('/api/stats/dashboard', {
        project: args.project,
        branch: args.branch,
        from,
        to,
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
          ranges: data.meta?.ranges,
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
