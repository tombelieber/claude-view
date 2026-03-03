# Landing Page + Docs Content Refresh — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix all content inaccuracies on the landing page and docs — update plugin references, enhance AEO/GEO/SEO metadata, update stale docs, and update README.

**Architecture:** Content-only changes across Astro landing page, Starlight docs, public metadata files, and README. No visual/layout changes. No backend changes. No new dependencies.

**Tech Stack:** Astro 5, Starlight, Tailwind CSS, Schema.org JSON-LD, Markdown/MDX

**Audit trail:**
- Design doc: `docs/plans/2026-03-04-landing-docs-refresh-design.md`
- Prove-it audit: passed (all 7 flagged issues resolved)
- Auditing-plans round 1: passed (3 findings resolved — VERSION guard, redirect verification, awk frontmatter fix)
- Adversarial review round 1: 71/100 → 8 issues found, all fixed
- Adversarial review round 2: 75/100 → 7 issues found, all fixed
- Adversarial review round 3: 82/100 → 2 issues found, all fixed (see changelog below)
- Mechanical consistency check: all 20 tasks pass (sequential numbering, commit steps, file refs)
- **Post-aurora revision (2026-03-04):** Aurora warm redesign landed on this branch, invalidating Tasks 1-3 and 15. Tasks marked DONE, Task 5 and 17 simplified. Remaining 14 tasks are clean to execute.
- Total tasks: 20 (5 DONE via aurora, 1 simplified, 14 to execute)

---

### Task 1: ~~Fix Agent Control "Coming Soon" badge~~ — DONE (aurora rewrite)

> **SKIP:** The aurora warm redesign replaced all `FeatureSection` components with a new layout. No `badge="Coming Soon"` exists anywhere in the current `index.astro`. The Agent Control section (lines 84-132) accurately describes shipped capabilities without any badge.

---

### Task 2: ~~Fix Mobile section accuracy~~ — DONE (aurora rewrite)

> **SKIP:** The aurora rewrite removed all `FeatureSection` components and "Coming Soon" badges. The mobile section (lines 134-151) has no badge and accurately describes the shipped feature.

---

### Task 3: ~~Add Conversation Sharing feature section~~ — DONE (aurora rewrite)

> **SKIP:** The aurora rewrite added a "Session Sharing" feat-card (lines 209-215): "Share any session via encrypted link. E2E encryption — zero server trust required." Sharing is present on the landing page.

---

### Task 4: Rename plugin guide file (before updating any links)

The guide file `mcp-integration.mdx` has the correct title ("Claude Code Plugin") but wrong URL slug. Rename the file FIRST — all link updates in subsequent tasks depend on this file existing at the new path.

**Why this is Task 4 (not later):** Every commit should leave the codebase in a valid state. Tasks 5+ update links to `/docs/guides/plugin/`. If the file doesn't exist at that path yet, those links are broken between commits. Rename first, update links after.

**Files:**
- Rename: `apps/landing/src/content/docs/docs/guides/mcp-integration.mdx` → `apps/landing/src/content/docs/docs/guides/plugin.mdx`
- Modify: `apps/landing/astro.config.mjs` (add redirect)

**Step 1: Rename the file**

```bash
cd apps/landing
mv src/content/docs/docs/guides/mcp-integration.mdx src/content/docs/docs/guides/plugin.mdx
```

**Step 2: Add Astro redirect from old URL**

Astro 4+ supports redirects in config. Add to `apps/landing/astro.config.mjs`:

```js
// BEFORE (line 10)
  trailingSlash: 'always',

// AFTER
  trailingSlash: 'always',
  redirects: {
    '/docs/guides/mcp-integration/': '/docs/guides/plugin/',
  },
```

This creates a proper 301 redirect. No meta refresh stub needed — Astro handles it natively.

**Why Astro `redirects` config, not meta refresh:** Meta refresh in an MDX body renders in `<body>`, not `<head>`. This is invalid HTML. AI crawlers (ClaudeBot, GPTBot, PerplexityBot) don't execute JavaScript or follow body-level meta refresh. Astro `redirects` generates a proper HTTP 301 (or a static meta redirect in `<head>` for static builds). Proven pattern: Astro docs, Starlight docs, Next.js all use config-level redirects. If Astro `redirects` doesn't work with Starlight (test in Step 3), fall back to a Cloudflare Pages `_redirects` file (`/docs/guides/mcp-integration/ /docs/guides/plugin/ 301`), which is the Cloudflare-native approach.

**Step 3: Build and verify redirect works (REQUIRED — do not skip)**

```bash
cd apps/landing && bun run build 2>&1 | tail -10
```

Expected: Build succeeds. Then verify the redirect was generated:
```bash
ls dist/docs/guides/mcp-integration/ 2>/dev/null && echo "REDIRECT EXISTS" || echo "NO REDIRECT"
```

If the output is `REDIRECT EXISTS`, the Astro config redirect is working — proceed to Step 4.

If the output is `NO REDIRECT` (Astro `redirects` config not compatible with Starlight), you MUST create a Cloudflare Pages `_redirects` fallback:
```bash
echo '/docs/guides/mcp-integration/ /docs/guides/plugin/ 301' >> apps/landing/public/_redirects
```
Then rebuild and re-verify:
```bash
cd apps/landing && bun run build 2>&1 | tail -10
```

**Do not proceed to Step 4 until one of the two redirect mechanisms is confirmed working.** External links and AI crawlers index the old URL — a broken redirect loses all accumulated link equity.

