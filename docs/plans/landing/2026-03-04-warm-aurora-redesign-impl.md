> **Status:** DONE (2026-03-04) — all 19 tasks implemented, shippable audit passed (SHIP IT)

## Completion Summary

Full visual theme swap from dark slate to "Warm Aurora" light theme. 19 tasks, ~23 files modified/created/deleted, 0 build errors, 19 pages.

**Deviations from plan:**
- Task 7: `ValueSection.astro` was NOT created as a separate component. Value sections were implemented inline in `index.astro` with CSS classes — simpler since each section has unique illustration markup. Plan intent fully met.
- Unused `InstallCommand` import caught and removed during shippable audit.

**Shippable audit:** Plan Compliance 19/19, Wiring Integrity PASS, Prod Hardening 0 blockers, Build 19 pages/0 errors.

**Commits:** Work is uncommitted at time of audit — all changes in working tree under `apps/landing/`.

---

# Landing Page Visual Redesign — "Warm Aurora" Theme

## Context

The landing page (`apps/landing/`) currently uses a dark slate theme (`#0f172a` bg, green `#22c55e` accents, Outfit/IBM Plex Sans fonts). Through 3 rounds of interactive mockups, a "Warm Aurora" visual direction was approved. Then a section-by-section design review confirmed the exact page structure and narrative approach.

**Goal:** Waitlist conversion. Sell the dream. Lead with wow-factor solutions (Control → Freedom → Visibility), not feature lists. Every section should drive toward the waitlist CTA.

**Visual target:** `apps/landing/theme-preview.html` — standalone HTML mockup of the Warm Aurora design.

**GTM context:** The page tells the story from `claude-view-gtm/plans/active/`: "Don't let your $200-400/mo AI plan sit idle. Leverage agents as your 24/7 workforce. Write plans from your phone, approve, debug, and launch."

## Design Decisions (confirmed section-by-section)

| Section | Decision | Notes |
|---------|----------|-------|
| **Nav** | Floating glass pill | `max-w-[1080px]`, rounded-14px, backdrop-blur, orange CTA button |
| **Hero** | Centered text + waitlist CTA (first impression) | Eyebrow badge, clamp headline, CTAs, install strip, WaitlistForm prominent |
| **Product Demo** | Browser chrome frame mock | Replaces AnimatedTerminal + DashboardPreview. Full dashboard mock with sidebar + agent rows |
| **Value Section 1** | "Your Agents, Your Rules" — Plan Runner | Copy left + plan runner mock right. Waitlist CTA after. BIGGEST wow moment |
| **Value Section 2** | "Ship From Your Phone" — Mobile | Phone mock left + copy right. Waitlist CTA after. Aspirational wow |
| **Value Section 3** | "See Everything" — Monitoring | Dashboard mock left + copy right. Reinforces product demo |
| **Secondary Features** | 4 compact glass cards | Search, AI Fluency Score, Session Sharing, 100% Local |
| **Metrics Bar** | 4 trust-building numbers | 142k sessions, <1ms startup, 15MB on disk, Zero telemetry |
| **Comparison Table** | From README | HTML table with Schema.org markup. claude-view vs opcode vs ccusage etc |
| **Install CTA** | One command section | Plugin install primary, npx secondary |
| **Pricing** | 3 tiers restyled | Light theme glass cards, orange accent CTAs |
| **FAQ** | NEW: 5 AEO/GEO questions | Schema.org FAQPage JSON-LD. "Is it free?" "Data to cloud?" etc |
| **Footer** | Light theme 4-column | Brand, Product, Resources, Legal |
| **Starlight docs** | Color token swap only | Accent: orange, bg: warm off-white, text: dark |

## Page Flow (top to bottom)

