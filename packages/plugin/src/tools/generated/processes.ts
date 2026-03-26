// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const processesGeneratedTools: ToolDef[] = [
  {
    name: 'processes_cleanup_processes',
    description: 'Cleanup Processes',
    inputSchema: z.object({
    targets: z.array(z.unknown()),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/processes/cleanup', { body: { targets: args.targets } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'processes_kill_process',
    description: 'Kill Process',
    inputSchema: z.object({
    pid: z.number(),
    force: z.boolean(),
    start_time: z.number(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', `/api/processes/${args.pid}/kill`, { body: { force: args.force, start_time: args.start_time } })
      return JSON.stringify(result, null, 2)
    },
  }
]
