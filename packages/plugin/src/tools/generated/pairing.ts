// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const pairingGeneratedTools: ToolDef[] = [
  {
    name: 'pairing_list_devices',
    description: 'GET /pairing/devices — List paired devices.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/pairing/devices')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'pairing_unpair_device',
    description: 'DELETE /pairing/devices/:id — Unpair a device.',
    inputSchema: z.object({
    id: z.string(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('DELETE', `/api/pairing/devices/${args.id}`)
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'pairing_generate_qr',
    description: 'GET /pairing/qr — Generate QR payload for mobile pairing.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/pairing/qr')
      return JSON.stringify(result, null, 2)
    },
  }
]
