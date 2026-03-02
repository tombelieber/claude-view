# Landing Page Follow-Up — L1 Launch Blocker

> Items identified during content audit (2026-03-01) that should be resolved before L1 launch.

## Status: In Progress — L1 Blocker

## Items

### Must fix before launch

| # | Item | File(s) | Notes |
|---|------|---------|-------|
| 1 | **App Store badges not functional** | `src/components/AppStoreBadges.astro` | **BLOCKED** — Badges rendered as non-clickable `<div>` with `opacity-70`. Real fix: link to actual App Store / Play Store listings once mobile app is published, or remove badges entirely until then. |
| 2 | ~~**Mobile setup "Sign up for early access" has no signup mechanism**~~ | `src/content/docs/docs/guides/mobile-setup.mdx` | **DONE** — Referral waitlist with Cloudflare Turnstile. Form in homepage hero + mobile-setup docs. Pricing cards link to waitlist. Supabase `waitlist` table with referral tracking. |
| 3 | **Twitter/X handle not created** | `src/layouts/MarketingLayout.astro`, `src/data/site.ts` | **BLOCKED** — `TWITTER_HANDLE` set to `null` (meta tag hidden). Real fix: create `@claude_view` on X, then set `TWITTER_HANDLE = '@claude_view'` in `site.ts`. |

### Nice to have (not blocking)

| # | Item | File(s) | Notes |
|---|------|---------|-------|
| 4 | ~~**Self-host fonts instead of Google Fonts**~~ | `src/styles/global.css`, `public/fonts/` | **DONE** — Already self-hosted with `@font-face` declarations and 6 `.woff2` files. No Google Fonts `@import`. |
| 5 | ~~**MIT License in footer not linked**~~ | `src/components/Footer.astro` | **DONE** — Now links to `${GITHUB_URL}/blob/main/LICENSE`. |
| 6 | ~~**Custom 404 page**~~ | `src/pages/404.astro`, `astro.config.mjs` | **DONE** — Branded terminal-style 404 page using MarketingLayout. Shows attempted path, suggested links as `$ cd /` commands, blinking cursor. Starlight's built-in 404 disabled via `disable404Route: true`. |

### MDX dynamic values

| # | Item | File(s) | Notes |
|---|------|---------|-------|
| 7 | ~~**MCP tool count hardcoded in prose**~~ | `src/content/docs/docs/guides/mcp-integration.mdx` | **DONE** — ESM import from `site.ts`, uses `{MCP_TOOL_COUNT}` in prose. |
| 8 | ~~**Port/version values in code blocks and tables**~~ | `cli-options.mdx`, `installation.mdx` | **DONE** — Replaced GFM tables with HTML `<table>` elements for JSX interpolation. Inline `<code>` tags for port in prose. ESM imports of `DEFAULT_PORT` and `PLATFORM` from `site.ts`. JSON-LD uses template literals. Only fenced code blocks with custom examples (e.g. `CLAUDE_VIEW_PORT=8080`) remain hardcoded by design. |

## Resolved (2026-03-01)

These were fixed across two sessions during the content audit:

- GitHub URL (anthropics → tombelieber) — all 5 files
- CLI Options page — fabricated flags removed, rewritten
- API Reference — corrected to match actual Rust routes
- Homepage/pricing "Linux" claim — changed to "macOS only"
- og:image — absolute URL
- AI Fluency MCP syntax — fake CLI command removed
- robots.txt sitemap URL — `sitemap-index.xml`
- Fluency chart bars inverted — fixed with `items-end` + ascending heights
- InstallCommand centering — `inline-flex` + `justify-center` + `mx-auto`
- llms.txt trailing slashes — all links
- Installation description — "macOS" only
- MarketingLayout operatingSystem schema — "macOS" only
- AnimatedTerminal hardcoded version — now reads from `Cargo.toml` at build time
- `site.ts` centralized constants — all mutable values in one file
- MIT License footer — now a clickable link
- Twitter meta tag — conditional on `TWITTER_HANDLE` in site.ts
- Mobile setup dead CTA — removed "Sign up for early access"
- MCP tool count — ESM import from site.ts in MDX prose
- Fonts — already self-hosted, no Google Fonts dependency
- MCP docs — updated to Claude Code plugin paradigm with "Coming Soon" notice
- Custom 404 — terminal-style branded page with MarketingLayout, `disable404Route: true` in Starlight
- MDX dynamic values — HTML tables + `<code>` tags for JSX interpolation, ESM imports from `site.ts`
