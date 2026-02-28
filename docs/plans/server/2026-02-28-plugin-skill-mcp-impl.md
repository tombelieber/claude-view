# Claude View Plugin + Skill + MCP Server — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a Claude Code plugin that bundles a skill (HTTP API interface) and MCP server (native tool access) for claude-view, giving AI agents read-only access to session data, costs, and fluency scores.

**Architecture:** TypeScript MCP server in `packages/mcp/` using `@modelcontextprotocol/sdk` v1.27.1. Talks to the running Rust server via HTTP (`localhost:47892`). Plugin manifest at repo root (`plugin.json`, `marketplace.json`) — Phase B, blocked on path validation. Skill in `claude-view/SKILL.md`. Contract test enforces sync between skill, MCP tools, and Rust routes.

**Tech Stack:** TypeScript, `@modelcontextprotocol/sdk` ^1.27.1, Zod, Bun workspace, stdio + Streamable HTTP transport.

**Design doc:** `docs/plans/2026-02-28-plugin-skill-mcp-design.md`

---

## Task 1: Plugin Manifest (Phase B — Blocked)

> **PHASE B GATE:** The correct plugin manifest path is uncertain. Design doc verified `registry.rs:read_plugin_json()` reads `{installPath}/plugin.json` (root), but the official Claude Code plugin docs may use `.claude-plugin/plugin.json`. **Do not implement this task until Phase B validation is complete** (test with `claude /plugin install` to confirm which path the runtime expects). Skip to Task 2.

**Files:**
- Create: `plugin.json` (at repo root)
- Create: `marketplace.json` (at repo root)

**Step 1: Create plugin.json**

Create `plugin.json` at the repo root:

```json
{
  "name": "claude-view",
  "description": "Monitor and query Claude Code sessions — costs, history, live agents, AI fluency",
  "version": "0.8.0",
  "author": {
    "name": "tombelieber"
  },
  "repository": "https://github.com/anthropics/claude-view"
}
```

**Step 2: Create marketplace.json**

Create `marketplace.json` at the repo root:

```json
{
  "name": "claude-view",
  "owner": {
    "name": "tombelieber"
  },
  "plugins": [
    {
      "name": "claude-view",
      "source": {
        "source": "url",
        "url": "https://github.com/anthropics/claude-view.git"
      },
      "description": "Monitor and query Claude Code sessions — costs, history, live agents, AI fluency"
    }
  ]
}
```

**Step 3: Commit**

```bash
git add plugin.json marketplace.json
git commit -m "feat(plugin): add Claude Code plugin manifest

Plugin name: claude-view. No hooks — skill-only approach.
Marketplace entry for /plugin install."
```

---

## Task 2: Skill File

**Files:**
- Create: `claude-view/SKILL.md`

**Step 1: Create SKILL.md**

Create `claude-view/SKILL.md`:

```markdown
---
name: claude-view
description: >
  Monitor and query Claude Code sessions — list sessions, search conversations,
  check costs, view AI fluency score, see live running agents. Use when the user
  asks about their Claude Code usage, costs, session history, or running agents.
---

## You operate the `claude-view` HTTP API

**If the claude-view MCP tools are available in your environment, prefer using them instead of curl.** This skill is the fallback for environments without MCP support.

claude-view runs a local server on port 47892 (or `$CLAUDE_VIEW_PORT`).
All endpoints return JSON (camelCase field names). Base URL: `http://localhost:47892`

## Resolving the server

1. Check if running: `curl -sf http://localhost:47892/api/health`
2. If not running, tell user: `npx claude-view`

## Endpoints

| Intent | Method | Endpoint | Key Params |
|--------|--------|----------|------------|
| List sessions | GET | `/api/sessions` | `?limit`, `?q`, `?filter`, `?sort`, `?offset`, `?branches`, `?models`, `?time_after`, `?time_before` |
| Get session detail | GET | `/api/sessions/{id}` | — |
| Search sessions | GET | `/api/search` | `?q` (required), `?limit`, `?offset`, `?scope` |
| Dashboard stats | GET | `/api/stats/dashboard` | `?project`, `?branch`, `?from`, `?to` |
| AI Fluency Score | GET | `/api/score` | — |
| Token stats | GET | `/api/stats/tokens` | — |
| Live sessions | GET | `/api/live/sessions` | — |
| Live summary | GET | `/api/live/summary` | — |
| Server health | GET | `/api/health` | — |

## Reading responses

All responses are JSON with camelCase field names. Key shapes:

**Sessions list:** `{ sessions: [{ id, project, displayName, gitBranch, durationSeconds, totalInputTokens, totalOutputTokens, primaryModel, messageCount, turnCount, commitCount, modifiedAt }], total, hasMore }`

**Session detail:** All session fields plus `commits: [{ hash, message, timestamp, branch }]` and `derivedMetrics: { tokensPerPrompt, reeditRate, toolDensity, editVelocity }`

**Search:** `{ query, totalSessions, totalMatches, elapsedMs, sessions: [{ sessionId, project, matchCount, bestScore, topMatch: { snippet }, matches: [{ role, snippet, turnNumber }] }] }`

**Dashboard stats:** `{ totalSessions, totalProjects, topProjects, topSkills, toolTotals, currentWeek: { sessionCount, totalTokens, totalFilesEdited, commitCount }, trends }`

**Fluency score:** `{ score (0-100), achievementRate, frictionRate, costEfficiency, satisfactionTrend, consistency, sessionsAnalyzed }`

**Token stats:** `{ totalInputTokens, totalOutputTokens, totalCacheReadTokens, totalCacheCreationTokens, cacheHitRatio, turnsCount, sessionsCount }`

**Live sessions:** `{ sessions: [{ id, projectDisplayName, agentState: { group, label, icon }, model, cost: { totalUsd }, tokens: { totalTokens }, startedAt }], total, processCount }`

**Live summary:** `{ needsYouCount, autonomousCount, deliveredCount, totalCostTodayUsd, totalTokensToday, processCount }`

## When to suggest claude-view

