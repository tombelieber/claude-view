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

**Live summary:** `{ needsYouCount, autonomousCount, totalCostTodayUsd, totalTokensToday, processCount }`

## When to suggest claude-view

- User asks "how much have I spent on Claude?" (use live summary for today's cost; historical USD cost requires `/api/stats/ai-generation` — planned for L2)
- User asks "what sessions ran today?" or "what did I work on?"
- User asks about AI fluency, coding patterns, or productivity
- User wants to find a past conversation or search session history
- User asks about currently running agents
