# Landing Page & Docs Site — Implementation Plan

> **Status:** DONE (2026-03-01) — all 12 tasks implemented, shippable audit passed (SHIP IT)
>
> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the placeholder `apps/landing/` with an Astro 5 + Starlight site that serves as the marketing homepage, documentation hub, blog, and changelog for claude-view — optimized for AI agent discoverability.

**Architecture:** Astro 5 SSG with Starlight docs engine. Marketing pages use custom Astro layouts. Docs use Starlight (Content Collections, built-in search/sidebar). **Zero client-side JavaScript** — all interactivity via vanilla `<script>` tags and CSS animations. Deployed to Cloudflare Pages via existing `wrangler.toml`.

**Tech Stack:** Astro 5, Starlight, Tailwind CSS 4, Cloudflare Pages, Lucide SVGs (inline), Outfit + IBM Plex Sans + JetBrains Mono fonts. **No React, no client-side framework.**

**Design doc:** `docs/plans/2026-02-28-landing-page-design.md`

### Completion Summary

| Task | Commit | Description |
|------|--------|-------------|
| 1 | `ebff3384` | Scaffold Astro + Starlight project |
| 2 | `5a2235cf` | Design tokens, fonts, Tailwind 4 theme |
| 3 | `cb657cc4` | MarketingLayout, Nav, Footer |
| 4 | `83545b32` | Hero section + animated terminal + dashboard preview |
| 5 | `658c2942` | Scroll-triggered feature sections |
| 6 | `6f69958b` | Pricing page + install section |
| 7-8 | (included in tasks 1-6 commits) | Starlight docs (13 pages), blog, changelog |
| 9 | `329c8028` | Agent SEO: llms.txt, og-image, robots.txt, Schema.org |
| 10-11 | `9b36e8c3` | GitHub stars badge, deep link, app store badges |
| 12 | `19ac335d`, `ec84a8db` | Final wiring, prefetch, build verification |
| fix | `b447939b` | Shippable audit fixes: /docs/ URL, orphan page, type warnings |

**Shippable audit (2026-03-01):** 0 blockers, 7 warnings (accessibility polish, sitemap URL, reduced-motion). Build passes (19 pages), typecheck clean, all components wired.

---

## Task 1: Scaffold Astro + Starlight Project

**Files:**
- Delete: `apps/landing/src/index.html`
- Rewrite: `apps/landing/package.json`
- Create: `apps/landing/astro.config.mjs`
- Create: `apps/landing/tsconfig.json`
- Keep: `apps/landing/wrangler.toml` (unchanged — see note below about `[site]` deprecation)

**Step 1: Clean out old landing page**

```bash
cd apps/landing
git rm -r --cached dist/ 2>/dev/null || true  # untrack old dist/ (currently committed)
rm -rf src dist .turbo
```

> **Audit fix:** `apps/landing/dist/index.html` is currently tracked in git. Must `git rm --cached` before the cleanup so the old built file is properly removed from git history.

> **Note on `wrangler.toml` `[site]` config:** The existing `wrangler.toml` uses `[site]` with `bucket = "./dist"`, which is the Workers Sites pattern (deprecated in favor of Cloudflare Pages). For Pages deployment via `wrangler pages deploy dist`, the `wrangler.toml` is optional but harmless — `wrangler pages deploy` ignores `[site]` and uses the positional `dist` argument instead. No changes needed for V1, but if maintaining this file long-term, consider replacing `[site]` with `[pages]` config or removing it entirely since Pages deployment doesn't require it.

**Step 2: Initialize Astro project manually**

We can't use `create astro` inside an existing workspace — instead, create files manually.

Create `apps/landing/package.json`:

```json
{
  "name": "@claude-view/landing",
  "version": "0.0.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "astro dev",
    "build": "astro build",
    "preview": "astro preview",
    "lint": "biome check .",
    "typecheck": "astro sync && astro check",
    "deploy": "wrangler pages deploy dist"
  },
  "dependencies": {
    "astro": "^5",
    "@astrojs/starlight": "^0.37",
    "@astrojs/sitemap": "^3",
    "tailwindcss": "^4.1",
    "@tailwindcss/vite": "^4.1",
    "@tailwindcss/typography": "^0.5.15"
  },
  "devDependencies": {
    "@astrojs/check": "^0.9",
    "typescript": "^5.7"
  }
}
```

> **Zero-JS architecture:** Removed `react`, `react-dom`, `@astrojs/react`, `lucide-react`. All interactivity via vanilla `<script>` tags. Lucide icons used as inline SVGs. Total client JS: ~1-2KB (vanilla scripts only).

**Step 3: Create Astro config**

Create `apps/landing/astro.config.mjs`:

```javascript
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import sitemap from '@astrojs/sitemap';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  site: 'https://claude-view.dev',
  trailingSlash: 'always',
  integrations: [
    starlight({
      title: 'claude-view',
      description: 'Mission Control for AI coding agents',
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/anthropics/claude-view' },
      ],
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Introduction', slug: '' },
            { label: 'Installation', slug: 'docs/installation' },
          ],
        },
        {
          label: 'Features',
          autogenerate: { directory: 'docs/features' },
        },
        {
          label: 'Guides',
          autogenerate: { directory: 'docs/guides' },
        },
        {
          label: 'Reference',
          autogenerate: { directory: 'docs/reference' },
        },
      ],
      customCss: ['./src/styles/starlight.css'],
      head: [
        {
          tag: 'script',
          attrs: { type: 'application/ld+json' },
          content: JSON.stringify({
            "@context": "https://schema.org",
            "@type": "TechArticle",
            "isPartOf": {
              "@type": "SoftwareApplication",
              "name": "claude-view",
              "applicationCategory": "DeveloperApplication"
            }
          }),
        },
      ],
    }),
    sitemap(),
  ],
  vite: {
    plugins: [tailwindcss()],
  },
});
```

> **Audit fixes applied:**
> - Added `site: 'https://claude-view.dev'` (required — `Astro.site` is used in MarketingLayout for canonical URLs; `undefined` causes TypeError)
> - Removed `output: 'static'` (Astro 5 defaults to static — explicit declaration unnecessary)
> - Removed `adapter: cloudflare()` (SSR adapter not needed for static site; `wrangler pages deploy dist` works directly)
> - Changed `social` from object `{ github: url }` to array `[{ icon, label, href }]` (Starlight broke the old API in v0.30)
> - Replaced `tailwind({ applyBaseStyles: false })` integration with `@tailwindcss/vite` in `vite.plugins` (correct Tailwind 4 integration path; `@astrojs/tailwind` is deprecated)
> - Added `sitemap()` integration (robots.txt references sitemap.xml)
>
> **Audit fix (B4):** Added `trailingSlash: 'always'` — Cloudflare Pages enforces trailing slashes via 308 redirects. Without this, internal links like `/docs` cause unnecessary redirect hops to `/docs/`. Setting this ensures Astro generates consistent URLs and all internal `<a>` links match CF Pages expectations.

**Step 4: Create TypeScript config**

Create `apps/landing/tsconfig.json`:

```json
{
  "extends": "../../tsconfig.base.json",
  "include": ["src", ".astro/types.d.ts"],
  "compilerOptions": {
    "lib": ["ES2022", "DOM", "DOM.Iterable"]
  }
}
```

> **Audit fixes:**
> - Added `"lib": ["ES2022", "DOM", "DOM.Iterable"]` — base tsconfig only has `["ES2022"]`, missing DOM types needed for `localStorage`, `navigator.clipboard`, `IntersectionObserver`, `document`, `window`. Without these, `astro check` fails.
> - Added `"include": ["src", ".astro/types.d.ts"]` — Astro generates type definitions at `.astro/types.d.ts` for virtual modules like `astro:content`. Without this include, `astro check` cannot resolve Content Layer API imports.

**Step 5: Update root `.gitignore` for Astro build artifacts and landing page images**

Append these lines to the root `.gitignore`:

```gitignore
# Astro build artifacts (apps/landing)
apps/landing/.astro/

# Allow landing page images (overrides root *.png ignore)
!apps/landing/public/*.png
```

> **Audit fixes (B2 + B3):**
> - `apps/landing/.astro/` — Astro generates `.astro/types.d.ts` and `.astro/settings.json` on first build. Without this ignore rule, `git add apps/landing/` would commit generated files.
> - `!apps/landing/public/*.png` — Root `.gitignore` has `*.png` (line 62) with exceptions only for mobile assets. Without this exception, `og-image.png` and any future landing page images would be silently ignored by git, breaking social sharing previews and CI deployments.

**Step 6: Install dependencies**

```bash
cd apps/landing && bun install
```

**Step 7: Create directory structure**

```bash
mkdir -p src/{pages,layouts,components,styles,assets}
mkdir -p src/content/{docs/features,docs/guides,docs/reference,blog,changelog}
mkdir -p public
```

**Step 8: Verify Astro builds**

Create a minimal `src/pages/index.astro`:

```astro
---
// Minimal homepage placeholder
---
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width" />
  <title>claude-view</title>
</head>
<body>
  <h1>claude-view</h1>
  <p>Landing page coming soon.</p>
</body>
</html>
```

Create minimal `src/content/docs/index.mdx`:

```mdx
---
title: Getting Started
description: Install and start using claude-view
---

claude-view documentation coming soon.
```

Create `src/styles/starlight.css`:

```css
/* Custom Starlight theme overrides */
:root {
  --sl-color-accent-low: #1e293b;
  --sl-color-accent: #22c55e;
  --sl-color-accent-high: #4ade80;
}
```

Create `src/content.config.ts` (note: Astro 5 moved this file from `src/content/config.ts` to `src/content.config.ts`):

```typescript
import { defineCollection } from 'astro:content';
import { z } from 'astro/zod';
import { glob } from 'astro/loaders';
import { docsLoader } from '@astrojs/starlight/loaders';
import { docsSchema } from '@astrojs/starlight/schema';

export const collections = {
  docs: defineCollection({
    loader: docsLoader(),
    schema: docsSchema(),
  }),
  blog: defineCollection({
    loader: glob({ pattern: '**/*.mdx', base: './src/content/blog' }),
    schema: z.object({
      title: z.string(),
      description: z.string(),
      date: z.coerce.date(),
      author: z.string().default('claude-view team'),
    }),
  }),
  changelog: defineCollection({
    loader: glob({ pattern: '**/*.md', base: './src/content/changelog' }),
    schema: z.object({
      title: z.string(),
      date: z.coerce.date(),
      version: z.string(),
    }),
  }),
};
```

