---
status: draft
date: 2026-02-16
---

# clawmini.ai — Vision & Market Opportunity

> **The iPhone for AI agents.** You don't need to know what JSONL, Agent Teams, or tmux are. You just open the app, pick a plugin, make decisions, and get results.

## One-Line Pitch

**clawmini is a provider-agnostic AI workforce manager that wraps multi-agent orchestration into a mobile-friendly consumer product with a plugin marketplace.**

## The Problem

AI agents in 2026 are powerful but inaccessible:

| What exists | The problem |
|-------------|-------------|
| Claude Code Agent Teams | Requires CLI, tmux, experimental flags, JSONL knowledge |
| OpenCode | Multi-provider but terminal-only, developer-only |
| Agent Swarm (Kimi/GLM) | Locked to one provider, no cross-platform |
| Cursor | Beautiful UX but single-session, code-only, closed ecosystem |
| Pencil (pencel.dev) | Prompt-to-design but no agent orchestration |
| CrewAI / AutoGen | Requires Python to define agents |
| Devin | $500/mo, code-only, single-task |

**Every tool assumes the user IS a developer.** There is no product that lets a non-technical person harness multi-agent AI for real workflows — not just Q&A, but multi-step autonomous execution with human-in-the-loop decision points.

Today's AI is a **vending machine**: put in a question, get one answer. Even "advanced" tools are single-turn or single-task. Users who want multi-step workflows must manually chain prompts, copy-paste between tools, and babysit every step.

clawmini turns AI into a **workforce**: define what you want, make decisions at key checkpoints, agents handle everything else.

## Positioning

**clawmini is a UX company, not an AI company.**

LLM providers (Anthropic, OpenAI, Google, Moonshot, Zhipu) compete on intelligence. clawmini competes on **accessibility**. They make the engine faster. We make it so anyone can drive.

```
Before clawmini                        After clawmini
──────────────                         ──────────────
"Want AI agents working for you?       Open app.
 Learn Claude Code CLI, set            Pick a plugin.
 CLAUDE_CODE_EXPERIMENTAL_AGENT_       Make decisions.
 TEAMS=1, configure tmux, read         Get results.
 JSONL files, write Agent SDK          From your phone.
 scripts, manage env vars..."
```

**We are NOT competing with powerful coding agents / LLM providers.** We cut the cognitive overload of all that technical background and wrap it into one product for general users. Like how iPhone users don't need to learn OS internals, Linux, C, Swift, mail server config, camera algorithms, or graphics programming — they just use it and get things done.

## The DNA

clawmini is the union of five existing tools, plus two things none of them have:

| Layer | Inspiration | What we take |
|-------|-------------|-------------|
| **OS foundation** | OpenClaw | Hub-and-spoke gateway, session lifecycle, agent spawning, concurrency controls, cascading stop |
| **Code intelligence** | Cursor | Polished UX, "AI that just works," context-aware agents |
| **Visual generation** | Pencil (pencel.dev) | Prompt-to-design, mockups as decision points, visual output |
| **Provider freedom** | OpenCode | Swap Claude/GPT/Gemini/Kimi/GLM per task or per step |
| **Team orchestration** | Open Cowork | Multiple agents on one task, agent-to-agent collaboration |
| **+ Mobile command** | **NEW** | Command, review, steer agents from your phone |
| **+ Plugin marketplace** | **NEW** | Two-sided creator economy for workflow templates |

## Market Opportunity

### Opportunity #1: The Orchestration Layer (K8s for AI Agents)

The play: be the management layer ON TOP of AI agents, not the AI itself.

```
┌──────────────────────────────────┐
│  clawmini (Orchestration + UX)   │  <- We own this
│  "Manage, dispatch, review"      │
├──────────────────────────────────┤
│  Claude  | OpenAI | Google | ... │  <- They compete here
│  Code    | Codex  | Gemini |     │
└──────────────────────────────────┘
```

Kubernetes doesn't care if you run Node or Go or Rust containers — it orchestrates all of them. clawmini doesn't care if the agent is Claude, GPT, or Gemini — it manages the workflow.

