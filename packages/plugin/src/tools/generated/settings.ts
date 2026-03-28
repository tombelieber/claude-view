// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const settingsGeneratedTools: ToolDef[] = [
  {
    name: 'settings_get_settings',
    description: 'Read current app settings.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/settings')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'settings_update_settings',
    description: 'Update app settings (partial).',
    inputSchema: z.object({
    llmModel: z.string().optional(),
    llmTimeoutSecs: z.number().optional(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('PUT', '/api/settings', { body: { llmModel: args.llmModel, llmTimeoutSecs: args.llmTimeoutSecs } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'settings_update_git_sync_interval',
    description: 'Update the git sync interval.',
    inputSchema: z.object({
    intervalSecs: z.number().describe('Interval in seconds. Must be between 10 and 3600.'),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('PUT', '/api/settings/git-sync-interval', { body: { intervalSecs: args.intervalSecs } })
      return JSON.stringify(result, null, 2)
    },
  }
]