> **Audit fixes applied:**
> - Moved file from `src/content/config.ts` to `src/content.config.ts` (Astro 5 Content Layer API requires this location)
> - Added `docsLoader()` import from `@astrojs/starlight/loaders` (required in Astro 5 — without it, Starlight can't find doc files)
> - Added `glob()` loader to `blog` and `changelog` collections (Astro 5 Content Layer API requires every collection to have a `loader`; without it, `getCollection('blog')` returns empty or throws)
> - Added `blog` and `changelog` collection schemas upfront (prevents parallel execution race: Tasks 7+8 can now run without schema errors)
> - Changed `z` import from `astro:content` to `astro/zod` (the `astro:content` re-export is deprecated in Astro 6)

**Step 9: Build and verify**

```bash
cd apps/landing && bun run build
```

Expected: Builds to `dist/` with index.html and docs/index.html. This also generates the `.astro/` directory with `types.d.ts` — required before `astro check` / `bun run typecheck` can resolve virtual modules like `astro:content`.

**Step 10: Commit**

```bash
git add apps/landing/ .gitignore
git commit -m "feat(landing): scaffold Astro + Starlight project

Replace bare HTML placeholder with Astro 5 + Starlight.
Cloudflare Pages deployment unchanged (wrangler.toml).
Root .gitignore updated: ignore .astro/ build artifacts, allow landing page PNGs."
```

---

## Task 2: Global Styles, Fonts, and Design Tokens

**Files:**
- Create: `apps/landing/src/styles/global.css`
- Modify: `apps/landing/src/styles/starlight.css`
- ~~Create: `apps/landing/tailwind.config.mjs`~~ — **NOT NEEDED** (Tailwind 4 uses CSS-first config)

**Step 1: Create global CSS with font imports and Tailwind 4 theme**

Create `apps/landing/src/styles/global.css`:

```css
@import url('https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@300;400;500;600&family=JetBrains+Mono:wght@400;500;600&family=Outfit:wght@400;500;600;700&display=swap');

@import "tailwindcss";
@plugin "@tailwindcss/typography";

/* Tailwind 4 CSS-first configuration — replaces tailwind.config.mjs */
@theme {
  --font-heading: 'Outfit', sans-serif;
  --font-body: 'IBM Plex Sans', sans-serif;
  --font-mono: 'JetBrains Mono', monospace;

  --color-surface: #1e293b;
  --color-cta: #22c55e;
  --color-attention: #f59e0b;
  --color-accent: #3b82f6;
}

/* Safelist classes that are only applied via JavaScript (Nav scroll effect).
   Without this, Tailwind JIT purges them from the production bundle.
   @source inline() accepts content that Tailwind scans for class patterns. */
@source inline("{bg-slate-900/80,backdrop-blur-md,border-slate-700,border-transparent}");

:root {
  --color-bg: #0f172a;
  --color-border: #334155;
  --color-text: #f8fafc;
  --color-text-secondary: #94a3b8;
}

html {
  scroll-behavior: smooth;
}

body {
  font-family: var(--font-body);
  background: var(--color-bg);
  color: var(--color-text);
}

h1, h2, h3, h4, h5, h6 {
  font-family: var(--font-heading);
}

code, pre, kbd {
  font-family: var(--font-mono);
}

@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}

/* Ensure scroll-reveal sections are visible when JavaScript is disabled.
   The scripting media feature (CSS Scripting Level 1) detects JS availability
   without requiring a <noscript> tag.
   Browser support (~88%): Chrome 120+, Firefox 113+, Safari 18+.
   Keep a <noscript> tag in MarketingLayout as fallback for older browsers. */
@media (scripting: none) {
  .reveal-on-scroll {
    opacity: 1 !important;
    transform: none !important;
  }
}
```

> **Audit fixes applied:**
> - Replaced `@tailwind base; @tailwind components; @tailwind utilities;` with `@import "tailwindcss";` (Tailwind 4 syntax)
> - Moved font/color customization into `@theme { }` block (Tailwind 4 CSS-first config replaces `tailwind.config.mjs`)
> - Added `@source inline(...)` to safelist Nav scroll-effect classes that only appear in JavaScript (without this, Tailwind JIT purges `bg-slate-900/80`, `backdrop-blur-md`, `border-slate-700` and the nav scroll effect silently breaks)
> - **No `tailwind.config.mjs` file** — Tailwind 4 does not use JavaScript config files. Content detection is automatic.

**Step 2: ~~Create Tailwind config~~ — SKIPPED**

> Tailwind 4 has no `tailwind.config.mjs`. All configuration is in `global.css` via `@theme`. See Step 1.

**Step 3: Update Starlight custom CSS**

Update `apps/landing/src/styles/starlight.css` to theme Starlight docs to match the product:

```css
/* Theme Starlight to match claude-view dark aesthetic */
:root {
  --sl-color-accent-low: #1e293b;
  --sl-color-accent: #22c55e;
  --sl-color-accent-high: #4ade80;
  --sl-color-white: #f8fafc;
  --sl-color-gray-1: #e2e8f0;
  --sl-color-gray-2: #cbd5e1;
  --sl-color-gray-3: #94a3b8;
  --sl-color-gray-4: #64748b;
  --sl-color-gray-5: #475569;
  --sl-color-gray-6: #334155;
  --sl-color-black: #0f172a;
  --sl-font: 'IBM Plex Sans', sans-serif;
  --sl-font-system-mono: 'JetBrains Mono', monospace;
}

[data-theme='dark'] {
  --sl-color-bg-nav: #0f172a;
  --sl-color-bg-sidebar: #0f172a;
}
```

**Step 4: Build and verify styles load**

```bash
cd apps/landing && bun run build
```

**Step 5: Commit**

```bash
git add apps/landing/src/styles/
git commit -m "feat(landing): add design tokens, fonts, and Tailwind 4 theme

Outfit headings, IBM Plex Sans body, JetBrains Mono code.
Slate-900 dark theme via CSS @theme (Tailwind 4 CSS-first config).
Starlight themed to match. Nav scroll classes safelisted."
```

---

## Task 3: Marketing Layout + Nav + Footer

**Files:**
- Create: `apps/landing/src/layouts/MarketingLayout.astro`
- Create: `apps/landing/src/components/Nav.astro`
- Create: `apps/landing/src/components/Footer.astro`

**Step 1: Create Nav component**

Create `apps/landing/src/components/Nav.astro`:

```astro
---
// Sticky nav with blur backdrop, appears on scroll
---
<nav
  class="fixed top-0 left-0 right-0 z-50 border-b border-transparent transition-all duration-300"
  id="site-nav"
>
  <div class="mx-auto max-w-7xl px-6 py-4 flex items-center justify-between">
    <a href="/" class="font-heading text-xl font-bold text-slate-50 hover:text-cta transition-colors cursor-pointer">
      claude-view
    </a>
    <div class="hidden md:flex items-center gap-8">
      <a href="/#features" class="text-sm text-slate-400 hover:text-slate-50 transition-colors cursor-pointer">Features</a>
      <a href="/docs" class="text-sm text-slate-400 hover:text-slate-50 transition-colors cursor-pointer">Docs</a>
      <a href="/pricing" class="text-sm text-slate-400 hover:text-slate-50 transition-colors cursor-pointer">Pricing</a>
      <a href="/blog" class="text-sm text-slate-400 hover:text-slate-50 transition-colors cursor-pointer">Blog</a>
      <a
        href="https://github.com/anthropics/claude-view"
        target="_blank"
        rel="noopener noreferrer"
        class="text-sm text-slate-400 hover:text-slate-50 transition-colors cursor-pointer flex items-center gap-1"
      >
        GitHub
      </a>
    </div>
    <!-- Mobile hamburger -->
    <button
      class="md:hidden text-slate-400 hover:text-slate-50 cursor-pointer"
      aria-label="Toggle menu"
      aria-expanded="false"
      aria-controls="mobile-menu"
      id="mobile-menu-btn"
    >
      <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M3 12h18M3 6h18M3 18h18" />
      </svg>
    </button>
  </div>
  <!-- Mobile menu -->
  <div class="md:hidden hidden border-t border-slate-700 bg-slate-900/95 backdrop-blur-md" id="mobile-menu">
    <div class="px-6 py-4 flex flex-col gap-4">
      <a href="/#features" class="text-sm text-slate-400 hover:text-slate-50 cursor-pointer">Features</a>
      <a href="/docs" class="text-sm text-slate-400 hover:text-slate-50 cursor-pointer">Docs</a>
      <a href="/pricing" class="text-sm text-slate-400 hover:text-slate-50 cursor-pointer">Pricing</a>
      <a href="/blog" class="text-sm text-slate-400 hover:text-slate-50 cursor-pointer">Blog</a>
    </div>
  </div>
</nav>

<script>
  // Blur backdrop on scroll
  const nav = document.getElementById('site-nav');
  window.addEventListener('scroll', () => {
    if (window.scrollY > 50) {
      nav?.classList.add('bg-slate-900/80', 'backdrop-blur-md', 'border-slate-700');
      nav?.classList.remove('border-transparent');
    } else {
      nav?.classList.remove('bg-slate-900/80', 'backdrop-blur-md', 'border-slate-700');
      nav?.classList.add('border-transparent');
    }
  });

  // Mobile menu toggle
  const btn = document.getElementById('mobile-menu-btn');
  const menu = document.getElementById('mobile-menu');
  btn?.addEventListener('click', () => {
    menu?.classList.toggle('hidden');
    const isOpen = !menu?.classList.contains('hidden');
    btn?.setAttribute('aria-expanded', String(isOpen));
  });

  // Close mobile menu on Escape key
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && !menu?.classList.contains('hidden')) {
      menu?.classList.add('hidden');
      btn?.setAttribute('aria-expanded', 'false');
      btn?.focus();
    }
  });
</script>
```

**Step 2: Create Footer component**

Create `apps/landing/src/components/Footer.astro`:

```astro
---
// Site footer
---
<footer class="border-t border-slate-800 bg-slate-900/50">
  <div class="mx-auto max-w-7xl px-6 py-12">
    <div class="grid grid-cols-2 md:grid-cols-4 gap-8">
      <div>
        <h3 class="font-heading text-sm font-semibold text-slate-50 mb-4">Product</h3>
        <ul class="space-y-2">
          <li><a href="/#features" class="text-sm text-slate-400 hover:text-slate-50 transition-colors cursor-pointer">Features</a></li>
          <li><a href="/pricing" class="text-sm text-slate-400 hover:text-slate-50 transition-colors cursor-pointer">Pricing</a></li>
          <li><a href="/changelog" class="text-sm text-slate-400 hover:text-slate-50 transition-colors cursor-pointer">Changelog</a></li>
        </ul>
      </div>
      <div>
        <h3 class="font-heading text-sm font-semibold text-slate-50 mb-4">Resources</h3>
        <ul class="space-y-2">
          <li><a href="/docs" class="text-sm text-slate-400 hover:text-slate-50 transition-colors cursor-pointer">Documentation</a></li>
          <li><a href="/blog" class="text-sm text-slate-400 hover:text-slate-50 transition-colors cursor-pointer">Blog</a></li>
          <li><a href="/docs/reference/api" class="text-sm text-slate-400 hover:text-slate-50 transition-colors cursor-pointer">API Reference</a></li>
        </ul>
      </div>
      <div>
        <h3 class="font-heading text-sm font-semibold text-slate-50 mb-4">Community</h3>
        <ul class="space-y-2">
          <li><a href="https://github.com/anthropics/claude-view" target="_blank" rel="noopener noreferrer" class="text-sm text-slate-400 hover:text-slate-50 transition-colors cursor-pointer">GitHub</a></li>
          <li><a href="https://twitter.com/claude_view" target="_blank" rel="noopener noreferrer" class="text-sm text-slate-400 hover:text-slate-50 transition-colors cursor-pointer">Twitter</a></li>
        </ul>
      </div>
      <div>
        <h3 class="font-heading text-sm font-semibold text-slate-50 mb-4">Legal</h3>
        <ul class="space-y-2">
          <li><span class="text-sm text-slate-500">MIT License</span></li>
        </ul>
      </div>
    </div>
    <div class="mt-12 pt-8 border-t border-slate-800 text-center">
      <p class="text-sm text-slate-500">Open source. Built with Rust + Astro.</p>
    </div>
  </div>
</footer>
```

**Step 3: Create MarketingLayout**

Create `apps/landing/src/layouts/MarketingLayout.astro`:

```astro
---
import { ClientRouter } from 'astro:transitions';
import Nav from '../components/Nav.astro';
import Footer from '../components/Footer.astro';
import '../styles/global.css';

interface Props {
  title: string;
  description?: string;
  ogImage?: string;
  ogType?: string;
  articleMeta?: { publishedTime?: string; author?: string };
}

const {
  title,
  description = 'Mission Control for AI coding agents',
  ogImage = '/og-image.png',
  ogType = 'website',
  articleMeta,
} = Astro.props;
const canonicalURL = new URL(Astro.url.pathname, Astro.site);
---
<!doctype html>
<html lang="en" class="dark">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title} | claude-view</title>
  <meta name="description" content={description} />
  <link rel="canonical" href={canonicalURL} />

  <!-- View Transitions — native CSS page-to-page animations (progressive enhancement).
       In supported browsers (Chrome 126+, Edge 126+): smooth fade between marketing pages.
       In unsupported browsers (Firefox, older Safari): normal full-page navigation, zero JS penalty.
       See: https://docs.astro.build/en/guides/view-transitions/ -->
  <ClientRouter />

  <!-- Open Graph -->
  <meta property="og:type" content={ogType} />
  <meta property="og:title" content={title} />
  <meta property="og:description" content={description} />
  <meta property="og:image" content={ogImage} />
  <meta property="og:url" content={canonicalURL} />
  {articleMeta?.publishedTime && <meta property="article:published_time" content={articleMeta.publishedTime} />}
  {articleMeta?.author && <meta property="article:author" content={articleMeta.author} />}

  <!-- Twitter -->
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:site" content="@claude_view" />
  <meta name="twitter:title" content={title} />
  <meta name="twitter:description" content={description} />
  <meta name="twitter:image" content={ogImage} />

  <!-- Schema.org -->
  <script type="application/ld+json" set:html={JSON.stringify({
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    "name": "claude-view",
    "applicationCategory": "DeveloperApplication",
    "operatingSystem": "macOS, Linux",
    "description": "Mission Control for AI coding agents",
    "offers": { "@type": "Offer", "price": "0" }
  })} />

  <!-- Google Fonts preconnect — saves ~100-300ms on LCP (DNS + TCP + TLS warmup) -->
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />

  <link rel="icon" type="image/svg+xml" href="/favicon.svg" />

  <!-- Focus ring for keyboard navigation (visible on dark backgrounds) -->
  <style>
    :focus-visible {
      outline: 2px solid #22c55e;
      outline-offset: 2px;
    }
  </style>
</head>
<body class="bg-slate-900 text-slate-50 antialiased">
  <!-- Skip link for keyboard navigation (WCAG 2.2 AA) -->
  <a href="#main-content" class="sr-only focus:not-sr-only focus:absolute focus:top-4 focus:left-4 focus:z-[100] focus:px-4 focus:py-2 focus:bg-green-600 focus:text-white focus:rounded">
    Skip to content
  </a>
  <Nav />
  <main id="main-content" class="pt-16">
    <slot />
  </main>
  <Footer />
</body>
</html>
```

> **View Transitions:** The `<ClientRouter />` component (imported from `astro:transitions`) enables the browser's View Transitions API for all marketing pages sharing this layout. This is the current Astro 5 API -- the older `<ViewTransitions />` component was renamed to `<ClientRouter />` in Astro 4.x. In supported browsers (Chrome 126+, Edge 126+), navigation between marketing pages gets a smooth crossfade animation. In unsupported browsers, pages load normally with zero JS overhead. This is pure progressive enhancement -- no fallback code needed.

**Step 4: Update homepage to use layout**

Replace `apps/landing/src/pages/index.astro`:

```astro
---
import MarketingLayout from '../layouts/MarketingLayout.astro';
---
<MarketingLayout title="Mission Control for AI Coding Agents">
  <section class="min-h-screen flex flex-col items-center justify-center px-6 text-center">
    <h1 class="font-heading text-5xl md:text-7xl font-bold mb-6">
      Mission Control for<br />AI Coding Agents
    </h1>
    <p class="text-lg md:text-xl text-slate-400 max-w-2xl mb-8">
      Monitor, control, and orchestrate your Claude Code sessions from desktop or phone.
    </p>
    <div class="bg-slate-800 border border-slate-700 rounded-lg px-6 py-3 font-mono text-cta text-lg">
      $ npx claude-view
    </div>
  </section>
</MarketingLayout>
```

**Step 5: Build and verify**

```bash
cd apps/landing && bun run build
```

**Step 6: Commit**

```bash
git add apps/landing/src/layouts/ apps/landing/src/components/Nav.astro apps/landing/src/components/Footer.astro apps/landing/src/pages/index.astro
git commit -m "feat(landing): add MarketingLayout, Nav, and Footer

Sticky nav with blur backdrop, responsive hamburger menu.
Footer with product/resources/community links.
OG tags and Schema.org on every page.
View Transitions via ClientRouter for smooth page navigation (progressive enhancement)."
```

---

## Task 4: Hero Section with Animated Terminal

**Files:**
- Create: `apps/landing/src/components/AnimatedTerminal.astro`
- Create: `apps/landing/src/components/DashboardPreview.astro`
- Create: `apps/landing/src/components/InstallCommand.astro`
- Modify: `apps/landing/src/pages/index.astro`

**Step 1: Create AnimatedTerminal**

Create `apps/landing/src/components/AnimatedTerminal.astro`:

```astro
---
// CSS-only typewriter terminal animation
---
<div class="bg-slate-950 border border-slate-700 rounded-xl overflow-hidden shadow-2xl max-w-2xl mx-auto" aria-hidden="true">
  <div class="flex items-center gap-2 px-4 py-3 border-b border-slate-800">
    <div class="w-3 h-3 rounded-full bg-red-500/70"></div>
    <div class="w-3 h-3 rounded-full bg-amber-500/70"></div>
    <div class="w-3 h-3 rounded-full bg-green-500/70"></div>
    <span class="ml-2 text-xs text-slate-500 font-mono">Terminal</span>
  </div>
  <div class="p-6 font-mono text-sm space-y-2">
    <div class="terminal-line" style="--delay: 0s">
      <span class="text-slate-500">$</span>
      <span class="text-cta typing-animation"> npx claude-view</span>
    </div>
    <div class="terminal-line" style="--delay: 1.5s">
      <span class="text-slate-400">▸ Downloading claude-view v0.8.0...</span>
    </div>
    <div class="terminal-line" style="--delay: 2.5s">
      <span class="text-slate-400">▸ Starting server on </span>
      <span class="text-accent">http://localhost:47892</span>
    </div>
    <div class="terminal-line" style="--delay: 3.5s">
      <span class="text-cta">✓</span>
      <span class="text-slate-300"> Ready — monitoring 12 sessions</span>
    </div>
  </div>
</div>

<style>
  .terminal-line {
    opacity: 0;
    animation: fadeIn 0.5s ease forwards;
    animation-delay: var(--delay);
  }

  .typing-animation {
    display: inline-block;
    overflow: hidden;
    white-space: nowrap;
    border-right: 2px solid #22c55e;
    animation:
      typing 1s steps(20) 0.2s forwards,
      blink 0.7s step-end infinite;
    width: 0;
  }

  @keyframes fadeIn {
    to { opacity: 1; }
  }

  @keyframes typing {
    to { width: 100%; }
  }

  @keyframes blink {
    50% { border-color: transparent; }
  }

  @media (prefers-reduced-motion: reduce) {
    .terminal-line {
      opacity: 1;
      animation: none;
    }
    .typing-animation {
      width: 100%;
      border-right: none;
      animation: none;
    }
  }
</style>
```

**Step 2: Create DashboardPreview**

Create `apps/landing/src/components/DashboardPreview.astro`:

```astro
---
// Animated CSS mockup of Mission Control dashboard
const agents = [
  { name: 'Auth Module', status: 'running', color: '#22c55e', cost: '$0.12' },
  { name: 'API Routes', status: 'waiting', color: '#f59e0b', cost: '$0.08' },
  { name: 'Test Suite', status: 'done', color: '#64748b', cost: '$0.03' },
  { name: 'Docs Gen', status: 'running', color: '#22c55e', cost: '$0.15' },
];
---
<div class="bg-slate-950 border border-slate-700 rounded-xl overflow-hidden shadow-2xl max-w-4xl mx-auto">
  <div class="flex items-center justify-between px-6 py-3 border-b border-slate-800">
    <span class="font-heading text-sm font-semibold text-slate-300">Mission Control</span>
    <span class="text-xs text-slate-500 font-mono">4 agents • $0.38 total</span>
  </div>
  <div class="p-6 grid grid-cols-2 md:grid-cols-4 gap-4">
    {agents.map((agent, i) => (
      <div class="bg-slate-800/50 border border-slate-700 rounded-lg p-4 agent-card" style={`--card-delay: ${i * 0.15}s`}>
        <div class="flex items-center gap-2 mb-3">
          <div
            class={`w-2 h-2 rounded-full ${agent.status === 'running' ? 'status-pulse' : ''}`}
            style={`background: ${agent.color}`}
          ></div>
          <span class="text-xs text-slate-400 capitalize">{agent.status}</span>
        </div>
        <div class="font-heading text-sm font-medium text-slate-200 mb-2">{agent.name}</div>
        <div class="font-mono text-xs text-slate-500">{agent.cost}</div>
      </div>
    ))}
  </div>
</div>

<style>
  .agent-card {
    opacity: 0;
    transform: translateY(10px);
    animation: slideUp 0.5s ease forwards;
    animation-delay: var(--card-delay);
  }

  .status-pulse {
    animation: pulse 2s ease-in-out infinite;
  }

  @keyframes slideUp {
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  @media (prefers-reduced-motion: reduce) {
    .agent-card {
      opacity: 1;
      transform: none;
      animation: none;
    }
    .status-pulse {
      animation: none;
    }
  }
</style>
```

**Step 3: Create InstallCommand component (pure Astro, zero JS framework)**

Create `apps/landing/src/components/InstallCommand.astro`:

```astro
---
// Click-to-copy install command — vanilla JS, no React
---
<button
  class="install-cmd group flex items-center gap-3 bg-slate-800 border border-slate-700 rounded-lg px-6 py-3 font-mono text-lg text-green-400 hover:border-green-500/50 transition-colors cursor-pointer"
  title="Click to copy"
  data-command="npx claude-view"
>
  <span class="text-slate-500">$</span>
  <span>npx claude-view</span>
  <span class="install-icon ml-2 text-slate-500 group-hover:text-slate-300 transition-colors">
    <!-- Lucide Clipboard SVG (inline, no dependency) -->
    <svg class="icon-clipboard" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <rect width="8" height="4" x="8" y="2" rx="1" ry="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>
    </svg>
    <!-- Lucide Check SVG (hidden by default, shown on copy) -->
    <svg class="icon-check hidden" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M20 6 9 17l-5-5"/>
    </svg>
  </span>
</button>

<script>
  document.querySelectorAll('.install-cmd').forEach(btn => {
    btn.addEventListener('click', async () => {
      const command = btn.getAttribute('data-command') ?? '';
      const clipboardIcon = btn.querySelector('.icon-clipboard');
      const checkIcon = btn.querySelector('.icon-check');
      try {
        if (navigator.clipboard) {
          await navigator.clipboard.writeText(command);
        } else {
          const el = document.createElement('textarea');
          el.value = command;
          el.style.position = 'fixed';
          el.style.opacity = '0';
          document.body.appendChild(el);
          el.select();
          document.execCommand('copy');
          document.body.removeChild(el);
        }
        clipboardIcon?.classList.add('hidden');
        checkIcon?.classList.remove('hidden');
        setTimeout(() => {
          clipboardIcon?.classList.remove('hidden');
          checkIcon?.classList.add('hidden');
        }, 2000);
      } catch { /* copy failed — no feedback */ }
    });
  });
</script>
```

> **Zero-JS architecture:** Pure Astro component with vanilla `<script>`. Both Lucide SVG icons (Clipboard + Check) are in the DOM; toggled via `hidden` class — no innerHTML, no XSS surface. Clipboard API guard with `execCommand` fallback. Astro deduplicates the `<script>` tag automatically. No React, no hydration, no framework overhead.

**Step 4: Update homepage with hero components**

> **IMPORTANT — `index.astro` editing pattern:** This file is modified by Tasks 4, 5, 6, 10, and 11. Each task shows (a) imports to ADD to the existing frontmatter and (b) HTML to APPEND inside the `<MarketingLayout>` wrapper. Do NOT create a new frontmatter block — merge into the existing one from Task 3. A complete final `index.astro` is provided in Task 12 Step 0 as the canonical reference.

Add these imports to the existing `index.astro` frontmatter (after the `MarketingLayout` import from Task 3):

```typescript
// Add to frontmatter of index.astro
import AnimatedTerminal from '../components/AnimatedTerminal.astro';
import DashboardPreview from '../components/DashboardPreview.astro';
import InstallCommand from '../components/InstallCommand.astro';
// Note: GitHubStars import is added in Task 10 (when the component is created)
```

Replace the placeholder hero content inside `<MarketingLayout>` with:

```astro
<!-- Hero Section -->
<section class="relative min-h-[90vh] flex items-center justify-center overflow-hidden">
  <!-- Gradient background -->
  <div class="absolute inset-0 bg-gradient-to-b from-slate-950 via-slate-900 to-slate-950"></div>
  <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_top,rgba(34,197,94,0.08),transparent_60%)]"></div>

  <div class="relative z-10 max-w-6xl mx-auto px-6 text-center">
    <h1 class="text-5xl md:text-7xl font-bold tracking-tight text-white mb-6">
      Mission Control for<br />
      <span class="bg-gradient-to-r from-green-400 to-emerald-400 bg-clip-text text-transparent">AI Coding Agents</span>
    </h1>
    <p class="text-xl text-slate-400 max-w-2xl mx-auto mb-8">
      Monitor, orchestrate, and command your Claude Code fleet — from desktop or phone.
    </p>

    <div class="flex flex-col sm:flex-row items-center justify-center gap-4 mb-8">
      <InstallCommand />
      <!-- GitHubStars component added here in Task 10 -->
    </div>

    <!-- MCP integrations row — first-third positioning for GEO (ChatGPT cites first third 44% of time) -->
    <p class="text-sm text-slate-500 mb-12">
      Works with Claude Code via
      <a href="/docs/guides/mcp-integration/" class="text-green-400 hover:text-green-300 transition-colors">MCP — 8 tools</a>
      for monitoring, cost tracking, and agent control.
    </p>

    <div class="grid md:grid-cols-2 gap-8 max-w-4xl mx-auto">
      <AnimatedTerminal />
      <DashboardPreview />
    </div>
  </div>
</section>
```

**Step 5: Build and verify**

```bash
cd apps/landing && bun run build && bun run preview
```

Open in browser, verify: animated terminal plays, dashboard cards animate in, copy button works.

**Step 6: Commit**

```bash
git add apps/landing/src/components/ apps/landing/src/pages/index.astro
git commit -m "feat(landing): hero section with animated terminal and dashboard preview

CSS typewriter terminal, animated agent cards, click-to-copy install command.
All animations respect prefers-reduced-motion."
```

---

## Task 5: Feature Sections with Scroll Reveals

**Files:**
- Create: `apps/landing/src/components/FeatureSection.astro`
- Create: `apps/landing/src/components/PhoneMockup.astro`
- Modify: `apps/landing/src/pages/index.astro`

**Step 1: Create FeatureSection component**

Create `apps/landing/src/components/FeatureSection.astro`:

```astro
---
interface Props {
  title: string;
  description: string;
  reverse?: boolean;
}
const { title, description, reverse = false } = Astro.props;
---

<section class={`reveal-on-scroll opacity-0 translate-y-8 transition-all duration-700 py-24 px-6`}>
  <div class={`max-w-6xl mx-auto flex flex-col ${reverse ? 'md:flex-row-reverse' : 'md:flex-row'} items-center gap-12`}>
    <div class="flex-1 space-y-4">
      <h2 class="text-3xl md:text-4xl font-bold text-white">{title}</h2>
      <p class="text-lg text-slate-400 leading-relaxed">{description}</p>
    </div>
    <div class="flex-1">
      <slot />
    </div>
  </div>
</section>

<style>
  .reveal-on-scroll.is-visible {
    opacity: 1;
    transform: translateY(0);
  }

  @media (prefers-reduced-motion: reduce) {
    .reveal-on-scroll {
      opacity: 1;
      transform: none;
      transition: none;
    }
  }
</style>
```

**Step 2: Create PhoneMockup component**

Create `apps/landing/src/components/PhoneMockup.astro`:

```astro
---
/**
 * CSS-only 3D phone mockup with animated notification badges.
 * No JavaScript — pure CSS perspective + keyframe animations.
 */
---

<div class="phone-mockup" aria-hidden="true">
  <div class="phone-frame">
    <div class="phone-notch"></div>
    <div class="phone-screen">
      <!-- Notification badges -->
      <div class="notification notification-1">
        <span class="notification-dot bg-green-400"></span>
        <span class="text-xs text-slate-300">Agent completed task</span>
      </div>
      <div class="notification notification-2">
        <span class="notification-dot bg-yellow-400"></span>
        <span class="text-xs text-slate-300">Awaiting approval</span>
      </div>
      <div class="notification notification-3">
        <span class="notification-dot bg-blue-400"></span>
        <span class="text-xs text-slate-300">3 agents active</span>
      </div>
    </div>
  </div>
</div>

<style>
  .phone-mockup {
    perspective: 1000px;
    display: flex;
    justify-content: center;
  }

  .phone-frame {
    width: 240px;
    height: 480px;
    background: linear-gradient(135deg, #1e293b, #0f172a);
    border-radius: 36px;
    border: 2px solid #334155;
    padding: 12px;
    transform: rotateY(-8deg) rotateX(2deg);
    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
  }

  .phone-notch {
    width: 80px;
    height: 24px;
    background: #0f172a;
    border-radius: 0 0 16px 16px;
    margin: 0 auto 16px;
  }

  .phone-screen {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 8px;
  }

  .notification {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    background: rgba(30, 41, 59, 0.8);
    border-radius: 12px;
    border: 1px solid #334155;
    opacity: 0;
    animation: slide-in 0.5s ease forwards;
  }

  .notification-1 { animation-delay: 0.5s; }
  .notification-2 { animation-delay: 1.2s; }
  .notification-3 { animation-delay: 1.9s; }

  .notification-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  @keyframes slide-in {
    from { opacity: 0; transform: translateX(20px); }
    to { opacity: 1; transform: translateX(0); }
  }

  @media (prefers-reduced-motion: reduce) {
    .notification {
      opacity: 1;
      animation: none;
    }
  }
</style>
```

**Step 3: Add feature sections to homepage**

Add these imports to the existing `index.astro` frontmatter:

```typescript
// Add to frontmatter of index.astro
import FeatureSection from '../components/FeatureSection.astro';
import PhoneMockup from '../components/PhoneMockup.astro';
```

Append these sections inside `<MarketingLayout>`, after the hero `</section>`:

```astro
<!-- Features anchor for nav scroll -->
<div id="features"></div>

<!-- Features -->
<FeatureSection
  title="See every agent. Every token. Every decision."
  description="Real-time dashboard shows all active Claude Code sessions, token usage, costs, and tool calls. Know exactly what your agents are doing."
>
  <div class="rounded-xl border border-slate-700 bg-slate-900/50 p-4 text-left">
    <div class="flex items-center gap-2 mb-3">
      <span class="w-3 h-3 rounded-full bg-green-400"></span>
      <span class="text-sm text-slate-300">3 agents active</span>
    </div>
    <div class="space-y-2 text-xs text-slate-500 font-mono">
      <div>agent-1 | refactor auth module | $0.42 | 12k tokens</div>
      <div>agent-2 | write unit tests | $0.18 | 5k tokens</div>
      <div>agent-3 | fix CI pipeline | $0.31 | 9k tokens</div>
    </div>
  </div>
</FeatureSection>

<FeatureSection
  title="Approve. Reject. Resume. From anywhere."
  description="Full agent control from any device. Review tool calls, approve changes, or kill a runaway session — all without leaving your browser."
  reverse
>
  <div class="rounded-xl border border-slate-700 bg-slate-900/50 p-4 space-y-3">
    <div class="text-sm text-slate-300">Agent requests permission:</div>
    <div class="text-xs text-slate-400 font-mono bg-slate-800 rounded p-2">git push origin main</div>
    <div class="flex gap-2">
      <span class="px-3 py-1 rounded bg-green-600 text-white text-xs">Approve</span>
      <span class="px-3 py-1 rounded bg-red-600/20 text-red-400 text-xs border border-red-600/30">Reject</span>
    </div>
  </div>
</FeatureSection>

<FeatureSection
  title="I shipped a feature from my phone."
  description="Native mobile app connects to your agents via cloud relay. Monitor, approve, and control from the couch, the train, or the beach."
>
  <PhoneMockup />
</FeatureSection>

<FeatureSection
  title="Your AI fluency, measured."
  description="AI Fluency Score tracks how effectively you collaborate with AI agents. Session patterns, cost efficiency, and productivity trends — all quantified."
  reverse
>
  <div class="rounded-xl border border-slate-700 bg-slate-900/50 p-4 text-center">
    <div class="text-5xl font-bold text-green-400 mb-2">87</div>
    <div class="text-sm text-slate-400">AI Fluency Score</div>
    <div class="mt-4 flex justify-center gap-1">
      {/* Sparkline bars */}
      <div class="w-2 h-8 bg-slate-700 rounded-sm"></div>
      <div class="w-2 h-12 bg-slate-600 rounded-sm"></div>
      <div class="w-2 h-10 bg-slate-600 rounded-sm"></div>
      <div class="w-2 h-16 bg-green-500 rounded-sm"></div>
      <div class="w-2 h-14 bg-green-500 rounded-sm"></div>
      <div class="w-2 h-20 bg-green-400 rounded-sm"></div>
    </div>
  </div>
</FeatureSection>
```

> Each section alternates text-left/text-right via the `reverse` prop for visual rhythm.

**Step 4: Add Intersection Observer script**

Add this `<script>` tag at the bottom of `index.astro`:

```astro
<script>
  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.classList.add('is-visible');
          observer.unobserve(entry.target);
        }
      });
    },
    { threshold: 0.2 }
  );

  document.querySelectorAll('.reveal-on-scroll').forEach((el) => {
    observer.observe(el);
  });
</script>
```

> Observes all `.reveal-on-scroll` elements and adds `is-visible` class on viewport entry (threshold 0.2). Unobserves after trigger to prevent re-animation. The CSS transition is defined in `FeatureSection.astro`.

**Step 5: Build and test scroll behavior**

```bash
cd apps/landing && bun run build && bun run preview
```

Scroll through the page. Verify each section fades in as it enters the viewport.

**Step 6: Commit**

```bash
git add apps/landing/src/components/FeatureSection.astro apps/landing/src/components/PhoneMockup.astro apps/landing/src/pages/index.astro
git commit -m "feat(landing): scroll-triggered feature sections

Monitor, Control, Mobile, Analyze sections with Intersection Observer reveals.
3D phone mockup with CSS perspective.
Alternating text-left/right layout."
```

---

## Task 6: Pricing Page + Install Section

**Files:**
- Create: `apps/landing/src/pages/pricing.astro`
- Create: `apps/landing/src/components/PricingCards.astro`
- Modify: `apps/landing/src/pages/index.astro` (add install section + pricing preview)

**Step 1: Create PricingCards component**

Create `apps/landing/src/components/PricingCards.astro`:

```astro
---
interface Tier {
  name: string;
  price: string;
  description: string;
  features: string[];
  cta: string;
  ctaHref: string;
  highlight?: boolean;
  comingSoon?: boolean;
}

const tiers: Tier[] = [
  {
    name: 'Free',
    price: '$0',
    description: 'Everything you need for solo development.',
    features: [
      'Unlimited local sessions',
      'Session browser & search',
      'Cost tracking',
      'AI Fluency Score',
      'Community support',
    ],
    cta: 'Get Started',
    ctaHref: '/docs',
  },
  {
    name: 'Pro',
    price: '$19/mo',
    description: 'For power users who need cloud access.',
    features: [
      'Everything in Free',
      'Cloud relay access',
      'Mobile app',
      'Remote agent control',
      'Priority support',
    ],
    cta: 'Coming Soon',
    ctaHref: '',
    highlight: true,
    comingSoon: true,
  },
  {
    name: 'Team',
    price: '$49/mo',
    description: 'Shared dashboards for engineering teams.',
    features: [
      'Everything in Pro',
      'Team dashboard',
      'Shared session history',
      'Usage analytics',
      'SSO & admin controls',
    ],
    cta: 'Coming Soon',
    ctaHref: '',
    comingSoon: true,
  },
];
---

<div class="grid md:grid-cols-3 gap-8 max-w-5xl mx-auto">
  {tiers.map((tier) => (
    <div class={`rounded-2xl border p-8 flex flex-col ${
      tier.highlight
        ? 'border-green-500/50 bg-green-500/5'
        : 'border-slate-700 bg-slate-900/50'
    }`}>
      <h3 class="text-xl font-bold text-white">{tier.name}</h3>
      <div class="mt-4 mb-2">
        <span class="text-4xl font-bold text-white">{tier.price}</span>
      </div>
      <p class="text-sm text-slate-400 mb-6">{tier.description}</p>
      <ul class="space-y-3 mb-8 flex-1">
        {tier.features.map((f) => (
          <li class="flex items-start gap-2 text-sm text-slate-300">
            <svg class="w-4 h-4 text-green-400 mt-0.5 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="20 6 9 17 4 12" />
            </svg>
            {f}
          </li>
        ))}
      </ul>
      {tier.comingSoon ? (
        <button
          disabled
          class="block w-full text-center py-3 px-6 rounded-lg font-medium text-sm bg-slate-800 text-slate-400 cursor-not-allowed opacity-70"
        >
          {tier.cta}
        </button>
      ) : (
        <a
          href={tier.ctaHref}
          class="block text-center py-3 px-6 rounded-lg font-medium text-sm transition-colors bg-green-600 text-white hover:bg-green-500"
        >
          {tier.cta}
        </a>
      )}
    </div>
  ))}
</div>
```

**Step 2: Create pricing page**

Create `apps/landing/src/pages/pricing.astro`:

```astro
---
import MarketingLayout from '../layouts/MarketingLayout.astro';
import PricingCards from '../components/PricingCards.astro';
---

<MarketingLayout title="Pricing" description="Simple, transparent pricing. Free forever for local use.">
  <!-- FAQPage structured data for GEO — pricing questions are top AI-search queries -->
  <script type="application/ld+json" set:html={JSON.stringify({
    "@context": "https://schema.org",
    "@type": "FAQPage",
    "mainEntity": [
      {
        "@type": "Question",
        "name": "Is claude-view free?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "Yes. claude-view is free and open source for local use. Just run npx claude-view. No signup, no API key, no config."
        }
      },
      {
        "@type": "Question",
        "name": "What does the Pro plan include?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "The Pro plan ($19/mo, coming soon) adds cloud relay access, the mobile app for iOS and Android, and remote agent control from any device."
        }
      },
      {
        "@type": "Question",
        "name": "How do I install claude-view?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "Run npx claude-view in your terminal. Requires Node.js 18+. Works on macOS and Linux. No global install needed."
        }
      },
      {
        "@type": "Question",
        "name": "Can I monitor multiple Claude Code sessions?",
        "acceptedAnswer": {
          "@type": "Answer",
          "text": "Yes. claude-view automatically discovers all active Claude Code sessions on your machine and shows them in a single dashboard with real-time cost tracking."
        }
      }
    ]
  })} />

  <section class="py-24 px-6">
    <div class="max-w-6xl mx-auto text-center">
      <h1 class="text-4xl md:text-5xl font-bold text-white mb-4">Simple, transparent pricing</h1>
      <p class="text-lg text-slate-400 mb-16">Free forever for local use. Pay only for cloud features.</p>
      <PricingCards />
    </div>
  </section>
</MarketingLayout>
```

**Step 3: Add install section to homepage**

Append this section to `index.astro` after the feature sections:

```astro
<!-- Install CTA Section -->
<section class="py-24 px-6 bg-gradient-to-b from-slate-950 to-slate-900">
  <div class="max-w-3xl mx-auto text-center">
    <h2 class="text-3xl md:text-4xl font-bold text-white mb-4">Get started in 10 seconds</h2>
    <p class="text-lg text-slate-400 mb-8">No signup. No config. Just run one command.</p>
    <InstallCommand />
    <p class="text-sm text-slate-500 mt-4">Requires Node.js 18+. Works on macOS and Linux.</p>
  </div>
</section>
```

**Step 4: Add pricing preview to homepage**

Append this section to `index.astro` after the install CTA:

```astro
<!-- Pricing Preview -->
<section class="py-24 px-6">
  <div class="max-w-6xl mx-auto text-center">
    <h2 class="text-3xl md:text-4xl font-bold text-white mb-4">Free. Forever. For local use.</h2>
    <p class="text-lg text-slate-400 mb-12">Cloud features coming soon.</p>
    <PricingCards />
    <a href="/pricing" class="inline-block mt-8 text-sm text-green-400 hover:text-green-300 transition-colors">
      See full comparison &rarr;
    </a>
  </div>
</section>
```

**Step 5: Build and verify**

```bash
cd apps/landing && bun run build && bun run preview
```

Navigate to `/pricing`. Verify pricing cards render correctly.

**Step 6: Commit**

```bash
git add apps/landing/src/pages/pricing.astro apps/landing/src/components/PricingCards.astro apps/landing/src/pages/index.astro
git commit -m "feat(landing): pricing page and install section

Three-tier pricing (Free/Pro/Team). Pro and Team show 'Coming Soon'.
Animated terminal install section on homepage."
```

---

## Task 7: Starlight Documentation Pages

**Files:**
- Create: `apps/landing/src/content/docs/index.mdx`
- Create: `apps/landing/src/content/docs/installation.mdx`
- Create: `apps/landing/src/content/docs/features/session-browser.mdx`
- Create: `apps/landing/src/content/docs/features/mission-control.mdx`
- Create: `apps/landing/src/content/docs/features/agent-control.mdx`
- Create: `apps/landing/src/content/docs/features/ai-fluency-score.mdx`
- Create: `apps/landing/src/content/docs/features/search.mdx`
- Create: `apps/landing/src/content/docs/features/cost-tracking.mdx`
- Create: `apps/landing/src/content/docs/guides/mobile-setup.mdx`
- Create: `apps/landing/src/content/docs/guides/mcp-integration.mdx`
- Create: `apps/landing/src/content/docs/reference/cli-options.mdx`
- Create: `apps/landing/src/content/docs/reference/api.mdx`
- Create: `apps/landing/src/content/docs/reference/keyboard-shortcuts.mdx`

**Step 1: Write docs landing page** (`index.mdx`)

Getting started guide: what claude-view is, quick install (`npx claude-view`), what you'll see. Keep it under 300 words. Link to feature pages.

**Step 2: Write installation page**

Detailed install: prerequisites (macOS/Linux, Node 18+), install command, port config, first run, troubleshooting.

**Step 3: Write feature pages** (6 pages)

Each feature page: what it does, screenshot placeholder, key capabilities, relevant CLI flags. Source content from existing README and PROGRESS.md.

**Step 4: Write guide pages** (2 pages)

- `mobile-setup.mdx`: Placeholder for mobile app setup (coming with M1)
- `mcp-integration.mdx`: MCP server setup and tools reference. Source content from `docs/plans/2026-02-28-plugin-skill-mcp-impl.md` and `packages/mcp/src/`. Cover: 8 tools, `settings.json` config snippet, prerequisites (claude-view server must be running).

**Step 5: Write reference pages** (3 pages)

- `cli-options.mdx`: All CLI flags and env vars
- `api.mdx`: HTTP API endpoints (from existing Rust routes)
- `keyboard-shortcuts.mdx`: Keyboard shortcuts list

**Step 6: Build and verify docs**

```bash
cd apps/landing && bun run build && bun run preview
```

Navigate to `/docs`. Verify sidebar generates correctly with all sections.

**Step 7: Commit**

```bash
git add apps/landing/src/content/docs/
git commit -m "docs(landing): initial Starlight documentation (13 pages)

Getting started, installation, 6 feature pages, 1 guide placeholder (mobile-setup),
1 guide (mcp-integration), 3 reference pages. Sidebar auto-generates from directory structure."
```

---

## Task 8: Blog + Changelog

**Files:**
- Verify: `apps/landing/src/content.config.ts` (blog + changelog schemas already added in Task 1)
- Create: `apps/landing/src/layouts/BlogLayout.astro`
- Create: `apps/landing/src/pages/blog/index.astro`
- Create: `apps/landing/src/pages/blog/[slug].astro`
- Create: `apps/landing/src/pages/changelog.astro`
- Create: `apps/landing/src/content/blog/introducing-claude-view.mdx`
- Create: `apps/landing/src/content/changelog/v0.8.0.md`

**Step 1: Verify content collection schemas**

Blog and changelog collection schemas were already defined in Task 1's `src/content.config.ts`. Verify that the `blog` and `changelog` collections exist with their `z.object` schemas before proceeding. No modification needed here.

> **Audit fix:** Schemas moved to Task 1 to prevent parallel execution race condition — Tasks 7 and 8 can now safely run in parallel.

**Step 2: Create BlogLayout and pages**

Create `apps/landing/src/layouts/BlogLayout.astro`:

```astro
---
import MarketingLayout from './MarketingLayout.astro';

interface Props {
  title: string;
  description: string;
  date: Date;
  author: string;
}

const { title, description, date, author } = Astro.props;
---
<MarketingLayout
  title={title}
  description={description}
  ogType="article"
  articleMeta={{ publishedTime: date.toISOString(), author }}
>
  <!-- BlogPosting structured data for GEO (Generative Engine Optimization) -->
  <script type="application/ld+json" set:html={JSON.stringify({
    "@context": "https://schema.org",
    "@type": "BlogPosting",
    "headline": title,
    "description": description,
    "datePublished": date.toISOString(),
    "author": { "@type": "Person", "name": author },
    "publisher": {
      "@type": "Organization",
      "name": "claude-view",
      "url": "https://claude-view.dev"
    },
    "isPartOf": {
      "@type": "Blog",
      "name": "claude-view Blog",
      "url": "https://claude-view.dev/blog"
    }
  })} />

  <article class="mx-auto max-w-3xl px-6 py-16">
    <header class="mb-12">
      <h1 class="font-heading text-4xl font-bold mb-4">{title}</h1>
      <div class="flex items-center gap-4 text-sm text-slate-400">
        <time datetime={date.toISOString()}>{date.toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' })}</time>
        <span>·</span>
        <span>{author}</span>
      </div>
    </header>
    <div class="prose prose-invert prose-slate max-w-none">
      <slot />
    </div>
  </article>
</MarketingLayout>
```

Create `apps/landing/src/pages/blog/index.astro`:

```astro
---
import { getCollection } from 'astro:content';
import MarketingLayout from '../../layouts/MarketingLayout.astro';

const posts = (await getCollection('blog')).sort((a, b) =>
  new Date(b.data.date).getTime() - new Date(a.data.date).getTime()
);
---
<MarketingLayout title="Blog">
  <section class="mx-auto max-w-3xl px-6 py-16">
    <h1 class="font-heading text-4xl font-bold mb-12">Blog</h1>
    <div class="space-y-8">
      {posts.map(post => (
        <a href={`/blog/${post.id.replace(/\.(mdx?|md)$/, '')}`} class="block group cursor-pointer">
          <article class="border-b border-slate-800 pb-8">
            <time class="text-sm text-slate-500" datetime={post.data.date.toISOString()}>
              {post.data.date.toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' })}
            </time>
            <h2 class="font-heading text-xl font-semibold mt-2 group-hover:text-cta transition-colors">{post.data.title}</h2>
            <p class="text-slate-400 mt-2">{post.data.description}</p>
          </article>
        </a>
      ))}
    </div>
  </section>
</MarketingLayout>
```

Create `apps/landing/src/pages/blog/[slug].astro`:

```astro
---
import { getCollection, render } from 'astro:content';
import BlogLayout from '../../layouts/BlogLayout.astro';

export async function getStaticPaths() {
  const posts = await getCollection('blog');
  return posts.map(post => ({
    params: { slug: post.id.replace(/\.(mdx?|md)$/, '') },
    props: { post },
  }));
}

const { post } = Astro.props;
const { Content } = await render(post);
---
<BlogLayout
  title={post.data.title}
  description={post.data.description}
  date={post.data.date}
  author={post.data.author}
>
  <Content />
</BlogLayout>
```

> **Audit fix:** Added full code for BlogLayout, blog index, and blog `[slug]` dynamic route. Previously prose-only — an executor unfamiliar with Astro 5's Content Layer API could not implement these without guessing.

**Step 3: Create changelog page**

Create `apps/landing/src/pages/changelog.astro`:

```astro
---
import { getCollection, render } from 'astro:content';
import MarketingLayout from '../layouts/MarketingLayout.astro';

const entries = (await getCollection('changelog')).sort((a, b) =>
  new Date(b.data.date).getTime() - new Date(a.data.date).getTime()
);

// Pre-render all entries in frontmatter to avoid async .map() in template
// (Astro's template engine needs sequential Content resolution for proper CSS/JS bundling)
const rendered = [];
for (const entry of entries) {
  const { Content } = await render(entry);
  rendered.push({ entry, Content });
}
---
<MarketingLayout title="Changelog">
  <section class="mx-auto max-w-3xl px-6 py-16">
    <h1 class="font-heading text-4xl font-bold mb-12">Changelog</h1>
    <div class="space-y-12">
      {rendered.map(({ entry, Content }) => (
        <article class="border-b border-slate-800 pb-8">
          <div class="flex items-center gap-3 mb-4">
            <span class="px-3 py-1 bg-green-500/10 text-green-400 text-sm font-mono rounded-full">{entry.data.version}</span>
            <time class="text-sm text-slate-500" datetime={entry.data.date.toISOString()}>
              {entry.data.date.toLocaleDateString('en-US', { year: 'numeric', month: 'long', day: 'numeric' })}
            </time>
          </div>
          <h2 class="font-heading text-xl font-semibold mb-4">{entry.data.title}</h2>
          <div class="prose prose-invert prose-slate max-w-none">
            <Content />
          </div>
        </article>
      ))}
    </div>
  </section>
</MarketingLayout>
```

> **Audit fix:** Moved `render()` calls from async `.map()` in template to sequential `for...of` loop in frontmatter. Astro's template engine resolves Content components sequentially for proper CSS/JS bundling — async `.map()` can cause broken styles ([withastro/astro#6672](https://github.com/withastro/astro/issues/6672)).

**Step 4: Write initial blog post**

Create `apps/landing/src/content/blog/introducing-claude-view.mdx`. Must include this frontmatter (schema validation will fail without it):

```mdx
---
title: Introducing claude-view
description: Monitor, orchestrate, and command your Claude Code sessions from a web dashboard or your phone.
date: 2026-02-28
author: claude-view team
---

<!-- ~500 words: what it is, why we built it, key features, how to try it.
     This is the L1 launch companion post. -->
```

**Step 5: Write initial changelog entry**

Create `apps/landing/src/content/changelog/v0.8.0.md`. Must include this frontmatter (schema validation will fail without it):

```md
---
title: v0.8.0 — Mission Control
date: 2026-02-28
version: v0.8.0
---

<!-- Current feature list: session browser, cost tracking, Mission Control, etc. -->
```

**Step 6: Build and verify**

```bash
cd apps/landing && bun run build && bun run preview
```

Navigate to `/blog` and `/changelog`. Verify posts render.

**Step 7: Commit**

```bash
git add apps/landing/src/content/blog/ apps/landing/src/content/changelog/ apps/landing/src/layouts/BlogLayout.astro apps/landing/src/pages/blog/ apps/landing/src/pages/changelog.astro
git commit -m "feat(landing): blog and changelog

Blog with MDX posts, changelog from markdown.
Initial 'Introducing claude-view' post and v0.8.0 changelog."
```

---

## Task 9: Agent SEO — llms.txt + OG Image + Sitemap + Structured Data + HowTo + BreadcrumbList

**Files:**
- Create: `apps/landing/public/llms.txt`
- Create: `apps/landing/public/llms-full.txt` (build script)
- Create: `apps/landing/public/robots.txt`
- Create: `apps/landing/public/favicon.svg`
- Create: `apps/landing/public/og-image.png`
- Modify: `apps/landing/src/layouts/MarketingLayout.astro` (add BreadcrumbList JSON-LD)
- Modify: `apps/landing/src/content/docs/installation.mdx` (add HowTo JSON-LD)
- Modify: `apps/landing/astro.config.mjs` (add BreadcrumbList to Starlight head config)
- Verify: Schema.org in MarketingLayout (already added in Task 3)
- Verify: OG tags in MarketingLayout (already added in Task 3)
- Verify: `@astrojs/sitemap` generates sitemap.xml (added in Task 1)

**Step 1: Create llms.txt**

Create `apps/landing/public/llms.txt` with the exact content specified in the design doc:

```markdown
# claude-view

> Mission Control for AI coding agents — monitor, orchestrate, and command your fleet from desktop or phone.

## About
claude-view is an open-source developer tool that monitors Claude Code sessions,
tracks costs, and lets you control AI agents from a web dashboard or native mobile app.
Zero config: `npx claude-view`. Rust backend, React web app, ~15MB binary.
Landing page built with Astro 5 (zero client-side JS).

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

> **Audit fix:** Inlined the exact content from the design doc rather than referencing it. An executor should be able to copy-paste this verbatim.

> **Note:** Some documentation platforms (e.g., Fern) offer proprietary `<llms-only>` and `<llms-ignore>` tags for controlling AI-visible content. These are platform-specific features, not part of the llms.txt spec. Vercel has separately proposed `<script type="text/llms.txt">` for inline LLM instructions (browsers ignore unknown script types). Neither is standardized — we skip both for V1 and rely on well-structured content instead.

**Step 2: Create llms-full.txt build script**

Create `apps/landing/scripts/generate-llms-full.mjs`:

```javascript
// Concatenates all MDX/MD content into public/llms-full.txt for AI agent consumption
import { readdir, readFile, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const contentDir = resolve(__dirname, '../src/content');
const outputPath = resolve(__dirname, '../public/llms-full.txt');

if (!existsSync(contentDir)) {
  console.warn('Content directory not found, skipping llms-full.txt generation');
  process.exit(0);
}

async function collectFiles(dir, base = dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...await collectFiles(fullPath, base));
    } else if (/\.(md|mdx)$/.test(entry.name)) {
      files.push(fullPath);
    }
  }
  return files;
}

