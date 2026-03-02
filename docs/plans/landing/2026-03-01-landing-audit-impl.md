# Landing Page Pre-Deploy Audit — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a two-phase pre-deploy audit (`bun run audit`) that gates landing page deploys on content quality (Claude CLI) and performance regression (LHCI).

**Architecture:** A single Bun/TypeScript orchestrator script runs Phase 1 (build, extract HTML, Claude CLI content audit) then Phase 2 (LHCI Lighthouse CI against preview server). Both phases must pass for exit code 0.

**Tech Stack:** Bun (script runner), Claude CLI (`claude -p`), LHCI (`@lhci/cli` via npx), Astro (build + preview)

**Design doc:** `docs/plans/landing/2026-03-01-landing-audit-design.md`

---

### Task 1: Create the LHCI configuration

**Files:**
- Create: `apps/landing/.lighthouserc.json`

**Step 1: Write the LHCI config**

```json
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

**Step 2: Add `.lighthouseci` to gitignore**

Create `apps/landing/.gitignore` (it does not currently exist):

```
.lighthouseci/
```

Also append to the root `.gitignore` to prevent accidental commits of raw Lighthouse report dumps:

```
# Lighthouse raw report dumps (LHCI archives go in apps/landing/lighthouse/)
/lighthouse*.json
```

**Step 3: Verify LHCI runs standalone**

Run from `apps/landing/`:

```bash
cd apps/landing && bun run build && npx @lhci/cli autorun
```

Expected: Lighthouse runs 3x per URL, assertions printed, exit 0 if all pass.

**Step 4: Commit**

```bash
git add apps/landing/.lighthouserc.json apps/landing/.gitignore .gitignore
git commit -m "feat(landing): add LHCI config for performance regression gating"
```

---

### Task 2: Create the audit prompt template

**Files:**
- Create: `scripts/audit-landing-prompt.md`

**Step 1: Write the prompt template**

This is the instructions file that gets fed to `claude -p`. It uses `{{SITE_TS}}`, `{{PAGES}}`, and `{{CHANGED_GROWTH_PAGES}}` as template variables that the orchestrator script substitutes (via `.replaceAll()`, not a template engine).

````markdown
You are a content auditor for claudeview.ai, a developer tool landing page.

## Ground Truth

The following is the canonical data source (`site.ts`). All rendered content must match these values exactly:

<site-ts>
{{SITE_TS}}
</site-ts>

## Core Pages (always audited)

{{PAGES}}

## Changed Growth Pages (blog/changelog — only recently modified)

{{CHANGED_GROWTH_PAGES}}

## Audit Checklist

Evaluate ALL pages against these 8 categories. For each category, output one line:
- PASS — no issues found
- WARN — minor issue, not blocking (include details)
- FAIL — must fix before deploy (include details with page path and exact mismatch)

### 1. Data Accuracy
Every number from site.ts (MCP_TOOL_COUNT=8, PLUGIN_SKILL_COUNT=3, pricing tiers, DEFAULT_PORT=47892, PLATFORM values) must match what's rendered. Check both visible text AND JSON-LD structured data.

### 2. Cross-Reference Alignment
Values from site.ts that appear on pages must match. Flag any hardcoded values that duplicate site.ts data (they'll drift).

### 3. Stale/Placeholder Content
Flag: "Lorem ipsum", "TODO", "TBD", "[placeholder]", empty sections, "Coming soon" without intentional comingSoon: true flag.

### 4. Dead Links
Flag: href="#", href="javascript:void(0)", href="" on non-comingSoon elements, links to obviously non-existent routes.

### 5. Marketing Claims
Feature descriptions must match what's documented. Version numbers must be current. Claims like "real-time" must be substantiated by docs.

### 6. Tone Consistency
Professional, developer-focused. No conflicting voice across pages. CTAs clear and actionable.

### 7. Traditional SEO
For each page verify:
- meta description present and unique
- canonical link present with absolute URL
- og:type, og:title, og:description, og:image present
- og:image is absolute URL (starts with https://)
- Blog posts have article:published_time

### 8. GEO & Agent Discoverability

#### 8a. Agent Discoverability Files
- llms.txt: Verify H1 name, blockquote summary, H2 sections. All doc links should correspond to real pages.
- robots.txt: Must have explicit Allow rules for ClaudeBot, GPTBot, PerplexityBot. Sitemap URL must be correct.

#### 8b. Schema.org JSON-LD (extract from script type application/ld+json blocks)
For each page, validate the JSON-LD:
- Homepage/marketing: SoftwareApplication with name, applicationCategory, operatingSystem, offers matching site.ts pricing
- /docs/ pages: TechArticle with headline
- /pricing/: FAQPage with questions matching the rendered FAQ content
- /blog/ posts: BlogPosting with headline, datePublished, author
- /docs/installation/: HowTo with steps matching install instructions
- All pages: BreadcrumbList matching URL hierarchy
- Every JSON-LD block must be valid JSON with @context and @type

#### 8c. Structural
- Every page has exactly one h1
- Heading hierarchy: no skipping levels (h1 to h3 without h2)
- Semantic landmarks: nav, main, footer present

## Three-Way Cross-Reference

For every data value, check consistency across:
1. site.ts (ground truth)
2. JSON-LD structured data
3. Rendered visible text

Any disagreement between these three is a FAIL.

## Output Format

Output EXACTLY this format (one line per category, then summary):

```
1. Data Accuracy: [PASS | WARN | FAIL] — [details]
2. Cross-References: [PASS | WARN | FAIL] — [details]
3. Stale Content: [PASS | WARN | FAIL] — [details]
4. Dead Links: [PASS | WARN | FAIL] — [details]
5. Marketing Claims: [PASS | WARN | FAIL] — [details]
6. Tone: [PASS | WARN | FAIL] — [details]
7. SEO: [PASS | WARN | FAIL] — [details]
8. GEO & Agent: [PASS | WARN | FAIL] — [details]