```
Nav (floating glass pill)
  ↓
Hero (centered: badge → h1 → subtitle → [Waitlist CTA] → install strip → plugin link)
  ↓
Product Demo (browser chrome frame: sidebar + stats + agent rows)
  ↓
Value 1: CONTROL — "Write a plan. Approve steps. Agents build. You sleep."
  (copy left, plan runner mock right, waitlist CTA)
  ↓
Value 2: FREEDOM — "Ship features from your phone."
  (phone mock left, copy right, waitlist CTA)
  ↓
Value 3: VISIBILITY — "Every agent. Every token. Every decision."
  (dashboard mock left, copy right)
  ↓
Secondary Features (4 compact glass cards in a row)
  ↓
Metrics Bar (4 numbers: 142k / <1ms / 15MB / Zero)
  ↓
Comparison Table (from README, Schema.org markup)
  ↓
Install CTA ("Get started in 10 seconds")
  ↓
Pricing (3 tiers, light theme)
  ↓
FAQ (5 questions, FAQPage JSON-LD)
  ↓
Footer (4-column light theme)
```

## Theme Tokens

**Accent:** `#d97757` (Anthropic orange)
**Background:** `#faf9f7` (warm off-white)
**Fonts:** Space Grotesk (headings) + Inter (body) + JetBrains Mono (code)
**Effects:** SVG grain overlay, floating aurora blobs, glass cards (backdrop-blur)

## Approach

**CSS-first theme swap + layout restructuring.** Change theme tokens first (colors/fonts in global.css), then update layout/components to match decisions above.

**CRITICAL: Port exact CSS values from `apps/landing/theme-preview.html`.** Do not reinvent styles — copy the precise `:root` vars (lines 13-31), grain texture (lines 42-46), aurora blobs (lines 48-68), nav styles (lines 70-102), glass card patterns, etc. The mockup was tested and approved.

## PRESERVED — DO NOT TOUCH

These are optimizations already in place. Changing them would be a regression.

| What | File(s) | Why |
|------|---------|-----|
| Astro config (Starlight, sidebar, sitemap, Vite) | `astro.config.mjs` | Starlight docs, sidebar auto-gen, TechArticle/BreadcrumbList JSON-LD |
| Schema.org JSON-LD | All pages/layouts | GEO: SoftwareApplication, FAQPage, BlogPosting, TechArticle, BreadcrumbList, HowTo |
| OG + Twitter meta tags | `MarketingLayout.astro` | Social sharing, SEO |
| Canonical URLs | `MarketingLayout.astro` | SEO |
| ClientRouter (View Transitions) | `MarketingLayout.astro` | Smooth page navigation |
| Speculation Rules (prefetch on hover) | `astro.config.mjs` | Page prerendering |
| `prebuild` script → `llms-full.txt` | `package.json` | AI crawler ingestion |
| `public/robots.txt`, `llms.txt`, `llms-full.txt` | `public/` | AI discoverability |
| `public/og-image.png`, `public/favicon.svg` | `public/` | Social/browser identity |
| Content collections (blog, docs, changelog) | `src/content/**` | No content file changes |
| `src/data/site.ts` constants | `src/data/site.ts` | Pricing tiers, platform info, tool counts |
| Deep link handler (`?k=...&t=...`) | `index.astro` script | QR pairing flow |
| `prefers-reduced-motion` | All animated components | WCAG accessibility |
| Skip-to-content link | `MarketingLayout.astro` | WCAG 2.4.1 (update color only) |
| Focus-visible ring | `MarketingLayout.astro` | WCAG 2.4.7 (update color only) |
| noscript fallback for `reveal-on-scroll` | `MarketingLayout.astro` | Graceful degradation |
| Turnstile integration logic | `WaitlistForm.astro` | Spam protection (change theme to `'light'` only) |
| GitHub star fetching + localStorage cache | `GitHubStars.astro` | Star count display (restyle only) |
| Zero-JS architecture | Entire site | No framework JS — keep all interactivity as vanilla `<script>` |
| Self-hosted fonts with `display=swap` | `global.css` | Performance (FOIT prevention) |
| CSS-only animations | All components | No GSAP, no Framer Motion — `@keyframes` only |

## Full Theme Replacement Checklist

Every file with old dark-theme classes (`text-slate-*`, `bg-slate-*`, `border-slate-*`, `text-green-*`, `prose-invert`, `text-white` on dark bg) gets replaced with the new Warm Aurora theme classes. This is a FULL swap — no backwards compatibility.

