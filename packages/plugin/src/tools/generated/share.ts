// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const shareGeneratedTools: ToolDef[] = [
  {
    name: 'share_create_share',
    description: 'POST /api/sessions/{session_id}/share',
    inputSchema: z.object({
    session_id: z.string(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', `/api/sessions/${args.session_id}/share`)
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'share_revoke_share',
    description: 'DELETE /api/sessions/{session_id}/share',
    inputSchema: z.object({
    session_id: z.string(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('DELETE', `/api/sessions/${args.session_id}/share`)
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'share_list_shares',
    description: 'GET /api/shares',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/shares')
      return JSON.stringify(result, null, 2)
    },
  }
]
