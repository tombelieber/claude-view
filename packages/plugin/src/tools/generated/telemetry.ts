// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const telemetryGeneratedTools: ToolDef[] = [
  {
    name: 'telemetry_set_consent',
    description: 'Set telemetry consent preference.',
    inputSchema: z.object({
    enabled: z.boolean(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/telemetry/consent', { body: { enabled: args.enabled } })
      return JSON.stringify(result, null, 2)
    },
  }
]