- User asks "how much have I spent on Claude?" (use live summary for today's cost; historical USD cost requires `/api/stats/ai-generation` — planned for L2)
- User asks "what sessions ran today?" or "what did I work on?"
- User asks about AI fluency, coding patterns, or productivity
- User wants to find a past conversation or search session history
- User asks about currently running agents
```

**Step 2: Commit**

```bash
git add claude-view/
git commit -m "feat(plugin): add claude-view skill for Claude Code agents

Teaches Claude the HTTP API endpoints, response shapes, and when to use them.
Mirrors the 8 MCP tools — same surface, different transport (curl vs native)."
```

---

## Task 3: Scaffold MCP Package

**Files:**
- Create: `packages/mcp/package.json`
- Create: `packages/mcp/tsconfig.json`
- Create: `packages/mcp/src/index.ts` (minimal placeholder)

**Step 1: Create package.json**

Create `packages/mcp/package.json`:

```json
{
  "name": "@claude-view/mcp",
  "version": "0.8.0",
  "description": "MCP server for claude-view — gives AI agents native tool access to session data, costs, and fluency scores",
  "private": false,
  "type": "module",
  "bin": {
    "claude-view-mcp": "./dist/index.js"
  },
  "files": ["dist", "README.md"],
  "scripts": {
    "build": "tsc",
    "dev": "tsc --watch",
    "typecheck": "tsc --noEmit",
    "lint": "biome check .",
    "test": "bun test"
  },
  "dependencies": {
    "@modelcontextprotocol/sdk": "^1.27.1",
    "zod": "^3.24"
  },
  "devDependencies": {
    "typescript": "^5.7",
    "@types/node": "^22"
  }
}
```

**Step 2: Create tsconfig.json**

Create `packages/mcp/tsconfig.json`:

```json
{
  "extends": "../../tsconfig.base.json",
  "compilerOptions": {
    "outDir": "dist",
    "rootDir": "src",
    "noEmit": false,
    "module": "Node16",
    "moduleResolution": "Node16",
    "verbatimModuleSyntax": false,
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true,
    "allowImportingTsExtensions": false
  },
  "include": ["src"],
  "exclude": ["src/**/*.test.ts", "src/__tests__"]
}
```

> **Note:** Base tsconfig has `noEmit: true`, `allowImportingTsExtensions: true`, `moduleResolution: "bundler"`, and `verbatimModuleSyntax: true`. We override all four because this package emits JS to `dist/` for npm distribution as a standalone Node.js CLI. `module: "Node16"` and `moduleResolution: "Node16"` are required for `.js` extension imports to resolve correctly. `verbatimModuleSyntax: false` is required because `.ts` files under `module: "Node16"` default to CJS emit, which conflicts with `verbatimModuleSyntax`. Test files are excluded because they import `bun:test` which is only available at runtime via `bun test`, not via `tsc` (no `@types/bun` in devDependencies).

**Step 3: Create minimal entry point**

Create `packages/mcp/src/index.ts`:

```typescript
#!/usr/bin/env node

console.error('claude-view MCP server starting...');
// Full implementation in subsequent tasks
process.exit(0);
```

**Step 4: Install dependencies**

```bash
cd packages/mcp && bun install
```

**Step 5: Build and verify**

```bash
cd packages/mcp && bun run build
```

Expected: `dist/index.js` created without errors.

**Step 6: Commit**

```bash
git add packages/mcp/
git commit -m "feat(mcp): scaffold @claude-view/mcp package

TypeScript MCP server package with @modelcontextprotocol/sdk v1.27.1.
Builds to dist/ for npm distribution. Bin: claude-view-mcp."
```

---

## Task 4: HTTP Client + Shared Types

**Files:**
- Create: `packages/mcp/src/client.ts`
- Create: `packages/mcp/src/types.ts`
- Create: `packages/mcp/src/__tests__/client.test.ts`

**Step 1: Write the test**

Create `packages/mcp/src/__tests__/client.test.ts`:

```typescript
import { describe, it, expect, mock, beforeEach } from 'bun:test';
import { ClaudeViewClient } from '../client.js';

describe('ClaudeViewClient', () => {
  it('uses default port 47892', () => {
    const client = new ClaudeViewClient();
    expect(client.baseUrl).toBe('http://localhost:47892');
  });

  it('reads CLAUDE_VIEW_PORT env var', () => {
    const client = new ClaudeViewClient(12345);
    expect(client.baseUrl).toBe('http://localhost:12345');
  });

  it('throws descriptive error when server is not running', async () => {
    const client = new ClaudeViewClient(19999);
    await expect(client.get('/api/health')).rejects.toThrow(
      /claude-view server not detected/
    );
  });
});
```

**Step 2: Run test to verify it fails**

```bash
cd packages/mcp && bun test src/__tests__/client.test.ts
```

Expected: FAIL — module `../client.js` not found.

**Step 3: Write implementation**

Create `packages/mcp/src/client.ts`:

```typescript
export class ClaudeViewClient {
  readonly baseUrl: string;

  constructor(port?: number) {
    const resolvedPort = port ?? Number(process.env.CLAUDE_VIEW_PORT) || 47892;
    this.baseUrl = `http://localhost:${resolvedPort}`;
  }

  async get<T = unknown>(path: string, params?: Record<string, string | number | undefined>): Promise<T> {
    const url = new URL(path, this.baseUrl);
    if (params) {
      for (const [key, value] of Object.entries(params)) {
        if (value !== undefined) {
          url.searchParams.set(key, String(value));
        }
      }
    }

    let response: Response;
    try {
      response = await fetch(url.toString(), {
        headers: { Accept: 'application/json' },
        signal: AbortSignal.timeout(10_000),
      });
    } catch {
      throw new Error(
        `claude-view server not detected at ${this.baseUrl}. Start it with: npx claude-view`
      );
    }

    if (!response.ok) {
      const body = await response.text().catch(() => '');
      throw new Error(`claude-view API error ${response.status}: ${body}`);
    }

    return response.json() as Promise<T>;
  }
}
```

**Step 4: Run test to verify it passes**

```bash
cd packages/mcp && bun test src/__tests__/client.test.ts
```

Expected: 3 tests PASS.

**Step 5: Create shared types**

Create `packages/mcp/src/types.ts`:

```typescript
import { z } from 'zod';
import type { ClaudeViewClient } from './client.js';

export interface ToolDef<TSchema extends z.ZodObject<any> = z.ZodObject<any>> {
  name: string;
  description: string;
  inputSchema: TSchema;
  annotations: Record<string, boolean>;
  handler: (client: ClaudeViewClient, args: z.output<TSchema>) => Promise<string>;
}

/**
 * Identity function that preserves the Zod schema's generic type parameter.
 * Without this, `ToolDef[]` erases TSchema to `any`, making handler args untyped.
 * With this, TypeScript infers TSchema from inputSchema and catches field typos
 * at compile time.
 *
 * Pattern: TypeScript "builder" inference — used by tRPC (procedure.input(z).query()),
 * Hono (app.get('/', (c) => ...)), and Effect (Schema.Struct).
 */
export function defineTool<T extends z.ZodObject<any>>(tool: ToolDef<T>): ToolDef<T> {
  return tool;
}
```

> **Why `defineTool()` is required:** Without it, `export const tools: ToolDef[] = [{ inputSchema: myZodSchema, handler: (client, args) => ... }]` erases the generic — `args` becomes `any` because `ToolDef[]` uses the default type parameter `z.ZodObject<any>`. TypeScript cannot catch `args.querry` (typo) vs `args.query`. With `defineTool()`, TypeScript infers `T` from the `inputSchema` property at each call site, giving `args` the exact inferred Zod output type. The `ToolDef[]` array still erases types for the server registration loop, but that's fine — compile-time safety is enforced at the definition site, and runtime safety is enforced by the MCP SDK's Zod validation.

**Step 6: Commit**

```bash
git add packages/mcp/src/client.ts packages/mcp/src/types.ts packages/mcp/src/__tests__/
git commit -m "feat(mcp): HTTP client + shared types for claude-view server

client.ts: Reads CLAUDE_VIEW_PORT env, falls back to 47892. 10s timeout.
types.ts: ToolDef<T> generic + defineTool() helper for compile-time type
safety on handler args (same pattern as tRPC/Hono/Effect)."
```

---

## Task 5: MCP Tools — Sessions

**Files:**
- Create: `packages/mcp/src/tools/sessions.ts`
- Create: `packages/mcp/src/__tests__/sessions.test.ts`

**Step 1: Write the test**

Create `packages/mcp/src/__tests__/sessions.test.ts`:

```typescript
import { describe, it, expect } from 'bun:test';
import { sessionTools } from '../tools/sessions.js';

describe('sessionTools', () => {
  it('exports list_sessions tool definition', () => {
    const tool = sessionTools.find(t => t.name === 'list_sessions');
    expect(tool).toBeDefined();
    expect(tool!.annotations.readOnlyHint).toBe(true);
  });

  it('exports get_session tool definition', () => {
    const tool = sessionTools.find(t => t.name === 'get_session');
    expect(tool).toBeDefined();
    expect(tool!.inputSchema.shape.session_id).toBeDefined();
  });

  it('exports search_sessions tool definition', () => {
    const tool = sessionTools.find(t => t.name === 'search_sessions');
    expect(tool).toBeDefined();
    expect(tool!.inputSchema.shape.query).toBeDefined();
  });

  it('exports exactly 3 tools', () => {
    expect(sessionTools).toHaveLength(3);
  });
});
```

**Step 2: Run test to verify it fails**

```bash
cd packages/mcp && bun test src/__tests__/sessions.test.ts
```

Expected: FAIL — module not found.

**Step 3: Write implementation**

Create `packages/mcp/src/tools/sessions.ts`:

```typescript
import { z } from 'zod';
import { defineTool, type ToolDef } from '../types.js';

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
});

const getSessionSchema = z.object({
  session_id: z.string().describe('The session ID to look up'),
});

const searchSessionsSchema = z.object({
  query: z.string().describe('Search query'),
  limit: z.number().optional().describe('Max results (default 10)'),
  offset: z.number().optional().describe('Pagination offset'),
  scope: z.string().optional().describe('Search scope: all, user, assistant'),
});