const files = (await collectFiles(contentDir)).sort();
const sections = [];
for (const file of files) {
  const content = await readFile(file, 'utf-8');
  const relative = file.replace(contentDir + '/', '');
  sections.push(`--- ${relative} ---\n\n${content}`);
}

await writeFile(outputPath, sections.join('\n\n'));
console.log(`llms-full.txt generated (${files.length} files)`);
```

> **Audit fix:** Replaced `readdir({ recursive: true })` + `e.parentPath` (Node 20.12+) with manual recursive traversal for Node 18 compatibility. The plan's installation docs specify "Node 18+" as minimum.

Add a `prebuild` script to `package.json`:
```json
"prebuild": "node scripts/generate-llms-full.mjs"
```

> **Audit fix:** `llms-full.txt` was in the design doc but completely absent from the plan. This script auto-generates it from all content before each build.

**Step 3: Create robots.txt**

```
User-agent: *
Allow: /

# Explicitly allow AI crawlers (prevents accidental blocking via Cloudflare WAF rules)
User-agent: ClaudeBot
Allow: /

User-agent: GPTBot
Allow: /

User-agent: PerplexityBot
Allow: /

Sitemap: https://claude-view.dev/sitemap.xml
```

> **Agent SEO:** Sitemap generated by `@astrojs/sitemap` (Task 1). AI crawlers (ClaudeBot, GPTBot, PerplexityBot) explicitly allowed — many sites accidentally block these via overzealous WAF rules. The `LLMs-Txt` directive was removed: it is not part of the robots.txt specification (RFC 9309) and no crawler reads it.

**Step 4: Create Cloudflare `_headers` file for cache control**

Create `apps/landing/public/_headers`:

```
/_astro/*
  Cache-Control: public, max-age=31536000, immutable

/*
  Cache-Control: public, max-age=0, must-revalidate

/llms.txt
  Cache-Control: public, max-age=3600

/llms-full.txt
  Cache-Control: public, max-age=3600
```

> **Audit fix (B5):** Without explicit cache headers, Cloudflare Pages uses defaults. Astro/Vite generates content-hashed assets (`_astro/*.{hash}.js/css`) that should be cached forever (`immutable`). HTML pages should always revalidate to pick up deploys immediately. LLM files cache for 1 hour.

**Step 5: Create favicon**

Create `apps/landing/public/favicon.svg` (referenced by `MarketingLayout.astro`):

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">
  <rect width="32" height="32" rx="6" fill="#0f172a"/>
  <text x="7" y="23" font-size="20" fill="#22c55e" font-family="monospace">▸</text>
</svg>
```

**Step 6: Create OG image**

Create `apps/landing/public/og-image.png` — a 1200x630 PNG with:
- Dark background (`#0F172A`)
- "claude-view" in Outfit Bold, white
- "Mission Control for AI Coding Agents" subtitle in IBM Plex Sans, slate-400
- Green terminal cursor accent

This can be created with any design tool or a simple HTML-to-PNG script. The exact design is flexible, but the file MUST exist — every page's OG tags reference `/og-image.png`.

> **AVIF note:** The OG image MUST remain PNG. Social platforms (Twitter/X, Slack, Discord, LinkedIn, Facebook) do not support AVIF or WebP in `og:image` meta tags — they will show a broken/missing preview. AVIF is only for on-page `<img>` and `<picture>` elements where the browser controls format negotiation.

> **Audit fix:** `og-image.png` was referenced in MarketingLayout's default `ogImage` prop but never created. Every social share (Twitter, Slack, Discord) would show a broken image without it.
>
> **Note:** This file is trackable by git because Task 1 Step 5 added `!apps/landing/public/*.png` to root `.gitignore` (overriding the global `*.png` ignore rule).

**Step 6a: AVIF image optimization pipeline (future-proofing)**

For V1, the landing page uses mostly CSS animations and minimal raster images (only `og-image.png`). When screenshots, illustrations, or hero images are added post-V1, use Astro's built-in image optimization:

1. Place source images in `src/assets/` (NOT `public/` — files in `public/` bypass Astro's image pipeline)
2. Use the `<Picture>` component from `astro:assets` with AVIF + WebP formats:

```astro
---
import { Picture } from 'astro:assets';
import dashboardScreenshot from '../assets/dashboard-screenshot.png';
---

<!-- Astro generates <source> tags for AVIF and WebP, with PNG fallback -->
<Picture
  src={dashboardScreenshot}
  formats={['avif', 'webp']}
  alt="claude-view dashboard showing 4 active Claude Code agents"
  widths={[640, 1024, 1440]}
  sizes="(max-width: 768px) 100vw, (max-width: 1024px) 80vw, 1200px"
/>
```

Generated HTML:
```html
<picture>
  <source srcset="/_astro/dashboard-screenshot.hash.avif 640w, ..." type="image/avif" />
  <source srcset="/_astro/dashboard-screenshot.hash.webp 640w, ..." type="image/webp" />
  <img src="/_astro/dashboard-screenshot.hash.png" alt="..." loading="lazy" decoding="async" />
</picture>
```

> **Why AVIF first in the formats array?** The browser picks the first supported format from `<source>` tags. AVIF is ~50% smaller than JPEG and ~20% smaller than WebP (Netflix, Google research). With 93%+ browser support (Chrome 85+, Firefox 93+, Safari 16.4+, Edge 121+), AVIF should always be listed before WebP. The fallback chain is: AVIF → WebP → original format (PNG/JPEG).
>
> **No external CDN needed:** Astro's `astro:assets` handles format conversion, resizing, and lazy loading at build time. The optimized images are output to `dist/_astro/` with content hashes (covered by the `immutable` cache header in `_headers`).

**Step 7: Enable Cloudflare Markdown for Agents**

> **Important caveats:**
> - This feature is NOT automatic — it must be explicitly enabled via Dashboard or API.
> - **Requires a paid Cloudflare plan (Pro+, $20/mo).** Availability on free Pages plans with custom domains is undocumented as of Feb 2026.
> - Pages compatibility (`.pages.dev` subdomains vs custom domains) is not explicitly documented.
> - **Fallback:** `llms.txt` and `llms-full.txt` serve as agent-readable content regardless of whether this feature works.
> - Test thoroughly after enabling — if the `curl` test returns HTML, the feature may not be available on your plan.

**Option A: Via Cloudflare Dashboard**
1. Log into Cloudflare Dashboard → select zone (`claude-view.dev`)
2. Quick Actions → toggle **Markdown for Agents** to ON

**Option B: Via API**
```bash
curl -X PATCH "https://api.cloudflare.com/client/v4/zones/{zone_tag}/settings/content_converter" \
  -H "Authorization: Bearer $CF_API_TOKEN" \
  -H "Content-Type: application/json" \
  --data '{"value": "on"}'
```

> **Known gotcha:** Cloudflare's Markdown for Agents does NOT work if the origin sends compressed (gzip/brotli) responses. If the `curl` test below returns HTML instead of markdown, check origin compression settings in Cloudflare → Speed → Optimization → Content Optimization.

**Step 8: Verify Cloudflare Markdown for Agents**

After deploying, test with:

```bash
curl -s -H "Accept: text/markdown" https://claude-view.dev/ | head -20
```

Should return clean markdown version of the homepage (starting with `# claude-view` or similar). If it returns HTML, the feature is not enabled or origin compression is interfering.

Also test docs:
```bash
curl -s -H "Accept: text/markdown" https://claude-view.dev/docs/ | head -20
```

**Step 9: Add BreadcrumbList JSON-LD to MarketingLayout.astro**

Add BreadcrumbList structured data to `MarketingLayout.astro`, dynamically built from the current URL path segments. This goes in the `<head>` after the existing `SoftwareApplication` JSON-LD block.

> **Why BreadcrumbList?** BreadcrumbList communicates site hierarchy to both search engines and AI parsers. Google continues to support BreadcrumbList rich results, and AI engines use it to understand page relationships when generating citations. **Starlight does NOT auto-generate BreadcrumbList JSON-LD** -- it renders visual breadcrumb UI in the sidebar but emits no `<script type="application/ld+json">` structured data. Marketing pages need their own implementation.

Add the following to the `MarketingLayout.astro` frontmatter (after the existing `canonicalURL` declaration):

```typescript
// Build BreadcrumbList from URL path segments
const pathSegments = Astro.url.pathname.split('/').filter(Boolean);
const breadcrumbItems = [
  { "@type": "ListItem" as const, position: 1, name: "Home", item: new URL('/', Astro.site).href },
];
let currentPath = '';
for (let i = 0; i < pathSegments.length; i++) {
  currentPath += `/${pathSegments[i]}`;
  const name = pathSegments[i].charAt(0).toUpperCase() + pathSegments[i].slice(1).replace(/-/g, ' ');
  const isLast = i === pathSegments.length - 1;
  breadcrumbItems.push({
    "@type": "ListItem" as const,
    position: i + 2,
    name,
    // Per Google's docs: last item omits `item` -- the containing page URL is used implicitly
    ...(isLast ? {} : { item: new URL(`${currentPath}/`, Astro.site).href }),
  });
}
```

Then add this `<script>` tag in the `<head>`, immediately after the existing `SoftwareApplication` JSON-LD:

```astro
  <!-- BreadcrumbList -- helps AI engines understand site hierarchy -->
  {pathSegments.length > 0 && (
    <script type="application/ld+json" set:html={JSON.stringify({
      "@context": "https://schema.org",
      "@type": "BreadcrumbList",
      "itemListElement": breadcrumbItems,
    })} />
  )}
```

> **Note:** The homepage (`/`) has zero path segments, so it correctly skips BreadcrumbList (a single-item breadcrumb is semantically meaningless). Pages like `/pricing/` emit `Home > Pricing`. Blog posts at `/blog/welcome/` emit `Home > Blog > Welcome`.

**Step 10: Add HowTo JSON-LD to installation docs page**

Add HowTo structured data to `src/content/docs/installation.mdx`. This makes the installation steps machine-readable for AI engines that parse JSON-LD when generating step-by-step instructions in their responses.

> **Google deprecation context:** Google removed HowTo rich results from SERPs in September 2023. However, Schema.org `HowTo` remains a valid vocabulary type. Microsoft confirmed in March 2025 (Fabrice Canel, Principal PM at Bing) that structured data helps their LLMs interpret web content. Perplexity, ChatGPT with browsing, and Claude with web search all parse JSON-LD. The implementation cost is one JSON-LD block; the AI citation upside is high.

Since `installation.mdx` is a Starlight docs page (not a custom Astro layout), inject the JSON-LD via the page's MDX content. Add this at the **top** of `src/content/docs/installation.mdx`, immediately after the frontmatter closing `---`:

```mdx
<script type="application/ld+json" set:html={JSON.stringify({
  "@context": "https://schema.org",
  "@type": "HowTo",
  "name": "Install claude-view",
  "description": "Install and start claude-view to monitor your Claude Code sessions. Zero config -- one command, 15-second setup.",
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
})} />
```

> **Schema.org property reference:**
> - `totalTime`: ISO 8601 duration (`PT15S` = 15 seconds). Reflects the actual wall-clock time for `npx claude-view` to download and start.
> - `tool`: Items needed but not consumed. Node.js is the only prerequisite.
> - `step[].position`: Integer starting at 1. Required by Schema.org for ordering.
> - `step[].name`: Short label for the step. `step[].text`: Detailed instruction text.
> - We omit `supply` (nothing consumed), `estimatedCost` (free), and `yield` (not applicable for software installation).

**Step 11: Add BreadcrumbList JSON-LD to Starlight docs pages**

Starlight renders visual breadcrumbs in the sidebar but does **not** emit BreadcrumbList structured data. Add a BreadcrumbList JSON-LD override to the Starlight `head` config in `astro.config.mjs`.

Since Starlight's `head` config only accepts static values (not dynamic per-page values), the best approach is to add a Starlight component override. However, for V1 simplicity, we add a static two-level breadcrumb to all docs pages via the existing `head` config:

Update the Starlight `head` array in `apps/landing/astro.config.mjs` (Task 1's config) to include BreadcrumbList alongside the existing TechArticle:

```javascript
head: [
  {
    tag: 'script',
    attrs: { type: 'application/ld+json' },
    content: JSON.stringify({
      "@context": "https://schema.org",
      "@type": "TechArticle",
      "isPartOf": {
        "@type": "SoftwareApplication",
        "name": "claude-view",
        "applicationCategory": "DeveloperApplication"
      }
    }),
  },
  {
    tag: 'script',
    attrs: { type: 'application/ld+json' },
    content: JSON.stringify({
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
          "name": "Documentation",
          "item": "https://claude-view.dev/docs/"
        },
        {
          "@type": "ListItem",
          "position": 3,
          "name": "Current Page"
        }
      ]
    }),
  },
],
```

> **Limitation (V1):** Starlight's static `head` config applies the same JSON-LD to all docs pages, so the third breadcrumb item always says "Current Page" rather than the actual page title. For V2, this can be improved by creating a Starlight [Head component override](https://starlight.astro.build/guides/overriding-components/) that reads `Astro.props.entry.data.title` and `Astro.url.pathname` to build accurate per-page breadcrumbs. The V1 static version still communicates the `Home > Documentation` hierarchy to AI parsers, which is the primary value.

**Step 12: Commit**

```bash
git add apps/landing/public/ apps/landing/scripts/ apps/landing/package.json apps/landing/astro.config.mjs apps/landing/src/layouts/MarketingLayout.astro apps/landing/src/content/docs/installation.mdx
git commit -m "feat(landing): agent SEO -- llms.txt, og-image, sitemap, favicon, GEO schema

llms.txt + llms-full.txt for AI agent discoverability.
OG image for social sharing. Sitemap via @astrojs/sitemap.
HowTo JSON-LD on installation page for AI citation.
BreadcrumbList JSON-LD on marketing + docs pages.
Cloudflare Markdown for Agents enabled (requires dashboard toggle)."
```

---

## Task 10: GitHub Stars Badge (Zero-JS Astro Component)

**Files:**
- Create: `apps/landing/src/components/GitHubStars.astro`
- Modify: `apps/landing/src/pages/index.astro` (add to hero)

**Step 1: Create GitHubStars component**

Create `apps/landing/src/components/GitHubStars.astro`:

```astro
---
interface Props {
  repo: string;
}
const { repo } = Astro.props;
---

<a
  class="github-stars hidden items-center gap-2 px-4 py-2 rounded-lg border border-slate-700 text-sm text-slate-300 hover:border-slate-500 hover:text-slate-100 transition-colors cursor-pointer"
  href={`https://github.com/${repo}`}
  target="_blank"
  rel="noopener noreferrer"
  data-repo={repo}
>
  <!-- Lucide Star icon (inline SVG) -->
  <svg class="w-4 h-4 fill-current" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" fill="currentColor" />
  </svg>
  <span class="star-count"></span>
</a>

<script>
  document.querySelectorAll('.github-stars').forEach((el) => {
    const link = el as HTMLAnchorElement;
    const repo = link.dataset.repo;
    if (!repo) return;

    const cacheKey = `gh-stars-${repo}`;
    const cacheTimeKey = `gh-stars-${repo}-at`;
    const cached = localStorage.getItem(cacheKey);
    const cachedAt = localStorage.getItem(cacheTimeKey);

    function show(count: number) {
      const span = link.querySelector('.star-count');
      if (span) span.textContent = count.toLocaleString();
      link.classList.remove('hidden');
      link.classList.add('inline-flex');
    }

    // Use cache if less than 1 hour old
    if (cached && cachedAt && Date.now() - Number(cachedAt) < 3600000) {
      show(Number(cached));
      return;
    }

    fetch(`https://api.github.com/repos/${repo}`)
      .then(r => r.json())
      .then(data => {
        if (data.stargazers_count != null) {
          const count = data.stargazers_count;
          localStorage.setItem(cacheKey, String(count));
          localStorage.setItem(cacheTimeKey, String(Date.now()));
          show(count);
        }
      })
      .catch(() => {/* silent fail — badge stays hidden */});
  });