**New theme class conventions** (derived from theme-preview.html `:root` vars):
- Primary text: `text-[var(--color-text)]` = `#1a1a1a`
- Secondary text: `text-[var(--color-text-secondary)]` = `rgba(0,0,0,0.45)`
- Borders: `border-[var(--color-border)]` = `rgba(0,0,0,0.06)`
- Accent: `text-[var(--accent)]` / `bg-[var(--accent)]` = `#d97757`
- Accent soft: `bg-[var(--accent-soft)]` = `rgba(217,119,87,0.07)`
- Glass card: `bg-white/50 backdrop-blur-xl border border-[var(--color-border)]`
- Prose: `prose prose-slate` (NO `prose-invert`)

**Files needing class replacement:**
- `MarketingLayout.astro` — remove `class="dark"`, swap body classes
- `BlogLayout.astro` — remove `prose-invert`, swap text colors
- `blog/index.astro` — swap border/text classes
- `changelog.astro` — swap border/text/badge classes, remove `prose-invert`
- `pricing.astro` — swap `text-white` → dark text, swap secondary text
- `404.astro` — terminal frame uses explicit dark bg/border (works fine on light), update accent colors only

## Files to Modify

| File | Change |
|------|--------|
| `apps/landing/public/fonts/` | Add Space Grotesk + Inter woff2; remove Outfit + IBM Plex Sans |
| `apps/landing/src/styles/global.css` | Replace color tokens, font-faces, add grain/aurora/glass CSS |
| `apps/landing/src/layouts/MarketingLayout.astro` | Remove `class="dark"`, add grain/aurora DOM, update focus ring |
| `apps/landing/src/components/Nav.astro` | Floating glass pill nav with CTA button |
| `apps/landing/src/pages/index.astro` | Complete restructure: new hero, value sections, metrics, comparison, FAQ |
| `apps/landing/src/components/ProductDemo.astro` | **NEW** — browser chrome frame dashboard mock |
| `apps/landing/src/components/ValueSection.astro` | **NEW** — reusable alternating section with waitlist CTA |
| `apps/landing/src/components/ComparisonTable.astro` | **NEW** — comparison table from README |
| `apps/landing/src/components/FAQ.astro` | **NEW** — FAQ accordion with Schema.org FAQPage |
| `apps/landing/src/components/MetricsBar.astro` | **NEW** — 4-column trust numbers |
| `apps/landing/src/components/FeatureSection.astro` | Remove (replaced by ValueSection + inline secondary cards) |
| `apps/landing/src/components/AnimatedTerminal.astro` | Remove from hero (replaced by ProductDemo) |
| `apps/landing/src/components/DashboardPreview.astro` | Remove from hero (replaced by ProductDemo) |
| `apps/landing/src/components/PhoneMockup.astro` | Restyle for light theme, used in Value Section 2 |
| `apps/landing/src/components/InstallCommand.astro` | Glass card styling for light theme |
| `apps/landing/src/components/PricingCards.astro` | Light theme glass cards, orange accent |
| `apps/landing/src/components/WaitlistForm.astro` | Light theme, prominent in hero (first impression) |
| `apps/landing/src/components/GitHubStars.astro` | Light theme border/text |
| `apps/landing/src/components/Footer.astro` | Light theme 4-column layout |
| `apps/landing/src/styles/starlight.css` | Warm palette color tokens |

## Tasks

### Task 1: Download new font files

Download Space Grotesk (variable, 400-700) and Inter (variable, 300-700) as self-hosted woff2 files into `apps/landing/public/fonts/`. Keep existing JetBrains Mono. Remove Outfit + IBM Plex Sans after migration.

Source: Google Fonts API woff2 endpoints (same approach as existing fonts).

### Task 2: Update global.css — theme tokens + fonts + effects

Replace `global.css` entirely:

**Fonts:** Replace `@font-face` declarations:
- Outfit → Space Grotesk (headings, 400-700)
- IBM Plex Sans → Inter (body, 300-700)
- JetBrains Mono stays (code, 400-600)

**Theme tokens (`@theme` block):**
```css
--font-heading: 'Space Grotesk', 'Space Grotesk Fallback', sans-serif;
--font-body: 'Inter', 'Inter Fallback', sans-serif;
--font-mono: 'JetBrains Mono', 'JetBrains Mono Fallback', monospace;
--color-surface: rgba(255,255,255,0.6);
--color-cta: #d97757;
--color-cta-hover: #c4674a;
--color-attention: #f59e0b;
--color-accent: #d97757;
```

