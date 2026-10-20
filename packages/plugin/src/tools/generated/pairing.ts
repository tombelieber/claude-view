// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const pairingGeneratedTools: ToolDef[] = [
  {
    name: 'pairing_generate_qr',
    description: 'Generate a QR payload via Supabase pair-offer.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/pairing/qr')
      return JSON.stringify(result, null, 2)
    },
  },
]
