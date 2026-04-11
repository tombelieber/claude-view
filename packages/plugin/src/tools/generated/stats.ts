// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const statsGeneratedTools: ToolDef[] = [
  {
    name: 'stats_ai_generation_stats',
    description: 'AI generation statistics with time range filtering.',
    inputSchema: z.object({
    from: z.number().optional(),
    to: z.number().optional(),
    project: z.string().optional(),
    branch: z.string().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/stats/ai-generation', { params: { from: args.from, to: args.to, project: args.project, branch: args.branch } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'stats_overview',
    description: 'Aggregate usage statistics.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/stats/overview')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'stats_storage_stats',
    description: 'Storage statistics for the settings page.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/stats/storage')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'stats_get_trends',
    description: 'Get week-over-week trend metrics.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/trends')
      return JSON.stringify(result, null, 2)
    },
  }
]
