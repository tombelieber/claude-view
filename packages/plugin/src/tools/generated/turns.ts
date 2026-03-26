// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const turnsGeneratedTools: ToolDef[] = [
  {
    name: 'turns_get_session_turns',
    description: 'Per-turn breakdown for a historical session.',
    inputSchema: z.object({
    id: z.string(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/sessions/${args.id}/turns`)
      return JSON.stringify(result, null, 2)
    },
  }
]