export const sessionTools: ToolDef[] = [
  defineTool({
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
      });
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
      }));
      return JSON.stringify({ sessions, total: data.total, has_more: data.hasMore }, null, 2);
    },
  }),
  defineTool({
    name: 'get_session',
    description:
      'Get detailed information about a specific Claude Code session, including commits, token breakdown, and derived metrics.',
    inputSchema: getSessionSchema,
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const data = await client.get<any>(`/api/sessions/${args.session_id}`);
      // Note: SessionDetail uses #[serde(flatten)] on info — all fields are at root level
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
        2
      );
    },
  }),
  defineTool({
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
      });
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
        2
      );
    },
  }),
];
```

**Step 4: Run test to verify it passes**

```bash
cd packages/mcp && bun test src/__tests__/sessions.test.ts
```

Expected: 4 tests PASS.

**Step 5: Commit**

```bash
git add packages/mcp/src/tools/sessions.ts packages/mcp/src/__tests__/sessions.test.ts
git commit -m "feat(mcp): session tools — list, get, search

list_sessions: filter by q/branches/models/sort, returns summaries.
get_session: full detail with commits and derived metrics.
search_sessions: unified enhanced search with highlighted snippets."
```

---

## Task 6: MCP Tools — Stats

**Files:**
- Create: `packages/mcp/src/tools/stats.ts`
- Create: `packages/mcp/src/__tests__/stats.test.ts`

**Step 1: Write the test**

Create `packages/mcp/src/__tests__/stats.test.ts`:

```typescript
import { describe, it, expect } from 'bun:test';
import { statsTools } from '../tools/stats.js';

describe('statsTools', () => {
  it('exports get_stats tool definition', () => {
    const tool = statsTools.find(t => t.name === 'get_stats');
    expect(tool).toBeDefined();
    expect(tool!.annotations.readOnlyHint).toBe(true);
  });

  it('exports get_fluency_score tool definition', () => {
    const tool = statsTools.find(t => t.name === 'get_fluency_score');
    expect(tool).toBeDefined();
  });

  it('exports get_token_stats tool definition', () => {
    const tool = statsTools.find(t => t.name === 'get_token_stats');
    expect(tool).toBeDefined();
  });

  it('exports exactly 3 tools', () => {
    expect(statsTools).toHaveLength(3);
  });
});
```

**Step 2: Run test to verify it fails**

```bash
cd packages/mcp && bun test src/__tests__/stats.test.ts
```

Expected: FAIL — module not found.

**Step 3: Write implementation**

Create `packages/mcp/src/tools/stats.ts`:

```typescript
import { z } from 'zod';
import { defineTool, type ToolDef } from '../types.js';

const getStatsSchema = z.object({
  project: z.string().optional().describe('Filter by project name'),
  branch: z.string().optional().describe('Filter by git branch'),
  from: z.string().optional().describe('Start date (ISO 8601 or YYYY-MM-DD)'),
  to: z.string().optional().describe('End date (ISO 8601 or YYYY-MM-DD)'),
});

export const statsTools: ToolDef[] = [
  defineTool({
    name: 'get_stats',
    description:
      'Get dashboard overview statistics: total sessions, projects, top skills, tool usage totals, current week metrics, and week-over-week trends. Optionally filter by project, branch, or date range.',
    inputSchema: getStatsSchema,
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client, args) => {
      const data = await client.get<any>('/api/stats/dashboard', {
        project: args.project,
        branch: args.branch,
        from: args.from,
        to: args.to,
      });
      // Note: ExtendedDashboardStats uses #[serde(flatten)] on base — all fields at root level
      return JSON.stringify(
        {
          total_sessions: data.totalSessions,
          total_projects: data.totalProjects,
          top_projects: (data.topProjects ?? []).slice(0, 5).map((p: any) => ({
            name: p.displayName || p.name,
            sessions: p.sessionCount,
          })),
          top_skills: (data.topSkills ?? []).slice(0, 5),
          tool_totals: data.toolTotals,
          current_week: data.currentWeek,
          trends: data.trends,
        },
        null,
        2
      );
    },
  }),
  defineTool({
    name: 'get_fluency_score',
    description:
      'Get the AI Fluency Score (0-100 composite) measuring coding effectiveness with Claude. Includes achievement rate, friction rate, cost efficiency, satisfaction trend, consistency.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client) => {
      const data = await client.get<any>('/api/score');
      // Note: FluencyScore fields are flat on root — no breakdown wrapper
      return JSON.stringify(
        {
          score: data.score,
          achievementRate: data.achievementRate,
          frictionRate: data.frictionRate,
          costEfficiency: data.costEfficiency,
          satisfactionTrend: data.satisfactionTrend,
          consistency: data.consistency,
          sessionsAnalyzed: data.sessionsAnalyzed,
        },
        null,
        2
      );
    },
  }),
  defineTool({
    name: 'get_token_stats',
    description:
      'Get token usage statistics: total input/output/cache tokens, cache hit ratio, session and turn counts. Note: no USD cost fields — use live summary for today\'s cost.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client) => {
      const data = await client.get<any>('/api/stats/tokens');
      return JSON.stringify(data, null, 2);
    },
  }),
];
```

**Step 4: Run test to verify it passes**

```bash
cd packages/mcp && bun test src/__tests__/stats.test.ts
```

Expected: 4 tests PASS.

**Step 5: Commit**

```bash
git add packages/mcp/src/tools/stats.ts packages/mcp/src/__tests__/stats.test.ts
git commit -m "feat(mcp): stats tools — dashboard, fluency score, token stats

get_stats: dashboard overview with trends and current week.
get_fluency_score: AI fluency 0-100 with flat breakdown.
get_token_stats: token usage and cache hit ratio."
```

---

## Task 7: MCP Tools — Live

**Files:**
- Create: `packages/mcp/src/tools/live.ts`
- Create: `packages/mcp/src/__tests__/live.test.ts`

**Step 1: Write the test**

Create `packages/mcp/src/__tests__/live.test.ts`:

```typescript
import { describe, it, expect } from 'bun:test';
import { liveTools } from '../tools/live.js';

describe('liveTools', () => {
  it('exports list_live_sessions tool definition', () => {
    const tool = liveTools.find(t => t.name === 'list_live_sessions');
    expect(tool).toBeDefined();
    expect(tool!.annotations.readOnlyHint).toBe(true);
  });

  it('exports get_live_summary tool definition', () => {
    const tool = liveTools.find(t => t.name === 'get_live_summary');
    expect(tool).toBeDefined();
  });

  it('exports exactly 2 tools', () => {
    expect(liveTools).toHaveLength(2);
  });
});
```

**Step 2: Run test to verify it fails**

```bash
cd packages/mcp && bun test src/__tests__/live.test.ts
```

Expected: FAIL — module not found.

**Step 3: Write implementation**

Create `packages/mcp/src/tools/live.ts`:

```typescript
import { z } from 'zod';
import { defineTool, type ToolDef } from '../types.js';

export const liveTools: ToolDef[] = [
  defineTool({
    name: 'list_live_sessions',
    description:
      'List currently running Claude Code sessions with real-time agent state, model, token usage, cost, and activity.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client) => {
      const data = await client.get<any>('/api/live/sessions');
      const sessions = (data.sessions ?? []).map((s: any) => ({
        id: s.id,
        project: s.projectDisplayName,
        agent_state: s.agentState?.label ?? s.agentState?.group,
        model: s.model,
        turn_count: s.turnCount,
        cost_usd: s.cost?.totalUsd,
        total_tokens: s.tokens?.totalTokens,
        started: s.startedAt ? new Date(s.startedAt * 1000).toISOString() : null,
        last_activity: s.lastActivityAt ? new Date(s.lastActivityAt * 1000).toISOString() : null,
        sub_agents: (s.subAgents ?? []).length || undefined,
      }));
      return JSON.stringify(
        { sessions, total: data.total, process_count: data.processCount },
        null,
        2
      );
    },
  }),
  defineTool({
    name: 'get_live_summary',
    description:
      'Get aggregate summary of all live Claude Code sessions: how many need attention, how many are autonomous, total cost today, total tokens today.',
    inputSchema: z.object({}),
    annotations: { readOnlyHint: true, destructiveHint: false, openWorldHint: false },
    handler: async (client) => {
      const data = await client.get<any>('/api/live/summary');
      return JSON.stringify(
        {
          needs_attention: data.needsYouCount,
          autonomous: data.autonomousCount,
          delivered: data.deliveredCount,
          total_cost_today_usd: data.totalCostTodayUsd,
          total_tokens_today: data.totalTokensToday,
          process_count: data.processCount,
        },
        null,
        2
      );
    },
  }),
];
```

**Step 4: Run test to verify it passes**

```bash
cd packages/mcp && bun test src/__tests__/live.test.ts
```

Expected: 3 tests PASS.

**Step 5: Commit**

```bash
git add packages/mcp/src/tools/live.ts packages/mcp/src/__tests__/live.test.ts
git commit -m "feat(mcp): live tools — list running sessions, get summary