</script>
```

> **Zero-JS architecture:** No React. The badge starts hidden and only appears after the GitHub API responds (or from localStorage cache). Inline `<script>` runs once on page load — Astro does not ship a framework runtime.

**Step 2: Add to hero**

In `index.astro`, import and render the GitHubStars component:

```astro
---
import GitHubStars from '../components/GitHubStars.astro';
---

<!-- In hero section, after InstallCommand -->
<GitHubStars repo="anthropics/claude-view" />
```

> No `client:visible` directive needed — Astro components are static by default. The inline `<script>` handles the dynamic behavior.

**Step 3: Build and verify**

```bash
cd apps/landing && bun run build
```

**Step 4: Commit**

```bash
git add apps/landing/src/components/GitHubStars.astro apps/landing/src/pages/index.astro
git commit -m "feat(landing): GitHub stars badge (vanilla JS)

Dynamic star count from GitHub API with 1h localStorage cache.
Zero-JS framework — inline script, no React hydration."
```

---

## Task 11: Deep Link Preservation + Mobile App Pairing

**Files:**
- Modify: `apps/landing/src/pages/index.astro`

**Step 1: Add deep link handler**

Add this script at the top of the template body in `index.astro` (inside `<MarketingLayout>`, before the hero section). The `is:inline` directive prevents Vite bundling, and `window.location.href` redirect fires before DOM rendering so there's no flash of content:

```astro
<script is:inline>
  // Deep link handler: if opened with ?k=...&t=..., redirect to app
  // Security: only forward whitelisted params (k, t) to prevent parameter injection
  (function() {
    var params = new URLSearchParams(window.location.search);
    if (params.has('k') && params.has('t')) {
      var safe = new URLSearchParams();
      safe.set('k', params.get('k'));
      safe.set('t', params.get('t'));
      window.location.href = 'claude-view://pair?' + safe.toString();
    }
  })();
