// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const exportGeneratedTools: ToolDef[] = [
  {
    name: 'export_sessions',
    description: 'Export all sessions.',
    inputSchema: z.object({
    format: z.string().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/export/sessions', { params: { format: args.format } })
      return JSON.stringify(result, null, 2)
    },
  }
]
