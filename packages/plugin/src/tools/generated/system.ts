// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const systemGeneratedTools: ToolDef[] = [
  {
    name: 'system_check_path',
    description: 'Check whether a filesystem path still exists.',
    inputSchema: z.object({
    path: z.string().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/check-path', { params: { path: args.path } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'system_get_system_status',
    description: 'Get comprehensive system status.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/system')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'system_clear_cache',
    description: 'Clear search index and cached data.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/system/clear-cache')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'system_trigger_git_resync',
    description: 'Trigger full git re-sync.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/system/git-resync')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'system_trigger_reindex',
    description: 'Trigger a full re-index.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/system/reindex')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'system_reset_all',
    description: 'Factory reset all data.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/system/reset')
      return JSON.stringify(result, null, 2)
    },
  }
]