**Step 4: Commit**

```bash
git add apps/landing/
git commit -m "refactor(docs): rename plugin guide URL from mcp-integration to plugin"
```

---

### Task 5: Update hero plugin link ~~and install CTA~~ (partially done — aurora rewrite handled install CTA)

The hero links to `/docs/guides/mcp-integration/` which is the old URL slug (redirect is in place from Task 4, but direct links should point to canonical URL). ~~Also, the install CTA only shows `npx`, not the plugin.~~

> **Note (post-aurora):** The install CTA section is already done — the aurora rewrite added a two-column plugin/standalone layout (lines 233-256). Only the hero link at line 72 still needs updating.

**Files:**
- Modify: `apps/landing/src/pages/index.astro:72` (hero plugin link)

**Step 1: Update hero plugin link**

In `apps/landing/src/pages/index.astro`, find the plugin-hint paragraph (line 72):

```astro
<!-- BEFORE -->
      <a href="/docs/guides/mcp-integration/" class="accent-link">Claude Code plugin</a>
```

```astro
<!-- AFTER -->
      <a href="/docs/guides/plugin/" class="accent-link">Claude Code plugin</a>
```

**Step 2: Commit**

```bash
git add apps/landing/src/pages/index.astro
git commit -m "fix(landing): update hero plugin link to canonical /docs/guides/plugin/ URL"
```

---

### Task 6: Update pricing free tier features and add VERSION constant

Add shipped features (sharing, plugin) to the free tier feature list. Also add a VERSION constant for Schema.org (avoids hardcoded version that goes stale).

**Files:**
- Modify: `apps/landing/src/data/site.ts:41-48`

**Step 1: Update free tier features**

In `apps/landing/src/data/site.ts`, replace the free tier features array (lines 41-47):

```typescript
// BEFORE
    features: [
      'Unlimited local sessions',
      'Session browser & search',
      'Cost tracking',
      'AI Fluency Score',
      'Community support',
    ],
```

```typescript
// AFTER
    features: [
      'Unlimited local sessions',
      'Session browser & search',
      'Cost tracking & AI Fluency Score',
      'Conversation sharing',
      'Claude Code plugin (8 tools, 3 skills)',
      'Community support',
    ],
```

**Step 2: Add VERSION constant to site.ts**

At the top of the file (after `GITHUB_URL`), add:

```typescript
// ---------------------------------------------------------------------------
// Version (single source of truth for Schema.org and other metadata)
// ---------------------------------------------------------------------------

/** Must match the version in root package.json and Cargo.toml workspace. */
export const VERSION = '0.8.0'
```

This prevents the hardcoded version in Schema.org (Task 16) from going stale — all references import from here.

**⚠️ FOLLOW-UP REQUIRED (not part of this plan):** The release script (`scripts/release.sh`) must be updated to also `sed` this line when bumping versions. Without that, `VERSION` drifts after the first release. File a separate ticket: "Update release.sh to bump VERSION in apps/landing/src/data/site.ts".

**Step 3: Commit**

```bash
git add apps/landing/src/data/site.ts
git commit -m "fix(landing): add sharing and plugin to free tier, add VERSION constant"
```

---

### Task 7: Update all internal links to new plugin URL

Now that the file is renamed (Task 4) and the redirect is in place, update all internal links to point directly to the canonical URL. This avoids unnecessary redirects for users and crawlers.

**Files:**
- Modify: `apps/landing/src/content/docs/docs.mdx:35`
- Modify: `apps/landing/src/content/docs/docs/installation.mdx:38`
- Modify: `apps/landing/src/content/docs/docs/features/ai-fluency-score.mdx:33`
- Modify: `apps/landing/public/llms.txt:25`

**Step 1: Update all links**

In each file, replace `/docs/guides/mcp-integration/` with `/docs/guides/plugin/`:

1. `apps/landing/src/content/docs/docs.mdx:35`:
   ```mdx
   - [Claude Code Plugin →](/docs/guides/plugin/) — 8 tools, 3 skills, auto-start hook
   ```

2. `apps/landing/src/content/docs/docs/installation.mdx:38` — change the URL only, preserve surrounding prose:
   ```mdx
   <!-- BEFORE -->
   After installing, every Claude Code session automatically starts the dashboard. No manual `npx claude-view` needed. See the [plugin guide](/docs/guides/mcp-integration/) for details.
   ```
   ```mdx
   <!-- AFTER -->
   After installing, every Claude Code session automatically starts the dashboard. No manual `npx claude-view` needed. See the [plugin guide](/docs/guides/plugin/) for details.
   ```

3. `apps/landing/src/content/docs/docs/features/ai-fluency-score.mdx:33`:
   ```mdx
   After [installing the plugin](/docs/guides/plugin/), ask Claude Code directly:
   ```

4. `apps/landing/public/llms.txt:25`:
   ```
   - [Claude Code Plugin](/docs/guides/plugin/): Native plugin — 8 tools, 3 skills, auto-start hook
   ```

Note: `index.astro:54` is updated in Task 5. `llms-full.txt` still contains stale `mcp-integration` references — it is fully regenerated in Task 19, not fixed here.

**Step 2: Verify no remaining old links in source files (except redirect config and llms-full.txt)**

```bash
grep -rn 'mcp-integration' apps/landing/src/ apps/landing/public/llms.txt --include="*.astro" --include="*.mdx" --include="*.txt"
```

