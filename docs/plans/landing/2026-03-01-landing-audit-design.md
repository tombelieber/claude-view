# Landing Page Pre-Deploy Audit — Design Doc

> **Status:** Approved
> **Date:** 2026-03-01
> **Scope:** Pre-deploy audit for `apps/landing/` — content quality (Claude CLI) + performance regression (LHCI)

---

## Problem

The landing page has 5 optimization layers (Agent/LLM discoverability, GEO, SEO, performance, accessibility) with structured data, cross-referenced values, and marketing copy that can drift out of sync. Manual review is error-prone and doesn't scale as content grows.

## Solution

A two-phase pre-deploy audit (`bun run audit`):

1. **Phase 1 — Content audit:** Build the site, extract rendered HTML, feed to `claude -p` for 8-category content/GEO validation
2. **Phase 2 — Performance audit:** Serve the built site, run Lighthouse CI (LHCI) against it, assert Core Web Vitals thresholds, track regression trend

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Trigger** | Manual pre-deploy script | No friction in local dev, cost-controlled, runs when it matters |
| **Content engine** | `claude -p` (Claude CLI) | Reuses existing CLI + API key, no new dependencies. **Note:** LLM-based content auditing is a novel pattern without established industry precedent — if unreliable, can be replaced with static assertions while keeping LHCI unchanged. |
| **Performance engine** | Lighthouse CI (`@lhci/cli`) | Industry standard (Google, Vercel, Netlify use it). 3x median runs, config-based assertions, optional dashboard + GitHub status checks. No rework path. |
| **Content source** | Rendered HTML from `dist/` | Catches rendering bugs, validates JSON-LD as emitted, audits what users see |
| **Scope strategy** | Tiered — always audit core pages, diff-only for blog/changelog | Scales as blog grows without auditing 500 unchanged posts |
| **Diff baseline** | Git tags `landing-v*` | Already created by `scripts/release-landing.sh` |

## Architecture

### Script Flow

```
bun run audit
  │
  ├─ Phase 1: CONTENT AUDIT
  │  │
  │  ├─ 1. Build site: `astro build` → dist/
  │  │
  │  ├─ 2. Collect content:
  │  │     ├─ Core pages (always): dist/index.html, dist/pricing/index.html,
  │  │     │   dist/docs/**/index.html (~12 pages)
  │  │     ├─ Growth pages (diff only): git diff since last `landing-v*` tag
  │  │     │   → only changed blog/changelog HTML
  │  │     └─ Ground truth: read apps/landing/src/data/site.ts raw
  │  │
  │  ├─ 3. Extract content:
  │  │     ├─ Strip <script>/<style> tags, collapse whitespace
  │  │     ├─ Separately extract JSON-LD blocks from <script type="application/ld+json">
  │  │     └─ Label each page with its URL path
  │  │
  │  ├─ 4. Construct prompt:
  │  │     ├─ System: structured audit instructions
  │  │     ├─ site.ts data as ground truth
  │  │     ├─ Each page's extracted text + JSON-LD
  │  │     └─ 8-category checklist (see below)
  │  │
  │  ├─ 5. Pre-flight: verify `claude` binary exists — if missing, exit with install instructions
  │  ├─ 6. Run: claude -p --output-format text --model sonnet "<prompt>"
  │  │
  │  └─ 7. Output: terminal pass/warn/fail report
  │
  └─ Phase 2: PERFORMANCE AUDIT (LHCI)
     │
     ├─ 8. Start preview server: `bun run preview` (serves dist/ on :4321)
     │
     ├─ 9. Run: `lhci autorun` (reads .lighthouserc.json)
     │     ├─ Runs Lighthouse 3x per URL (median = stable scores)
     │     ├─ URLs: /, /pricing/, /docs/, /blog/
     │     ├─ Asserts thresholds: Performance ≥ 95, A11y ≥ 95, SEO ≥ 95, CLS < 0.1
     │     └─ Saves JSON results to .lighthouseci/
     │
     ├─ 10. Archive: copy results to lighthouse/ dir (git-tracked for trend)
     │
     ├─ 11. Kill preview server
     │
     └─ 12. Output:
           ├─ Terminal: LHCI assertion results (pass/fail per metric)
           ├─ Exit code: 0 if both phases pass, 1 if either fails
           └─ JSON in lighthouse/ for historical regression tracking
```

### Tiered Content Strategy

| Tier | Content | Audit frequency | Rationale |
|------|---------|----------------|-----------|
| **Core** | Homepage, pricing, all docs (~12 pages), site.ts | Every audit | Brand promise, small + stable |
| **Growth** | Blog posts, changelog entries | Only when changed since last deploy | Grows unbounded, old posts don't drift |

### Three-Way Cross-Reference

Every data value is validated across three surfaces:

