# Landing Page & Docs Site — Design Document

> **Date:** 2026-02-28
> **Status:** Done (implemented 2026-03-01)
> **Scope:** Replace `apps/landing/` placeholder with Astro + Starlight site on Cloudflare Pages

---

## Problem

The current landing page is a single `index.html` with inline CSS — a deep-link handler and placeholder. For L1 launch ("I shipped a feature from my phone"), claude-view needs:

1. A marketing landing page that converts visitors
2. Comprehensive documentation that AI agents can parse and cite
3. A blog and changelog for ongoing content
4. Agent/LLM discoverability infrastructure (the 2026 growth flywheel)

## Strategy: Supabase + Cursor Hybrid

**Model:** Supabase for growth mechanics (docs as competitive moat), Cursor for landing page (product-first hero), Resend for design taste (dark, code-centric, minimal).

**Growth flywheel:**

```
Good docs → organic search → users try claude-view → GitHub stars →
blog posts/tutorials mention it → training data → AI familiarity →
agent recommendations → more users → more docs
```

The landing page gets the first click. The docs build long-term organic discovery. Cloudflare Markdown for Agents and llms.txt are supplementary layers — not the primary mechanism.

> **Honest note:** No developer tool has demonstrably grown via AI agent recommendations driven by Markdown for Agents (launched Feb 12, 2026 — 16 days before this plan). The real AI recommendation pathway is training data from authoritative sources (GitHub, HN, blog posts, Stack Overflow). We invest in agent discoverability as low-cost future-proofing.

---

## Tech Stack

| Component | Choice | Why |
|-----------|--------|-----|
| **Framework** | Astro 5 (SSG) | Content-first, zero JS by default, zero-config Cloudflare Pages |
| **Docs engine** | Starlight | Built-in search/sidebar/i18n, Content Collections, MDX |
| **Styling** | Tailwind CSS 4 | Already used in `apps/web`, consistent design language |
| **Hosting** | Cloudflare Pages | Already wired (`wrangler.toml`), free, Markdown for Agents |
| **Interactive elements** | Inline `<script>` tags (zero framework JS) | No hydration needed, vanilla JS only |
| **Icons** | Lucide (SVG) | Consistent with `apps/web`, no emojis as icons |
| **Fonts** | Outfit + IBM Plex Sans + JetBrains Mono | See Typography section |