Expected: Zero results. The `astro.config.mjs` redirect is outside `src/`. `apps/landing/public/llms-full.txt` is NOT checked here — it is regenerated from scratch in Task 19 and will pick up the corrected URLs automatically.

**Step 3: Commit**

```bash
git add apps/landing/src/content/docs/ apps/landing/public/llms.txt
git commit -m "fix(docs): update all internal links to canonical /docs/guides/plugin/ URL"
```

---

### Task 8: Update Agent Control docs page

The docs page says cloud relay is required for all control. Web dashboard control works locally.

**Files:**
- Modify: `apps/landing/src/content/docs/docs/features/agent-control.mdx`

**Step 1: Rewrite agent-control.mdx**

Replace the entire file content:

```mdx
---
title: Agent Control
description: Send messages, approve tool calls, and resume Claude Code sessions from your web dashboard.
---

Agent Control lets you interact with running Claude Code sessions directly from the web dashboard — approve tool calls, send follow-up messages, review plans, or stop runaway agents.

## What you can do

- **Approve tool calls** — when an agent requests permission (e.g., `git push`), approve or reject from the dashboard
- **Send messages** — inject a follow-up message into the agent's conversation
- **Review plans** — approve or reject agent plans before implementation starts
- **Answer questions** — respond to agent clarification prompts (AskUserQuestion, Elicitation)
- **Stop agent** — gracefully stop a running session

## How it works

Agent control uses a Node.js sidecar process powered by the Claude Agent SDK. The sidecar connects to your running Claude Code sessions via IPC and exposes control actions through the claude-view web dashboard.

All control actions work **locally** — no cloud relay needed for web dashboard control.

## Remote control (mobile)

For controlling agents from your phone or a remote browser, the cloud relay is required. The relay forwards encrypted control messages between your device and the desktop server.

> **Pro plan** — Cloud relay access is included in the Pro tier (coming soon). The web dashboard is always free.

## Chat interface

The dashboard includes a full chat interface with:
- Streaming message display
- Interactive permission cards with countdown auto-deny
- Plan approval cards
- Elicitation/question response cards
- Session resume with cost estimation (ResumePreFlight)

## Related

- [Mission Control →](/docs/features/mission-control/) — live agent monitoring
- [Mobile Setup →](/docs/guides/mobile-setup/) — iOS and Android app setup
- [Claude Code Plugin →](/docs/guides/plugin/) — auto-start, 8 tools, 3 skills
```

**Step 2: Commit**

```bash
git add apps/landing/src/content/docs/docs/features/agent-control.mdx
git commit -m "fix(docs): update agent-control — local control works, relay only for remote"
```

---

### Task 9: Update Mission Control docs page

The docs page is generic and doesn't mention shipped capabilities like custom layouts, sub-agent viz, or Phase F integration.

**Files:**
- Modify: `apps/landing/src/content/docs/docs/features/mission-control.mdx`

**Step 1: Rewrite mission-control.mdx**

Replace the entire file content:

```mdx
---
title: Mission Control
description: Real-time dashboard for monitoring and controlling all active Claude Code agents.
---

Mission Control is a live view of all running Claude Code sessions. It updates in real time via WebSocket — no refresh needed.

## What you see

Each agent card shows:
- **Status** — running (green), waiting for input (amber), done (gray), error (red)
- **Current task** — the most recent tool call or user message
- **Cost** — cumulative token cost for the session
- **Duration** — time elapsed since session start
- **Context gauge** — real-time context window usage
- **Cache countdown** — time until prompt cache expires
- **IDE file** — which file the user was editing when they last messaged

## Views

Mission Control offers three layout modes:
- **Grid** — compact card grid for monitoring many agents at once
- **List** — detailed single-column view with more information per session
- **Monitor** — live chat grid showing streaming conversations side by side

You can also create **custom layouts** using drag-and-drop panels (powered by dockview). Save and load layout presets.

## Cost tracking

Token costs update live as agents work. claude-view uses current Anthropic pricing per model (with tiered pricing for large contexts). The total across all active agents is shown in the header.

## Sub-agent visualization

When a session spawns sub-agents (via the Agent tool), Mission Control shows the full agent tree — parent and children, each with their own status and cost tracking.

## Session control

From Mission Control, you can:
- **Click a session** to open the full session detail view with chat history
- **Send messages** to a running agent
- **Approve or reject** tool call permissions
- **Review plans** before the agent implements them
- **Stop a session** (with confirmation)

## Real-time updates

Mission Control connects via WebSocket to the claude-view server. Data refreshes every second. If the connection drops (network issue, server restart), it reconnects automatically with exponential backoff.

## Related

- [Agent Control →](/docs/features/agent-control/) — interactive control details
- [Cost Tracking →](/docs/features/cost-tracking/) — detailed token usage breakdown
- [Session Browser →](/docs/features/session-browser/) — historical session list
```

**Step 2: Commit**

```bash
git add apps/landing/src/content/docs/docs/features/mission-control.mdx
git commit -m "fix(docs): expand mission-control with shipped features — layouts, sub-agents, control"
```

---

### Task 10: Add Sharing docs page

Conversation sharing is shipped but has no documentation.

**Files:**
- Create: `apps/landing/src/content/docs/docs/features/sharing.mdx`

**Step 1: Create the sharing docs page**

Create `apps/landing/src/content/docs/docs/features/sharing.mdx`:

```mdx
---
title: Conversation Sharing
description: Share Claude Code sessions via encrypted links. No account needed to view.
---

Share any Claude Code session with a link. The conversation is end-to-end encrypted — the share server never sees decrypted content.

## How to share

1. Open a session in the session browser
2. Click the **Share** button in the session header
3. Choose **Copy Link** or **Copy as Markdown**
4. Send the link to anyone — no account needed to view

## How encryption works

When you share a session:
1. claude-view encrypts the conversation locally in your browser
2. The encrypted blob is uploaded to the share server (`share.claudeview.ai`)
3. The decryption key is embedded in the URL fragment (after `#`) — it never leaves your browser or reaches the server
4. Recipients open the link, download the encrypted blob, and decrypt it client-side

The share server stores only encrypted data. It cannot read your conversations.

## What's included in a shared session

- All messages (user, assistant, tool calls, tool results)
- Session metadata (project, model, duration, cost, token counts)
- Git context (branch, commit)

## Viewing shared sessions

Shared sessions render in a standalone viewer at `share.claudeview.ai` with:
- Full conversation with markdown rendering and syntax highlighting
- Chat and Debug view toggles (compact/verbose)
- Session info panel with metadata
- No login or installation required

## Deleting a shared session

Shared sessions can be deleted by the creator. Navigate to the share link and click **Delete** (requires authentication).

## Privacy

- **Encryption:** AES-256-GCM (Web Crypto API)
- **Key storage:** Key is in the URL fragment only — never sent to the server
- **Data retention:** Shared sessions expire after 30 days
- **No tracking:** The share viewer has no analytics or tracking

## Related

- [Session Browser →](/docs/features/session-browser/) — browse and search all sessions
- [Agent Control →](/docs/features/agent-control/) — interact with running agents
```

**Step 2: Commit**

```bash
git add apps/landing/src/content/docs/docs/features/sharing.mdx
git commit -m "feat(docs): add conversation sharing documentation"
```

---

### Task 11: Update Mobile Setup docs

The mobile docs say "currently in development" and list everything as "coming soon". M1 is shipped.

**Files:**
- Modify: `apps/landing/src/content/docs/docs/guides/mobile-setup.mdx`

**Step 1: Rewrite mobile-setup.mdx**

Replace the entire file content:

```mdx
---
title: Mobile App Setup
description: Set up the claude-view mobile app on iOS or Android — monitor agents, get push notifications.
---
import WaitlistForm from '../../../../components/WaitlistForm.astro';

The claude-view mobile app lets you monitor your Claude Code agents from your phone with push notifications for agent events.

> **Status:** The mobile app is in beta. QR pairing and monitoring work. Agent control (approve/reject from phone) is coming in the next release.

<WaitlistForm variant="compact" />

## What works today (M1)

- **Session dashboard** — see all running and recent sessions, grouped by project
- **Session detail** — view messages, tool calls, and costs for any session
- **Push notifications** — get alerted when agents finish, error, or need input
- **QR code pairing** — scan once from your desktop to connect
- **End-to-end encryption** — all data in transit encrypted with NaCl (libsodium)

## Coming next (M2)

- Approve/reject tool calls from your phone
- Send messages to running agents
- Spawn new agents from mobile

## Prerequisites

- claude-view server running on your desktop (`npx claude-view`)
- iOS 16+ or Android 12+
- Cloud relay connection (Pro tier) for push notifications

## Pairing flow

1. Install the claude-view app (TestFlight beta or direct APK)
2. Open claude-view on desktop and click **Pair Mobile**
3. A QR code appears — scan it with the mobile app
4. Your phone connects securely via the cloud relay
5. Push notifications activate automatically via OneSignal

## Notifications

The mobile app receives push notifications when:
- An agent requests tool approval
- A long-running session completes
- An agent encounters an error

Notification delivery requires the cloud relay (Pro tier).
```

**Step 2: Commit**

```bash
git add apps/landing/src/content/docs/docs/guides/mobile-setup.mdx
git commit -m "fix(docs): update mobile-setup — M1 shipped, M2 coming next"
```

---

### Task 12: Update Getting Started docs — fix Agent Control text and add sharing

The plugin link is already updated in Task 7. This task fixes the stale "(with cloud relay)" text on Agent Control (Task 8 corrected the docs page — this page must match) and adds the missing sharing feature.

**Files:**
- Modify: `apps/landing/src/content/docs/docs.mdx:22-23`

**Step 1: Fix Agent Control description and add sharing to the feature list**

In `apps/landing/src/content/docs/docs.mdx`, replace line 22:

```mdx
<!-- BEFORE -->
- **Agent Control** — approve/reject tool calls, send messages (with cloud relay)
```

```mdx
<!-- AFTER -->
- **Agent Control** — approve/reject tool calls, send messages from your browser
- **Conversation sharing** — share any session via encrypted link
```

**Step 2: Commit**

```bash
git add apps/landing/src/content/docs/docs.mdx
git commit -m "fix(docs): add sharing to getting-started feature list"
```

---

### Task 13: Update blog post mobile status

The blog post says "The mobile app (coming soon)" but M1 monitoring is now shipped. This was identified in the prove-it audit as a dropped requirement from the design doc.

**Files:**
- Modify: `apps/landing/src/content/blog/introducing-claude-view.mdx:40-42`

**Step 1: Update the mobile section**

Replace lines 40-42:

```mdx
<!-- BEFORE -->
## The mobile app (coming soon)