```
site.ts (ground truth) ←→ JSON-LD schema (structured data) ←→ rendered HTML (what users see)
```

If any of the three disagree, that's a finding.

## Audit Checklist (8 Categories)

### 1. Data Accuracy

Every number in `site.ts` (MCP_TOOL_COUNT, PLUGIN_SKILL_COUNT, pricing tiers, DEFAULT_PORT, PLATFORM) must match what's rendered on pages that reference them.

### 2. Cross-Reference Alignment

Values from `site.ts` that appear on pages must match exactly. No hardcoded duplicates that could drift.

### 3. Stale/Placeholder Content

Detect "Lorem ipsum", "TODO", "TBD", "Coming soon" (without intent), empty sections, `[placeholder]` markers.

### 4. Dead Links

URLs pointing to `#`, `javascript:void(0)`, placeholder domains, or non-existent internal routes.

### 5. Marketing Claims

Feature descriptions must match docs. Claims like "real-time" must be substantiated. Version numbers must be current.

### 6. Tone Consistency

Professional, developer-focused voice. No conflicting tone between pages. CTAs are clear and actionable.

### 7. Traditional SEO

- Every page has a unique `<meta name="description">`
- Every page has `<link rel="canonical">` with absolute URL
- `og:type`, `og:title`, `og:description`, `og:image` present on every page
- `og:image` uses absolute URL (not relative), 1200x630
- Blog posts have `article:published_time` and `article:author`
- Twitter cards present (conditional on `TWITTER_HANDLE`)
- `sitemap-index.xml` referenced in `robots.txt`

### 8. GEO & Agent Discoverability

#### Layer 1: Agent/LLM Discoverability

| Check | Validation |
|-------|-----------|
| **llms.txt** | H1 name, blockquote summary, H2 sections present. All doc links resolve to real pages in sitemap. |
| **llms-full.txt** | Every MDX/MD file in `src/content/` has a corresponding section. No stale entries for deleted pages. |
| **robots.txt** | `ClaudeBot`, `GPTBot`, `PerplexityBot` all have explicit `Allow: /` rules. Sitemap URL correct. |

#### Layer 2: Schema.org JSON-LD

| Schema Type | Page(s) | Required Fields |
|-------------|---------|----------------|
| **SoftwareApplication** | All marketing pages | `name`, `applicationCategory`, `operatingSystem`, `offers` (pricing matches `site.ts`) |
| **TechArticle** | All `/docs/**` pages | `headline`, `isPartOf` (links to SoftwareApplication) |
| **FAQPage** | `/pricing/` | Questions + answers match rendered FAQ content on page |
| **BlogPosting** | Each `/blog/*` post | `headline`, `datePublished`, `author`, `publisher` match rendered content |
| **HowTo** | `/docs/installation/` | `step[]` entries match actual install instructions |
| **BreadcrumbList** | All pages | Hierarchy matches actual page URL structure |

#### Layer 3: Structural Validation

| Check | Validation |
|-------|-----------|
| **Heading hierarchy** | Every page has exactly one `<h1>`, proper H1→H2→H3 nesting (no level skips) |
| **Semantic landmarks** | `<nav>`, `<main>`, `<footer>`, `<article>` properly nested |
| **JSON-LD validity** | Every `<script type="application/ld+json">` block is valid JSON with `@context` and `@type` |

## Output Format

```
Landing Page Content Audit
==========================

✅ PASS  Data accuracy — all site.ts values match rendered content
✅ PASS  Cross-references — no hardcoded values diverge from site.ts
⚠️ WARN  Placeholder — pricing.astro "Contact us" CTA has no link
❌ FAIL  Marketing mismatch — homepage claims "10 MCP tools", site.ts says MCP_TOOL_COUNT=8
❌ FAIL  GEO — FAQPage schema on /pricing/ has 4 questions but page renders 5
✅ PASS  llms.txt — all doc links resolve, structure valid
✅ PASS  robots.txt — AI crawler rules present (ClaudeBot, GPTBot, PerplexityBot)
✅ PASS  Schema.org — all 6 types present with correct data

Summary: 2 failures, 1 warning, 5 passes
```

Exit code: `0` if no failures, `1` if any failures exist. Warnings don't block.

## Deploy Integration

```jsonc
// apps/landing/package.json
{
  "scripts": {
    "audit": "bun run ../../scripts/audit-landing.ts",
    "deploy": "bun run audit && wrangler pages deploy dist",
    "deploy:force": "wrangler pages deploy dist"
  }
}
```

The existing `scripts/release-landing.sh` already creates `landing-v{VERSION}` tags. The audit script uses these as the diff baseline — no additional tagging needed.

## Error Handling

