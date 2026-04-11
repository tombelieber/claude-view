// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const liveGeneratedTools: ToolDef[] = [
  {
    name: 'live_get_pricing',
    description: '- Return the model pricing table.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/live/pricing')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'live_get_live_session',
    description: '- Get a single live session by ID.',
    inputSchema: z.object({
    id: z.string(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/live/sessions/${encodeURIComponent(String(args.id))}`)
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'live_get_live_session_messages',
    description: '- Get the most recent messages for a live session.',
    inputSchema: z.object({
    id: z.string(),
    limit: z.number().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/live/sessions/${encodeURIComponent(String(args.id))}/messages`, { params: { limit: args.limit } })
      return JSON.stringify(result, null, 2)
    },
  }
]
