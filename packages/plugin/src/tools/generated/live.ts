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
    name: 'live_dismiss_all_closed',
    description: '- Dismiss all recently closed (in-memory only).',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('DELETE', '/api/live/recently-closed')
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
      const result = await client.request(
        'GET',
        `/api/live/sessions/${encodeURIComponent(String(args.id))}`,
      )
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'live_dismiss_session',
    description: '- Dismiss from recently closed (in-memory only).',
    inputSchema: z.object({
      id: z.string(),
    }),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request(
        'DELETE',
        `/api/live/sessions/${encodeURIComponent(String(args.id))}/dismiss`,
      )
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
      const result = await client.request(
        'GET',
        `/api/live/sessions/${encodeURIComponent(String(args.id))}/messages`,
        { params: { limit: args.limit } },
      )
      return JSON.stringify(result, null, 2)
    },
  },
]
