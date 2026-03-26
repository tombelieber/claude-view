// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const monitorGeneratedTools: ToolDef[] = [
  {
    name: 'monitor_snapshot',
    description: '- One-shot JSON snapshot of current resources.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/monitor/snapshot')
      return JSON.stringify(result, null, 2)
    },
  }
]