The next milestone is the mobile app — native iOS and Android. Monitor your agents from your phone, receive push notifications when agents need approval, and ship features from anywhere.
```

```mdx
<!-- AFTER -->
## The mobile app

The mobile app monitors your agents from your phone with push notifications — QR pairing, session dashboard, and real-time alerts when agents need input. Agent control (approve/reject from phone) is coming next.
```

**Step 2: Commit**

```bash
git add apps/landing/src/content/blog/introducing-claude-view.mdx
git commit -m "fix(blog): update mobile section — M1 monitoring shipped"
```

---

### Task 14: Enhance llms.txt for AEO

The current `llms.txt` is sparse. AI models need structured facts, not just doc links.

**Files:**
- Modify: `apps/landing/public/llms.txt`

**Step 1: Rewrite llms.txt**

Replace the entire file:

```
# claude-view

> Mission Control for AI coding agents — monitor, control, and share your Claude Code sessions.

## What it does

claude-view is an open-source developer tool that:
- Monitors all active Claude Code sessions in real time (dashboard, cost tracking, token usage)
- Controls running agents from a web dashboard (approve/reject tool calls, send messages, review plans)
- Shares any session via encrypted link (end-to-end encrypted, no account needed to view)
- Tracks AI coding effectiveness via AI Fluency Score (0-100 metric)
- Provides full-text search across all session history (powered by Tantivy)
- Ships as a Claude Code plugin with 8 MCP tools and 3 skills

## Install

Two ways to install:

1. Claude Code plugin (recommended): `claude plugin add @claude-view/plugin`
   - Auto-starts the dashboard every session
   - Adds 8 read-only MCP tools (list/search/get sessions, costs, scores)
   - Adds 3 skills (/session-recap, /daily-cost, /standup)

2. Standalone: `npx claude-view`
   - Downloads a ~15MB Rust binary
   - Opens dashboard at http://localhost:47892
   - Zero config, no account needed

## Key facts

- License: MIT
- Language: Rust backend, React frontend
- Size: ~15MB download, ~27MB on disk
- Platform: macOS (Apple Silicon + Intel). Linux planned.
- Data: 100% local by default. No telemetry. Cloud relay optional (Pro tier).
- Sharing: End-to-end encrypted. Share server cannot read conversations.
- Port: 47892 (configurable via CLAUDE_VIEW_PORT)
- Search: Full-text via Tantivy engine, <50ms for thousands of sessions

## Docs

- [Getting Started](/docs/): Install and first run
- [Installation](/docs/installation/): Detailed setup, ports, and config
- [Mission Control](/docs/features/mission-control/): Live agent monitoring with custom layouts
- [Agent Control](/docs/features/agent-control/): Send messages, approve tools, review plans
- [Conversation Sharing](/docs/features/sharing/): Share sessions via encrypted links
- [Cost Tracking](/docs/features/cost-tracking/): Token usage and model costs
- [AI Fluency Score](/docs/features/ai-fluency-score/): Measure AI coding effectiveness
- [Search](/docs/features/search/): Full-text search across sessions
- [Session Browser](/docs/features/session-browser/): Browse and filter all sessions
- [Claude Code Plugin](/docs/guides/plugin/): Auto-start, 8 tools, 3 skills
- [Mobile App](/docs/guides/mobile-setup/): iOS/Android monitoring (beta)
- [CLI Reference](/docs/reference/cli-options/): Command line options
- [API Reference](/docs/reference/api/): HTTP API endpoints
- [Keyboard Shortcuts](/docs/reference/keyboard-shortcuts/): Hotkeys and navigation

## Optional

- [Blog](/blog/): Release announcements
- [Changelog](/changelog/): Version history
- [Pricing](/pricing/): Free forever for local use. Pro for cloud relay.
```

**Step 2: Commit**

```bash
git add apps/landing/public/llms.txt
git commit -m "feat(landing): enhance llms.txt for AEO — structured capabilities and facts"
```

---

### Task 15: ~~Add FAQ section with server-rendered Schema.org FAQPage~~ — DONE (aurora rewrite)

> **SKIP:** The aurora rewrite added a `<FAQ />` component (`apps/landing/src/components/FAQ.astro`) that already implements:
> - All 5 FAQ questions from this plan
> - Server-rendered FAQPage JSON-LD via `<script is:inline type="application/ld+json" set:html={faqJsonLd} />`
> - Accordion `<details>` pattern
> - Rendered in `index.astro` at line 269 via `<FAQ />`
>
> The FAQ data lives in the component itself rather than `site.ts`, which is fine. The AEO goal is achieved.

---

### Task 16: Enhance Schema.org SoftwareApplication

The current Schema.org markup is minimal. Add feature list, version, and download URL. Uses `VERSION` constant from `site.ts` (added in Task 6) to avoid hardcoded version that goes stale on release.

**Dependency:** Task 6 must be completed first (defines `VERSION` in `site.ts`).

**Files:**
- Modify: `apps/landing/src/layouts/MarketingLayout.astro:5` (add VERSION import)
- Modify: `apps/landing/src/layouts/MarketingLayout.astro:73-81` (expand JSON-LD)

**Prerequisite: Verify Task 6 completed (VERSION exists)**

```bash
grep -n 'export const VERSION' apps/landing/src/data/site.ts
```

Expected: One match (e.g. `17:export const VERSION = '0.8.0'`). If missing, complete Task 6 first — the import in Step 1 will cause a build error without it.

**Step 1: Add VERSION to the import**

In `apps/landing/src/layouts/MarketingLayout.astro`, update the import at line 5:

```astro
<!-- BEFORE -->
import { SITE_URL, PLATFORM, TWITTER_HANDLE } from '../data/site';
```

```astro
<!-- AFTER -->
import { SITE_URL, PLATFORM, TWITTER_HANDLE, VERSION } from '../data/site';
```

**Step 2: Expand the SoftwareApplication JSON-LD**

Replace the Schema.org SoftwareApplication block (lines 73-81):

```astro
<!-- BEFORE -->
  <script is:inline type="application/ld+json" set:html={JSON.stringify({
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    "name": "claude-view",
    "applicationCategory": "DeveloperApplication",
    "operatingSystem": PLATFORM.current,
    "description": "Mission Control for AI coding agents",
    "offers": { "@type": "Offer", "price": "0" }
  })} />