</script>
```

**Step 2: Create AppStoreBadges component**

Create `apps/landing/src/components/AppStoreBadges.astro`:

```astro
---
/**
 * App Store and Play Store badge links.
 * Placeholder URLs until the mobile app is published.
 */
---

<div class="flex flex-wrap items-center justify-center gap-4">
  <a
    href="#"
    class="inline-flex items-center gap-3 px-5 py-3 rounded-xl border border-slate-700 bg-slate-900/50 hover:border-slate-500 transition-colors"
    aria-label="Download on the App Store (coming soon)"
  >
    <!-- Apple icon (inline SVG) -->
    <svg class="w-6 h-6 text-white" viewBox="0 0 24 24" fill="currentColor">
      <path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.8-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z"/>
    </svg>
    <div class="text-left">
      <div class="text-[10px] text-slate-400 leading-none">Download on the</div>
      <div class="text-sm font-semibold text-white leading-tight">App Store</div>
    </div>
  </a>

  <a
    href="#"
    class="inline-flex items-center gap-3 px-5 py-3 rounded-xl border border-slate-700 bg-slate-900/50 hover:border-slate-500 transition-colors"
    aria-label="Get it on Google Play (coming soon)"
  >
    <!-- Play Store icon (inline SVG) -->
    <svg class="w-6 h-6 text-white" viewBox="0 0 24 24" fill="currentColor">
      <path d="M3 20.5v-17c0-.59.34-1.11.84-1.35L13.69 12l-9.85 9.85c-.5-.24-.84-.76-.84-1.35zm13.81-5.38L6.05 21.34l8.49-8.49 2.27 2.27zm.91-.91L19.59 12l-1.87-2.21-2.27 2.27 2.27 2.15zM6.05 2.66l10.76 6.22-2.27 2.27-8.49-8.49z"/>
    </svg>
    <div class="text-left">
      <div class="text-[10px] text-slate-400 leading-none">Get it on</div>
      <div class="text-sm font-semibold text-white leading-tight">Google Play</div>
    </div>
  </a>
