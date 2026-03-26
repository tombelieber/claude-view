// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const coachingGeneratedTools: ToolDef[] = [
  {
    name: 'coaching_list_rules',
    description: 'List all coaching rules from the rules directory.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/coaching/rules')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'coaching_apply_rule',
    description: 'Create a new coaching rule file.',
    inputSchema: z.object({
    impactScore: z.number(),
    patternId: z.string(),
    recommendation: z.string(),
    sampleSize: z.number(),
    scope: z.string(),
    title: z.string(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/coaching/rules', { body: { impactScore: args.impactScore, patternId: args.patternId, recommendation: args.recommendation, sampleSize: args.sampleSize, scope: args.scope, title: args.title } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'coaching_remove_rule',
    description: 'Remove a coaching rule file.',
    inputSchema: z.object({
    id: z.string(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('DELETE', `/api/coaching/rules/${encodeURIComponent(String(args.id))}`)
      return JSON.stringify(result, null, 2)
    },
  }
]
