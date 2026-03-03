# Landing Page + Docs Content Refresh — Design

**Status:** Design approved (revised post-aurora 2026-03-04)
**Date:** 2026-03-04
**Scope:** Content correctness, docs accuracy, AEO/GEO/SEO metadata. No visual/layout changes (separate effort).
**Branch:** `worktree-monorepo-expo`

> **Post-aurora note:** The warm aurora redesign landed on this branch, completing sections 1.1-1.3 (hero, features, sharing) and 3.3 (FAQ). The impl plan has been revised to skip these. See `2026-03-04-landing-docs-refresh-impl.md` for task-level status.

---

## Context

This branch has 199 commits ahead of main. Major features shipped:
- **Plugin model** (`@claude-view/plugin`) — replaces standalone MCP package
- **Agent Control (Phase F)** — chat, permissions, plan approval from web dashboard
- **Conversation Sharing** — encrypted share links via `share.claudeview.ai`
- **Mobile M1** — Expo app with QR pairing, push notifications, session dashboard (monitor only)
- **Mission Control A-F** — all phases done, including custom layouts

The landing page and docs were written before some of these shipped and contain stale "Coming Soon" badges, missing features, and outdated architecture references.

---

## Section 1: Landing Page Content Fixes

### 1.1 Hero Section (`index.astro`)
- Add plugin install command alongside `npx claude-view`
- Update plugin link from `/docs/guides/mcp-integration/` to `/docs/guides/plugin/`

### 1.2 Feature Sections (`index.astro`)
- **Agent Control:** Remove `badge="Coming Soon"`. Rewrite description to reflect shipped capabilities (web dashboard control). Clarify: mobile control is M2, not shipped.
- **Mobile:** Keep `Coming Soon` badge but update description: M1 monitoring is live (QR pairing, push, session dashboard). Control coming next.
- **Add new section: Conversation Sharing** — "Share any session with a link. End-to-end encrypted." Link to share docs.

### 1.3 Install CTA Section (`index.astro`)
- Show plugin install as primary path: `claude plugin add @claude-view/plugin`
- Show `npx claude-view` as secondary/standalone path

### 1.4 Pricing (`site.ts`)
- Add "Conversation sharing" and "Claude Code plugin" to free tier features list
- Verify all feature lists are accurate

### 1.5 Plugin Data (`site.ts`)
- Verify MCP_TOOLS and PLUGIN_SKILLS arrays match actual plugin capabilities
- No changes expected — these were recently updated

---

## Section 2: Docs Accuracy (Starlight)

### 2.1 Rename `guides/mcp-integration.mdx` → `guides/plugin.mdx`
- It's a plugin, not an "MCP integration" guide
- Update frontmatter title/description
- Add Astro redirect from `/docs/guides/mcp-integration/` → `/docs/guides/plugin/`
- Update all internal links that reference the old URL

### 2.2 Update `features/agent-control.mdx`
- Remove/clarify "Prerequisites: cloud relay required" — web dashboard control works locally
- Cloud relay is only needed for mobile/remote access
- Add details about shipped capabilities: chat, permission dialog, plan approval, elicitation

### 2.3 Update `features/mission-control.mdx`
- Add shipped capabilities: custom layouts (dockview), 3 presets, layout save/load
- Update session control section with Phase F capabilities
- Reference sub-agent visualization, hook events, action log

### 2.4 Update `installation.mdx`
- Add plugin install path as recommended method
- Keep `npx claude-view` as alternative
- Verify Node.js version requirement is current

### 2.5 Add `features/sharing.mdx`
- New doc page for conversation sharing feature
- Cover: how to share, encryption, share links, viewing shared conversations

### 2.6 Grep for stale MCP references
- Search all docs for references to standalone MCP package
- Replace with `@claude-view/plugin` where appropriate
- Ensure `packages/mcp/` is described as internal/bundled, not user-facing

### 2.7 Blog post accuracy
- Check `introducing-claude-view.mdx` for stale information
- Update if needed to reflect current feature set

---

## Section 3: AEO/GEO/SEO Metadata

### 3.1 `llms.txt` Enhancement
- Add structured capabilities section: what claude-view can do, what it can't
- Add installation methods with one-line descriptions
- Add feature list with factual capability statements (not marketing copy)
- Target: AI models should be able to answer "what does claude-view do?" from this file alone

### 3.2 Schema.org Enhancement (`MarketingLayout.astro`)
- Expand `SoftwareApplication` with: `featureList`, `softwareVersion`, `downloadUrl`
- Add `FAQPage` JSON-LD on index page

### 3.3 FAQ Section (`index.astro`)
Target questions for both human visitors and AI citation:
- "Is claude-view free?" → Yes, free forever for local use
- "Does it send data to the cloud?" → No, 100% local by default
- "What's the difference between the plugin and npx?" → Plugin auto-starts + adds tools/skills; npx is standalone
- "Does it work with Cursor/Windsurf?" → It monitors Claude Code sessions specifically
- "How much does it cost?" → Free for local, Pro for cloud relay + mobile

### 3.4 Comparison Table on Landing Page
- Port the comparison table from README to a section on `index.astro`
- Proper HTML table with Schema.org `Table` or `ItemList` markup
- Include: claude-view, opcode, ccusage, CodePilot, claude-run

---

## Section 4: README Updates

### 4.1 Add Plugin Section
- Add `@claude-view/plugin` to "What You Get" section
- Brief description: auto-start, 8 tools, 3 skills

### 4.2 Update Installation Table
- Add plugin as recommended install alongside npx
- `claude plugin add @claude-view/plugin`

### 4.3 Workspace Layout Table
- Add `packages/plugin/` entry

### 4.4 Add Conversation Sharing
- Mention sharing as a feature in "What You Get"
- Brief description: share via encrypted link

---

## Non-goals (don't do these)

- Visual/layout redesign (separate effort already underway)
- New blog posts or changelog entries
- Pricing page restructure
- i18n README updates (EN only this pass)
- Mobile app store listing content
- Video embed or demo recording
