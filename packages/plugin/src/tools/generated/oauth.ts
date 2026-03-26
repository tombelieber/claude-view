// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const oauthGeneratedTools: ToolDef[] = [
  {
    name: 'oauth_get_auth_identity',
    description: 'GET /api/oauth/identity',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/oauth/identity')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'oauth_get_oauth_usage',
    description: 'GET /api/oauth/usage',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/oauth/usage')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'oauth_post_oauth_usage_refresh',
    description: 'POST /api/oauth/usage/refresh',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/oauth/usage/refresh')
      return JSON.stringify(result, null, 2)
    },
  }
]