**Commercial advantages:**
- Not locked to one provider's pricing or capabilities
- When a better model drops, users benefit instantly
- Charge for orchestration + UX, not for tokens
- Provider-agnostic = larger TAM

### Opportunity #2: The Plugin Marketplace

The #1 reason horizontal platforms fail is the "blank canvas problem." User opens the app, sees infinite possibility, and leaves. Plugins solve this.

**What a plugin IS:** A pre-built multi-step workflow with decision points. Not code. A recipe — the user follows it, makes choices at each step, agents do the work.

**Examples across verticals:**

| Vertical | Plugin example |
|----------|-----------------|
| Software SDLC | Spec → ASCII mockups → pick → HD mockups → pick → system design → pick → agents code → PR |
| KOL/Content | Idea → scripts → pick → produce → edit → schedule → publish |
| Trading/Research | Thesis → research → analyze → present → user decides → execute |

**The economics:**
- Plugin creators (domain experts) build and sell workflows ($29/mo or $199 one-time)
- clawmini takes 20% of each transaction
- Creators earn passive income, buyers get expert-level workflows
- A fitness YouTuber packages their content pipeline, sells to 10,000 other fitness creators
- A quant trader packages their Polymarket research workflow, sells to retail bettors

**Network effects:** More plugins = more verticals = more users = more plugins. This is the hardest moat to copy.

### Opportunity #3: Mobile-First = Uncontested

Every competitor is desktop/terminal. clawmini owns mobile.

| Competitor | Mobile story |
|-----------|-------------|
| OpenClaw | Terminal only |
| Claude Code Agent Teams | CLI + tmux |
| Devin | Web dashboard, not mobile-optimized |
| Cursor | Desktop app |
| CrewAI / AutoGen | Python scripts, no UI |

Nobody is even trying. The entire AI agent space assumes you're at a desk with a keyboard. But indie hackers and team leads aren't at their desk 24/7.

**Three mobile interaction patterns (must-have):**

| Pattern | When | Example |
|---------|------|---------|
| **Command & review** | Morning coffee | "Launch today's tasks, review last night's PRs" |
| **Real-time steering** | Quick glance | "Agent stuck on step 3, redirect it" |
| **Async inbox** | Notification | "Agent needs your decision on mockup A vs B" |

Desktop-first for planning & dispatch (good-to-have for v1, the 4th pattern).

### Opportunity #4: The Timing Window

Six months ago, this product was impossible. Today, every piece exists:

| Piece | What happened | When |
|-------|--------------|------|
| Agent SDK | Anthropic shipped session resume, subagents, auto-compaction | Late 2025 |
| Claude Code Agent Teams | Multi-agent orchestration with shared tasks + mailbox | Feb 2026 |
| Agent Swarm (Kimi/GLM) | Validates multi-agent across providers | 2025-2026 |
| Image generation APIs | DALL-E 3, Flux, Midjourney — mockup generation is commodity | 2024-2025 |
| Mobile AI UX patterns | ChatGPT mobile proved people want AI on their phone | 2023-2024 |

**The window closes when:**
- OpenClaw ships Agent Teams + dashboard (they have 160K stars)
- Anthropic/OpenAI build orchestration into their products

**Our advantage:** Working Rust backend + React frontend, deep session data model, SDLC vertical ready to dogfood. Competitors need 3-6 months to get where we are today.

## Three Moats (Ranked by Defensibility)

```
1. PLAYBOOK MARKETPLACE (hardest to copy)
   Network effects. Community lock-in. Creator economy.

2. MOBILE-FIRST UX (hard to copy)
   Competitors would need to rebuild from scratch.

3. PROVIDER-AGNOSTIC ORCHESTRATION (medium to copy)
   K8s for AI agents. Works with Claude, GPT, Gemini, Kimi, GLM, etc.
```

## Architecture

```
┌──────────────────────────────────────┐
│  clawmini                             │
│  Dashboard, Mobile, Plugins,        │
│  Marketplace, Cost Tracking           │
├──────────────────────────────────────┤
│  Orchestration Layer (Rust)           │
│  Provider abstraction, team configs,  │
│  crash recovery, history, state mgmt  │
├────────────┬─────────┬───────────────┤
│ Claude Code│ OpenCode│ Future        │
│ + Agent    │ + Agent │ providers     │
│ Teams      │ Swarm   │               │
└────────────┴─────────┴───────────────┘
```