</div>

<p class="text-center text-xs text-slate-500 mt-3">Coming soon — sign up to be notified.</p>
```

Add the import to `index.astro` frontmatter and update the Mobile feature section:

```typescript
// Add to frontmatter of index.astro
import AppStoreBadges from '../components/AppStoreBadges.astro';
```

Update the Mobile FeatureSection in `index.astro` to include badges below the phone mockup:

```astro
<FeatureSection
  title="I shipped a feature from my phone."
  description="Native mobile app connects to your agents via cloud relay. Monitor, approve, and control from the couch, the train, or the beach."
>
  <PhoneMockup />
  <div class="mt-6">
    <AppStoreBadges />
  </div>
</FeatureSection>
```

**Step 3: Commit**

```bash
git add apps/landing/src/pages/index.astro apps/landing/src/components/AppStoreBadges.astro
git commit -m "feat(landing): preserve deep linking for mobile app pairing

?k=...&t=... params redirect to claude-view:// deep link.
App Store badges in Mobile feature section."
```

---

## Task 12: Final Wiring, Build Verification, and Turbo Integration

**Files:**
- Verify: `apps/landing/src/pages/index.astro` (canonical final version below)
- Verify: `apps/landing/wrangler.toml` (should still work with new dist/)
- Verify: Root `turbo.json` (build task already covers apps/*)
- Verify: Root `package.json` (workspaces already covers apps/*)

**Step 0: Canonical `index.astro` — the final assembled file**

> This is the COMPLETE `apps/landing/src/pages/index.astro` after all tasks (3-11) are applied. If Tasks 4-11 were applied as incremental diffs, verify the final state matches this file exactly. If any section was missed, copy from here.

```astro
---
import MarketingLayout from '../layouts/MarketingLayout.astro';
import AnimatedTerminal from '../components/AnimatedTerminal.astro';
import DashboardPreview from '../components/DashboardPreview.astro';
import InstallCommand from '../components/InstallCommand.astro';
import GitHubStars from '../components/GitHubStars.astro';
import FeatureSection from '../components/FeatureSection.astro';
import PhoneMockup from '../components/PhoneMockup.astro';
import AppStoreBadges from '../components/AppStoreBadges.astro';
import PricingCards from '../components/PricingCards.astro';
---

<MarketingLayout title="Mission Control for AI Coding Agents">
  <!-- Deep link handler (is:inline prevents Vite bundling) -->
  <script is:inline>
    (function() {
      var params = new URLSearchParams(window.location.search);
      if (params.has('k') && params.has('t')) {
        var safe = new URLSearchParams();
        safe.set('k', params.get('k'));
        safe.set('t', params.get('t'));
        window.location.href = 'claude-view://pair?' + safe.toString();
      }
    })();
  </script>

  <!-- Hero Section -->
  <section class="relative min-h-[90vh] flex items-center justify-center overflow-hidden">
    <div class="absolute inset-0 bg-gradient-to-b from-slate-950 via-slate-900 to-slate-950"></div>
    <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_top,rgba(34,197,94,0.08),transparent_60%)]"></div>

    <div class="relative z-10 max-w-6xl mx-auto px-6 text-center">
      <h1 class="text-5xl md:text-7xl font-bold tracking-tight text-white mb-6">
        Mission Control for<br />
        <span class="bg-gradient-to-r from-green-400 to-emerald-400 bg-clip-text text-transparent">AI Coding Agents</span>
      </h1>
      <p class="text-xl text-slate-400 max-w-2xl mx-auto mb-8">
        Monitor, orchestrate, and command your Claude Code fleet — from desktop or phone.
      </p>

      <div class="flex flex-col sm:flex-row items-center justify-center gap-4 mb-8">
        <InstallCommand />
        <GitHubStars repo="anthropics/claude-view" />
      </div>

      <!-- MCP integrations row — first-third positioning for GEO (ChatGPT cites first third 44% of time) -->
      <p class="text-sm text-slate-500 mb-12">
        Works with Claude Code via
        <a href="/docs/guides/mcp-integration/" class="text-green-400 hover:text-green-300 transition-colors">MCP — 8 tools</a>
        for monitoring, cost tracking, and agent control.
      </p>

      <div class="grid md:grid-cols-2 gap-8 max-w-4xl mx-auto">
        <AnimatedTerminal />
        <DashboardPreview />
      </div>
    </div>
  </section>

  <!-- Features anchor for nav scroll -->
  <div id="features"></div>

  <!-- Feature: Monitor -->
  <FeatureSection
    title="See every agent. Every token. Every decision."
    description="Real-time dashboard shows all active Claude Code sessions, token usage, costs, and tool calls. Know exactly what your agents are doing."
  >
    <div class="rounded-xl border border-slate-700 bg-slate-900/50 p-4 text-left">
      <div class="flex items-center gap-2 mb-3">
        <span class="w-3 h-3 rounded-full bg-green-400"></span>
        <span class="text-sm text-slate-300">3 agents active</span>
      </div>
      <div class="space-y-2 text-xs text-slate-500 font-mono">
        <div>agent-1 | refactor auth module | $0.42 | 12k tokens</div>
        <div>agent-2 | write unit tests | $0.18 | 5k tokens</div>
        <div>agent-3 | fix CI pipeline | $0.31 | 9k tokens</div>
      </div>
    </div>
  </FeatureSection>

  <!-- Feature: Control -->
  <FeatureSection
    title="Approve. Reject. Resume. From anywhere."
    description="Full agent control from any device. Review tool calls, approve changes, or kill a runaway session — all without leaving your browser."
    reverse
  >
    <div class="rounded-xl border border-slate-700 bg-slate-900/50 p-4 space-y-3">
      <div class="text-sm text-slate-300">Agent requests permission:</div>
      <div class="text-xs text-slate-400 font-mono bg-slate-800 rounded p-2">git push origin main</div>
      <div class="flex gap-2">
        <span class="px-3 py-1 rounded bg-green-600 text-white text-xs">Approve</span>
        <span class="px-3 py-1 rounded bg-red-600/20 text-red-400 text-xs border border-red-600/30">Reject</span>
      </div>
    </div>
  </FeatureSection>

  <!-- Feature: Mobile -->
  <FeatureSection
    title="I shipped a feature from my phone."
    description="Native mobile app connects to your agents via cloud relay. Monitor, approve, and control from the couch, the train, or the beach."
  >
    <PhoneMockup />
    <div class="mt-6">
      <AppStoreBadges />
    </div>
  </FeatureSection>

  <!-- Feature: Analyze -->
  <FeatureSection
    title="Your AI fluency, measured."
    description="AI Fluency Score tracks how effectively you collaborate with AI agents. Session patterns, cost efficiency, and productivity trends — all quantified."
    reverse
  >
    <div class="rounded-xl border border-slate-700 bg-slate-900/50 p-4 text-center">
      <div class="text-5xl font-bold text-green-400 mb-2">87</div>
      <div class="text-sm text-slate-400">AI Fluency Score</div>
      <div class="mt-4 flex justify-center gap-1">
        <div class="w-2 h-8 bg-slate-700 rounded-sm"></div>
        <div class="w-2 h-12 bg-slate-600 rounded-sm"></div>
        <div class="w-2 h-10 bg-slate-600 rounded-sm"></div>
        <div class="w-2 h-16 bg-green-500 rounded-sm"></div>
        <div class="w-2 h-14 bg-green-500 rounded-sm"></div>
        <div class="w-2 h-20 bg-green-400 rounded-sm"></div>
      </div>
    </div>
  </FeatureSection>

  <!-- Install CTA Section -->
  <section class="py-24 px-6 bg-gradient-to-b from-slate-950 to-slate-900">
    <div class="max-w-3xl mx-auto text-center">
      <h2 class="text-3xl md:text-4xl font-bold text-white mb-4">Get started in 10 seconds</h2>
      <p class="text-lg text-slate-400 mb-8">No signup. No config. Just run one command.</p>
      <InstallCommand />
      <p class="text-sm text-slate-500 mt-4">Requires Node.js 18+. Works on macOS and Linux.</p>
    </div>
  </section>

  <!-- Pricing Preview -->
  <section class="py-24 px-6">
    <div class="max-w-6xl mx-auto text-center">
      <h2 class="text-3xl md:text-4xl font-bold text-white mb-4">Free. Forever. For local use.</h2>
      <p class="text-lg text-slate-400 mb-12">Cloud features coming soon.</p>
      <PricingCards />
      <a href="/pricing" class="inline-block mt-8 text-sm text-green-400 hover:text-green-300 transition-colors">
        See full comparison &rarr;
      </a>
    </div>
  </section>

  <!-- Intersection Observer for scroll reveals -->
  <script>
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            entry.target.classList.add('is-visible');
            observer.unobserve(entry.target);
          }
        });
      },
      { threshold: 0.2 }
    );

    document.querySelectorAll('.reveal-on-scroll').forEach((el) => {
      observer.observe(el);
    });
  </script>