```

```astro
<!-- AFTER -->
  <script is:inline type="application/ld+json" set:html={JSON.stringify({
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    "name": "claude-view",
    "applicationCategory": "DeveloperApplication",
    "operatingSystem": PLATFORM.current,
    "description": "Mission Control for AI coding agents — monitor, control, and share Claude Code sessions in real time.",
    "url": SITE_URL,
    "downloadUrl": "https://www.npmjs.com/package/claude-view",
    "installUrl": "https://www.npmjs.com/package/claude-view",
    "softwareVersion": VERSION,
    "license": "https://opensource.org/licenses/MIT",
    "featureList": "Real-time agent monitoring, Agent control (approve/reject/message), Conversation sharing (E2E encrypted), AI Fluency Score, Full-text search, Claude Code plugin (8 MCP tools, 3 skills), Cost tracking, Sub-agent visualization, Custom dashboard layouts, Push notifications (mobile)",
    "offers": { "@type": "Offer", "price": "0", "priceCurrency": "USD" }
  })} />
```

**Step 3: Commit**

```bash
git add apps/landing/src/layouts/MarketingLayout.astro
git commit -m "feat(landing): expand Schema.org SoftwareApplication — use VERSION constant, add features"
```

---

### Task 17: ~~Add comparison table to landing page~~ — Partially done (aurora rewrite added table, needs 2 columns)

The `ComparisonTable.astro` component already exists (added by aurora rewrite) and is rendered in `index.astro` at line 230. However, it is **missing two columns**: Agent Control and Sharing — both shipped features that differentiate claude-view.

**Files:**
- Modify: `apps/landing/src/components/ComparisonTable.astro`

**Step 1: Add `control` and `sharing` fields to tool data and table columns**

In `apps/landing/src/components/ComparisonTable.astro`, update the tools array (lines 6-12) to add `control` and `sharing` boolean fields:

```astro
<!-- BEFORE -->
const tools = [
  { name: 'claude-view', category: 'Full platform', stack: 'Rust + React', size: '15 MB', live: true, search: true, analytics: true },
  { name: 'opcode', category: 'Viewer', stack: 'TypeScript', size: '~5 MB', live: false, search: false, analytics: false },
  { name: 'ccusage', category: 'Cost tracker', stack: 'TypeScript', size: '~3 MB', live: false, search: false, analytics: false },
  { name: 'CodePilot', category: 'Dashboard', stack: 'Python', size: '~20 MB', live: false, search: false, analytics: true },
  { name: 'claude-run', category: 'Runner', stack: 'TypeScript', size: '~2 MB', live: false, search: false, analytics: false },
];
```

```astro
<!-- AFTER -->
const tools = [
  { name: 'claude-view', category: 'Full platform', stack: 'Rust + React', size: '15 MB', live: true, search: true, analytics: true, control: true, sharing: true },
  { name: 'opcode', category: 'Viewer', stack: 'TypeScript', size: '~5 MB', live: false, search: false, analytics: false, control: false, sharing: false },
  { name: 'ccusage', category: 'Cost tracker', stack: 'TypeScript', size: '~3 MB', live: false, search: false, analytics: false, control: false, sharing: false },
  { name: 'CodePilot', category: 'Dashboard', stack: 'Python', size: '~20 MB', live: false, search: false, analytics: true, control: false, sharing: false },
  { name: 'claude-run', category: 'Runner', stack: 'TypeScript', size: '~2 MB', live: false, search: false, analytics: false, control: false, sharing: false },
];
```

Then add the two new `<th>` columns in `<thead>` (after Analytics):

```html
<th>Agent Control</th>
<th>Sharing</th>
```

And add the corresponding `<td>` cells in the `tools.map` loop (after the analytics cell):

```astro
<td>{tool.control ? '✓' : '—'}</td>
<td>{tool.sharing ? '✓' : '—'}</td>
```

**Step 2: Commit**

```bash
git add apps/landing/src/components/ComparisonTable.astro
git commit -m "feat(landing): add Agent Control and Sharing columns to comparison table"
```

---

> **Deleted from original plan:** The inline `<table>` HTML in `index.astro` below is no longer needed — the component approach is cleaner. Keeping the original for reference but it should NOT be executed.

<details>
<summary>Original Task 17 (dead code — do not execute)</summary>

The original plan added a raw HTML table directly in `index.astro`. The aurora rewrite replaced this with a `ComparisonTable.astro` component. The inline HTML below references stale dark-theme classes (`slate-700`, `slate-800`, `green-500/5`) that don't match the aurora warm design.

(Original inline HTML table omitted — replaced by ComparisonTable.astro component approach above)

</details>

---

### Task 18: Update README — add plugin, sharing, plugin workspace

The README is missing plugin install, sharing feature, and plugin workspace entry.

**Files:**
- Modify: `README.md`

**Step 1: Add plugin install to Installation section**

In `README.md`, after the install methods table (around line 246, after the `| **Git clone** | ... |` row), add a new paragraph before the "Only requirement" line:

```markdown
**Claude Code plugin (recommended):**

