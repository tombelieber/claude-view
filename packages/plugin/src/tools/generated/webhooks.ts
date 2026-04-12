// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const webhooksGeneratedTools: ToolDef[] = [
  {
    name: 'webhooks_list_webhooks',
    description: 'list all webhooks (secrets excluded).',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/webhooks')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'webhooks_create_webhook',
    description: 'create a new webhook (returns signing secret once).',
    inputSchema: z.object({
      events: z.array(z.unknown()),
      format: z.string(),
      name: z.string(),
      url: z.string(),
    }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/webhooks', {
        body: { events: args.events, format: args.format, name: args.name, url: args.url },
      })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'webhooks_get_webhook',
    description: 'get a single webhook by ID.',
    inputSchema: z.object({
      id: z.string(),
    }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request(
        'GET',
        `/api/webhooks/${encodeURIComponent(String(args.id))}`,
      )
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'webhooks_update_webhook',
    description: 'update an existing webhook (partial update).',
    inputSchema: z.object({
      id: z.string(),
      enabled: z.boolean().optional(),
      events: z.array(z.unknown()).optional(),
      format: z.string().optional(),
      name: z.string().optional(),
      url: z.string().optional(),
    }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request(
        'PUT',
        `/api/webhooks/${encodeURIComponent(String(args.id))}`,
        {
          body: {
            enabled: args.enabled,
            events: args.events,
            format: args.format,
            name: args.name,
            url: args.url,
          },
        },
      )
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'webhooks_delete_webhook',
    description: 'remove a webhook from config and secrets.',
    inputSchema: z.object({
      id: z.string(),
    }),
    annotations: { readOnlyHint: false, destructiveHint: true, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request(
        'DELETE',
        `/api/webhooks/${encodeURIComponent(String(args.id))}`,
      )
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'webhooks_test_send',
    description: 'send a synthetic test payload.',
    inputSchema: z.object({
      id: z.string(),
    }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request(
        'POST',
        `/api/webhooks/${encodeURIComponent(String(args.id))}/test`,
      )
      return JSON.stringify(result, null, 2)
    },
  },
]
