// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const searchGeneratedTools: ToolDef[] = [
  {
    name: 'search_handler',
    description: 'Full-text search across sessions.',
    inputSchema: z.object({
    q: z.string().optional(),
    limit: z.number().optional(),
    offset: z.number().optional(),
    project: z.string().optional(),
    branch: z.string().optional(),
    model: z.string().optional(),
    after: z.string().optional(),
    before: z.string().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/search', { params: { q: args.q, limit: args.limit, offset: args.offset, project: args.project, branch: args.branch, model: args.model, after: args.after, before: args.before } })
      return JSON.stringify(result, null, 2)
    },
  }
]
