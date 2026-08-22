// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const devicesGeneratedTools: ToolDef[] = [
  {
    name: 'devices_list_devices_handler',
    description: 'List Devices Handler',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/devices')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'devices_terminate_others_handler',
    description: 'Terminate Others Handler (POST /api/devices/terminate-others)',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('POST', '/api/devices/terminate-others')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'devices_delete_device_handler',
    description: 'Delete Device Handler',
    inputSchema: z.object({
      device_id: z.string(),
    }),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request(
        'DELETE',
        `/api/devices/${encodeURIComponent(String(args.device_id))}`,
      )
      return JSON.stringify(result, null, 2)
    },
  },
]
