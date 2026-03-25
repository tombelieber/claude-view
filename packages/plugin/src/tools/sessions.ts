import { z } from 'zod'
import type { ClaudeViewClient } from '../client.js'

interface ToolDef<TSchema extends z.ZodObject<any> = z.ZodObject<any>> {
  name: string
  description: string
  inputSchema: TSchema
  annotations: Record<string, boolean>
  handler: (client: ClaudeViewClient, args: z.output<TSchema>) => Promise<string>
}

const listSessionsSchema = z.object({
  limit: z.number().optional().describe('Max sessions to return (default 30)'),
  q: z.string().optional().describe('Text search query'),
  filter: z.string().optional().describe('Filter: all, has_commits, high_reedit, long_session'),
  sort: z.string().optional().describe('Sort: recent, tokens, prompts, files_edited, duration'),
  offset: z.number().optional().describe('Pagination offset'),
  branches: z.string().optional().describe('Comma-separated branch names'),
  models: z.string().optional().describe('Comma-separated model names'),
  time_after: z.number().optional().describe('Unix timestamp lower bound'),
  time_before: z.number().optional().describe('Unix timestamp upper bound'),
})

const getSessionSchema = z.object({
  session_id: z.string().describe('The session ID to look up'),
})

const searchSessionsSchema = z.object({
  query: z.string().describe('Search query'),
  limit: z.number().optional().describe('Max results (default 10)'),
  offset: z.number().optional().describe('Pagination offset'),
  scope: z.string().optional().describe('Search scope: all, user, assistant'),
})

export const sessionTools: ToolDef[] = [
  {
    name: 'list_sessions',
    description:
      'List Claude Code sessions with optional filters. Returns session summaries including project, model, duration, and token usage.',
    inputSchema: listSessionsSchema,
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const data = await client.get<any>('/api/sessions', {
        limit: args.limit,
        q: args.q,
        filter: args.filter,
        sort: args.sort,
        offset: args.offset,
        branches: args.branches,
        models: args.models,
        time_after: args.time_after,
        time_before: args.time_before,
      })
      const sessions = (data.sessions ?? []).map((s: any) => ({
        id: s.id,
        project: s.displayName || s.project,
        branch: s.gitBranch,
        model: s.primaryModel,
        turns: s.turnCount,
        messages: s.messageCount,
        commits: s.commitCount,
        duration_min: Math.round((s.durationSeconds ?? 0) / 60),
        input_tokens: s.totalInputTokens,
        output_tokens: s.totalOutputTokens,
        modified: s.modifiedAt ? new Date(s.modifiedAt * 1000).toISOString() : null,
      }))
      return JSON.stringify({ sessions, total: data.total, has_more: data.hasMore }, null, 2)
    },
  },
  {
    name: 'get_session',
    description:
      'Get detailed information about a specific Claude Code session, including commits, token breakdown, and derived metrics.',
    inputSchema: getSessionSchema,
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const data = await client.get<any>(`/api/sessions/${args.session_id}`)
      return JSON.stringify(
        {
          id: data.id,
          project: data.displayName || data.project,
          branch: data.gitBranch,
          model: data.primaryModel,
          summary: data.preview,
          turns: data.turnCount,
          messages: data.userPromptCount,
          commits: data.commits?.length ?? data.commitCount,
          duration_min: Math.round((data.durationSeconds ?? 0) / 60),
          input_tokens: data.totalInputTokens,
          output_tokens: data.totalOutputTokens,
          cache_read_tokens: data.totalCacheReadTokens,
          derived_metrics: data.derivedMetrics,
          recent_commits: (data.commits ?? []).slice(0, 10).map((c: any) => ({
            hash: c.hash?.slice(0, 8),
            message: c.message,
            branch: c.branch,
          })),
        },
        null,
        2,
      )
    },
  },
  {
    name: 'search_sessions',
    description:
      'Search across all Claude Code sessions using unified enhanced search. Returns matching sessions with highlighted snippets.',
    inputSchema: searchSessionsSchema,
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const data = await client.get<any>('/api/search', {
        q: args.query,
        limit: args.limit,
        offset: args.offset,
        scope: args.scope,
      })
      return JSON.stringify(
        {
          query: data.query,
          total_sessions: data.totalSessions,
          total_matches: data.totalMatches,
          elapsed_ms: data.elapsedMs,
          results: (data.sessions ?? []).map((s: any) => ({
            session_id: s.sessionId,
            project: s.project,
            branch: s.branch,
            match_count: s.matchCount,
            best_score: s.bestScore,
            top_matches: (s.matches ?? []).slice(0, 3).map((m: any) => ({
              role: m.role,
              snippet: m.snippet?.replace(/<\/?mark>/g, '**'),
              turn: m.turnNumber,
            })),
          })),
        },
        null,
        2,
      )
    },
  },
]
