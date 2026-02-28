# Landing Page Follow-Up — L1 Launch Blocker

> Items identified during content audit (2026-03-01) that should be resolved before L1 launch.

## Status: Open

## Items

### Must fix before launch

| # | Item | File(s) | Notes |
|---|------|---------|-------|
| 1 | ~~**App Store badges link to `#`**~~ | `src/components/AppStoreBadges.astro` | **DONE** — Already non-clickable `<div>` elements with `cursor-default` and `opacity-70`. |
| 2 | ~~**Mobile setup "Sign up for early access" has no signup mechanism**~~ | `src/content/docs/docs/guides/mobile-setup.mdx` | **DONE** — Removed dead CTA, now just says "currently in development". |
| 3 | ~~**Twitter/X handle `@claude_view` may not exist**~~ | `src/layouts/MarketingLayout.astro`, `src/data/site.ts` | **DONE** — Added `TWITTER_HANDLE` to site.ts (set to `null`). Meta tag conditionally rendered. Footer Twitter link was already removed. Set to `@claude_view` when handle is verified. |

### Nice to have (not blocking)

| # | Item | File(s) | Notes |
|---|------|---------|-------|
| 4 | ~~**Self-host fonts instead of Google Fonts**~~ | `src/styles/global.css`, `public/fonts/` | **DONE** — Already self-hosted with `@font-face` declarations and 6 `.woff2` files. No Google Fonts `@import`. |
| 5 | ~~**MIT License in footer not linked**~~ | `src/components/Footer.astro` | **DONE** — Now links to `${GITHUB_URL}/blob/main/LICENSE`. |
| 6 | **Custom 404 page** | — | **Deferred** — Starlight provides a built-in 404 with search bar, dark theme, and sidebar navigation. Good enough for a developer tool. A custom marketing 404 would require overriding Starlight's NotFound component. |

### MDX dynamic values

| # | Item | File(s) | Notes |
|---|------|---------|-------|
| 7 | ~~**MCP tool count hardcoded in prose**~~ | `src/content/docs/docs/guides/mcp-integration.mdx` | **DONE** — ESM import from `site.ts`, uses `{MCP_TOOL_COUNT}` in prose. |
| 8 | **Port/version values in code blocks and tables** | `cli-options.mdx`, `installation.mdx` | **Cannot fix** — MDX can't interpolate inside markdown code blocks or tables. Source-of-truth comments guide manual updates. |

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
