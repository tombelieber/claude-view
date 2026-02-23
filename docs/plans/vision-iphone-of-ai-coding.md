---
status: draft
date: 2026-02-24
type: vision
revisit: next-session
---

# Vision: The iPhone of AI Coding

> **Status:** Draft vision document. NOT the current build target. Discuss and refine in a future session after the near-term "Command Center" (Approach A) is shipped.

## The Insight

Every AI coding tool today — Claude Code, Kiro, OpenCode, Cursor, Windsurf, Codex — requires expertise to use effectively. You need to understand:

- CLI conventions and flags
- Git worktrees and branching strategies
- Prompt engineering and CLAUDE.md conventions
- Context window limits and prompt caching economics
- Tool permissions and safety models
- Session management and when to start fresh vs resume
- Model selection (Opus vs Sonnet vs Haiku) and cost trade-offs
- MCP servers, hooks, skills, and agent orchestration

This is like smartphones in 2006. Palm Treos, BlackBerries, Windows Mobile — powerful but expert-only. Then iPhone made it so your grandmother could use one.

**The vision: make AI coding best practices as easy as breathing.**

Users don't need to learn any of the above. They describe what they want, and it happens. The complexity is hidden behind a consumer-grade interface that makes the right decisions for you.

## What "Just Works" Means

### Today (expert mode)
```
1. Open terminal
2. cd to project
3. Create worktree (if parallel work needed)
4. Run `claude` with the right flags
5. Write a good prompt (requires skill)
6. Monitor context usage (might need to compact)
7. Approve/deny tool calls (requires judgment)
8. Check if cache is warm (timing matters for cost)
9. Review output, iterate
10. Merge worktree back
11. Repeat for next task
```

### Vision (it just works)
```
1. Open claude-view
2. "I want to add authentication to my app"
3. See a plan. Click "Run."
4. Watch it happen. Approve when asked.
5. Done.
```

Behind the scenes, claude-view:
- Broke "add authentication" into 4 sub-tasks (middleware, routes, tests, docs)
- Created 4 worktrees
- Chose the right model for each (Haiku for tests, Sonnet for middleware)
- Managed context windows automatically
- Timed API calls to stay in prompt cache window
- Ran tests automatically
- Flagged the one test that failed, auto-retried with the error context
- Merged all 4 branches
- Showed you the result

The user made ONE decision ("add authentication") instead of 50.

## Design Principles

### 1. Every prompt = 1 decision = friction
Never ask the user to make a decision about something they don't understand yet. If a default is sensible, just do it.

| Decision | Expert tool asks | iPhone layer decides |
|----------|-----------------|---------------------|
| Which model? | "Select: Opus / Sonnet / Haiku" | Auto-selects based on task complexity |
| Worktree? | "Create a worktree for isolation?" | Auto-creates when parallel work detected |
| Cache timing? | "Cache expires in 2m, send now?" | Auto-times messages to stay in cache |
| Context full? | "Compact context?" | Auto-compacts before hitting limit |
| Permission? | "Allow: rm -rf node_modules?" | Auto-allows safe ops, flags dangerous ones |
| Retry? | "Test failed, retry?" | Auto-retries with error context, escalates if stuck |

### 2. Progressive disclosure
Show simplicity first, reveal complexity when asked.

- Default view: progress bar + result
- One click deeper: plan breakdown, per-task status
- Another click: full conversation log, diffs, token usage
- Expert mode: raw JSONL, manual prompt editing, full CLI access

### 3. Opinionated defaults, overridable settings
The 1% who care about model selection can find it in settings. The 99% never need to know.

### 4. Visual, not textual
Show diffs visually, not as terminal output. Show cost as a gauge, not a number. Show progress as a timeline, not log lines.

## What This Product Looks Like

### Desktop: The Dashboard