```bash
claude plugin add @claude-view/plugin
```

Auto-starts the dashboard every session, adds 8 MCP tools (session/cost/fluency queries), and 3 skills (`/session-recap`, `/daily-cost`, `/standup`).
```

**Step 2: Add sharing to "What You Get" section**

In `README.md`, after the "Advanced Search" section (around line 78), add:

```markdown
### Conversation Sharing

| Feature | Why it matters |
|---------|---------------|
| **Share via link** | One click to generate an encrypted share link — send to anyone |
| **End-to-end encrypted** | Decryption key stays in the URL fragment, never reaches the server |
| **No account needed** | Recipients view the full conversation without logging in |
| **Rich viewer** | Shared sessions render with full markdown, code highlighting, and metadata |
```

**Step 3: Add plugin to Workspace Layout table**

In `README.md`, in the Workspace Layout table (around line 298-302), add after the `packages/shared/` row (to group all `packages/` entries together):

```markdown
| `packages/plugin/` | `@claude-view/plugin` | Claude Code plugin (MCP tools, skills, auto-start hook) |
```

**Step 4: Commit**

```bash
git add README.md
git commit -m "docs(readme): add plugin install, sharing feature, plugin workspace"
```

---

### Task 19: Update llms-full.txt generation script to strip frontmatter and imports

The `llms-full.txt` is for AI model consumption. Raw YAML frontmatter (`---\ntitle:...\n---`) and MDX import statements (`import { Steps } from '...'`) are noise that AI models don't need.

**IMPORTANT:** A generation script already exists at `apps/landing/scripts/generate-llms-full.mjs` and is called via the `prebuild` hook (`"prebuild": "node scripts/generate-og-image.mjs && node scripts/generate-llms-full.mjs"`). This means `llms-full.txt` is regenerated on every `bun run build` (which triggers the `prebuild` hook). **Note:** `npx astro build` does NOT trigger lifecycle hooks — always use `bun run build`. Do NOT use a manual bash/awk loop — it would be overwritten by the next build. Update the generation script itself.

**Files:**
- Modify: `apps/landing/scripts/generate-llms-full.mjs`

**Step 1: Update the generation script to strip frontmatter and imports**

In `apps/landing/scripts/generate-llms-full.mjs`, replace lines 32-36:

```js
// BEFORE
for (const file of files) {
  const content = await readFile(file, 'utf-8')
  const relative = file.replace(`${contentDir}/`, '')
  sections.push(`--- ${relative} ---\n\n${content}`)
}
```

```js
// AFTER
/**
 * Strip YAML frontmatter (first --- to second ---) and MDX import lines.
 * Preserves body --- (horizontal rules) by tracking whether frontmatter was already stripped.
 * All Starlight content files require frontmatter, so the first --- is always a frontmatter delimiter.
 */
function stripFrontmatterAndImports(raw) {
  const lines = raw.split('\n')
  const out = []
  let inFront = false
  let doneFront = false
  for (const line of lines) {
    if (line === '---' && !inFront && !doneFront) {
      inFront = true
      continue
    }
    if (line === '---' && inFront) {
      inFront = false
      doneFront = true
      continue
    }
    if (inFront) continue
    if (/^import .+ from /.test(line)) continue
    out.push(line)
  }
  return out.join('\n').trim()
}

