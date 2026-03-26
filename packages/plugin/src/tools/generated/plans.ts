// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const plansGeneratedTools: ToolDef[] = [
  {
    name: 'plans_get_session_plans',
    description: '- returns plan documents for the session\'s slug.',
    inputSchema: z.object({
    id: z.string(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/sessions/${args.id}/plans`)
      return JSON.stringify(result, null, 2)
    },
  }
]