**CSS variables (`:root`):**
```css
--accent: #d97757;
--accent-hover: #c4674a;
--accent-soft: rgba(217,119,87,0.07);
--accent-border: rgba(217,119,87,0.14);
--accent-text: #b85a3a;
--color-bg: #faf9f7;
--color-border: rgba(0,0,0,0.06);
--color-text: #1a1a1a;
--color-text-secondary: rgba(0,0,0,0.45);
--color-text-tertiary: rgba(0,0,0,0.22);
```

**Add from theme-preview.html:**
- `.grain` — fixed SVG noise overlay (pointer-events: none, z-index: 9999)
- `.aurora-bg` / `.blob` — floating radial gradient blobs with keyframe animations
- `.glass-card` — `rgba(255,255,255,0.5)` + `backdrop-filter: blur(20px)` + border

**Update safelist** for Nav (remove dark class refs, add light theme equivalents).

### Task 3: Update MarketingLayout.astro

- Remove `class="dark"` from `<html>` tag
- `<body>` classes: `bg-[var(--color-bg)] text-[var(--color-text)]`
- Add grain + aurora DOM after `<body>` opening
- Focus ring: `#22c55e` → `var(--accent)`
- Skip link: `bg-green-600` → `bg-[var(--accent)]`

### Task 4: Update Nav.astro — floating glass pill

- Outer: `sticky top-0 z-50`, centered `max-w-[1080px] mx-auto`, `pt-4 px-4`
- Inner: glass card — `rgba(255,255,255,0.55)` + `backdrop-blur-xl` + `rounded-[14px]` + `border border-[rgba(0,0,0,0.06)]`
- Logo: Space Grotesk 600, `#1a1a1a`
- Links: `rgba(0,0,0,0.45)` → hover `#1a1a1a`
- CTA button: `bg-[var(--accent)] text-white rounded-lg px-4 py-2`
- Mobile menu: light theme glass panel
- Remove dark scroll-effect JS (already glass at rest)

### Task 5: Restructure index.astro — hero + full page

Complete rewrite of `index.astro`. New page flow:

**Hero (first impression — waitlist CTA must be prominent):**
- Remove dark gradient bg divs
- Centered layout, `max-w-[760px] mx-auto`, `pt-[100px] pb-14`
- Eyebrow badge: accent-soft bg, pulsing dot, "Now in public beta"
- H1: Space Grotesk, `clamp(36px, 5.5vw, 60px)`, `#111`
- Subtitle: 17px Inter, `rgba(0,0,0,0.45)`
- **WaitlistForm** — prominent, first impression CTA
- CTA group: primary accent button + ghost GitHub button
- Install strip: glass card `$ npx claude-view` + copy hint
- Plugin link: accent color (not green)

**After hero:** ProductDemo → 3 ValueSections → Secondary cards → MetricsBar → ComparisonTable → Install CTA → PricingCards → FAQ → Footer

### Task 6: Create ProductDemo.astro

New component (port from theme-preview.html):
- Browser chrome frame: glass card + traffic light dots + URL bar showing `localhost:47892`
- Mock dashboard: sidebar (Dashboard, Sessions, Search, Patterns, Settings) + main area
- Stats row: Active Agents (accent), Tokens Today, Sessions, Est. Cost
- Agent rows: 3 rows with status dots (green=running, amber=waiting, grey=idle)
- Responsive: sidebar hidden on mobile
- Placed between hero and first value section, `mt-16`

### Task 7: Create ValueSection.astro + 3 value sections

**New reusable component** replacing FeatureSection.astro:
- Props: `title`, `description`, `reverse` (swap illustration/copy sides), `cta` (optional waitlist CTA text)
- Full-width section with generous vertical padding
- Alternating layout: copy one side, illustration slot other side
- Optional waitlist CTA link after description
- Scroll-reveal animation (respect `prefers-reduced-motion`)

**Three instances in index.astro:**

1. **"Your Agents, Your Rules"** (copy left, illustration right)
   - Copy: "Write a plan. Approve steps. Agents build. You sleep. Don't let your $200/mo plan sit idle."
   - Illustration: Plan runner mock — glass card showing plan steps with checkmarks, approve/reject buttons
   - CTA: "Join the waitlist →"