for (const file of files) {
  const content = await readFile(file, 'utf-8')
  const relative = file.replace(`${contentDir}/`, '')
  const cleaned = stripFrontmatterAndImports(content)
  sections.push(`--- ${relative} ---\n\n${cleaned}`)
}
```

**Step 2: Rebuild to regenerate llms-full.txt via prebuild hook**

```bash
cd apps/landing && bun run build 2>&1 | tail -5
```

The `prebuild` hook runs `generate-llms-full.mjs` automatically before the Astro build.

**Step 3: Verify clean output**

```bash
# First 20 lines should show section header + clean prose (no --- or import lines)
head -20 apps/landing/public/llms-full.txt
# Verify no import statements leaked through
grep -c "^import .* from" apps/landing/public/llms-full.txt
```

Expected: Heading line `--- docs/filename.mdx ---` followed by clean prose. Zero import statements.

**Step 4: Verify no stale mcp-integration references remain**

```bash
grep -n "mcp-integration" apps/landing/public/llms-full.txt
```

Expected: Zero results (the old file was renamed in Task 4).

**Step 5: Commit**

```bash
git add apps/landing/scripts/generate-llms-full.mjs apps/landing/public/llms-full.txt
git commit -m "chore(landing): update llms-full.txt generator — strip frontmatter and imports for clean AI consumption"
```

---

### Task 20: Build verification and final review

Verify everything builds and no links are broken.

**Step 1: Build the landing site**

```bash
cd apps/landing && bun run build 2>&1 | tail -20
```

Expected: Build succeeds with no errors.

**Step 2: Check for broken internal links**

```bash
grep -rn 'href="/docs/guides/mcp-integration/"' apps/landing/src/ --include="*.astro" --include="*.mdx"
```

Expected: Zero results. All `href` references to the old URL were updated in Tasks 5 and 7. The only remaining `mcp-integration` reference is in `astro.config.mjs` (redirect config, outside `src/`).

**Step 3: Verify no "Coming Soon" or "Beta" badges on landing page**

```bash
grep -rn 'Coming Soon\|badge=' apps/landing/src/pages/index.astro
```

Expected: Zero results. All badges have been removed — Agent Control badge removed (shipped), Mobile badge removed (no public build available).

**Step 4: Run linting**

```bash
cd apps/landing && npx astro check 2>&1 | tail -10
```

Expected: No type errors.

**Step 5: Commit any fixes**

If any issues found, fix and commit:

```bash
git add -A apps/landing/
git commit -m "fix(landing): resolve build issues from content refresh"
```

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Round | Issue | Severity | Fix Applied |
|---|-------|-------|----------|-------------|
| 1 | prove-it | FAQPage JSON-LD client-side injection invisible to AI crawlers | Blocker | Switched to server-rendered `set:html` pattern (Task 15) |
| 2 | prove-it | Meta refresh redirect in MDX body is invalid HTML | Blocker | Changed to Astro `redirects` config with `_redirects` fallback (Task 4) |
| 3 | prove-it | Task ordering — links updated before file rename | Blocker | Reordered: rename = Task 4, links = Tasks 5+7 |
| 4 | prove-it | Mobile "Beta" badge when no public build exists | Warning | Removed badge entirely (Task 2) |
| 5 | prove-it | Hardcoded version "0.8.0" in Schema.org | Warning | Added VERSION constant to site.ts (Task 6), imported in layout (Task 16) |
| 6 | prove-it | Blog post update requirement dropped | Warning | Added Task 13 for blog post update |
| 7 | prove-it | Comparison table staleness | Minor | Added "Data as of March 2026" disclaimer (Task 17) |
| 8 | audit-r1 | Task 16 missing VERSION guard step | Warning | Added Step 0 with grep verification (Task 16) |
| 9 | audit-r1 | Redirect fallback not explicitly required | Warning | Made Step 3 explicitly required with pass/fail gates (Task 4) |
| 10 | audit-r1 | awk script strips body `---` (horizontal rules) | Minor | Added `done_front` flag to awk script (Task 19) |
| 11 | adversarial | Task 7 grep misses `llms-full.txt` — stale refs undetected | Blocker | Updated grep scope note; `llms-full.txt` handled by Task 19 regeneration |
| 12 | adversarial | VERSION constant has no release script integration | Blocker | Added explicit follow-up ticket requirement in Task 6 |
| 13 | adversarial | Task 15 FAQ JSON-LD body vs head placement misleading | Warning | Added NOTE clarifying body placement is intentional, matches pricing.astro |
| 14 | adversarial | Plugin npm publication not verified | Warning | Added prerequisite check in Task 5 |
| 15 | adversarial | Task 18 workspace table insertion point ambiguous | Warning | Changed to "after `packages/shared/`" for correct grouping |
| 16 | adversarial | Task 20 "redirect stub file" wording misleading | Warning | Clarified expected output: zero results |
| 17 | adversarial | Task 15 duplicate import — standalone line creates build error | Minor | Changed to "append to existing destructured import" with BEFORE/AFTER |
| 18 | adversarial | awk script no-frontmatter edge case | Minor | Added assumption comment (all Starlight content has frontmatter) |
| 19 | adversarial-r2 | `generate-llms-full.mjs` prebuild hook overwrites manual awk output | Critical | Rewrote Task 19 to patch the JS script instead of manual bash loop |
| 20 | adversarial-r2 | `installation.mdx:38` BEFORE snippet incomplete — executor may delete prose | Critical | Added full BEFORE/AFTER with surrounding sentence context |
| 21 | adversarial-r2 | `docs.mdx` Agent Control insertion point off by 1 line | Minor | Fixed Task 12 to reference line 22 (actual line) |
| 22 | adversarial-r2 | `faqJsonLd` const has no placement anchor in frontmatter | Important | Added explicit anchor: "AFTER last import, BEFORE closing `---`" |
| 23 | adversarial-r2 | `claude plugin add` CLI subcommand validity not gated | Important | Added second prerequisite check in Task 5 |
| 24 | adversarial-r2 | README plugin insertion point ambiguous relative to table | Important | Changed to "after install methods table, before 'Only requirement'" |
| 25 | adversarial-r3 | `npx astro build` doesn't trigger `prebuild` hook — llms-full.txt never regenerated | Critical | Replaced all `npx astro build` with `bun run build` across Tasks 4, 15, 19, 20 |
| 26 | adversarial-r3 | `docs.mdx:22` still says "(with cloud relay)" — contradicts Task 8 correction | Important | Task 12 now also updates Agent Control text to "from your browser" |

## Follow-up Tickets (not part of this plan)

1. **Update `scripts/release.sh` to bump VERSION in `apps/landing/src/data/site.ts`** — without this, the VERSION constant drifts after the first release
2. **Publish `@claude-view/plugin` to npm** — Task 5 adds it as primary CTA; the prerequisite check gates this
