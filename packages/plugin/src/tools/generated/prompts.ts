// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const promptsGeneratedTools: ToolDef[] = [
  {
    name: 'prompts_list_prompts',
    description: 'List prompt history with optional search/filter.',
    inputSchema: z.object({
    q: z.string().optional(),
    project: z.string().optional(),
    intent: z.string().optional(),
    complexity: z.string().optional(),
    has_paste: z.string().optional(),
    sort: z.string().optional(),
    time_after: z.number().optional(),
    time_before: z.number().optional(),
    template_match: z.string().optional(),
    limit: z.number().optional(),
    offset: z.number().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/prompts', { params: { q: args.q, project: args.project, intent: args.intent, complexity: args.complexity, has_paste: args.has_paste, sort: args.sort, time_after: args.time_after, time_before: args.time_before, template_match: args.template_match, limit: args.limit, offset: args.offset } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'prompts_get_prompt_stats',
    description: 'Aggregate prompt statistics.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/prompts/stats')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'prompts_get_prompt_templates',
    description: 'Detected prompt templates.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/prompts/templates')
      return JSON.stringify(result, null, 2)
    },
  }
]