2. **"Ship Features From Your Phone"** (illustration left, copy right) — `reverse`
   - Copy: "The couch. The train. The beach. Push notification → review → approve → shipped."
   - Illustration: PhoneMockup component (restyled light theme)
   - CTA: "Get early access →"

3. **"See Everything"** (copy left, illustration right)
   - Copy: "Every agent. Every token. Every tool call. Real-time."
   - Illustration: Agent cards mock showing costs, tokens, cache timers, sub-agent trees
   - No CTA (monitoring is already shown in product demo)

### Task 8: Secondary feature cards (inline in index.astro)

4 compact glass cards in a row below value sections:
- Full-text Search: "Search across all sessions — messages, tool calls, file paths"
- AI Fluency Score: "Track your effectiveness with AI agents over time"
- Session Sharing: "Share any session via encrypted link"
- 100% Local: "All data stays on your machine. Zero telemetry."

Glass card styling: `rgba(255,255,255,0.5)` + blur + border, accent icon, h3 + description.
Responsive: 2x2 grid on tablet, single column on mobile.

### Task 9: Create MetricsBar.astro

4-column grid of trust-building numbers:
- "142k" — Sessions monitored
- "<1ms" — Startup time
- "15MB" — On disk
- "Zero" — Telemetry

Glass cards, Space Grotesk for numbers, Inter for labels. Responsive: 2x2 on mobile.

### Task 10: Create ComparisonTable.astro

Port comparison table from README:
- HTML `<table>` with proper headers
- Tools: claude-view, opcode, ccusage, CodePilot, claude-run
- Columns: Category, Stack, Size, Live monitor, Search, Analytics
- Schema.org Table markup for GEO
- Glass card container, light borders
- Responsive: horizontal scroll on mobile

### Task 11: Update Install CTA section (inline in index.astro)

- "Get started in 10 seconds"
- Plugin install as primary: `claude plugin add @claude-view/plugin`
- npx as secondary: `npx claude-view`
- Glass card styling, accent highlight
- Node.js requirement text

### Task 12: Restyle PricingCards.astro for light theme

- Card bg: `rgba(255,255,255,0.5)` + backdrop-blur
- Borders: `rgba(0,0,0,0.06)`
- Highlight tier: `rgba(217,119,87,0.07)` bg + accent border
- Text: `#1a1a1a` headings, `rgba(0,0,0,0.45)` descriptions
- CTA buttons: accent bg primary, accent-soft secondary
- Checkmarks: accent color

### Task 13: Create FAQ.astro

New component with 5 questions (from design doc):
- "Is claude-view free?" → Yes, free forever for local use
- "Does it send data to the cloud?" → No, 100% local by default
- "What's the difference between plugin and npx?" → Plugin auto-starts + tools/skills; npx is standalone
- "Does it work with Cursor/Windsurf?" → Monitors Claude Code sessions specifically
- "How much does it cost?" → Free for local, Pro for cloud relay + mobile

Schema.org FAQPage JSON-LD in `<head>`.
Accordion-style expand/collapse (vanilla JS, no framework).
Glass card styling.

### Task 14: Restyle remaining components for light theme

**WaitlistForm.astro:**
- Input: white bg, `rgba(0,0,0,0.06)` border, dark text
- Button: accent bg, white text
- Success state: accent color
- Turnstile theme: `'light'`
- Must be PROMINENT in hero — this is the conversion target

**GitHubStars.astro:**
- Border: `rgba(0,0,0,0.06)`, text: `rgba(0,0,0,0.45)`

**PhoneMockup.astro:**
- Light frame: subtle border instead of dark gradient
- Notification badges: light glass cards
- Used in Value Section 2

**InstallCommand.astro:**
- Glass card, accent highlight on hover

### Task 15: Update Footer.astro — light theme

- Border-top: `rgba(0,0,0,0.06)`
- 4-column grid: brand + Product + Resources + Legal
- Text: `rgba(0,0,0,0.45)` links, `#1a1a1a` headings
- Copyright row with border-top

### Task 16: Fix all non-index pages (dark→light breakage)

**BlogLayout.astro:**
- Line 43: `text-slate-400` → `text-[var(--color-text-secondary)]`
- Line 49: `prose-invert prose-slate` → `prose prose-slate` (remove `prose-invert`)