RESULT: [PASS | FAIL] ([N] failures, [N] warnings)
```
````

**Step 2: Commit**

```bash
git add scripts/audit-landing-prompt.md
git commit -m "feat(landing): add audit prompt template for content/GEO validation"
```

---

### Task 3: Create the orchestrator script

**Files:**
- Create: `scripts/audit-landing.ts`

**Step 1: Write the orchestrator**

The script uses `Bun.spawnSync` / `Bun.spawn` for all child process calls (Bun-native, no shell injection risk). It reads files via `Bun.file().text()` where possible.

Key design notes for the implementer:
- Use `Bun.spawnSync()` for all subprocess calls (security: no shell expansion)
- Use `Bun.file(path).text()` for reading files
- The script runs from the repo root: `bun run scripts/audit-landing.ts`
- Template variables `{{SITE_TS}}`, `{{PAGES}}`, `{{CHANGED_GROWTH_PAGES}}` are string-replaced via `.replaceAll()` before passing to Claude
- CLI flags: `--content-only` (skip LHCI), `--perf-only` (skip Claude content audit)
- The `claude` CLI is invoked as: `claude -p --output-format text --model sonnet "<prompt>"` (matches the proven pattern in `crates/core/src/llm/claude_cli.rs`)
- LHCI is invoked as: `npx @lhci/cli autorun` from `apps/landing/`
- **Note:** This is the first TypeScript script in `scripts/` (existing convention is shell scripts). Bun runs `.ts` natively.

Script structure:

```
scripts/audit-landing.ts

1. Parse CLI flags (--content-only, --perf-only)
2. Phase 1 (if not --perf-only):
   a. Build: Bun.spawnSync(['bun', 'run', 'build'], { cwd: LANDING })
   b. Collect core HTML files from dist/ (recursive walk)
   c. For each HTML file:
      - extractText(): strip <script>, <style>, tags → visible text
      - extractJsonLd(): regex extract <script type="application/ld+json"> blocks
      - Label with URL path (dist/pricing/index.html → /pricing/)
   d. Also read dist/llms.txt, dist/robots.txt for GEO checks
   e. Determine diff baseline: git tag --sort=-creatordate | grep landing-v | head -1
   f. If tag exists: git diff --name-only <tag>..HEAD -- apps/landing/src/content/blog/ apps/landing/src/content/changelog/
      - Map changed source paths to dist HTML paths
   g. If no tag: include ALL blog/changelog pages
   h. Read prompt template, substitute {{SITE_TS}}, {{PAGES}}, {{CHANGED_GROWTH_PAGES}}
   i. Pre-flight: verify `claude` binary exists (Bun.spawnSync(['which', 'claude'])) — if missing, print "Claude CLI required. Install: npm i -g @anthropic-ai/claude-code" and exit(1)
   j. Run: Bun.spawnSync(['claude', '-p', '--output-format', 'text', '--model', 'sonnet', prompt])
   k. Print output, check for "RESULT: FAIL" → contentPass = false
3. Phase 2 (if not --content-only):
   a. Ensure dist/ exists (build if needed)
   b. Run: Bun.spawnSync(['npx', '@lhci/cli', 'autorun'], { cwd: LANDING })
   c. Check exit code → perfPass = false if non-zero
   d. Archive: copy .lighthouseci/manifest.json → lighthouse/<timestamp>.json
      (timestamp format: `new Date().toISOString().replace(/:/g, '').replace(/\..+/, '')` → e.g. `2026-03-01T120000.json`)
