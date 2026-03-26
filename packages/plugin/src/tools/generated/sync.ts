// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const syncGeneratedTools: ToolDef[] = [
  {
    name: 'sync_indexing_status',
    description: 'lightweight JSON snapshot of indexing progress.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/indexing/status')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'sync_trigger_deep_index',
    description: 'Trigger a full deep index rebuild.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/sync/deep-index')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'sync_trigger_git_sync',
    description: 'Trigger git commit scanning (A8.5).',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/sync/git')
      return JSON.stringify(result, null, 2)
    },
  }
]
