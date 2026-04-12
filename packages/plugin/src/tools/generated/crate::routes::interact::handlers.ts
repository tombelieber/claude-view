// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const crateroutesinteracthandlersGeneratedTools: ToolDef[] = [
  {
    name: 'crate_routes_interact_handlers_interact_handler',
    description: 'Resolve a pending interaction (permission, question, plan, elicitation).',
    inputSchema: z.object({
      session_id: z.string(),
    }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request(
        'POST',
        `/api/sessions/${encodeURIComponent(String(args.session_id))}/interact`,
      )
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'crate_routes_interact_handlers_get_interaction_handler',
    description: "Fetch the full interaction data for a session's pending interaction.",
    inputSchema: z.object({
      session_id: z.string(),
    }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request(
        'GET',
        `/api/sessions/${encodeURIComponent(String(args.session_id))}/interaction`,
      )
      return JSON.stringify(result, null, 2)
    },
  },
]