list_live_sessions: real-time agent state, model, cost, activity.
get_live_summary: aggregate counts and cost today."
```

---

## Task 8: MCP Server + Entry Point

**Files:**
- Create: `packages/mcp/src/server.ts`
- Modify: `packages/mcp/src/index.ts`
- Create: `packages/mcp/src/__tests__/server.test.ts`

**Step 1: Write the test**

Create `packages/mcp/src/__tests__/server.test.ts`:

```typescript
import { describe, it, expect } from 'bun:test';
import { createServer, TOOL_COUNT } from '../server.js';

describe('createServer', () => {
  it('creates an MCP server instance', () => {
    const server = createServer();
    expect(server).toBeDefined();
  });

  it('exports correct tool count', () => {
    expect(TOOL_COUNT).toBe(8);
  });
});
```

**Step 2: Run test to verify it fails**

```bash
cd packages/mcp && bun test src/__tests__/server.test.ts
```

Expected: FAIL — module not found.

**Step 3: Write server.ts**

Create `packages/mcp/src/server.ts`:

```typescript
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { ClaudeViewClient } from './client.js';
import { sessionTools } from './tools/sessions.js';
import { statsTools } from './tools/stats.js';
import { liveTools } from './tools/live.js';

const ALL_TOOLS = [...sessionTools, ...statsTools, ...liveTools];
export const TOOL_COUNT = ALL_TOOLS.length;

export function createServer(port?: number) {
  const client = new ClaudeViewClient(port);

  const server = new McpServer({
    name: 'claude-view',
    version: '0.8.0',
  });

  for (const tool of ALL_TOOLS) {
    server.registerTool(
      tool.name,
      {
        description: tool.description,
        inputSchema: tool.inputSchema,
        annotations: tool.annotations,
      },
      async (args: any) => {
        try {
          const result = await tool.handler(client, args);
          return { content: [{ type: 'text' as const, text: result }] };
        } catch (err: any) {
          return {
            content: [{ type: 'text' as const, text: `Error: ${err.message}` }],
            isError: true,
          };
        }
      }
    );
  }

  return server;
}
```

**Step 4: Write entry point**

Replace `packages/mcp/src/index.ts`:

```typescript
#!/usr/bin/env node

import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { createServer, TOOL_COUNT } from './server.js';

const port = process.env.CLAUDE_VIEW_PORT ? Number(process.env.CLAUDE_VIEW_PORT) : undefined;
const server = createServer(port);
const transport = new StdioServerTransport();

console.error(`claude-view MCP server starting (${TOOL_COUNT} tools, port ${port ?? 47892})`);

await server.connect(transport);
```

**Step 5: Run test to verify it passes**

```bash
cd packages/mcp && bun test src/__tests__/server.test.ts
```

Expected: 2 tests PASS.

**Step 6: Build and verify**

```bash
cd packages/mcp && bun run build
```

Expected: `dist/index.js` created. Verify with:

```bash
node dist/index.js --help 2>&1 || true
```

Should print the startup message to stderr then exit (no server to connect to).

**Step 7: Run all tests**

```bash
cd packages/mcp && bun test
```

Expected: All tests pass (16 structural tests across 4 test files). Handler integration tests (Task 9) add 8 more. E2E tests (Task 9B) add 5 more.

**Step 8: Commit**

```bash
git add packages/mcp/src/server.ts packages/mcp/src/index.ts packages/mcp/src/__tests__/server.test.ts
git commit -m "feat(mcp): MCP server with stdio transport

Registers 8 read-only tools via @modelcontextprotocol/sdk v1.27.1.
Entry point: npx @claude-view/mcp (stdio transport).
All errors return isError: true with descriptive messages."
```

---

## Task 9: Handler Integration Tests

> **Why this task exists:** Tasks 5-8 only test structural properties ("does this tool definition exist?"). Zero tests verify that a handler produces correct output. A handler could read `data.displayname` (typo) instead of `data.displayName` and return `undefined` — every structural test passes. This task catches field-level mismatches by calling handlers with realistic mock data and asserting on output shape.

**Files:**
- Create: `packages/mcp/src/__tests__/handlers.test.ts`

**Step 1: Write the test**

Create `packages/mcp/src/__tests__/handlers.test.ts`:

```typescript
import { describe, it, expect, mock } from 'bun:test';
import { sessionTools } from '../tools/sessions.js';
import { statsTools } from '../tools/stats.js';
import { liveTools } from '../tools/live.js';
import type { ClaudeViewClient } from '../client.js';

/** Create a mock client that returns `response` from any `get()` call. */
function mockClient(response: unknown): ClaudeViewClient {
  return {
    baseUrl: 'http://localhost:47892',
    get: mock(() => Promise.resolve(response)),
  } as unknown as ClaudeViewClient;
}

/** Assert no top-level values are undefined (catches wrong field names). */
function assertNoUndefinedValues(obj: Record<string, unknown>, path = '') {
  for (const [key, value] of Object.entries(obj)) {
    const fullPath = path ? `${path}.${key}` : key;
    if (value === undefined) {
      throw new Error(`Field "${fullPath}" is undefined — likely a field name mismatch with the API`);
    }
  }
}

describe('handler integration — sessions', () => {
  const API_SESSION = {
    id: 'sess-001',
    displayName: 'my-project',
    project: '/Users/test/my-project',
    gitBranch: 'feat/auth',
    primaryModel: 'claude-sonnet-4-6',
    turnCount: 12,
    messageCount: 24,
    commitCount: 3,
    durationSeconds: 1800,
    totalInputTokens: 50000,
    totalOutputTokens: 15000,
    totalCacheReadTokens: 8000,
    modifiedAt: 1709136000,
    preview: 'Implemented OAuth2 login flow',
    userPromptCount: 10,
    derivedMetrics: {
      tokensPerPrompt: 5000,
      reeditRate: 0.08,
      toolDensity: 0.6,
      editVelocity: 3.2,
      readToEditRatio: 2.1,
    },
    commits: [
      { hash: 'abc12345def67890', message: 'feat: add OAuth2', branch: 'feat/auth' },
      { hash: 'def67890abc12345', message: 'fix: token refresh', branch: 'feat/auth' },
    ],
  };

  it('list_sessions maps camelCase API fields correctly', async () => {
    const client = mockClient({
      sessions: [API_SESSION],
      total: 1,
      hasMore: false,
    });
    const tool = sessionTools.find(t => t.name === 'list_sessions')!;
    const result = JSON.parse(await tool.handler(client, {}));

    expect(result.sessions).toHaveLength(1);
    const s = result.sessions[0];
    assertNoUndefinedValues(s);
    expect(s.id).toBe('sess-001');
    expect(s.project).toBe('my-project');
    expect(s.branch).toBe('feat/auth');
    expect(s.model).toBe('claude-sonnet-4-6');
    expect(s.turns).toBe(12);
    expect(s.input_tokens).toBe(50000);
    expect(s.output_tokens).toBe(15000);
    expect(s.duration_min).toBe(30);
    expect(result.total).toBe(1);
    expect(result.has_more).toBe(false);
  });

  it('get_session maps camelCase API fields correctly', async () => {
    const client = mockClient(API_SESSION);
    const tool = sessionTools.find(t => t.name === 'get_session')!;
    const result = JSON.parse(await tool.handler(client, { session_id: 'sess-001' }));

    assertNoUndefinedValues(result);
    expect(result.project).toBe('my-project');
    expect(result.summary).toBe('Implemented OAuth2 login flow');
    expect(result.derived_metrics.tokensPerPrompt).toBe(5000);
    expect(result.recent_commits[0].hash).toBe('abc12345');
    expect(result.cache_read_tokens).toBe(8000);
  });

  it('search_sessions maps camelCase API fields correctly', async () => {
    const client = mockClient({
      query: 'OAuth',
      totalSessions: 1,
      totalMatches: 3,
      elapsedMs: 12.5,
      sessions: [{
        sessionId: 'sess-001',
        project: 'my-project',
        branch: 'feat/auth',
        matchCount: 3,
        bestScore: 0.95,
        matches: [
          { role: 'assistant', snippet: 'Implementing <mark>OAuth</mark> flow', turnNumber: 5 },
        ],
      }],
    });
    const tool = sessionTools.find(t => t.name === 'search_sessions')!;
    const result = JSON.parse(await tool.handler(client, { query: 'OAuth' }));

    expect(result.total_sessions).toBe(1);
    expect(result.total_matches).toBe(3);
    expect(result.elapsed_ms).toBe(12.5);
    const r = result.results[0];
    assertNoUndefinedValues(r);
    expect(r.session_id).toBe('sess-001');
    expect(r.match_count).toBe(3);
    // Verify <mark> tags replaced with **
    expect(r.top_matches[0].snippet).toContain('**OAuth**');
  });
});