**Key architectural principle:** For providers that already have multi-agent primitives (Claude Code Agent Teams, Agent Swarm), clawmini reads their native coordination files and provides the visual + mobile layer on top. For providers that don't, clawmini's orchestration layer provides equivalent coordination.

**Evolved from claude-view.** The existing Rust backend (Axum, 4 crates) + React frontend becomes the clawmini foundation. Existing capabilities carried forward:

| Existing | Reused as |
|----------|-----------|
| JSONL parser | Claude Code provider adapter |
| Session viewer | Team history browser |
| Fluency score / metrics | Per-agent cost tracking & analytics |
| SSE infrastructure | Real-time agent streaming |
| SQLite via sqlx | Team configs, plugin storage, marketplace data |
| Tantivy search | Cross-session / cross-team search |
| File watcher | Agent session discovery |

**What's new to build:**

| Capability | For |
|-----------|-----|
| Provider abstraction trait | Multi-provider support |
| WebSocket gateway | Real-time bi-directional agent control |
| Agent spawning (not just reading) | Starting/stopping agents from dashboard |
| Team config format (declarative YAML) | Reusable plugins |
| Marketplace API | Creator uploads, purchases, ratings |
| Mobile-responsive views | Phone-first command & control |
| Plugin engine | Step-by-step execution with decision points |

## Agent Teams Integration (Claude Code)

Claude Code Agent Teams (shipped Feb 5, 2026, experimental) provides multi-agent primitives that clawmini leverages:

**What Agent Teams gives us for free:**
- Shared task list (pending/in_progress/completed + dependencies)
- Mailbox system (agent-to-agent messaging)
- Delegate mode (lead coordinates, never codes)
- Plan approval gates
- Quality hook scripts (TeammateIdle, TaskCompleted)

**What Agent Teams lacks (our value-add):**

| Gap | clawmini fills it with |
|-----|----------------------|
| No visual dashboard (CLI-only) | Grid/List/Kanban/Monitor views |
| No cross-provider teams | Provider abstraction layer |
| No reusable team configs | Declarative plugin YAML |
| No per-agent cost tracking | Metrics dashboard |
| No session resume for teams | Crash recovery + persistent state |
| No team history browser | Session viewer (already built) |
| No mobile/remote access | Mobile-first responsive UI |

**How clawmini reads Agent Teams data:**
```
~/.claude/teams/{team-name}/config.json    → Team structure
~/.claude/teams/{team-name}/messages/      → Agent communications
~/.claude/tasks/{team-name}/               → Shared task state
~/.claude/projects/**/**.jsonl             → Session history (existing parser)
```

## MVP Strategy

### Phase 1: Agent Dashboard (observe & steer)

The smallest useful product for an indie hacker running multiple AI agents.

**Core loop:** See every active AI agent session. Send commands. Review output. From browser (mobile-responsive from day 1).

**4 views:**

| View | Purpose | Mobile? |
|------|---------|---------|
| Grid | Quick glance at all agents, status cards | Yes — primary mobile view |
| List | Sortable table with details | Yes — compact rows |
| Kanban | Agents grouped by status (Working/Paused/Done) | Yes — horizontal scroll |
| Monitor | Live terminal output (xterm.js) | Desktop only for v1 |

**v1 provider:** Claude Code only (leveraging existing JSONL parser + new Agent Teams file reading). Provider abstraction trait in place for future providers.

**Day 1 users:** Solo indie hackers + small team leads running multiple Claude Code sessions/teams.

### Phase 2: Plan Runner (autonomous execution)

Agents execute plans autonomously. User watches via the dashboard.

**Core loop:** Plan file in → agents break it into steps → execute → 3-layer verification → PR out.