**blog/index.astro:**
- Line 15: `border-slate-800` → `border-[var(--color-border)]`
- Line 16: `text-slate-500` → `text-[var(--color-text-secondary)]`
- Line 20: `text-slate-400` → `text-[var(--color-text-secondary)]`

**changelog.astro:**
- Line 21: `border-slate-800` → `border-[var(--color-border)]`
- Line 23: `bg-green-500/10 text-green-400` → `bg-[var(--accent-soft)] text-[var(--accent-text)]`
- Line 24: `text-slate-500` → `text-[var(--color-text-secondary)]`
- Line 29: `prose-invert prose-slate` → `prose prose-slate`

**pricing.astro:**
- Line 50: `text-white` → `text-[var(--color-text)]`
- Line 51: `text-slate-400` → `text-[var(--color-text-secondary)]`
- Preserve FAQPage JSON-LD exactly as-is

**404.astro — KEEP DARK:**
- Wrap the entire section content in a `<div class="dark-terminal">` with scoped dark overrides
- Or: the terminal frame already uses explicit dark classes (`bg-slate-900/80`, `border-slate-700`) — these work fine on a light bg since they're explicit, not inherited
- The radial glow `rgba(34,197,94,0.06)` → update to accent: `rgba(217,119,87,0.06)`
- Update `text-green-400` accents → `text-[var(--accent)]` for the help link only
- Keep terminal green (`text-green-400`) for the dollar sign / cursor — terminals are green

### Task 17: Update starlight.css

Current (dark):
```css
--sl-color-accent-low: #1e293b;
--sl-color-accent: #22c55e;
--sl-color-accent-high: #4ade80;
--sl-color-black: #0f172a;
--sl-font: 'IBM Plex Sans', sans-serif;
```

New (warm):
```css
--sl-color-accent-low: var(--accent-soft, rgba(217,119,87,0.07));
--sl-color-accent: #d97757;
--sl-color-accent-high: #c4674a;
--sl-color-black: #1a1a1a;
--sl-font: 'Inter', sans-serif;
--sl-font-system-mono: 'JetBrains Mono', monospace;
```

Update `[data-theme='dark']` block for warm neutrals instead of slate. Note: Starlight manages its own dark mode toggle — we update tokens, NOT the toggle behavior.

### Task 18: Clean up removed assets

- Delete `apps/landing/public/fonts/outfit-*.woff2`
- Delete `apps/landing/public/fonts/ibm-plex-sans-*.woff2`
- Delete `apps/landing/src/components/FeatureSection.astro` (replaced by ValueSection)
- Verify no references to Outfit, IBM Plex Sans, or removed components

### Task 19: Build & verify

**Build:**
- `cd apps/landing && bun run build` — 0 errors
- `cd apps/landing && bun run typecheck` — 0 errors

**Visual verify at localhost:4321:**
- Homepage: hero, product demo, 3 value sections, secondary cards, metrics, comparison, pricing, FAQ, footer
- `/pricing/` — light theme, FAQPage JSON-LD still present
- `/blog/` — light theme, post list readable
- `/blog/introducing-claude-view/` — BlogPosting JSON-LD, prose renders correctly (no white-on-white)
- `/changelog/` — version badges use accent color, prose readable
- `/docs/` — Starlight docs use warm accent, body font is Inter
- 404 — terminal stays dark, rest of page is light

**Responsive:** 375px, 768px, 1024px, 1440px — no horizontal scroll, no overflow
**Accessibility:** `prefers-reduced-motion` stops all animations, focus ring visible, skip-to-content works
**Performance:** grain/aurora don't cause jank (check `will-change`, blur)
**Functionality:** waitlist form submits, deep link handler (`?k=...&t=...`) works, GitHub star count loads
**SEO:** `curl -s localhost:4321/ | grep 'application/ld+json' | wc -l` — should show multiple schema blocks

## Non-goals

- No content refresh (separate plan: `docs/plans/2026-03-04-landing-docs-refresh-design.md`)
- No Starlight docs layout changes (just color tokens)
- No pricing content changes
- No dark mode toggle (light only)
- No new blog posts or changelog entries