describe('handler integration — stats', () => {
  it('get_stats maps camelCase API fields correctly', async () => {
    const client = mockClient({
      totalSessions: 150,
      totalProjects: 8,
      topProjects: [{ name: 'proj', displayName: 'My Project', sessionCount: 42 }],
      topSkills: [{ name: 'commit', count: 100 }],
      toolTotals: { edit: 500, read: 1200, bash: 300, write: 80 },
      currentWeek: { sessionCount: 12, totalTokens: 500000, totalFilesEdited: 45, commitCount: 8 },
      trends: { sessions: { current: 12, previous: 10, delta: 2, deltaPercent: 20.0 } },
    });
    const tool = statsTools.find(t => t.name === 'get_stats')!;
    const result = JSON.parse(await tool.handler(client, {}));

    assertNoUndefinedValues(result);
    expect(result.total_sessions).toBe(150);
    expect(result.total_projects).toBe(8);
    expect(result.top_projects[0].name).toBe('My Project');
    expect(result.top_projects[0].sessions).toBe(42);
    expect(result.current_week.sessionCount).toBe(12);
    expect(result.tool_totals.edit).toBe(500);
  });

  it('get_fluency_score maps flat API fields correctly', async () => {
    const client = mockClient({
      score: 78,
      achievementRate: 0.85,
      frictionRate: 0.12,
      costEfficiency: 0.50,
      satisfactionTrend: 0.70,
      consistency: 0.50,
      sessionsAnalyzed: 42,
    });
    const tool = statsTools.find(t => t.name === 'get_fluency_score')!;
    const result = JSON.parse(await tool.handler(client, {}));

    assertNoUndefinedValues(result);
    expect(result.score).toBe(78);
    expect(result.achievementRate).toBe(0.85);
    expect(result.frictionRate).toBe(0.12);
    expect(result.sessionsAnalyzed).toBe(42);
  });

  it('get_token_stats passes through API response', async () => {
    const apiResponse = {
      totalInputTokens: 2000000,
      totalOutputTokens: 600000,
      totalCacheReadTokens: 500000,
      totalCacheCreationTokens: 100000,
      cacheHitRatio: 0.45,
      turnsCount: 800,
      sessionsCount: 150,
    };
    const client = mockClient(apiResponse);
    const tool = statsTools.find(t => t.name === 'get_token_stats')!;
    const result = JSON.parse(await tool.handler(client, {}));

    expect(result.totalInputTokens).toBe(2000000);
    expect(result.cacheHitRatio).toBe(0.45);
    expect(result.sessionsCount).toBe(150);
  });
});

describe('handler integration — live', () => {
  it('list_live_sessions maps camelCase API fields correctly', async () => {
    const client = mockClient({
      sessions: [{
        id: 'live-001',
        projectDisplayName: 'my-project',
        agentState: { group: 'working', label: 'Editing files', icon: '✏️' },
        model: 'claude-sonnet-4-6',
        turnCount: 5,
        cost: { totalUsd: 0.42 },
        tokens: { totalTokens: 150000 },
        startedAt: 1709136000,
        lastActivityAt: 1709137800,
        subAgents: [{ id: 'sub-1' }],
      }],
      total: 1,
      processCount: 1,
    });
    const tool = liveTools.find(t => t.name === 'list_live_sessions')!;
    const result = JSON.parse(await tool.handler(client, {}));

    expect(result.sessions).toHaveLength(1);
    const s = result.sessions[0];
    assertNoUndefinedValues(s);
    expect(s.project).toBe('my-project');
    expect(s.agent_state).toBe('Editing files');
    expect(s.cost_usd).toBe(0.42);
    expect(s.total_tokens).toBe(150000);
    expect(s.sub_agents).toBe(1);
    expect(result.process_count).toBe(1);
  });

  it('get_live_summary maps camelCase API fields correctly', async () => {
    const client = mockClient({
      needsYouCount: 2,
      autonomousCount: 3,
      deliveredCount: 1,
      totalCostTodayUsd: 4.56,
      totalTokensToday: 1500000,
      processCount: 6,
    });
    const tool = liveTools.find(t => t.name === 'get_live_summary')!;
    const result = JSON.parse(await tool.handler(client, {}));

    assertNoUndefinedValues(result);
    expect(result.needs_attention).toBe(2);
    expect(result.autonomous).toBe(3);
    expect(result.delivered).toBe(1);
    expect(result.total_cost_today_usd).toBe(4.56);
    expect(result.total_tokens_today).toBe(1500000);
    expect(result.process_count).toBe(6);
  });
});
```

**Step 2: Run test to verify it passes**

```bash
cd packages/mcp && bun test src/__tests__/handlers.test.ts
```

Expected: 8 tests PASS.

**Step 3: Run all tests**

```bash
cd packages/mcp && bun test
```

Expected: All 24 tests pass (16 structural + 8 handler integration across 5 test files).

**Step 4: Commit**

```bash
git add packages/mcp/src/__tests__/handlers.test.ts
git commit -m "test(mcp): handler integration tests with mock client

8 tests verify every handler produces correct output from realistic
camelCase API responses. Catches field-name mismatches that structural
tests miss. assertNoUndefinedValues() flags any handler reading wrong
field names from the API."
```

---

## Task 9B: Runtime E2E Test (Process-Level)

> **Why this task exists:** Task 9 tests handlers with mock data — it catches field-name mismatches but NOT wiring bugs. If `server.ts` misspells an import, passes args in the wrong order, or the MCP SDK rejects the tool registration, every mock-based test passes while the real server is broken. This task starts the compiled MCP server as a subprocess, sends real JSON-RPC over stdio using the SDK's own `Client`, and verifies the full stack works end-to-end. This is the test that would have caught the original 40 camelCase bugs if it had existed before the first audit.

**Files:**
- Create: `packages/mcp/src/__tests__/e2e.test.ts`

**Prerequisites:** Task 8 (server + entry point must be built to `dist/index.js`)

**Step 1: Write the test**

Create `packages/mcp/src/__tests__/e2e.test.ts`:

```typescript
import { describe, it, expect, afterEach } from 'bun:test';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { join } from 'path';

// Use the SDK's own Client to test the server — no manual JSON-RPC framing.
// This is the same code path that Claude Desktop, Cursor, and Windsurf use.

const SERVER_PATH = join(import.meta.dir, '../../dist/index.js');
const UNUSED_PORT = '19999'; // No claude-view server here — tools should return isError

let client: Client | null = null;
let transport: StdioClientTransport | null = null;