**Architecture:**
```
Plan Runner
  1. Parse plan file → dependency DAG
  2. For each plan (topological order):
     a. Create git worktree
     b. Spawn "Planner" agent → break into steps
     c. For each step:
        - Spawn agent → do the work
        - Layer 1: Deterministic checks (lint, test, CI — fast, free)
        - If Layer 1 fails: agent retries once (bounded, max 2 rounds)
        - Layer 2: AI review (PR readiness, plan compliance, quality)
        - If Layer 2 rejects: agent retries with feedback
     d. Layer 3: Human decision on phone (approve/reject/redirect)
     e. Create PR
  3. Persist state to pipeline.json (resume from crash)
```

**3-Layer Verification Model (informed by Stripe Minions):**

```
Layer 1: DETERMINISTIC (plugin-defined, varies by vertical)
├── Code:     lint, test, CI, type check, build (~5 sec, free) ← THICK
├── Content:  char limits, link check, format check            ← THIN
├── Research: source URL check, data freshness                 ← MINIMAL
├── Purpose:  catch obvious failures before burning tokens
└── Note:     each plugin defines its own Layer 1 checks (0 to many)

Layer 2: AI REVIEW (semantic — quality-driven iteration)
├── PR readiness:    "Does this diff match the spec?"
├── Plan compliance: "Does output satisfy ALL acceptance criteria?"
├── Domain quality:  "Is this on-brand? Is this analysis thorough?"
├── Iterate until quality score reaches threshold (100/100)
├── Uses plan auditing skill pattern (typically 3-4 rounds)
└── Purpose:  the MAIN quality gate for all verticals

Layer 3: HUMAN DECISION (final call — mobile-first)
├── Approve → ship
├── Reject with feedback → agent retries
├── Redirect → change approach
├── MORE important when Layer 1 is thin (no lint/CI safety net)
└── Purpose:  user stays in control, builds trust over time

Plugin config per vertical:
  layer1_checks: [...]     # 0 to many deterministic gates
  layer2_quality: 100      # target quality score for AI audit
  layer3_human: required   # or "auto-approve if layer2 >= threshold"
```