</MarketingLayout>
```

> This is the single source of truth. If tasks were executed sequentially and the file differs, replace it with this version.

**Step 1: Verify Turbo builds the landing page**

```bash
# From repo root
bun run build
```

Expected: Turbo builds all apps including `@claude-view/landing`.

**Step 2: Verify Cloudflare deployment**

```bash
cd apps/landing && bun run deploy
```

Or if using `wrangler pages deploy dist` directly.

**Step 3: Full site walkthrough**

Open the deployed site and verify:

**Functional:**
- [ ] Homepage loads with animated terminal and dashboard preview
- [ ] Nav links work (Features anchor, Docs, Pricing, Blog)
- [ ] Scroll reveals animate on scroll
- [ ] Copy-to-clipboard works (Lucide icons, no emoji)
- [ ] GitHub stars badge loads (if repo exists)
- [ ] `/docs` renders Starlight with sidebar
- [ ] `/pricing` renders three-tier cards
- [ ] `/blog` lists blog posts
- [ ] `/changelog` shows version history
- [ ] `/llms.txt` serves the LLM-readable summary
- [ ] `/llms-full.txt` serves concatenated docs content
- [ ] `/sitemap.xml` is generated and accessible
- [ ] `/robots.txt` explicitly allows ClaudeBot, GPTBot, PerplexityBot
- [ ] Deep link handler redirects `?k=...&t=...` to `claude-view://pair?...`
- [ ] Custom 404 page renders for non-existent marketing routes

**Agent SEO / GEO:**
- [ ] Homepage has `SoftwareApplication` JSON-LD
- [ ] Pricing page has `FAQPage` JSON-LD (4 questions)
- [ ] Blog posts have `BlogPosting` JSON-LD
- [ ] Doc pages have `TechArticle` JSON-LD (via Starlight `head` config)
- [ ] Installation page has `HowTo` JSON-LD with 3 steps (`npx claude-view` flow)
- [ ] Marketing pages have `BreadcrumbList` JSON-LD (dynamically built from URL path; homepage correctly skips it)
- [ ] Starlight docs pages have `BreadcrumbList` JSON-LD (static `Home > Documentation > Current Page` via `head` config)

**View Transitions:**
- [ ] `<ClientRouter />` present in MarketingLayout `<head>` (imported from `astro:transitions`)
- [ ] Navigation between marketing pages (home → pricing → blog) has smooth crossfade in Chrome 126+
- [ ] Same navigation works normally (full page load) in Firefox/Safari — no errors, no broken behavior
- [ ] Deep link handler (`?k=...&t=...`) still works correctly with View Transitions enabled

**Image Optimization (AVIF):**
- [ ] OG image is PNG (`public/og-image.png`) — NOT AVIF (social platforms require PNG/JPEG)
- [ ] Any on-page raster images (post-V1) use `<Picture>` from `astro:assets` with `formats={['avif', 'webp']}`
- [ ] Source images stored in `src/assets/` (not `public/`) so Astro's build pipeline can optimize them

**Optional (requires paid Cloudflare Pro+, $20/mo):**
- [ ] Cloudflare Markdown for Agents toggled ON in dashboard
- [ ] `curl -H "Accept: text/markdown" https://claude-view.dev/` returns markdown (not HTML)
- [ ] `curl -H "Accept: text/markdown" https://claude-view.dev/docs/` returns markdown

**Visual Quality (from design doc):**
- [ ] No emojis used as icons — all icons are Lucide SVGs
- [ ] Hover states don't cause layout shift
- [ ] `cursor-pointer` on all clickable elements
- [ ] Transitions 150–300ms on interactive elements
- [ ] Color contrast 4.5:1 minimum for all text

**Accessibility:**
- [ ] Skip-to-content link works (Tab on page load → visible "Skip to content" link)
- [ ] Focus rings visible on dark background (green `outline: 2px solid #22c55e`)
- [ ] Focus states visible for keyboard navigation
- [ ] `prefers-reduced-motion` disables all animations (static fallbacks)
- [ ] Alt text on all images
- [ ] Semantic HTML (headings, landmarks, lists)

**Responsive:**
- [ ] Tested at 375px, 768px, 1024px, 1440px
- [ ] No horizontal scroll on mobile
- [ ] Touch targets 44x44px minimum
- [ ] Navigation collapses to hamburger on mobile

> **Optional enhancement:** Create `apps/landing/src/pages/404.astro` using `MarketingLayout` for a branded 404 page. Starlight handles 404s for `/docs/` routes, but marketing routes (`/nonexistent`) show the default Cloudflare 404 without this.

**Step 4: Run Lighthouse audit + Performance budget**

```bash
# Use Chrome DevTools → Lighthouse → Performance, Accessibility, Best Practices, SEO
```

Targets (from design doc performance budget):
- Lighthouse: **95+** on all 4 metrics
- Total JS: **~0KB framework** (vanilla scripts only, no React) — verify with `du -sh dist/_astro/*.js`
- LCP: **<1.5s**
- CLS: **0**
- INP: **<200ms** (Interaction to Next Paint — Core Web Vital since March 2024, replaces FID)

> **Audit fix:** Expanded from 12 items to full design doc pre-delivery checklist (18 items across 5 categories). Added explicit performance budget verification.

**Step 5: Enable Speculation Rules API for near-instant page navigations**

Astro has built-in prefetch support that uses the browser-native Speculation Rules API where available. On Chromium browsers (Chrome 121+, Edge, Opera), this prerenders linked pages ahead of navigation — the target page loads near-instantly when the user clicks. On Firefox and Safari, Astro falls back to standard `<link rel="prefetch">`. This is a progressive enhancement with zero downside.

Update `apps/landing/astro.config.mjs` — add `prefetch` at the top level and `experimental.clientPrerender`:

```javascript
export default defineConfig({
  site: 'https://claude-view.dev',
  trailingSlash: 'always',
  prefetch: {
    defaultStrategy: 'hover',
    prefetchAll: true,
  },
  experimental: {
    clientPrerender: true,
  },
  integrations: [
    // ... existing integrations unchanged
  ],
  vite: {
    plugins: [tailwindcss()],
  },
});
```

What this does:
- `prefetch.prefetchAll: true` — all `<a>` links are prefetch-eligible (no need for `data-astro-prefetch` attribute on every link)
- `prefetch.defaultStrategy: 'hover'` — prefetch triggers when user hovers a link (default, lowest overhead)
- `experimental.clientPrerender: true` — upgrades prefetch to use the Speculation Rules API, which prerenders (not just prefetches) the target page including client-side JS execution. On Chromium, Astro injects `<script type="speculationrules">` instead of standard prefetch tags. Unsupported browsers fall back to the configured prefetch strategy automatically.

Browser support:
- **Chromium (Chrome 121+, Edge, Opera):** Full Speculation Rules support — pages are prerendered in the background
- **Firefox:** Falls back to standard `<link rel="prefetch">` (fetch-only, no prerender)
- **Safari:** Falls back to standard `<link rel="prefetch">` (fetch-only, no prerender)

