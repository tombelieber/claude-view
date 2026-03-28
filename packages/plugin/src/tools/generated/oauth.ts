// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const oauthGeneratedTools: ToolDef[] = [
  {
    name: 'oauth_get_auth_identity',
    description: 'Get Auth Identity',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/oauth/identity')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'oauth_get_oauth_usage',
    description: 'Get Oauth Usage',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/oauth/usage')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'oauth_post_oauth_usage_refresh',
    description: 'Post Oauth Usage Refresh',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/oauth/usage/refresh')
      return JSON.stringify(result, null, 2)
    },
  }
]