**Key design rules:**
- Deterministic harness around the LLM — the orchestration is code, not LLM decisions
- Layer 1 (deterministic): Bounded retry — max 2 rounds (Stripe's pattern, diminishing returns after 2)
- Layer 2 (AI review/audit): Quality-driven iteration — audit until 100/100 quality score, not a fixed retry count. Uses plan auditing skills (typically 3-4 rounds). Bound on max cost/time, not attempt count.
- Reuse human infrastructure — integrate with users' existing CI/CD, linters, tools
- MCP-native tool layer — users plug in their own MCP servers
- Every task gets enforced limits: max cost, max wall-clock time (safety net, not quality ceiling)

### Phase 3+: Verticals & Marketplace

| Phase | What | Target |
|-------|------|--------|
| 3a | KOL/Content plugins | Content creators, influencers |
| 3b | Plugin marketplace (creator uploads + purchases) | All users |
| 3c | Polymarket research plugins | Retail prediction market users |

## Verticals

### Software SDLC (v1 — dogfood)

**The plugin:**
1. Agent interviews user about what to build (multiple choice questions)
2. Presents 3 ASCII mockups → user picks one
3. Generates HD mockups via image model → user picks
4. Presents system design diagrams → user approves
5. Agents code it in parallel (Agent Teams)
6. Tests run automatically
7. User reviews PR on phone

### KOL/Content (v2)

Content creators have their own development lifecycle — ideation, scripting, production, editing, scheduling, engagement. clawmini packages this into plugins.

**Example plugin:** "YouTube Video Pipeline"
1. Agent researches trending topics in your niche
2. Presents 5 video concepts with title/thumbnail ideas → user picks
3. Agent writes script + generates thumbnail options → user picks
4. Agent creates description, tags, scheduling recommendations
5. User approves → auto-schedule

### Trading/Research (roadmap — Polymarket only)

Scoped to prediction markets (no securities regulation). AI agents research events, analyze probabilities, present informed positions.

**Example plugin:** "Polymarket Event Analyzer"
1. Agent scans current markets for user's interest areas
2. Deep research on specific event (news, data, expert opinions)
3. Presents analysis with confidence levels and reasoning
4. User makes their own decision (tool never executes trades)

## Business Model

```
FREE TIER (Open Source)
├── Agent Dashboard (all 4 views)
├── Claude Code provider (read sessions, Agent Teams)
├── Mobile-responsive UI
├── Basic plugins (community)
├── Cost tracking
└── npx clawmini — zero friction install

PRO TIER (Individual — $X/mo)
├── All providers (OpenCode, future)
├── Plan Runner (autonomous execution)
├── Premium plugins
├── Advanced analytics & insights
├── Crash recovery & persistent state
└── Priority support

MARKETPLACE (Creator Economy)
├── Creators upload & sell plugins
├── 80/20 revenue split (creator/platform)
├── Ratings, reviews, usage analytics
└── Featured plugins (curated by clawmini)

ENTERPRISE (Team — $X/seat/mo)
├── Team aggregation (multi-user)
├── Manager dashboards
├── Orchestration policies & cost budgets
├── Audit logs & compliance export
└── SSO / SAML
```

## Prior Art: Stripe Minions (1,000+ merged PRs/week)

Stripe's internal coding agent "Minions" is the most production-proven agent system publicly documented. Key lessons integrated into clawmini's design:

**What Minions does:** Single-agent, one-shot coding. Agent writes code → deterministic lint/CI → max 2 retries → create PR → human reviews. Built on a fork of Block's Goose agent. 400+ MCP tools via centralized "Toolshed."

**Lessons adopted:**

| Stripe pattern | How clawmini uses it |
|---------------|---------------------|
| Deterministic harness around LLM | Our orchestration layer is Rust code enforcing gates, not LLMs deciding when to verify |
| Bounded execution (max 2 CI runs) | Every task gets enforced limits: max iterations, max cost, max time |
| MCP as universal tool interface | clawmini is MCP-native. Users plug in their own MCP servers. |
| Reuse human infrastructure | Integrate with users' existing CI/CD, linters, not rebuild them |
| Pre-warmed environments (10s spinup) | Environment provisioning needed for parallel agent dispatch |
| Multi-surface invocation (Slack, CLI, web) | Trigger from mobile, web, Slack, Discord, CLI |
| Conditional rules per subdirectory | Plugin rules are context-aware, not global |
| Human review is a feature | Optimize for "review-ready" output, not "merge-ready" |

**Where clawmini goes beyond Minions:** Stripe is code-only, single-provider, single-agent-per-task, no mobile, no marketplace. clawmini adds multi-provider, multi-vertical (content, research, trading), AI semantic review (Layer 2), mobile command & control, and a plugin marketplace.

Source: https://stripe.dev/blog/minions-stripes-one-shot-end-to-end-coding-agents

## Key Decisions Made

| Decision | Choice | Why |
|----------|--------|-----|
| Name | clawmini.ai (working name) | Nods to OpenClaw foundation, can rename later |
| Codebase | Evolve claude-view, not fresh start | Months of Rust/React work, no waste |
| MVP | Agent Dashboard first, Plan Runner second | Observability before autonomy. Dashboard debugs the runner. |
| Day 1 provider | Claude Code only | Already have deep JSONL parsing. Provider trait ready for expansion. |
| Day 1 users | Indie hackers + small team leads | Dogfood persona. Technical enough to adopt early. |
| Mobile | Responsive web, not native app | Ship faster. Native app is a v2+ decision. |
| First vertical | Software SDLC | Dogfood. Expand to KOL, then Polymarket. |
| We are NOT | An AI company | We are a UX company. We don't compete on model intelligence. |

## Open Design Questions

1. **Plugin format** — Declarative YAML? JSON? Visual builder? What's the authoring experience?
2. **Provider plugin API** — How do third parties add new providers? Rust trait + dynamic loading? WASM plugins?
3. **Marketplace infrastructure** — Self-hosted or use Stripe Connect / Gumroad-style?
4. **Mobile framework** — Responsive web only? PWA? React Native later?
5. **Team config persistence** — Where does team state live across crashes? SQLite? Filesystem?
6. **Plan Runner context limits** — How to handle plan steps too large for a single session even with auto-compaction?
7. **Pricing** — What's the Pro tier price point? Anchor against Devin ($500/mo) or Cursor ($20/mo)?