```
┌──────────────────────────────────────────────────────────────────┐
│  claude-view                              [Search] [Settings] [?]│
│──────────────────────────────────────────────────────────────────│
│                                                                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │  What do you want to build?                                  │ │
│  │  ┌───────────────────────────────────────────────────────┐  │ │
│  │  │ Add user authentication with JWT and OAuth...         │  │ │
│  │  └───────────────────────────────────────────────────────┘  │ │
│  │                                          [Generate Plan →]  │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│  Active Work                                                      │
│  ┌───────────┐ ┌───────────┐ ┌───────────┐                      │
│  │ Auth MW   │ │ DB Schema │ │ API Tests │                      │
│  │ ████████░ │ │ ██████████│ │ ████░░░░░ │                      │
│  │ 80% $0.31 │ │ Done $0.18│ │ 40% $0.09 │                      │
│  │ [View]    │ │ [Approve] │ │ [View]    │                      │
│  └───────────┘ └───────────┘ └───────────┘                      │
│                                                                   │
│  Recent                                                           │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │ ✅ Fix CORS headers          2h ago    $0.12   [View]     │  │
│  │ ✅ Add rate limiting          5h ago    $0.44   [View]     │  │
│  │ ✅ Refactor auth module       1d ago    $1.21   [View]     │  │
│  └────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

### Mobile: The Command Center

```
┌─────────────────────────────┐
│  claude-view          ●  5G │
│─────────────────────────────│
│                              │
│  ┌──────────────────────┐   │
│  │ What do you want?    │   │
│  │ [Voice 🎤] [Type ⌨️]│   │
│  └──────────────────────┘   │
│                              │
│  🔄 Auth middleware   80%   │
│  ✅ DB schema         Done  │
│  🔄 API tests         40%  │
│  ⏳ E2E tests      Queued  │
│                              │
│  Total: $0.58 / ~$0.80 est │
│                              │
│  ┌────────────────────────┐ │
│  │ DB Schema ready ✓      │ │
│  │ 3 files · all tests ✓  │ │
│  │ [View Diff] [Approve]  │ │
│  └────────────────────────┘ │
│                              │
│─────────────────────────────│
│  ⚡ 2 running  1 needs you  │
└─────────────────────────────┘
```

## How This Differs from Lovable/Base44

Lovable and Base44 generate apps from scratch for non-developers. The "iPhone of AI coding" is fundamentally different:

| | Lovable/Base44 | claude-view vision |
|---|---|---|
| Target user | Non-developers, founders | Professional developers |
| Starting point | Empty project | **Existing codebase** |
| AI role | Generates entire app | Works within your architecture |
| Output | Hosted app (their platform) | Changes in **your** git repo |
| Control | Black box | Full visibility + approval gates |
| Scale | One app at a time | **Parallel tasks across your codebase** |
| Lock-in | High (their hosting, their framework) | Zero (your code, your tools) |

The key difference: Lovable replaces the developer. claude-view **amplifies** the developer.

## What Has to Be True

For this vision to work, several things must be validated:

1. **Plan generation quality:** Can an LLM reliably break "add auth" into correct sub-tasks with proper dependencies? (Kiro's Specs suggest yes, but needs validation at scale)
2. **Parallel execution reliability:** Can 3-10 agents work on the same codebase in parallel worktrees without stepping on each other? (Git worktrees provide isolation, but merge conflicts are real)
3. **Auto-review accuracy:** Can LLM-as-judge catch real bugs without drowning users in false positives? (Emerging technique, needs calibration)
4. **Cost predictability:** Can per-plan budgets and cache optimization keep costs reasonable? (Prompt caching helps enormously, but parallel agents multiply base cost)
5. **UX simplicity:** Can the interface truly hide complexity without removing necessary control? (Progressive disclosure is the pattern, but getting the layers right is hard)

## Path from Here to There

```
NOW (Month 1-3): Command Center
├── Ship mobile monitoring (M1)
├── Ship session control (Phase F)
├── Ship plan runner MVP (Phase K, 3 parallel max)
└── Dogfood daily — learn what breaks, what's missing

NEXT (Month 4-6): Smart Defaults
├── Auto-model selection based on task complexity
├── Auto-cache timing (batch messages within cache window)
├── Auto-retry on test failure (with error context)
├── Plan templates (common patterns: "add API endpoint", "add auth", etc.)
└── One-click plan generation from natural language

LATER (Month 7-12): The iPhone Layer
├── "What do you want to build?" → full plan → execute → approve
├── Visual artifact inspector (see the app, not the code)
├── Progressive disclosure (simple → detailed → expert)
├── Voice input on mobile
└── Team features (shared plans, review workflows, cost allocation)
```

## Open Questions (for future discussion)

1. **Plan generation:** Build our own spec generator, or integrate with Kiro's Specs format as input? Or both?
2. **Artifact inspection:** How far do we go? Screenshots of running app? Embedded preview? Or just diffs and test output?
3. **Voice on mobile:** Is "talk to your AI fleet" a real use case or a gimmick? Happy has voice — does it actually get used?
4. **Team workflows:** When does single-player become multi-player? What's the trigger to invest in team features?
5. **Multi-provider:** Should plan runner support OpenCode/Codex agents alongside Claude Code? Or stay Claude-focused?

## References

- iPhone launch (2007): Simplified smartphones from expert-only to universal
- Stripe (2010): Made payments "just work" — developers stopped thinking about payment plumbing
- Linear (2019): Made project management feel fast — built for themselves first, then teams
- Kiro Specs: Validated that spec-driven development works, but only single-agent sequential
- Lovable ($1.8B): Proved the market wants "just describe it and it happens" — but only for new apps, not existing codebases
