// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const insightsGeneratedTools: ToolDef[] = [
  {
    name: 'insights_get_insights',
    description: 'Compute and return behavioral insights.',
    inputSchema: z.object({
    from: z.number().optional(),
    to: z.number().optional(),
    min_impact: z.number().optional(),
    categories: z.string().optional(),
    limit: z.number().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/insights', { params: { from: args.from, to: args.to, min_impact: args.min_impact, categories: args.categories, limit: args.limit } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'insights_get_benchmarks',
    description: 'Compute personal progress benchmarks.',
    inputSchema: z.object({
    range: z.string().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/insights/benchmarks', { params: { range: args.range } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'insights_get_categories',
    description: 'Returns hierarchical category data.',
    inputSchema: z.object({
    from: z.number().optional(),
    to: z.number().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/insights/categories', { params: { from: args.from, to: args.to } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'insights_get_insights_trends',
    description: 'Get time-series trend data for charts.',
    inputSchema: z.object({
    metric: z.string().optional(),
    range: z.string().optional(),
    granularity: z.string().optional(),
    from: z.number().optional(),
    to: z.number().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/insights/trends', { params: { metric: args.metric, range: args.range, granularity: args.granularity, from: args.from, to: args.to } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'insights_list_invocables',
    description: 'List all invocables with their usage counts.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/invocables')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'insights_get_fluency_score',
    description: 'Get the current AI Fluency Score.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/score')
      return JSON.stringify(result, null, 2)
    },
  }
]
