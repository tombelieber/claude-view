// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const projectsGeneratedTools: ToolDef[] = [
  {
    name: 'projects_list_projects',
    description: 'List all projects as lightweight summaries.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/projects')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'projects_list_project_branches',
    description: 'List distinct branches with session counts.',
    inputSchema: z.object({
    id: z.string(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/projects/${args.id}/branches`)
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'projects_list_project_sessions',
    description: 'Paginated sessions for a project.',
    inputSchema: z.object({
    id: z.string(),
    limit: z.number().optional(),
    offset: z.number().optional(),
    sort: z.string().optional(),
    branch: z.string().optional(),
    includeSidechains: z.boolean().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/projects/${args.id}/sessions`, { params: { limit: args.limit, offset: args.offset, sort: args.sort, branch: args.branch, includeSidechains: args.includeSidechains } })
      return JSON.stringify(result, null, 2)
    },
  }
]