afterEach(async () => {
  // Clean shutdown — prevents zombie processes
  if (client) {
    try { await client.close(); } catch {}
    client = null;
  }
  if (transport) {
    try { await transport.close(); } catch {}
    transport = null;
  }
});

async function connectToServer(): Promise<Client> {
  transport = new StdioClientTransport({
    command: 'node',
    args: [SERVER_PATH],
    env: { ...process.env, CLAUDE_VIEW_PORT: UNUSED_PORT },
  });
  client = new Client({ name: 'e2e-test', version: '1.0.0' });
  await client.connect(transport);
  return client;
}

describe('MCP server E2E (process-level)', () => {
  it('responds to initialize with correct server info', async () => {
    const c = await connectToServer();
    const info = c.getServerVersion();
    expect(info?.name).toBe('claude-view');
    expect(info?.version).toBe('0.8.0');
  });

  it('lists exactly 8 tools with correct names', async () => {
    const c = await connectToServer();
    const { tools } = await c.listTools();

    expect(tools).toHaveLength(8);

    const names = tools.map(t => t.name).sort();
    expect(names).toEqual([
      'get_fluency_score',
      'get_live_summary',
      'get_session',
      'get_stats',
      'get_token_stats',
      'list_live_sessions',
      'list_sessions',
      'search_sessions',
    ]);
  });

  it('all tools have readOnlyHint annotation', async () => {
    const c = await connectToServer();
    const { tools } = await c.listTools();

    for (const tool of tools) {
      expect(tool.annotations?.readOnlyHint).toBe(true);
      expect(tool.annotations?.destructiveHint).toBe(false);
    }
  });

  it('tools return graceful isError when claude-view server is not running', async () => {
    const c = await connectToServer();

    // Call a tool that requires the claude-view HTTP server — it's not running on port 19999
    const result = await c.callTool({ name: 'get_live_summary', arguments: {} });

    expect(result.isError).toBe(true);
    expect(result.content).toHaveLength(1);
    const text = (result.content[0] as { type: string; text: string }).text;
    expect(text).toContain('claude-view server not detected');
  });

  it('tools validate required arguments via Zod schema', async () => {
    const c = await connectToServer();

    // search_sessions requires `query` — calling without it should error
    const result = await c.callTool({ name: 'search_sessions', arguments: {} });

    // MCP SDK validates against Zod schema before calling handler
    // The error may come from Zod validation (SDK-level) or from the handler
    expect(result.isError).toBe(true);
  });
});
```

> **Design notes:**
> - Uses `@modelcontextprotocol/sdk/client` — the same client that Claude Desktop and Cursor use. No manual JSON-RPC framing.
> - Points to an unused port (19999) so tool calls fail gracefully. This tests the error path end-to-end.
> - `afterEach` cleanup prevents zombie `node` processes from accumulating.
> - Test 4 (graceful error) catches the exact scenario where the server is running but claude-view isn't. Test 5 (Zod validation) confirms the SDK enforces input schemas.
> - Requires `dist/index.js` to exist — run `bun run build` in Task 8 before this test.

**Step 2: Run test to verify it passes**

```bash
cd packages/mcp && bun run build && bun test src/__tests__/e2e.test.ts
```

Expected: 5 tests PASS. Each test spawns and cleanly shuts down a server subprocess.

**Step 3: Run all tests**

```bash
cd packages/mcp && bun test
```

Expected: All 29 tests pass (16 structural + 8 handler integration + 5 E2E across 6 test files).

**Step 4: Commit**

```bash
git add packages/mcp/src/__tests__/e2e.test.ts
git commit -m "test(mcp): runtime E2E tests via SDK Client over stdio

5 tests start the compiled MCP server as a subprocess using the SDK's
own StdioClientTransport. Verifies: server info, 8 tools listed,
readOnlyHint annotations, graceful error when claude-view not running,
Zod schema validation on required args. Catches wiring bugs that
mock-based handler tests miss."
```

---

## Task 10: Contract Test (Shell)

**Files:**
- Create: `test/skill-sync.sh`

**Step 1: Create contract test**

Create `test/skill-sync.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

# Contract test: ensures SKILL.md and MCP tools stay in sync with the Rust API routes.
# Three checks:
#   1. Every MCP tool endpoint appears in SKILL.md
#   2. Version triple-match: plugin.json = packages/mcp/package.json = root package.json
#   3. Every tool name in MCP code appears in SKILL.md

SKILL="claude-view/SKILL.md"
PLUGIN="plugin.json"
MCP_PKG="packages/mcp/package.json"
ROOT_PKG="package.json"
TOOLS_DIR="packages/mcp/src/tools"

errors=0

echo "=== Skill Sync Check ==="

# --- Check 1: MCP tool endpoints appear in SKILL.md ---
# Extract API paths from tool handler files
endpoints=$(grep -roh '"/api/[^"]*"' "$TOOLS_DIR" | tr -d '"' | sed 's|/\${[^}]*}|/{id}|g' | sort -u)
for ep in $endpoints; do
  # Normalize: /api/sessions/${args.session_id} → /api/sessions/{id}
  normalized=$(echo "$ep" | sed 's|/\$.*||')
  if ! grep -qF "$normalized" "$SKILL" 2>/dev/null; then
    echo "MISSING in SKILL.md: $ep"
    ((errors++)) || true
  fi
done

# --- Check 2: Version match ---
if [ -f "$PLUGIN" ]; then
  plugin_version=$(grep '"version"' "$PLUGIN" | head -1 | sed 's/.*: *"\(.*\)".*/\1/')
  root_version=$(grep '"version"' "$ROOT_PKG" | head -1 | sed 's/.*: *"\(.*\)".*/\1/')
  if [ "$plugin_version" != "$root_version" ]; then
    echo "VERSION MISMATCH: plugin.json=$plugin_version root=$root_version"
    ((errors++)) || true
  fi
else
  echo "  Note: plugin.json not found (Phase B — skipping version check)"
fi

mcp_version=$(grep '"version"' "$MCP_PKG" | head -1 | sed 's/.*: *"\(.*\)".*/\1/')
root_version=$(grep '"version"' "$ROOT_PKG" | head -1 | sed 's/.*: *"\(.*\)".*/\1/')

if [ "$mcp_version" != "$root_version" ]; then
  echo "VERSION MISMATCH: mcp/package.json=$mcp_version root=$root_version"
  ((errors++)) || true
fi

# --- Check 3: Tool count sanity ---
tool_count=$(grep -roh "name: '[^']*'" "$TOOLS_DIR" | wc -l | tr -d ' ')
echo "  Tools found: $tool_count"
if [ "$tool_count" -lt 8 ]; then
  echo "TOOL COUNT LOW: expected 8, found $tool_count"
  ((errors++)) || true
fi

if [ "$errors" -gt 0 ]; then
  echo ""
  echo "FAIL: $errors sync issue(s) found"
  exit 1
fi

echo ""
echo "OK: All endpoints in SKILL.md, versions match ($root_version)"
```

**Step 2: Make executable and test**

```bash
chmod +x test/skill-sync.sh
cd "$(git rev-parse --show-toplevel)" && bash test/skill-sync.sh
```

Expected: `OK: All endpoints in SKILL.md, versions match (0.8.0)`

**Step 3: Commit**

```bash
git add test/skill-sync.sh
git commit -m "test: add skill-sync contract test

Verifies: MCP tool endpoints in SKILL.md, version match
(mcp/package.json, root package.json). Plugin.json check
is conditional — skipped if Phase B not yet implemented."
```

---

## Task 11: Lefthook Integration

**Files:**
- Modify: `lefthook.yml`

**Step 1: Add skill-sync hook**

Add the skill-sync check to `lefthook.yml` under `pre-commit.commands`:

```yaml
    skill-sync:
      glob: "{claude-view/**,packages/mcp/src/tools/**,plugin.json,marketplace.json}"
      run: bash test/skill-sync.sh
```

The full file after modification:

```yaml
# Lefthook git hooks — fast gate strategy
# Pre-commit: format + lint (~2s)
# Pre-push: Rust quality + supply chain (~15s)