4. Print summary, exit(0) if both pass, exit(1) otherwise
```

Helper functions to implement:

```typescript
/** Recursively find HTML files in a directory. */
function findHtml(dir: string): string[]

/** Strip HTML tags, scripts, styles — extract visible text. */
function extractText(html: string): string

/** Extract JSON-LD blocks from HTML. */
function extractJsonLd(html: string): string[]

/** Map dist path to URL path: dist/pricing/index.html → /pricing/ */
function toUrlPath(filePath: string): string
```

`extractText` implementation:
1. Remove `<script>...</script>` blocks (greedy, case-insensitive)
2. Remove `<style>...</style>` blocks
3. Replace HTML tags with space
4. Decode common entities (`&nbsp;`, `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&#39;`)
5. Collapse whitespace, trim

`extractJsonLd` implementation:
1. Regex: `/<script\s+type=["']application\/ld\+json["'][^>]*>([\s\S]*?)<\/script>/gi`
2. Return array of captured group contents

**Step 2: Make executable**

```bash
chmod +x scripts/audit-landing.ts
```

**Step 3: Verify the script runs**

```bash
bun run scripts/audit-landing.ts --perf-only
```

Expected: Builds, runs LHCI, prints scores.

**Step 4: Commit**

```bash
git add scripts/audit-landing.ts
git commit -m "feat(landing): add pre-deploy audit orchestrator (content + LHCI)"
```

---

### Task 4: Wire into package.json scripts

**Files:**
- Modify: `apps/landing/package.json`

**Step 1: Add audit and deploy scripts**

Add to the `scripts` block in `apps/landing/package.json`:

```json
"audit": "bun run ../../scripts/audit-landing.ts",
"audit:content": "bun run ../../scripts/audit-landing.ts --content-only",
"audit:perf": "bun run ../../scripts/audit-landing.ts --perf-only",
"deploy": "bun run audit && wrangler pages deploy dist",
"deploy:force": "wrangler pages deploy dist"
```

**Replace** the existing `"deploy"` script with the gated version (audit-then-deploy). The raw wrangler command moves to `deploy:force` as an explicit escape hatch. This matches the design doc: `bun run deploy` is safe-by-default, `deploy:force` is the conscious opt-out.

**Step 2: Verify the scripts work**

```bash
cd apps/landing && bun run audit:perf
```

Expected: Builds the site, runs LHCI, shows Lighthouse scores for 4 URLs.

**Step 3: Commit**

```bash
git add apps/landing/package.json
git commit -m "feat(landing): wire audit scripts into package.json"
```

---

### Task 5: Create the lighthouse archive directory

**Files:**
- Create: `apps/landing/lighthouse/.gitkeep`
- Modify: `apps/landing/.gitignore`

**Step 1: Create the archive directory**

```bash
mkdir -p apps/landing/lighthouse
touch apps/landing/lighthouse/.gitkeep
```

**Step 2: Ensure .lighthouseci is gitignored but lighthouse/ is tracked**

Verify `apps/landing/.gitignore` has `.lighthouseci/` but does NOT have `lighthouse/`.

**Step 3: Commit**

```bash
git add apps/landing/lighthouse/.gitkeep apps/landing/.gitignore
git commit -m "feat(landing): add lighthouse archive dir for regression tracking"
```

---

### Task 6: Verify deploy tag baseline works

**Files:**
- Read only: `scripts/release-landing.sh`

**Step 1: Verify existing tags serve as diff baseline**

The existing release script already creates `landing-v{VERSION}` tags (line 28 of `scripts/release-landing.sh`). The audit script uses these tags for diff detection. No additional `landing-deployed-*` tags needed.

```bash
git tag --sort=-creatordate | grep '^landing-v' | head -3
```

Expected: Shows existing landing version tags (if any). If none exist, the audit script falls back to auditing all content.

**Step 2: No commit needed (no changes)**

---

### Task 7: End-to-end test

**Step 1: Run the full audit**

```bash
cd apps/landing && bun run audit
```

Expected output structure:

```
Landing Page Pre-Deploy Audit
============================================================
[CONTENT] Starting content audit...
[CONTENT] Building landing page...
[CONTENT] Found N core pages
[CONTENT] No previous deploy tag found — auditing ALL growth content
[CONTENT] Running Claude CLI audit...

============================================================
CONTENT AUDIT RESULTS
============================================================
1. Data Accuracy: PASS — ...
...
8. GEO & Agent: PASS — ...

RESULT: PASS (0 failures, 0 warnings)

[PERF] Starting Lighthouse CI audit...
...Lighthouse scores...

============================================================
PERFORMANCE AUDIT RESULTS
============================================================
All Lighthouse assertions passed

============================================================
FINAL RESULT
============================================================
Content audit: PASS
Performance audit: PASS

All audits passed — safe to deploy
```

**Step 2: Verify exit codes**

```bash
bun run audit && echo "EXIT 0" || echo "EXIT 1"
```

**Step 3: Test individual phases and escape hatch**

```bash
bun run audit:content  # Content only (faster, no Lighthouse)
bun run audit:perf     # LHCI only (no Claude API cost)
```

Verify the `deploy:force` escape hatch resolves correctly (don't actually deploy — just check the script exists):

```bash
bun run --silent deploy:force --help 2>&1 | head -1  # Should show wrangler help, not "Script not found"
```

**Step 4: Commit any fixes discovered during testing**

```bash
git add -A
git commit -m "fix(landing): address issues found during audit end-to-end test"
```

---

### Task 8: Update docs

**Files:**
- Modify: `apps/landing/README.md`

**Step 1: Add Audit section to README**

Add after the "Dev Commands" section:

```markdown
## Pre-Deploy Audit

| Command | What |
|---------|------|
| `bun run audit` | Full audit: content (Claude CLI) + performance (LHCI) |
| `bun run audit:content` | Content audit only (8-category checklist, ~$0.04/run; first run ~$0.10–0.20) |
| `bun run audit:perf` | Lighthouse CI only (4 URLs x 3 runs, desktop preset) |
| `bun run deploy` | Audit then deploy (blocks deploy on failure) |
| `bun run deploy:force` | Deploy without audit (escape hatch) |

The content audit validates 8 categories: data accuracy, cross-references, stale content, dead links, marketing claims, tone, SEO, and GEO/agent discoverability. It performs a three-way cross-reference between `site.ts`, JSON-LD structured data, and rendered HTML.

LHCI thresholds: Performance >= 95, Accessibility >= 95, Best Practices >= 95, SEO >= 95, CLS < 0.1.

Historical Lighthouse results are archived in `lighthouse/` for regression tracking.
```

**Step 2: Commit**

```bash
git add apps/landing/README.md
git commit -m "docs(landing): add pre-deploy audit section to README"
```

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
| --- | ----- | -------- | ----------- |
| B1 | `changelog/` git diff path wrong — silently matches nothing | Blocker | Changed to `apps/landing/src/content/changelog/` (full repo-root-relative path) |
| B2 | Deploy script keeps raw `deploy` as default, `deploy:safe` as gated — defeats audit gate | Blocker | Replaced `deploy` with gated version, moved raw to `deploy:force`. Matches design doc. |
| B3 | Design doc creates `landing-deployed-*` tags, impl uses `landing-v*` — orchestrator would never find deployed tags | Blocker | Removed from design doc. Impl plan's use of existing `landing-v*` tags is correct. |
| W1 | `claude --print --model sonnet -p` mixes long/short flags, missing `--output-format text` | Warning | Changed to `claude -p --output-format text --model sonnet` (matches `crates/core/src/llm/claude_cli.rs`) |
| W2 | `startServerCommand: "npx astro preview"` bypasses pinned local astro | Warning | Changed to `"bun run preview"` (uses `package.json` script with local `astro@^5`) |
| W3 | No pre-flight check for `claude` binary — cryptic ENOENT on missing | Warning | Added `which claude` pre-flight step before Phase 1 content audit |
| W4 | No `apps/landing/.gitignore` exists; root `.gitignore` missing `lighthouse*.json` | Warning | Create `apps/landing/.gitignore` explicitly; add `/lighthouse*.json` to root `.gitignore` |
| W5 | `interactive` (TTI) threshold 2000ms too tight — existing lighthouse.json shows 1905ms | Warning | Raised to `maxNumericValue: 3000` (TTI is weight=0 in Lighthouse 13+, warn-level only) |
| M1 | Archive timestamp format unspecified | Minor | Added format spec: `new Date().toISOString().replace(/:/g, '').replace(/\..+/, '')` |
| M2 | Design doc prose says `audit:landing`, impl says `audit` | Minor | Aligned both to `audit` |
| M3 | Cost estimate `~$0.04` doesn't caveat first-run cost | Minor | Added "per steady-state run" with first-run caveat ($0.10–0.20) |
| M4 | First TypeScript script in `scripts/` — new convention | Minor | Added note in design notes acknowledging this |