**Not using:** Next.js (overkill for content), Vercel (already on Cloudflare), plain HTML (doesn't scale).

---

## Site Architecture

```
apps/landing/
  astro.config.mjs          # Astro 5 + Starlight (static output, no adapter)
  package.json              # @claude-view/landing
  tsconfig.json             # extends ../../tsconfig.base.json
  wrangler.toml             # Cloudflare Pages (unchanged, deploys dist/)
  src/
    content.config.ts        # Content collection schemas (Astro 5: src root)
    content/
      docs/                 # Starlight docs — auto-generates sidebar
        index.mdx           # Docs home / getting started
        installation.mdx
        features/
          session-browser.mdx
          mission-control.mdx
          agent-control.mdx
          ai-fluency-score.mdx
          search.mdx
          cost-tracking.mdx
        guides/
          mobile-setup.mdx
          mcp-integration.mdx    # MCP server setup and tools reference
        reference/
          cli-options.mdx
          api.mdx
          keyboard-shortcuts.mdx
      blog/                 # Blog posts (MDX)
        welcome.mdx         # Initial "introducing claude-view" post
      changelog/            # Changelog entries (Markdown)
        v0.8.0.md
    pages/                  # Marketing pages (custom Astro layout, NOT Starlight)
      index.astro           # Homepage — hero + features + pricing + CTA
      pricing.astro         # Pricing page (Free / Pro / Team)
    layouts/
      MarketingLayout.astro # Shared layout for marketing pages (nav + footer)
      BlogLayout.astro      # Blog post layout
    components/             # Astro components (zero framework JS)
      Nav.astro             # Sticky nav with blur backdrop
      Footer.astro          # Site footer
      Hero.astro            # Hero section with animated terminal
      AnimatedTerminal.astro # CSS typewriter terminal animation
      DashboardPreview.astro # Animated dashboard mockup (CSS animations)
      PhoneMockup.astro     # 3D perspective phone with notification animations
      FeatureSection.astro  # Scroll-triggered feature reveal
      InstallCommand.astro  # Astro component: click-to-copy `npx claude-view`
      GitHubStars.astro     # Astro component: dynamic star count from GitHub API
      PricingCards.astro    # Three-tier pricing comparison
      AppStoreBadges.astro  # iOS/Android badges with deep linking (preserved)
    assets/                 # Images, screenshots (use <Picture> from astro:assets for AVIF+WebP)
    styles/
      global.css            # Global styles, font imports, CSS custom properties
  public/
    llms.txt                # LLM-readable site summary (hand-written)
    llms-full.txt           # Full docs dump (auto-generated from content)
    favicon.svg
    og-image.png            # Open Graph image for social sharing (must remain PNG — social platforms don't support AVIF)
    robots.txt
```

---

## Pages & Routes

| Route | Layout | Content Source | Purpose |
|-------|--------|---------------|---------|
| `/` | MarketingLayout | `src/pages/index.astro` | Homepage: hero + features + pricing + CTA |
| `/pricing` | MarketingLayout | `src/pages/pricing.astro` | Pricing tiers (Free / Pro / Team) |
| `/blog` | MarketingLayout | `src/content/blog/*.mdx` | Blog post listing |
| `/blog/[slug]` | BlogLayout | `src/content/blog/*.mdx` | Individual blog post |
| `/changelog` | MarketingLayout | `src/content/changelog/*.md` | Version history |
| `/docs/**` | Starlight | `src/content/docs/**/*.mdx` | Documentation site |

---

## Visual Design System

### Aesthetic Direction

"Cinematic Mission Control" — dark, precise, alive. Product-first hero (Cursor pattern). Code-centric (Resend pattern). Comprehensive docs (Supabase pattern).

### Typography

| Role | Font | Weight Range | Usage |
|------|------|-------------|-------|
| Headings | **Outfit** | 500–700 | Page titles, section headings, hero text |
| Body | **IBM Plex Sans** | 300–600 | Paragraphs, descriptions, nav |
| Code/Terminal | **JetBrains Mono** | 400–600 | Code blocks, animated terminal, install command |

```css
@import url('https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@300;400;500;600&family=JetBrains+Mono:wght@400;500;600&family=Outfit:wght@400;500;600;700&display=swap');
```

> **Performance note (2026 SOTA):** Self-hosting fonts as WOFF2 with `font-display: swap` is 200-300ms faster than Google Fonts CDN for first-time visitors on high-latency connections (eliminates third-party DNS/TCP/TLS). This is what Vercel, Linear, and Resend all do. For V1 we use Google Fonts for simplicity; consider migrating to self-hosted fonts post-launch for optimal LCP. The `<link rel="preconnect">` hints in MarketingLayout save ~100ms typical, up to 300ms in high-latency conditions (not 500ms as sometimes claimed).

**Why not Space Grotesk?** Every AI/SaaS tool in 2026 uses it. Outfit is geometric, bold, and distinctive without being overused.

### Color Palette

Matches the product's existing design language:

| Role | Value | Tailwind | Usage |
|------|-------|----------|-------|
| Background | `#0F172A` | slate-900 | Page background |
| Surface | `#1E293B` | slate-800 | Cards, elevated surfaces |
| Border | `#334155` | slate-700 | Subtle borders |
| Text Primary | `#F8FAFC` | slate-50 | Headings, body text |
| Text Secondary | `#94A3B8` | slate-400 | Subheadings, descriptions |
| CTA / Running | `#22C55E` | green-500 | Primary CTA, "running" status |
| Attention | `#F59E0B` | amber-500 | "Waiting" status, highlights |
| Accent | `#3B82F6` | blue-500 | Links, secondary accent |

### Animation Strategy

All animations respect `prefers-reduced-motion: reduce` with static fallbacks.

| Element | Animation | Duration | Trigger |
|---------|-----------|----------|---------|
| Hero terminal | Typewriter effect (green monospace) | 2s | Page load (0.5s delay) |
| Dashboard preview | Status dots pulse, cost counters tick | CSS continuous | Viewport visible |
| Feature sections | Slide-up + fade-in | 600ms | Intersection Observer (0.2 threshold) |
| Phone mockup | 3D tilt + notification slide-in | 800ms | Scroll position |
| Install terminal | Line-by-line typewriter | 3s | Intersection Observer |
| CTA glow | Subtle green pulse | 2s | CSS continuous |

### Image Optimization

| Concern | Choice | Details |
|---------|--------|---------|
| **Primary format** | AVIF | ~50% smaller than JPEG, ~20% smaller than WebP. Browser support: 93%+ globally (Chrome 85+, Firefox 93+, Safari 16.4+, Edge 121+). Backed by AV1 codec from Alliance for Open Media (Google, Netflix, Amazon, Microsoft). |
| **Fallback chain** | AVIF → WebP → PNG | Use Astro's `<Picture>` component from `astro:assets` with `formats={['avif', 'webp']}` — auto-generates `<source>` tags per format with fallback `<img>`. |
| **OG image** | PNG only | Social platforms (Twitter, Slack, Discord, LinkedIn) do not support AVIF or WebP in `og:image` tags. The OG image MUST remain PNG. |
| **On-page images** | AVIF primary | Any screenshots, illustrations, or hero images added post-V1 should use `<Picture>` with AVIF+WebP. For V1 (mostly CSS animations, minimal raster images), this is future-proofing the pipeline. |
| **Astro integration** | `astro:assets` built-in | Import images from `src/assets/`, use `<Picture>` or `<Image>` — Astro handles format conversion, sizing, and lazy loading at build time. No external image CDN needed for static site. |

> **Why AVIF over WebP alone?** At equivalent visual quality, AVIF is typically 20% smaller than WebP and 50% smaller than JPEG (Netflix, Google research). The codec handles gradients and photographic content better than WebP due to AV1's superior intra-frame compression. Safari 16.4+ (released March 2023) was the last holdout — as of Feb 2026, AVIF has universal modern browser support.

### View Transitions

| Concern | Choice | Details |
|---------|--------|---------|
| **API** | View Transitions API via Astro's `<ClientRouter />` | Native CSS page-to-page animations without client-side routing. Browser handles the transition; unsupported browsers get normal navigation (progressive enhancement). |
| **Astro component** | `import { ClientRouter } from 'astro:transitions'` | Add `<ClientRouter />` to `<head>` in `MarketingLayout.astro`. One line, zero config. Astro intercepts navigation clicks and uses the browser's View Transition API. |
| **Browser support** | Chrome 126+, Edge 126+ (cross-document); Safari/Firefox: graceful fallback | Cross-document view transitions (MPA) shipped in Chrome 126 (June 2024). In unsupported browsers, Astro falls back to normal full-page navigation — zero JS penalty, zero broken behavior. |
| **Scope** | Marketing pages only | View transitions apply to pages sharing the `MarketingLayout`. Starlight docs have their own navigation. The transition creates a smooth fade between marketing pages (home → pricing → blog). |
| **JS cost** | ~0KB additional | The `<ClientRouter />` component adds a small inline script that Astro manages. In browsers without View Transitions support, the script is a no-op. |

> **Why View Transitions?** Linear, Vercel, and the Astro docs site itself use View Transitions for polished MPA navigation. The API is a W3C standard (CSS View Transitions Module Level 1, shipped in Chromium). For a zero-JS static site, it adds app-like feel with zero framework overhead — the browser does all the work.

### Performance Budget

| Metric | Target |
|--------|--------|
| Total JS | <5KB (inline scripts only) |
| LCP | <1.5s |
| CLS | < 0.1 |
| INP | < 200ms | Static SSG with minimal JS — should easily pass Google's Core Web Vital threshold |
| Speculation Rules | Prefetch/prerender linked pages on hover | Astro's built-in `prefetch` uses the Speculation Rules API on Chromium (Chrome 121+, Edge, Opera) to prerender pages ahead of navigation for near-instant transitions. Firefox/Safari ignore the speculation rules and fall back to standard `<link rel="prefetch">`. Zero risk — progressive enhancement only. |
| Lighthouse (all) | 95+ |

---

## Homepage Layout

```
┌──────────────────────────────────────────────────────────┐
│  STICKY NAV (blur backdrop, appears on scroll)           │
│  Logo | Features | Docs | Pricing | Blog | [GitHub ★ N] │
├──────────────────────────────────────────────────────────┤
│                                                          │
│  HERO (full viewport)                                    │
│                                                          │
│  "Mission Control for AI Coding Agents"                  │
│  Monitor, control, and orchestrate your Claude Code      │
│  sessions from desktop or phone.                         │
│                                                          │
│  ┌────────────────────────────────────┐                  │
│  │ $ npx claude-view        [📋 Copy]│  ← animated type │
│  └────────────────────────────────────┘                  │
│  [View Docs →]    [GitHub ★ N]                           │
│                                                          │
│  Works with Claude Code via MCP — 8 tools for            │
│  monitoring, cost tracking, and agent control. [→ docs]  │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │  ANIMATED DASHBOARD PREVIEW                        │  │
│  │  Mission Control — 4 agents running                │  │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌──────┐       │  │
│  │  │ Auth   │ │ API    │ │ Tests  │ │ Docs │       │  │
│  │  │ ●run   │ │ ●wait  │ │ ●done  │ │ ●run │       │  │
│  │  │ $0.12  │ │ $0.08  │ │ $0.03  │ │ $0.15│       │  │
│  │  └────────┘ └────────┘ └────────┘ └──────┘       │  │
│  └────────────────────────────────────────────────────┘  │
│     CSS-animated: pulsing dots, ticking costs            │
│                                                          │
├─────────── scroll-triggered reveal ──────────────────────┤
│                                                          │
│  FEATURE 1: MONITOR                                      │
│  "See every agent. Every token. Every decision."         │
│  [text left]              [animated dashboard right]     │
│                                                          │
├─────────── scroll-triggered reveal ──────────────────────┤
│                                                          │
│  FEATURE 2: CONTROL                                      │
│  "Approve. Reject. Resume. From anywhere."               │
│  [animated chat UI left]  [text right]                   │
│                                                          │
├─────────── scroll-triggered reveal ──────────────────────┤
│                                                          │
│  FEATURE 3: MOBILE                                       │
│  "I shipped a feature from my phone."                    │
│  [3D phone mockup with animated notifications]           │
│  [App Store]  [Play Store]                               │
│                                                          │
├─────────── scroll-triggered reveal ──────────────────────┤
│                                                          │
│  FEATURE 4: ANALYZE                                      │
│  "Your AI fluency, measured."                            │
│  [animated sparklines, heatmap, fluency score]           │
│                                                          │
├─────────── scroll-triggered reveal ──────────────────────┤
│                                                          │
│  INSTALL SECTION                                         │
│  ┌──────────────────────────────────────────────────┐    │
│  │  $ npx claude-view                               │    │
│  │  ▸ Downloading claude-view v0.8.0...             │    │
│  │  ▸ Starting server on http://localhost:47892     │    │
│  │  ✓ Ready — monitoring 12 sessions                │    │
│  └──────────────────────────────────────────────────┘    │
│  Animated terminal with typewriter effect                │
│                                                          │
├──────────────────────────────────────────────────────────┤
│                                                          │
│  PRICING (3 cards)                                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │ Free     │  │ Pro      │  │ Team     │              │
│  │ $0/mo    │  │ $X/mo    │  │ $X/seat  │              │
│  │          │  │          │  │          │              │
│  │ npx      │  │ Hosted   │  │ 10+      │              │
│  │ install  │  │ relay    │  │ parallel │              │
│  │ Full     │  │ 3 plans  │  │ Team     │              │
│  │ analytics│  │ No setup │  │ dashbd   │              │
│  │          │  │          │  │          │              │
│  │[Get      │  │[Coming   │  │[Coming   │              │
│  │ Started] │  │ Soon]    │  │ Soon]    │              │
│  └──────────┘  └──────────┘  └──────────┘              │
│                                                          │
├──────────────────────────────────────────────────────────┤
│  FOOTER                                                  │
│  Docs | Blog | Changelog | GitHub | Twitter              │
│  "Open source. MIT License."                             │
└──────────────────────────────────────────────────────────┘
```

---

## Interactive Components (Astro + Inline Scripts)

All interactivity uses inline `<script>` tags and CSS animations — zero framework JS.

| Component | Tech | Size Est. | Description |
|-----------|------|-----------|-------------|
| `InstallCommand.astro` | Astro + inline script | ~1KB | Click-to-copy `npx claude-view` with checkmark animation |
| `GitHubStars.astro` | Astro + inline script | ~1KB | Fetches star count from GitHub API, caches in localStorage |
| `PricingCards.astro` | Astro + inline script | ~1KB | Monthly/annual toggle (future use) |
| `AnimatedTerminal.astro` | Astro + CSS | ~0KB JS | Typewriter terminal effect via CSS `@keyframes` |
| `DashboardPreview.astro` | Astro + CSS | ~0KB JS | Pulsing dots, ticking costs via CSS `@keyframes` |
| `PhoneMockup.astro` | Astro + CSS | ~0KB JS | 3D perspective via CSS `transform: perspective()` |
| `FeatureSection.astro` | Astro + inline script | <1KB | Scroll-triggered reveals via Intersection Observer |

---

## Agent/LLM Discoverability

> **Honest assessment:** Agent discoverability is an emerging, unproven field. The techniques below are future-proofing investments — not current traffic drivers. The real growth mechanism remains: good docs → organic search → users → GitHub stars → training data → AI familiarity. These layers supplement that flywheel; they do not replace it.

### Layer 1: `llms.txt` + `llms-full.txt` (hand-written + auto-generated)

The `llms.txt` spec ([llmstxt.org](https://llmstxt.org/), proposed by Jeremy Howard, founder of Answer.AI (and fast.ai co-founder)) provides a machine-readable site summary. As of Feb 2026, no major AI company has confirmed reading it — Google's John Mueller has compared it to `<meta keywords>`. We implement it as low-cost future-proofing, not a proven growth lever.

```markdown
# claude-view

> Mission Control for AI coding agents — monitor, orchestrate, and command your fleet from desktop or phone.

## About
claude-view is an open-source developer tool that monitors Claude Code sessions,
tracks costs, and lets you control AI agents from a web dashboard or native mobile app.
Zero config: `npx claude-view`. Rust backend, React web app, ~15MB binary.

## Docs
- [Getting Started](/docs): Install and first run
- [Mission Control](/docs/features/mission-control): Live agent monitoring
- [Agent Control](/docs/features/agent-control): Send messages, approve tools
- [Mobile App](/docs/guides/mobile-setup): iOS/Android setup and pairing
- [Cost Tracking](/docs/features/cost-tracking): Token usage and model costs
- [AI Fluency Score](/docs/features/ai-fluency-score): Measure AI coding effectiveness
- [Search](/docs/features/search): Full-text search across sessions
- [CLI Reference](/docs/reference/cli-options): Command line options
- [API Reference](/docs/reference/api): HTTP API endpoints

## Optional
- [Blog](/blog): Technical deep dives and release announcements
- [Changelog](/changelog): Version history
- [MCP Integration](/docs/guides/mcp-integration): AI agent integration (8 tools via stdio)
```

`llms-full.txt` is auto-generated from all content at build time.

### Layer 2: Schema.org Structured Data

Pages with schema markup are ~36% more likely to appear in AI-generated summaries (according to WPRiders (WordPress agency) — industry analysis, not peer-reviewed research). We apply appropriate types:

| Page Type | Schema Type | Why |
|-----------|-------------|-----|
| Homepage | `SoftwareApplication` | Identifies the product for AI parsers |
| Doc pages | `TechArticle` | Links back to parent SoftwareApplication entity |
| Pricing | `FAQPage` | FAQ-formatted pricing questions perform well in AI search |
| Blog posts | `BlogPosting` | Standard article schema with author/date |
| Installation docs | `HowTo` | Step-by-step instructions — AI assistants heavily favor structured procedural content for citation. Google deprecated HowTo rich results (Sept 2023) but Schema.org HowTo remains valid and is consumed by AI engines (Microsoft Bing/Copilot confirmed structured data helps LLMs interpret content, March 2025). |
| Marketing pages | `BreadcrumbList` | Communicates site hierarchy to AI parsers and search engines. Built dynamically from URL path segments. **Starlight does NOT auto-generate BreadcrumbList JSON-LD** — it renders visual breadcrumb navigation but emits no structured data. Docs pages get BreadcrumbList via Starlight `head` config override. |

```json
{
  "@context": "https://schema.org",
  "@type": "SoftwareApplication",
  "name": "claude-view",
  "applicationCategory": "DeveloperApplication",
  "operatingSystem": "macOS, Linux",
  "description": "Mission Control for AI coding agents",
  "offers": { "@type": "Offer", "price": "0" },
  "url": "https://claude-view.dev"
}
```

**HowTo JSON-LD** (for `content/docs/installation.mdx`):

```json
{
  "@context": "https://schema.org",
  "@type": "HowTo",
  "name": "Install claude-view",
  "description": "Install and start claude-view to monitor your Claude Code sessions. Zero config — one command, 15-second setup.",
  "totalTime": "PT15S",
  "tool": [
    {
      "@type": "HowToTool",
      "name": "Node.js (v18 or later)"
    }
  ],
  "step": [
    {
      "@type": "HowToStep",
      "position": 1,
      "name": "Run the install command",
      "text": "Open your terminal and run: npx claude-view"
    },
    {
      "@type": "HowToStep",
      "position": 2,
      "name": "Wait for download",
      "text": "The npx command downloads the pre-built binary (~15MB) automatically. No Rust toolchain or compilation required."
    },
    {
      "@type": "HowToStep",
      "position": 3,
      "name": "Open the dashboard",
      "text": "claude-view starts a local server and opens your browser to http://localhost:47892. You'll see all active Claude Code sessions automatically detected."
    }
  ]
}
```

> **Why HowTo still matters for GEO:** Google dropped HowTo rich results from SERPs in September 2023, but the Schema.org `HowTo` type remains a valid vocabulary entry. AI engines (Bing Copilot, Perplexity, ChatGPT with browsing) parse JSON-LD structured data to understand page content — step-by-step structured instructions are significantly easier for LLMs to extract and cite than unstructured prose. The cost to implement is near-zero (one JSON-LD block), and the upside for AI citation is high.

**BreadcrumbList JSON-LD** (for marketing pages via `MarketingLayout.astro`):

```json
{
  "@context": "https://schema.org",
  "@type": "BreadcrumbList",
  "itemListElement": [
    {
      "@type": "ListItem",
      "position": 1,
      "name": "Home",
      "item": "https://claude-view.dev/"
    },
    {
      "@type": "ListItem",
      "position": 2,
      "name": "Pricing"
    }
  ]
}
```

> **Note on BreadcrumbList:** The last item in the list omits the `item` URL property -- per Google's structured data documentation, the trailing breadcrumb uses the containing page's URL implicitly. The breadcrumb list is built dynamically in `MarketingLayout.astro` from `Astro.url.pathname` segments, so every marketing page gets correct breadcrumbs without manual configuration. For Starlight docs pages, BreadcrumbList is injected via the Starlight `head` config override in `astro.config.mjs`.

### Layer 3: Cloudflare Markdown for Agents (opt-in, requires configuration)

> **Important caveats:**
> - **Not automatic.** Must be explicitly enabled via Cloudflare Dashboard or API.
> - **Requires Pro plan ($20/mo) or higher.** Not available on the free Cloudflare plan.
> - **Pages compatibility is unconfirmed.** The feature is documented as zone-level; compatibility with Cloudflare Pages zones vs standard zones is unconfirmed. Behavior on `.pages.dev` subdomains vs custom domains is not guaranteed.

When enabled, Cloudflare converts HTML pages to clean markdown when a client sends `Accept: text/markdown`. Claimed up to 80% token reduction on boilerplate-heavy pages (Cloudflare's figure — for clean SSG output like Astro, actual reduction is likely 40-60%). Claude Code and OpenCode's `web_fetch` tools send this header.

**Fallback strategy:** `llms.txt` and `llms-full.txt` serve as the primary agent-readable content regardless of whether CF Markdown for Agents is available.

### Layer 4: Open Graph / Social (standard web hygiene)

Every page gets proper OG tags for social sharing. Note: OG tags are consumed by social platforms (Twitter, Slack, Discord) for link previews — they are **not** consumed by AI systems for citation. This is standard web practice, not a GEO technique.

- `og:title`, `og:description`, `og:image`, `og:url`
- `twitter:card`: `summary_large_image`
- Custom `og:type` per page type (`website` for marketing, `article` for blog posts)
- `article:published_time` + `article:author` on blog posts

### Layer 5: AI Crawler Access (robots.txt)

Explicitly allow known AI crawlers in `robots.txt`:

```
User-agent: *
Allow: /

User-agent: ClaudeBot
Allow: /

User-agent: GPTBot
Allow: /

User-agent: PerplexityBot
Allow: /

Sitemap: https://claude-view.dev/sitemap.xml
```

### Layer 6: Content-Level GEO (highest impact — applies to all written content)

The Princeton "GEO: Generative Engine Optimization" research found content-level techniques have the highest impact on AI citation:

| Technique | Impact | How We Apply |
|-----------|--------|-------------|
| **Fluency optimization** | ~15-30% improvement | Clean, authoritative, precise writing in all docs |
| **Statistics addition** | ~32% improvement (Princeton GEO benchmark) | Include concrete numbers (binary size, token counts, benchmark results) |
| **Citation from authoritative sources** | ~27% standalone; 30-40% when combined with other techniques (Princeton GEO benchmark, Position-Adjusted Word Count metric) | Reference Anthropic docs, Rust ecosystem, established patterns |
| **First-third content positioning** | ChatGPT cites from first third 44% of time | Put key information (install command, description, capabilities) in the first 33% of every page |

These content-level techniques outperform technical markup (Schema.org, llms.txt) for GEO. Apply them to every documentation and blog page.

---

## Content Plan (Start → Growth)

### V1 (launch): ~15 pages

| Type | Count | Pages |
|------|-------|-------|
| Marketing | 2 | Homepage, Pricing |
| Docs | 11 | Getting started, Installation, 5 feature pages, 2 guide pages, 2 reference pages |
| Blog | 1 | "Introducing claude-view" launch post |
| Changelog | 1 | v0.8.0 |

### V2 (post-launch): grow to 30+

- Add docs for each new feature (Plan Runner, etc.)
- Blog posts: technical deep dives, use case stories, benchmark results
- Changelog entries per release
- Comparison pages ("claude-view vs ccusage", "claude-view vs Happy Coder")

### V3 (maturity): 50+

- API reference auto-generated from Rust doc comments
- Tutorial series (video + text)
- Community showcase / integrations
- Localized docs (zh-TW, zh-CN via Starlight i18n)

---

## MCP Server (Shipped — `@claude-view/mcp`)

The MCP server is built and working. Package: `packages/mcp/`. 8 read-only tools over stdio transport:

| Tool | Description |
|------|-------------|
| `list_sessions` | Browse sessions with filters |
| `get_session` | Session detail with commits/metrics |
| `search_sessions` | Full-text search across conversations |
| `get_stats` | Dashboard overview + trends |
| `get_fluency_score` | AI Fluency Score (0-100) |
| `get_token_stats` | Token usage + cache hit ratio |
| `list_live_sessions` | Currently running agents |
| `get_live_summary` | Aggregate cost/status today |

**Source docs:** `docs/plans/2026-02-28-plugin-skill-mcp-impl.md`, `packages/mcp/src/` (tool implementations)

**Landing page mention:** Feature bullet on homepage ("Works with Claude Code via MCP — 8 tools for session monitoring, cost tracking, and agent control").
**Docs:** Full guide page at `/docs/guides/mcp-integration` (setup config, tool reference, prerequisites).

---

## Pricing Page

Based on GTM strategy (open-core model):

| Tier | Price | Features |
|------|-------|----------|
| **Free** | $0/mo | `npx claude-view`, self-host, full analytics, monitoring, search |
| **Pro** | TBD | Hosted relay, 3 parallel plans, zero setup |
| **Team** | TBD/seat | 10+ parallel plans, shared dashboards, team analytics, cost budgets |

Pro and Team show "Coming Soon" with email signup for notifications.

---

## Deep Linking (Preserved)

The current mobile app deep linking (`?k=...&t=...` → `claude-view://pair?...`) is preserved in the homepage script. The Astro page includes the same redirect logic for mobile app pairing.

---

## Monorepo Integration

| Concern | How |
|---------|-----|
| Package name | `@claude-view/landing` (unchanged) |
| Workspace | `apps/landing/` (unchanged) |
| Turbo pipeline | `build` task in `turbo.json` |
| Dev command | `bun run dev:landing` or `cd apps/landing && bun run dev` |
| Deploy | `wrangler pages deploy dist` (unchanged) |
| TypeScript | `tsconfig.json` extends `../../tsconfig.base.json` |
| Shared types | Can import from `@claude-view/shared` if needed |

---

## Pre-Delivery Checklist

### Visual Quality
- [ ] No emojis as icons (use Lucide SVGs)
- [ ] All icons from Lucide (consistent with `apps/web`)
- [ ] Hover states don't cause layout shift
- [ ] `cursor-pointer` on all clickable elements
- [ ] Transitions 150–300ms

### Accessibility
- [ ] Color contrast 4.5:1 minimum for all text
- [ ] Focus states visible for keyboard navigation
- [ ] `prefers-reduced-motion` respected (all animations have static fallbacks)
- [ ] Alt text on all images
- [ ] Semantic HTML (headings, landmarks, lists)

### Responsive
- [ ] Tested at 375px, 768px, 1024px, 1440px
- [ ] No horizontal scroll on mobile
- [ ] Touch targets 44x44px minimum (per WCAG 2.2 SC 2.5.5 AAA; AA minimum is 24x24px per WCAG 2.2 SC 2.5.8)
- [ ] Navigation collapses to hamburger on mobile

### Performance
- [ ] Lighthouse 95+ on all metrics
- [ ] Total JS <5KB (inline scripts only, no framework)
- [ ] Images optimized (AVIF primary, WebP fallback, PNG for OG tags; use `<Picture>` from `astro:assets` with `formats={['avif', 'webp']}`)
- [ ] OG image remains PNG (social platforms do not support AVIF/WebP in og:image)
- [ ] CLS < 0.1 (all dimensions pre-set; font-swap and dynamic star count make absolute zero unachievable)
- [ ] View Transitions enabled via `<ClientRouter />` in MarketingLayout (progressive enhancement — no impact on unsupported browsers)

### Agent SEO
- [ ] `llms.txt` at site root
- [ ] Schema.org SoftwareApplication on homepage
- [ ] Schema.org HowTo on installation docs page (`content/docs/installation.mdx`)
- [ ] Schema.org BreadcrumbList on all marketing pages (dynamically built from URL path)
- [ ] Schema.org BreadcrumbList on Starlight docs pages (via `head` config override)
- [ ] Open Graph tags on every page
- [ ] Cloudflare Markdown for Agents evaluated (requires Pro+ plan — optional for V1, llms.txt provides coverage regardless)

### Agent Crawlers
- [ ] `robots.txt` explicitly allows ClaudeBot, GPTBot, PerplexityBot
- [ ] No accidental AI crawler blocks in Cloudflare WAF/firewall rules

### Content-Level GEO
- [ ] Key information in first third of every page
- [ ] Concrete statistics and numbers in feature descriptions
- [ ] Authoritative source citations where applicable
- [ ] Clean, fluent, precise prose (not marketing fluff)

### JavaScript Fallbacks
- [ ] `.reveal-on-scroll` sections visible without JS (`@media (scripting: none)` or `<noscript>`)
- [ ] Hamburger menu has `aria-expanded`, `aria-controls`, Escape key handler
- [ ] Decorative components (PhoneMockup, AnimatedTerminal) have `role="img"` + `aria-label`
- [ ] Deep link handler only forwards whitelisted params (`k` and `t`)

### Deployment
- [ ] `_headers` file for Cloudflare cache control (immutable for hashed assets)
- [ ] Internal links use trailing slashes (or `trailingSlash: 'always'` in config)
- [ ] Custom 404 page exists for marketing routes
- [ ] `.astro/` directory in `.gitignore`

---

## Reference Sites

| Site | Pattern We Borrow | Link |
|------|-------------------|------|
| Supabase | Docs architecture, agent discoverability, growth flywheel | https://supabase.com |
| Cursor | Product-first hero, interactive demo | https://cursor.com |
| Resend | Dark theme, code-centric, developer-first aesthetic | https://resend.com |
| Linear | Micro-animations, type scale, precision design | https://linear.app |
| v0 | AI-first interaction pattern | https://v0.app |

---

## Next Step

Create implementation plan (invoke writing-plans skill).