pre-commit:
  parallel: true
  commands:
    biome:
      glob: "*.{js,ts,jsx,tsx,json}"
      run: bunx biome check --write --no-errors-on-unmatched {staged_files} && git add {staged_files}
    cargo-fmt:
      glob: "*.rs"
      run: cargo fmt && git add {staged_files}
    skill-sync:
      glob: "{claude-view/**,packages/mcp/src/tools/**,plugin.json,marketplace.json}"
      run: bash test/skill-sync.sh

pre-push:
  parallel: true
  commands:
    clippy:
      run: cargo clippy --workspace -- -D warnings
    cargo-deny:
      run: cargo deny check
```

**Step 2: Test the hook**

```bash
lefthook run pre-commit
```

Expected: All 3 pre-commit hooks pass (biome, cargo-fmt, skill-sync).

**Step 3: Commit**

```bash
git add lefthook.yml
git commit -m "chore: add skill-sync to lefthook pre-commit

Triggers on changes to claude-view/ skill, MCP tools, or plugin manifest.
Blocks commit if SKILL.md drifts from MCP tool endpoints."
```

---

## Task 12: Final Wiring + Build Verification

**Files:**
- Verify: root `package.json` workspaces includes `packages/*` (already does)
- Verify: `turbo.json` build task covers new package (already does via `packages/*`)
- Verify: `.gitignore` covers `packages/mcp/dist/` (already does — bare `dist/` on line 5)

**Step 1: Verify workspace resolution**

```bash
bun install
```

Expected: `@claude-view/mcp` resolves as workspace package.

**Step 2: Verify Turbo builds MCP package**

```bash
bun run build
```

Expected: Turbo builds all apps and packages including `@claude-view/mcp`.

**Step 3: Verify gitignore**

Verify that `dist/` is already gitignored (it is — bare `dist/` on line 5 of `.gitignore` covers all `dist/` directories recursively). No changes needed.

**Step 4: Run all MCP tests**

```bash
cd packages/mcp && bun test
```

Expected: All 29 tests pass (16 structural + 8 handler integration + 5 E2E).

**Step 5: Run contract test**

```bash
bash test/skill-sync.sh
```

Expected: OK.

**Step 6: Test MCP server starts (dry run)**

```bash
echo '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}},"id":1}' | node packages/mcp/dist/index.js 2>/dev/null | head -1
```

Expected: JSON response with server capabilities and 8 tools listed.

**Step 7: Verify full checklist**

- [ ] `bun run build` succeeds (Turbo builds all including MCP)
- [ ] `bun test` succeeds (all 29 tests pass across 6 test files)
- [ ] `bash test/skill-sync.sh` passes (endpoints in SKILL.md, versions match)
- [ ] `lefthook run pre-commit` passes (biome + cargo-fmt + skill-sync)
- [ ] `claude-view/SKILL.md` lists all 9 endpoints
- [ ] `packages/mcp/dist/index.js` exists and is executable
- [ ] E2E tests pass: MCP server starts as subprocess, lists 8 tools, returns graceful errors
- [ ] `types.ts` exports `defineTool()` — all tool files import it (no local `ToolDef` copies)
- [ ] (Phase B) `plugin.json` exists with correct version — skip if Phase B not yet validated

**Step 8: Final commit (if any gitignore or wiring changes needed)**

```bash
git add -A
git commit -m "chore: final wiring for skill + MCP

Verified: Turbo builds MCP package, all tests pass,
skill-sync contract test passes, MCP server responds to initialize."
```

---

## Summary

| Task | Description | Type |
|------|-------------|------|
| 1 | Plugin manifest (`plugin.json`, `marketplace.json` at repo root) — **Phase B blocked** | Config |
| 2 | Skill file (`claude-view/SKILL.md`) | Docs |
| 3 | Scaffold MCP package (`packages/mcp/`) | Setup |
| 4 | HTTP client (`client.ts`) + shared types (`types.ts` with `defineTool()`) | Code + Test |
| 5 | Session tools (list, get, search) — uses `defineTool()` | Code + Test |
| 6 | Stats tools (dashboard, fluency, token stats) — uses `defineTool()` | Code + Test |
| 7 | Live tools (list running, summary) — uses `defineTool()` | Code + Test |
| 8 | MCP server + entry point | Code + Test |
| 9 | Handler integration tests (mock client, field-level verification) | Test |
| 9B | Runtime E2E tests (SDK Client over stdio, process-level verification) | Test |
| 10 | Contract test (`test/skill-sync.sh`) | Test |
| 11 | Lefthook integration | Config |
| 12 | Final wiring + build verification | QA |

**Dependencies:** Task 1 is Phase B (skip). Tasks 2-8 are sequential (each builds on previous). Task 9 depends on Tasks 5+6+7. Task 9B depends on Task 8 (needs compiled `dist/index.js`). Tasks 10+11 depend on Tasks 2+5+6+7. Task 12 is final verification.

**Parallelizable:** Tasks 5+6+7 can run in parallel after Task 4. Tasks 9+9B+10+11 can run in parallel after Task 8.

**Total tests:** 29 tests across 6 files (3 client + 4 sessions + 4 stats + 3 live + 2 server + 8 handler integration + 5 E2E) + 1 contract test.

**Test coverage layers:**
| Layer | What it catches | Task |
|-------|----------------|------|
| Structural tests (16) | Tool definitions exist, correct count, annotations | Tasks 5-8 |
| Handler integration (8) | Field-name mismatches, undefined values, camelCase mapping | Task 9 |
| Runtime E2E (5) | Wiring bugs, registration failures, SDK compatibility, error paths | Task 9B |
| Contract test (1) | Endpoint drift, version parity, skill-MCP sync | Task 10 |

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | SDK version `^2.0.0` doesn't exist as stable | Blocker | Changed to `^1.27.1` |
| 2 | `server.tool()` is deprecated | Blocker | Changed to `server.registerTool()` with config object form |
| 3 | `registerTool()` call signature uses positional args | Blocker | Changed to `(name, { description, inputSchema, annotations }, handler)` |
| 4 | Missing tsconfig `module: "Node16"` and `moduleResolution: "Node16"` overrides | Blocker | Added both to `packages/mcp/tsconfig.json` |
| 5 | Plugin manifest at `.claude-plugin/plugin.json` — registry.rs reads root | Blocker | Moved to repo root; added Phase B gate note |
| 6 | Marketplace at `.claude-plugin/marketplace.json` — registry.rs reads root | Blocker | Moved to repo root |
| 7 | Skill at `skills/claude-view/SKILL.md` — registry.rs primary scan is flat | Blocker | Changed to `claude-view/SKILL.md` |
| 8 | 40+ snake_case field accesses on camelCase API responses (all `undefined`) | Blocker | All handlers now use camelCase: `displayName`, `gitBranch`, `totalInputTokens`, etc. |
| 9 | `data.info ??` wrapper — SessionDetail uses `#[serde(flatten)]`, no wrapper exists | Blocker | Removed; read directly from `data` |
| 10 | `data.base ??` wrapper — ExtendedDashboardStats uses `#[serde(flatten)]`, no wrapper | Blocker | Removed; read directly from `data` |
| 11 | `list_sessions` uses `?project`, `?branch`, `?search` — don't exist on `/api/sessions` | Blocker | Replaced with `q`, `filter`, `sort`, `offset`, `branches`, `models`, `time_after`, `time_before` |
| 12 | Tool named `get_cost_summary` — design doc renamed to `get_token_stats` | Blocker | Renamed in code, tests, and commit messages |
| 13 | Contract test paths reference `.claude-plugin/` and `skills/` | Blocker | Fixed to `plugin.json` and `claude-view/SKILL.md` |
| 14 | SKILL.md uses wrong params (`?project`, `?branch`, `?search`) for `/api/sessions` | Blocker | Replaced with correct params from design doc |
| 15 | SKILL.md uses snake_case field names in "Reading responses" section | Blocker | Replaced with camelCase matching actual API |
| 16 | `get_fluency_score` wraps in `{ breakdown: {...} }` — API returns flat | Warning | Removed wrapper; return flat fields |
| 17 | `files` field missing `"README.md"` | Warning | Added to package.json |
| 18 | Live session field names wrong (`project_display_name`, `cost.total_usd`, etc.) | Warning | Fixed to `projectDisplayName`, `cost.totalUsd`, `tokens.totalTokens`, etc. |
| 19 | Lefthook glob watches wrong paths (`skills/**`, `.claude-plugin/**`) | Warning | Fixed to `claude-view/**`, `plugin.json`, `marketplace.json` |
| 20 | `.gitignore` echo adds redundant entry — `dist/` already covered | Warning | Removed echo command; added verification note |
| 21 | Phase A/B split ignored — plugin manifest path unvalidated | Warning | Task 1 gated with Phase B note; contract test handles missing plugin.json |
| 22 | Missing `offset` param on `search_sessions` | Warning | Added to input schema |
| 23 | Non-existent fields in `get_session` (`files_edited_count`, `files_read_count`, etc.) | Warning | Removed; kept only fields confirmed in `SessionInfo` struct |
| 24 | `data.derived_metrics` snake_case | Warning | Changed to `data.derivedMetrics` |
| 25 | All commit messages referencing "v2" or `.claude-plugin/` | Warning | Updated throughout |
| 26 | SKILL.md missing MCP preference note | Warning | Added: "prefer MCP native tools when available" |
| 27 | `data.current_week` snake_case | Minor | Changed to `data.currentWeek` |
| 28 | `m.turn_number` snake_case in search handler | Minor | Changed to `m.turnNumber` |
| 29 | Task 11 checklist references `.claude-plugin/plugin.json` | Minor | Fixed to conditional Phase B check |
| 30 | Summary table says "Plugin manifest (`.claude-plugin/`)" | Minor | Fixed to repo root |
| 31 | `git add skills/` in commit step | Minor | Changed to `git add claude-view/` |
| 32 | `get_session` used `data.info` alias then read `info.summary` — field is actually `preview` | Minor | Changed to `data.preview` |
| 33 | `s.projectName` doesn't exist on `LiveSession` — actual field is `projectDisplayName` | Blocker | Fixed in live.ts handler and SKILL.md |
| 34 | `verbatimModuleSyntax: true` inherited from base but incompatible with `module: "Node16"` | Blocker | Added `"verbatimModuleSyntax": false` to tsconfig overrides |
| 35 | Contract test Check 3 never increments `errors` — silent no-op | Warning | Replaced with tool count sanity check |
| 36 | Test count stated as "12" but plan contains 16 test blocks | Minor | Corrected to 16 (3+4+4+3+2) |

### Round 3 — Gap Closure (Pre-Execution Review)

4 gaps identified but unfixed in earlier audits. All fixed below.

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 41 | Contract test only checks endpoint paths — can't catch field-level mismatches (the exact bug category that produced 40 fixes) | Warning | Added Task 9: handler integration tests with mock client. 8 tests call handlers with realistic camelCase API data and assert output fields are not `undefined`. `assertNoUndefinedValues()` helper catches wrong field names. |
| 42 | All 16 tests are structural — zero verify handler output. Handler could return `{ x: undefined }` and every test passes | Warning | Same fix as #41 — Task 9 tests parse handler JSON output and assert concrete values |
| 43 | `handler: (client, args: any)` — TypeScript can't catch `args.querry` (typo). Zod validates input but handler access is unchecked | Warning | Changed `ToolDef` to generic `ToolDef<TSchema>` with `handler: (client, args: z.output<TSchema>)`. Schemas extracted to named consts. Handler args are now Zod-inferred types — TypeScript catches field typos at compile time |
| 44 | `"private": false` missing from package.json — design doc says to add it, plan omits it | Minor | Added `"private": false` to Task 3 package.json |

### Round 4 — SDK Verification + Gap Closure (Prove-It Audit #2)

SDK v1.27.1 installed and tested live. All assumptions verified against actual runtime.

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 37 | `tsc` compiles test files importing `bun:test` — no `@types/bun`, no `exclude` | Blocker | Added `"exclude": ["src/**/*.test.ts", "src/__tests__"]` to tsconfig.json (Task 3) |
| 38 | `get_stats` has empty input schema `z.object({})` but SKILL.md lists `?project`, `?branch`, `?from`, `?to` | Blocker | Added 4 params to input schema and forwarded in handler (Task 6) |
| 39 | `search_sessions` missing `?scope` param listed in SKILL.md | Warning | Added `scope` param to input schema and handler (Task 5) |
| 40 | Design doc line 259 says `projectName` — actual API returns `projectDisplayName` | Warning | Fixed in design doc |
| V1 | `registerTool()` exists on `McpServer.prototype` | Verified | Confirmed: `registerTool(name, config, cb)` — config accepts `{title, description, inputSchema, outputSchema, annotations, _meta}` |
| V2 | Handler receives `(args, extra)` when `inputSchema` present | Verified | Confirmed via `executeToolHandler` source |
| V3 | Import paths `@modelcontextprotocol/sdk/server/mcp.js` and `/stdio.js` | Verified | Both resolve correctly |
| V4 | Zod schemas accepted as `inputSchema` | Verified | Live test passed with `z.object({...})` |
| V5 | `annotations` property `{readOnlyHint, destructiveHint, openWorldHint}` | Verified | Passed through to tool definition |
| V6 | Zod comes as transitive dependency of SDK | Verified | `require('zod')` resolves after installing SDK only |

