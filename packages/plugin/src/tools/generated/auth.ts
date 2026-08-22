// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const authGeneratedTools: ToolDef[] = [
  {
    name: 'auth_post_session',
    description: 'Post Session (POST /api/auth/session)',
    inputSchema: z.object({
      access_token: z.string().describe('Supabase JWT (short-lived access token).').optional(),
      email: z.string().optional(),
      expires_in: z
        .number()
        .describe('Seconds until `access_token` expires (Supabase returns `expires_in`).')
        .optional(),
      refresh_token: z.string().describe('Supabase opaque refresh token (long-lived).').optional(),
      user_id: z
        .string()
        .describe(
          "UUID from the JWT's `sub` claim. Web UI reads this from supabase.auth.getUser() and forwards it so we don't double-parse.",
        )
        .optional(),
    }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/auth/session', {
        body: {
          access_token: args.access_token,
          email: args.email,
          expires_in: args.expires_in,
          refresh_token: args.refresh_token,
          user_id: args.user_id,
        },
      })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'auth_delete_session',
    description: 'Delete Session (DELETE /api/auth/session)',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('DELETE', '/api/auth/session')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'auth_get_status',
    description: 'Get Status (GET /api/auth/status)',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/auth/status')
      return JSON.stringify(result, null, 2)
    },
  },
]
