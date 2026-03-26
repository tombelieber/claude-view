// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const healthGeneratedTools: ToolDef[] = [
  {
    name: 'health_config',
    description: 'Get config',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/config')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'health_check',
    description: 'Health check endpoint.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/health')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'health_get_status',
    description: 'Get index metadata and data freshness info.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/status')
      return JSON.stringify(result, null, 2)
    },
  }
]