| Scenario | Behavior |
|----------|----------|
| `astro build` fails | Exit with build error (deploy blocked anyway) |
| `claude` CLI not found | Error: "Claude CLI required. Install: npm i -g @anthropic-ai/claude-code" |
| Claude API timeout | Retry once, then exit with warning |
| No deploy tag found | Audit all content (core + all growth), warn "first audit — checking everything" |
| Audit finds failures | Exit code 1, deploy blocked |
| Audit passes | Exit code 0, deploy proceeds |

## File Structure

```
scripts/
  audit-landing.ts              # Main audit script (Bun/TypeScript)
  audit-landing-prompt.md       # Audit prompt template (separate for easy iteration)
apps/landing/
  .lighthouserc.json            # LHCI config (URLs, assertions, storage)
  lighthouse/                   # Git-tracked LHCI result archives (regression trend)
    .gitkeep
```

Prompt template is a separate file so audit instructions can evolve without touching script logic.

## LHCI Configuration

```jsonc
// apps/landing/.lighthouserc.json
{
  "ci": {
    "collect": {
      "startServerCommand": "bun run preview",
      "startServerReadyPattern": "localhost",
      "startServerReadyTimeout": 15000,
      "url": [
        "http://localhost:4321/",
        "http://localhost:4321/pricing/",
        "http://localhost:4321/docs/",
        "http://localhost:4321/blog/"
      ],
      "numberOfRuns": 3,
      "settings": {
        "preset": "desktop"
      }
    },
    "assert": {
      "assertions": {
        "categories:performance": ["error", { "minScore": 0.95 }],
        "categories:accessibility": ["error", { "minScore": 0.95 }],
        "categories:best-practices": ["error", { "minScore": 0.95 }],
        "categories:seo": ["error", { "minScore": 0.95 }],
        "cumulative-layout-shift": ["error", { "maxNumericValue": 0.1 }],
        "largest-contentful-paint": ["warn", { "maxNumericValue": 1500 }],
        "interactive": ["warn", { "maxNumericValue": 3000 }]
      }
    },
    "upload": {
      "target": "filesystem",
      "outputDir": ".lighthouseci"
    }
  }
}
```

The `desktop` preset matches the target audience (developers on desktop). Mobile testing can be added later as a second config.

## Regression Tracking

LHCI stores results in `.lighthouseci/` (gitignored). The audit script copies the manifest + summary JSON to `lighthouse/` (git-tracked) after each run:

```
lighthouse/
  2026-03-01T120000.json    # LHCI manifest with scores
  2026-03-02T143000.json    # Next run
  ...
```

To view trend: `cat lighthouse/*.json | jq '.[] | {url, performance, accessibility}'`

Future upgrade: `lhci server start` for a visual dashboard.

## Cost Estimate

### Phase 1 (Content audit)

- ~15 core pages × ~500 tokens = ~7,500 tokens content
- JSON-LD + site.ts + prompt: ~4,500 tokens
- Total: ~12,000 input tokens → ~$0.04 (Sonnet pricing)

### Phase 2 (LHCI)

- Zero API cost. Runs locally via Chromium.
- Time: ~30-45s (3 runs × 4 URLs, ~3s each)

### Combined

~$0.04 + ~45s local compute per steady-state audit run. First run (no baseline tag = all content audited) may cost ~$0.10–0.20.

---

## Changelog of Fixes Applied (Audit → Final Design)

| # | Issue | Fix Applied |
| --- | ----- | ----------- |
| B3 | `landing-deployed-*` post-deploy tagging contradicts impl plan's `landing-v*` tags | Removed `landing-deployed-*` tag creation. Existing `landing-v*` tags from `release-landing.sh` are the correct diff baseline. |
| M2 | Prose says `bun run audit:landing`, impl says `bun run audit` | Aligned to `audit` throughout |
| W1 | `claude --print` references (prose, decisions table, architecture diagram) | Changed to `claude -p` with `--output-format text` throughout |
| W2 | `startServerCommand` and architecture diagram said `astro preview` / `npx astro preview` | Changed LHCI config to `"bun run preview"`, updated architecture diagram step 8 to match. Added `startServerReadyTimeout: 15000` to match impl plan. |
| W3 | No pre-flight check for `claude` binary | Added step 5 in architecture diagram: verify `claude` binary exists before invocation |
| W4 | Root `.gitignore` missing `lighthouse*.json` entries | Noted in impl plan (design doc defers file-level details to impl plan) |
| W5 | `interactive` TTI threshold 2000ms too tight for actual 1905ms measurement | Raised to `maxNumericValue: 3000` |
| M3 | Cost estimate doesn't caveat first-run cost | Added first-run caveat ($0.10–0.20) |
| C1 | LLM content audit is a novel pattern without industry precedent | Added explicit acknowledgment in decisions table with fallback strategy |