> **References:** [Astro Prefetch docs](https://docs.astro.build/en/guides/prefetch/), [Astro experimental clientPrerender](https://docs.astro.build/en/reference/experimental-flags/client-prerender/), [MDN Speculation Rules API](https://developer.mozilla.org/en-US/docs/Web/API/Speculation_Rules_API)

**Step 6: Final commit**

```bash
git add apps/landing/
git commit -m "feat(landing): complete Astro + Starlight landing page

Marketing homepage with animated hero, 4 feature sections, pricing.
Starlight docs (13 pages), blog, changelog.
Agent SEO: llms.txt, Schema.org, Cloudflare Markdown for Agents.
Speculation Rules API for near-instant Chromium page navigations.
Lighthouse 95+ target. All animations respect reduced-motion."
```

---

## Summary

| Task | Description | Estimated Effort |
|------|-------------|-----------------|
| 1 | Scaffold Astro + Starlight project | Setup |
| 2 | Global styles, fonts, design tokens | Styles |
| 3 | Marketing layout + Nav + Footer | Layout |
| 4 | Hero with animated terminal + dashboard preview | Interactive |
| 5 | Feature sections with scroll reveals | Content |
| 6 | Pricing page + install section | Pages |
| 7 | Starlight documentation (13 pages) | Content |
| 8 | Blog + changelog | Content |
| 9 | Agent SEO + GEO (llms.txt, robots.txt, favicon, structured data, HowTo + BreadcrumbList JSON-LD, CF Markdown for Agents) | SEO |
| 10 | GitHub stars badge (vanilla JS) | Interactive |
| 11 | Deep link preservation | Feature |
| 12 | Final wiring + build verification | QA |

**Dependencies:** Tasks 1-3 are sequential (foundation). Tasks 4-11 depend only on Task 3 and can mostly run in parallel, with these constraints: **Tasks 4, 5, 6, 10, and 11 all modify `src/pages/index.astro`** — if using parallel agents, these must serialize on that file. **Task 10 (GitHubStars) must complete before Task 4's build step** since Task 4's hero references the GitHubStars placeholder. Task 7 (docs) and Task 9 (SEO) are fully independent. Task 12 is final verification and provides the canonical assembled `index.astro`.

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Starlight `social` config uses deprecated object syntax | Blocker | Changed to array syntax `[{ icon, label, href }]` in `astro.config.mjs` |
| 2 | `content/config.ts` at wrong path for Astro 5 | Blocker | Moved to `src/content.config.ts` (Astro 5 Content Layer API location) |
| 3 | Missing `docsLoader()` in content config | Blocker | Added `docsLoader()` import from `@astrojs/starlight/loaders` |
| 4 | `@astrojs/tailwind` deprecated for Tailwind 4 | Blocker | Removed; replaced with `@tailwindcss/vite` as Vite plugin |
| 5 | `@tailwind base/components/utilities` are Tailwind 3 directives | Blocker | Replaced with `@import "tailwindcss"` (Tailwind 4 syntax) |
| 6 | `tailwind.config.mjs` is Tailwind 3 config model | Blocker | Deleted; moved customization to CSS `@theme {}` block |
| 7 | `Astro.site` undefined — `site` not set in config | Blocker | Added `site: 'https://claude-view.dev'` to `astro.config.mjs` |
| 8 | `@astrojs/cloudflare` adapter unnecessary for static output | Warning | Removed from deps and config (static `dist/` deploys directly) |
| 9 | React version `^19.0.0` violates monorepo pin convention | Warning | Removed — React eliminated entirely (zero-JS architecture) |
| 10 | tsconfig missing DOM lib — typecheck fails | Warning | Added `"lib": ["ES2022", "DOM", "DOM.Iterable"]` |
| 11 | Emoji `📋` in InstallCommand violates design doc | Warning | Replaced with inline Lucide SVGs (Clipboard / Check) — no React |
| 12 | Clipboard API unguarded for insecure contexts | Warning | Added `navigator.clipboard` check + `execCommand` fallback |
| 13 | Blog/changelog schema missing in initial scaffold | Warning | Moved all 3 collection schemas to Task 1's `content.config.ts` |
| 14 | Nav scroll classes purged by Tailwind JIT | Warning | Added `@source inline(...)` safelist in `global.css` |
| 15 | `lucide-react` version doesn't match `apps/web` | Warning | Removed — using inline Lucide SVGs instead (zero-JS) |
| 16 | `og-image.png` referenced but never created | High | Added Step 5 in Task 9 to create the OG image |
| 17 | `sitemap.xml` never generated (robots.txt 404) | High | Added `@astrojs/sitemap` to deps and `astro.config.mjs` |
| 18 | `llms-full.txt` completely absent from plan | High | Added build script + `prebuild` npm script in Task 9 |
| 19 | Most pre-delivery checklist items dropped | High | Expanded Task 12 Step 3 to full 18-item design doc checklist |
| 20 | `Hero.astro` not standalone component | Medium | Acknowledged — hero inlined in `index.astro` is acceptable for V1 |
| 21 | Blog/changelog schemas — no code, only prose | Medium | Full schemas with `z.object` now in Task 1 Step 7 |
| 22 | `llms.txt` content not inlined | Medium | Verbatim content from design doc now in Task 9 Step 1 |
| 23 | BlogLayout full code added | Medium | Complete BlogLayout.astro code provided in Task 8 Step 2 |
| 24 | Performance budget not explicitly verified | Medium | Added explicit budget targets to Task 12 Step 4 |
| 25 | `PricingToggle` omitted | Low | Intentionally deferred — design says "future use" (will be `.astro` when added) |
| 26 | Old `dist/` tracked in git | Minor | Added `git rm --cached dist/` to Task 1 Step 1 cleanup |
| 27 | `dev:landing` root script not added | Minor | Acknowledged — `cd apps/landing && bun run dev` is sufficient |
| 28 | Cursor blinks forever after typing | Minor | Cosmetic — acceptable for V1 launch |
| 29 | Blog/changelog collections missing `loader: glob()` | Blocker | Added `glob()` loader from `astro/loaders` to both collections |
| 30 | tsconfig missing `.astro/types.d.ts` include | Important | Added `"include": ["src", ".astro/types.d.ts"]` for Astro virtual module resolution |
| 31 | `z` imported from deprecated `astro:content` | Important | Changed to `import { z } from 'astro/zod'` (forward-compatible with Astro 6) |
| 32 | `@astrojs/check` + `typescript` as runtime deps | Suggestion | Moved to `devDependencies` |
| 33 | GitHubStars uses hardcoded SVG instead of Lucide | Suggestion | Uses inline Lucide Star SVG (no React, zero-JS) |
| 34 | Blog/changelog pages have no implementation code | Important | Added full code for BlogLayout, blog index, blog [slug], and changelog page |
| 35 | `llms-full.txt` script requires Node 20.12+ | Warning | Replaced with manual recursive traversal for Node 18 compat |
| 36 | Parallel tasks would conflict on `index.astro` | Warning | Added explicit note that Tasks 4/5/6/10/11 must serialize on that file |
| 37 | Missing `@tailwindcss/typography` for `prose` classes | Critical | Added dep to `package.json` + `@plugin` directive in `global.css` |
| 38 | Async `.map()` + `render()` in changelog template | Important | Moved rendering to frontmatter `for...of` loop (Astro bundling issue) |
| 39 | BlogLayout `readingTime` always returns 1 | Low | Removed (accurate reading time requires remark plugin — out of scope for V1) |
| 40 | `.astro/types.d.ts` must be generated before typecheck | Low | Added note that first `bun run build` generates it |
| 41 | React + lucide-react deps unnecessary for landing page | Architectural | Removed `react`, `react-dom`, `@astrojs/react`, `lucide-react` — zero-JS architecture |
| 42 | `react()` integration in `astro.config.mjs` | Architectural | Removed import and integration entry |
| 43 | `jsx`/`jsxImportSource` in tsconfig.json | Architectural | Removed — no JSX needed without React |
| 44 | `InstallCommand.tsx` was a React component | Architectural | Rewritten as `InstallCommand.astro` with inline `<script>` and Lucide SVGs |
| 45 | `GitHubStars.tsx` was a React island with `client:visible` | Architectural | Rewritten as `GitHubStars.astro` with inline `<script>`, localStorage cache, starts hidden |
| 46 | Updated changelog entries 9, 11, 15, 25, 33 | Consistency | Removed all references to `lucide-react` and React pinning |
| 47 | Task 4 Step 4 `index.astro` hero wiring was prose-only | Important | Added full code block wiring AnimatedTerminal, DashboardPreview, InstallCommand, GitHubStars into hero |
| 48 | Task 5 `FeatureSection.astro` was prose-only | Important | Added full code with reveal-on-scroll CSS, `reverse` prop, `<slot>` for visual content |
| 49 | Task 5 `PhoneMockup.astro` was prose-only | Important | Added full code with CSS 3D perspective, animated notification badges, prefers-reduced-motion |
| 50 | Task 5 feature sections and IO script were prose-only | Important | Added full code for 4 feature sections in `index.astro` + Intersection Observer script |
| 51 | Task 6 `PricingCards.astro` and `pricing.astro` were prose-only | Important | Added full code for 3-tier pricing component + pricing page + homepage sections |
| 52 | Task 11 `AppStoreBadges.astro` was prose-only | Important | Added full code with Apple/Play Store inline SVG badges and placeholder URLs |
| 53 | `@source inline()` syntax ambiguous for Tailwind 4 | Warning | ~~Changed to HTML-string format~~ — **Superseded by #106:** final format is brace expansion `@source inline("{...}")` |
| 54 | Task 4 Step 4 missing `MarketingLayout` wrapper | Blocker | Restructured as diff-style snippet (add to frontmatter + template body), not standalone file |
| 55 | Tasks 5/6/10/11 append snippets had orphaned `---` frontmatter | Blocker | Changed all to "Add imports to existing frontmatter" + "Append HTML" pattern |
| 56 | `h-18` invalid Tailwind class in feature sparkline | Warning | Replaced with `h-20` (valid default scale value) |
| 57 | Deep link script placement ambiguous — `<head>` unreachable from child page | Warning | Clarified: place in template body with `is:inline`, redirect fires before DOM render |
| 58 | `AppStoreBadges` created but never imported/wired in `index.astro` | Blocker | Added import + updated Mobile FeatureSection code block with `<AppStoreBadges />` |
| 59 | Blog/changelog content steps missing frontmatter skeleton | Warning | Added mandatory frontmatter templates (schema validation fails without required fields) |
| 60 | Favicon step prose-only but file is hard-referenced in `<head>` | Warning | Added inline SVG code block (green terminal cursor on dark background) |
| 61 | `bg-cta/10` opacity modifier fails with hex custom property | Warning | Replaced with `bg-green-500/10 text-green-400` (standard palette with alpha support) |
| 62 | No final canonical `index.astro` — 6 tasks modify file incrementally | Important | Added complete assembled `index.astro` as Task 12 Step 0 (single source of truth) |
| 63 | Task 4 imports `GitHubStars.astro` before Task 10 creates it | Blocker | Removed import from Task 4; added comment placeholder, import deferred to Task 10 |
| 64 | `@source inline()` HTML string format incorrect for Tailwind 4 | Warning | ~~Reverted to space-separated class names~~ — **Superseded by #106:** final format is brace expansion `@source inline("{...}")` |
| 65 | BlogLayout trailing `·` separator after author with no content | Minor | Removed dangling separator |
| 66 | Task dependency note incorrectly says "no ordering dependency" | Warning | Updated: Task 10 must complete before Task 4's build step |
| 67 | Missing skip-to-content link (WCAG 2.2 AA) | Medium | Added skip link before `<Nav>` and `id="main-content"` on `<main>` in MarketingLayout |
| 68 | No custom focus ring styles for dark background | Medium | Added `:focus-visible { outline: 2px solid #22c55e }` in MarketingLayout `<head>` |
| 69 | `robots.txt` missing AI crawler allow rules | Low | Added explicit `User-agent: ClaudeBot/GPTBot/PerplexityBot` allow rules; removed non-standard `LLMs-Txt` directive |
| 70 | Cloudflare Markdown for Agents not explicitly enabled | Critical | Added Step 6 with dashboard toggle / API PATCH + compression gotcha note |
| 71 | No per-page `TechArticle` schema on Starlight docs | High | Already included in Task 1's `astro.config.mjs` Starlight `head` config — no separate step needed |
| 72 | Pricing page missing `FAQPage` schema | Medium | Added JSON-LD with 4 FAQ entries to `pricing.astro` |
| 73 | Blog posts missing `BlogPosting` schema | Medium | Added JSON-LD in `BlogLayout.astro` using existing frontmatter props |
| 74 | No `<llms-only>` / `<llms-ignore>` content tags guidance | Low | Clarified these are Fern-proprietary, not part of llms.txt spec — skipped for V1 |
| 75 | Task 12 checklist missing Agent SEO verification items | Medium | Added 7-item Agent SEO/GEO section to verification checklist |
| 76 | Blog posts use `og:type: website` instead of `article` | High | Added `ogType` + `articleMeta` props to MarketingLayout; BlogLayout passes `article` + `article:published_time` + `article:author` |
| 77 | Missing Google Fonts `<link rel="preconnect">` hints | High | Added preconnect to `fonts.googleapis.com` + `fonts.gstatic.com` (with `crossorigin`) in MarketingLayout `<head>` — saves ~100-300ms LCP |
| 78 | Missing `twitter:site` meta tag | Low | Added `<meta name="twitter:site" content="@claude_view" />` to MarketingLayout |
| 79 | `id="features"` missing — Nav links to `/#features` but no anchor exists | Blocker | Added `<div id="features"></div>` anchor before first FeatureSection in canonical `index.astro` and Task 5 |
| 80 | `og-image.png` blocked by root `*.png` gitignore | Blocker | Added Task 1 Step 5 to append `!apps/landing/public/*.png` to `.gitignore` |
| 81 | `.astro/` generated dir not gitignored + `git add -A` commits it | Blocker | Added `apps/landing/.astro/` to gitignore step; replaced `git add -A` with `git add apps/landing/` |
| 82 | Redundant `<title>` — `{title} | claude-view` doubled "claude-view" | Warning | Removed "claude-view" from title props on homepage and pricing page |
| 83 | Task 9 Step 8 duplicates TechArticle schema already in Task 1 | Warning | Removed Task 9 Step 8; schema lives in Task 1's `astro.config.mjs` |
| 84 | Task 9 commit message lists TechArticle/FAQPage/BlogPosting — none added by Task 9 | Minor | Removed stale schema lines from commit message body |
| 85 | Design doc `astro.config.mjs` comment still shows `@astrojs/cloudflare` | Minor | Changed to `Astro 5 + Starlight (static output, no adapter)` |
| 86 | `tailwindcss` and `@tailwindcss/vite` pinned `^4` but `@source inline()` requires v4.1+ | Warning | Changed to `^4.1` for both packages |
| 86 | Fabricated `<llms-only>` / `<llms-ignore>` tags claimed as llms.txt spec | Blocker | Removed all references; clarified these are Fern-proprietary, not part of any spec |
| 87 | Princeton GEO citation misattributed — claimed Schema.org 30-40% | Blocker | Corrected: Princeton paper covers content-level techniques (fluency, statistics, citations); ~36% figure is from WPRiders/LLMrefs for Schema.org markup |
| 88 | Non-standard `LLMs-Txt:` robots.txt directive | Blocker | Removed from robots.txt; replaced with explicit `User-agent: ClaudeBot/GPTBot/PerplexityBot` allow rules (RFC 9309 compliant) |
| 89 | CF Markdown for Agents described as "zero-effort CRITICAL" | Warning | Rewritten with honest caveats: requires paid plan (Pro+), Pages compatibility undocumented, llms.txt is the reliable fallback |
| 90 | Font preconnect savings overstated as ~500ms | Warning | Corrected to ~100-300ms (DNS+TCP+TLS savings; actual LCP improvement depends on connection) |
| 91 | Starlight version `^0.34` — semver pre-1.0 only matches `0.34.x` | Blocker | Changed to `^0.37` (latest Starlight with Content Layer API support) |
| 92 | `trailingSlash` unset — Cloudflare Pages 308 redirects cause SEO issues | Warning | Added `trailingSlash: 'always'` to `astro.config.mjs` |
| 93 | `typecheck` script runs `astro check` without prior `astro sync` | Warning | Changed to `"astro sync && astro check"` (generates `.astro/types.d.ts` first) |
| 94 | `generate-llms-full.mjs` crashes if content directory missing | Warning | Added `existsSync` guard before directory traversal |
| 95 | No `_headers` file for Cloudflare cache control | Medium | Added `_headers` file: immutable caching for hashed `/_astro/*` assets, 1h for HTML |
| 96 | Nav hamburger button missing `aria-expanded` + Escape key handler | Medium | Added `aria-expanded="false"`, `aria-controls="mobile-menu"`, toggle updates attribute, Escape key closes menu |
| 97 | AnimatedTerminal + PhoneMockup are decorative, not meaningful images | Medium | Changed from `role="img" aria-label="..."` to `aria-hidden="true"` per WCAG 2.2 AA (decorative elements) |
| 98 | Deep link handler forwards ALL URL params — parameter injection risk | High | Changed to whitelist-only: only `k` and `t` params forwarded to custom scheme |
| 99 | No `@media (scripting: none)` fallback for JS-dependent sections | Medium | Added noscript CSS fallback in `global.css` — `.reveal-on-scroll` visible when JS disabled |
| 100 | 404 page not mentioned in Task 12 functional checklist | Low | Added 404 page verification item |
| 101 | Design doc Agent/LLM Discoverability section contained false claims | Blocker | Rewrote entire section with 6 honest layers, proper caveats, adoption status for each technique |
| 102 | Design doc growth flywheel overstated CF Markdown for Agents impact | Warning | Rewritten to show organic search as primary mechanism, CF Markdown as optional bonus |
| 103 | Design doc missing content-level GEO techniques (highest impact per research) | High | Added fluency optimization, statistics injection, first-third positioning to pre-delivery checklist |
| 104 | Design doc pre-delivery checklist incomplete | Medium | Expanded with 4 new sections: Agent Crawlers, Content-Level GEO, JavaScript Fallbacks, Deployment |
| 105 | README.md contained false claims matching design/impl plan errors | Blocker | Fixed: removed fabricated llms.txt tags row, corrected Princeton citation, replaced LLMs-Txt with AI crawler rules, fixed font preconnect claim, added CF paid plan caveat |
| 106 | `@source inline()` uses space-separated format — wrong for Tailwind CSS 4 | Warning | Changed to brace expansion: `@source inline("{bg-slate-900/80,backdrop-blur-md,border-slate-700,border-transparent}")` |
| 107 | `_headers` file uses `/*.html` pattern — not supported on CF Pages | Warning | Changed to `/*` (more specific `/_astro/*` takes precedence in CF Pages `_headers`) |
| 108 | Starlight sidebar `slug: 'docs'` should be empty string for index page | Blocker | Changed `{ label: 'Introduction', slug: 'docs' }` to `slug: ''` |
| 109 | `post.id` includes `.mdx` extension in Astro 5 Content Layer API | Blocker | Added `.replace(/\.(mdx?\|md)$/, '')` in `[slug].astro` getStaticPaths and `blog/index.astro` link href |
| 110 | `role="img"` on decorative CSS animations violates WCAG 2.2 | Medium | Changed AnimatedTerminal and PhoneMockup from `role="img" aria-label="..."` to `aria-hidden="true"` |
| 111 | CF Markdown "May require" vague about paid plan | Warning | Changed to "Requires a paid Cloudflare plan (Pro+, $20/mo)"; moved CF Markdown checks to optional section in Task 12 |
| 112 | `@tailwindcss/typography` `^0.5` too broad — needs minimum 0.5.15 for TW4 | Warning | Changed to `"@tailwindcss/typography": "^0.5.15"` |
| 113 | Changelog entry #23 contradicts reality — BlogLayout code exists | Medium | Fixed entry to: "BlogLayout full code added / Complete BlogLayout.astro code provided in Task 8 Step 2" |
| 114 | "Coming Soon" pricing CTAs use `<a href="#">` — inaccessible | Medium | Changed to `<button disabled>` with `cursor-not-allowed opacity-70` styling |
| 115 | Task 9 "Step 3.5" numbering inconsistent | Minor | Renumbered: 3.5 becomes Step 4, old Steps 4-8 become Steps 5-9 |
| 116 | WCAG 2.1 AA references outdated | Minor | Changed all "WCAG 2.1 AA" to "WCAG 2.2 AA" |
| 117 | Missing INP target in performance budget | Medium | Added INP < 200ms target to Task 12 Step 4 (Core Web Vital since March 2024) |
| 118 | No Speculation Rules API / prefetch configuration | Medium | Added `prefetch` + `experimental.clientPrerender` to `astro.config.mjs` in Task 12 Step 5. Chromium prerenders pages on hover; Firefox/Safari fall back to standard prefetch. Design doc performance table updated. |
| 119 | README "Font display swap" row claimed "CLS: 0" | Warning | Changed to "CLS: minimal" — `display=swap` prevents FOIT but causes FOUT (flash of unstyled text), which contributes small CLS. True zero is not achievable with font-swap. |
| 120 | `wrangler.toml` uses deprecated `[site]` config (Workers Sites pattern) | Low | Added deprecation comment to `wrangler.toml` and note in Task 1 explaining `[site]` is ignored by `wrangler pages deploy dist`. Harmless but not required for Pages. |
| 121 | Changelog entries #53 and #64 describe superseded `@source inline()` formats | Minor | Both entries now marked as superseded by #106 (brace expansion format is the final correct syntax) |
| 122 | `@media (scripting: none)` missing browser support note | Low | Added comment in Task 2 `global.css`: ~88% support (Chrome 120+, Firefox 113+, Safari 18+). Recommend keeping `<noscript>` tag as fallback for older browsers. |
| 123 | README missing Speculation Rules, View Transitions, AVIF rows in Performance table | Medium | Added three rows: Speculation Rules (prefetch/prerender on hover), View Transitions (smooth navigation), AVIF images (50% smaller than JPEG with fallbacks) |
| 124 | No View Transitions for marketing pages | Enhancement | Added `<ClientRouter />` from `astro:transitions` to MarketingLayout `<head>`. Progressive enhancement: Chrome 126+/Edge 126+ get smooth crossfade between marketing pages; unsupported browsers get normal navigation. Zero JS penalty. Astro renamed `<ViewTransitions />` to `<ClientRouter />` in v4.x. |
| 125 | No AVIF image optimization pipeline | Enhancement | Added AVIF image optimization section to design doc and Task 9 Step 6a in impl plan. OG image remains PNG (social platforms don't support AVIF). On-page images use `<Picture>` from `astro:assets` with `formats={['avif', 'webp']}`. AVIF is ~50% smaller than JPEG, ~20% smaller than WebP, with 93%+ browser support (Chrome 85+, Firefox 93+, Safari 16.4+, Edge 121+). |
| 124 | README missing HowTo and BreadcrumbList rows in GEO table | Medium | Added HowTo schema for installation docs and BreadcrumbList for site hierarchy navigation |
| 125 | README layer count said "four layers" but section has five | Minor | Changed to "five layers" to match actual Layer 1-5 structure |
| 126 | README CF Markdown for Agents said "May require" paid plan | Warning | Changed to "Requires paid plan (Pro+, $20/mo)" per changelog #111 consistency |
| 127 | README Lighthouse target referenced "Task 12 verification" (impl detail) | Minor | Changed File(s) column to "All pages" — README is user-facing, not an impl plan |
| 128 | No HowTo structured data on installation docs page | Medium | Added `HowTo` JSON-LD (Schema.org) to `installation.mdx` with 3 steps for `npx claude-view` flow. Google deprecated HowTo rich results (Sept 2023) but Schema.org HowTo remains valid for AI engines (Bing Copilot, Perplexity, ChatGPT). Properties: `name`, `description`, `totalTime` (PT15S), `tool` (Node.js), `step[]` with `position`/`name`/`text`. |
| 129 | No BreadcrumbList structured data on marketing pages | Medium | Added dynamic `BreadcrumbList` JSON-LD to `MarketingLayout.astro`, built from `Astro.url.pathname` segments. Homepage skips breadcrumbs (single-item list is meaningless). Last item omits `item` URL per Google spec. |
| 130 | No BreadcrumbList structured data on Starlight docs pages | Medium | Added static `BreadcrumbList` JSON-LD to Starlight `head` config (`Home > Documentation > Current Page`). V1 limitation: static third item; V2 can use Starlight component override for per-page titles. |
| 131 | Starlight assumed to auto-generate BreadcrumbList JSON-LD | Clarification | Starlight renders visual breadcrumb navigation in the sidebar UI but does NOT emit `<script type="application/ld+json">` structured data. Must be implemented manually. |
| 132 | Task 9 had 9 steps; now has 12 after adding HowTo + BreadcrumbList steps | Structural | Renumbered: old Step 9 (Commit) is now Step 12. New Steps 9-11 cover BreadcrumbList (MarketingLayout), HowTo (installation.mdx), and BreadcrumbList (Starlight head config). |
| 133 | Task 12 Agent SEO checklist missing HowTo and BreadcrumbList verification | Medium | Added 3 checklist items: HowTo on installation page, BreadcrumbList on marketing pages, BreadcrumbList on docs pages. |
