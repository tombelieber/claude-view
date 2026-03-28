// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const pluginsGeneratedTools: ToolDef[] = [
  {
    name: 'plugins_list_plugins',
    description: 'Unified view of installed + available plugins.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/plugins')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'plugins_list_marketplaces',
    description: 'List Marketplaces (GET /api/plugins/marketplaces)',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/plugins/marketplaces')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'plugins_refresh_all',
    description: 'Refresh All (POST /api/plugins/marketplaces/refresh-all)',
    inputSchema: z.object({
    names: z.array(z.unknown()).optional(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/plugins/marketplaces/refresh-all', { body: { names: args.names } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'plugins_refresh_status',
    description: 'Refresh Status (GET /api/plugins/marketplaces/refresh-status)',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/plugins/marketplaces/refresh-status')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'plugins_list_ops_handler',
    description: 'List all queued/running/completed ops.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/plugins/ops')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'plugins_enqueue_op',
    description: 'Enqueue a plugin mutation, return immediately.',
    inputSchema: z.object({
    action: z.string().describe('"install" | "update" | "uninstall" | "enable" | "disable"'),
    name: z.string().describe('Plugin name or full ID (e.g. "superpowers" or "superpowers@marketplace")'),
    projectPath: z.string().describe('For project-scoped plugins: the project directory where it was installed. Required for uninstall of project-scoped plugins (CLI needs correct CWD).').optional(),
    scope: z.string().describe('"user" | "project"').optional(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/plugins/ops', { body: { action: args.action, name: args.name, projectPath: args.projectPath, scope: args.scope } })
      return JSON.stringify(result, null, 2)
    },
  }
]
