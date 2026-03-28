// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const contributionsGeneratedTools: ToolDef[] = [
  {
    name: 'contributions_get_contributions',
    description: 'Main contributions page data.',
    inputSchema: z.object({
    range: z.string().optional(),
    from: z.string().optional(),
    to: z.string().optional(),
    projectId: z.string().optional(),
    branch: z.string().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/contributions', { params: { range: args.range, from: args.from, to: args.to, projectId: args.projectId, branch: args.branch } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'contributions_get_branch_sessions',
    description: 'Sessions for a branch.',
    inputSchema: z.object({
    name: z.string(),
    range: z.string().optional(),
    from: z.string().optional(),
    to: z.string().optional(),
    projectId: z.string().optional(),
    limit: z.number().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/contributions/branches/${encodeURIComponent(String(args.name))}/sessions`, { params: { range: args.range, from: args.from, to: args.to, projectId: args.projectId, limit: args.limit } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'contributions_get_session_contribution',
    description: 'Session contribution detail.',
    inputSchema: z.object({
    id: z.string(),
    range: z.string(),
    from: z.string(),
    to: z.string(),
    projectId: z.string(),
    branch: z.string(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/contributions/sessions/${encodeURIComponent(String(args.id))}`)
      return JSON.stringify(result, null, 2)
    },
  }
]
