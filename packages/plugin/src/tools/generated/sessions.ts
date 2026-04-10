// AUTO-GENERATED — DO NOT EDIT
// Generated from openapi.json by scripts/codegen-from-openapi.ts

import { z } from 'zod'
import type { ToolDef } from '../types.js'

export const sessionsGeneratedTools: ToolDef[] = [
  {
    name: 'sessions_list_branches',
    description: 'Get distinct list of branch names across all sessions.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, _args) => {
      const result = await client.request('GET', '/api/branches')
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'sessions_estimate_cost',
    description: 'cost estimation (Rust-only, no sidecar).',
    inputSchema: z.object({
    model: z.string().optional(),
    session_id: z.string(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/estimate', { body: { model: args.model, session_id: args.session_id } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'sessions_session_activity',
    description: 'Activity histogram for sparkline chart.',
    inputSchema: z.object({
    time_after: z.number().optional(),
    time_before: z.number().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', '/api/sessions/activity', { params: { time_after: args.time_after, time_before: args.time_before } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'sessions_bulk_archive_handler',
    description: 'Bulk Archive Handler',
    inputSchema: z.object({
    ids: z.array(z.unknown()),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/sessions/archive', { body: { ids: args.ids } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'sessions_bulk_unarchive_handler',
    description: 'Bulk Unarchive Handler',
    inputSchema: z.object({
    ids: z.array(z.unknown()),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', '/api/sessions/unarchive', { body: { ids: args.ids } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'sessions_archive_session_handler',
    description: 'Archive Session Handler',
    inputSchema: z.object({
    id: z.string(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', `/api/sessions/${encodeURIComponent(String(args.id))}/archive`)
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'sessions_get_file_history',
    description: 'List all file changes for a session.',
    inputSchema: z.object({
    id: z.string(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/sessions/${encodeURIComponent(String(args.id))}/file-history`)
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'sessions_get_file_diff',
    description: 'Get File Diff (GET /api/sessions/file-history/diff)',
    inputSchema: z.object({
    id: z.string(),
    file_hash: z.string(),
    from: z.number().optional(),
    to: z.number().optional(),
    file_path: z.string().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/sessions/${encodeURIComponent(String(args.id))}/file-history/${encodeURIComponent(String(args.file_hash))}/diff`, { params: { from: args.from, to: args.to, file_path: args.file_path } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'sessions_get_session_hook_events',
    description: 'Fetch hook events for a session.',
    inputSchema: z.object({
    id: z.string(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/sessions/${encodeURIComponent(String(args.id))}/hook-events`)
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'sessions_get_session_messages_by_id',
    description: 'Get paginated messages by session ID.',
    inputSchema: z.object({
    id: z.string(),
    limit: z.number().optional(),
    offset: z.number().optional(),
    raw: z.boolean().optional(),
    format: z.string().optional(),
  }),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('GET', `/api/sessions/${encodeURIComponent(String(args.id))}/messages`, { params: { limit: args.limit, offset: args.offset, raw: args.raw, format: args.format } })
      return JSON.stringify(result, null, 2)
    },
  },
  {
    name: 'sessions_unarchive_session_handler',
    description: 'Unarchive Session Handler',
    inputSchema: z.object({
    id: z.string(),
  }),
    annotations: { readOnlyHint: false, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const result = await client.request('POST', `/api/sessions/${encodeURIComponent(String(args.id))}/unarchive`)
      return JSON.stringify(result, null, 2)
    },
  }
]
