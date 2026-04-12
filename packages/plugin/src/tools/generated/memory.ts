// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const memoryGeneratedTools: ToolDef[] = [
  {
    name: 'memory_get_all_memories',
    description: 'returns all memory entries grouped by scope.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/memory')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'memory_get_memory_file',
    description: 'read a single memory file.',
    inputSchema: z.object({
      path: z.string().optional(),
    }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/memory/file', {
        params: { path: args.path },
      })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'memory_get_project_memories',
    description: 'returns memories for a specific project.',
    inputSchema: z.object({
      project: z.string(),
    }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request(
        'GET',
        `/api/memory/${encodeURIComponent(String(args.project))}`,
      )
      return JSON.stringify(result, null, 2)
    },
  },
]