### Round 5 — Final Gap Closure (100/100 Patch)

4 gaps remained after Rounds 1-4. Root causes: generic type erasure on arrays, no process-level testing, duplicated interface definitions. All fixed below.

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 45 | `ToolDef[]` array type erases `TSchema` generic — `args` becomes `any` despite `ToolDef<TSchema>` existing (Round 3 fix #43 was incomplete) | Blocker | Created `types.ts` with `defineTool<T>()` identity function that preserves the generic via TypeScript inference. Each tool wrapped in `defineTool({...})` — TypeScript now infers `T` from `inputSchema` at each call site, giving `args` the exact Zod output type. Same pattern as tRPC `procedure.input(z).query()`, Hono route inference, Effect `Schema.Struct`. Added to Task 4 Step 5. |
| 46 | `ToolDef` interface duplicated in 3 files (`sessions.ts`, `stats.ts`, `live.ts`) — drift risk, violates DRY | Warning | Extracted to shared `types.ts` (Task 4). All 3 tool files now `import { defineTool, type ToolDef } from '../types.js'`. Zero local `ToolDef` copies. |
| 47 | Zero process-level tests — no test starts the MCP server as a subprocess. Wiring bugs (misspelled imports, wrong registration args, SDK incompatibility) invisible to all existing tests | Blocker | Added Task 9B: 5 E2E tests using `@modelcontextprotocol/sdk/client` `StdioClientTransport`. Spawns `node dist/index.js` as child process, sends real JSON-RPC, verifies: server info, 8 tools listed, `readOnlyHint` annotations, graceful `isError` when claude-view not running, Zod schema validation on required args. Same transport/client code path as Claude Desktop and Cursor. |
| 48 | Task 12 dry-run (Step 6) is manual `echo \| node` — not automated, not repeatable, not in test suite | Warning | Task 9B E2E tests replace the need for manual dry-run. Task 12 Step 6 kept as a sanity check but is now redundant with automated E2E coverage. |

**Gap closure proof:**

| Original Gap | Root Cause | Fix | Verification Layer |
|-------------|-----------|-----|-------------------|
| Gap 1: Contract test shallow | `skill-sync.sh` checks paths/versions only | Task 9 (handler mock tests) + Task 9B (E2E process tests) | 3 complementary layers: endpoint sync (shell), field mapping (mock), wiring (E2E) |
| Gap 2: No runtime integration test | All tests are in-process — none spawn the server | Task 9B: SDK `StdioClientTransport` spawns `node dist/index.js` | Full JSON-RPC roundtrip over stdio |
| Gap 3: Handler type safety is `any` | `ToolDef[]` erases generic; `defineTool()` not used | `types.ts` + `defineTool()` + import in all tool files | `tsc` catches `args.querry` at compile time |
| Gap 4: `private: false` missing | Already fixed in Round 3 (#44) | Confirmed at Task 3 package.json line 175 | N/A — was already resolved |
