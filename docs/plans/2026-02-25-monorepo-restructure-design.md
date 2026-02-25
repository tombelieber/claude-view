---
status: approved
date: 2026-02-25
purpose: Restructure claude-view into Turborepo monorepo with Expo mobile app and landing page
---

# Monorepo Restructure: Desktop + Mobile + Landing

> **Goal:** Restructure claude-view from a single Vite SPA + Rust backend into a Turborepo monorepo containing three apps (web, mobile, landing) and shared packages. Enable Expo/React Native mobile app from day 0.

---

## 1. Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Monorepo tool | Turborepo + Bun workspaces | Industry standard (Vercel, Expo examples). Turborepo: one `turbo.json`, task caching, dependency-aware parallel execution. Bun: already used in project (CLAUDE.md). |
| Package manager | Bun | Project already uses Bun for development. `bunfig.toml` with `linker = "hoisted"` works around Metro transitive dep issue ([oven-sh/bun#25870](https://github.com/oven-sh/bun/issues/25870)). |
| Mobile framework | Expo SDK 54 + React Native | PWA dropped — zero successful PWA-only mobile products, iOS PWA broken background sync. All competing dev tools (Happy, Replit, v0) use Expo. |
| Mobile styling | NativeWind v4 | Same Tailwind class names on React Native. Shares mental model with web's Tailwind CSS. Production-stable (updated Feb 2026). |
| Landing page | Static HTML on Cloudflare Pages | M1 landing is App Store badges + universal links + hero. Astro/Next.js overkill. Add framework later if marketing site grows. |
| Code sharing | Shared TS packages (types, crypto, relay, theme) | Web and mobile are separate UIs with different component libraries (Radix vs RN). Share business logic, not UI components. Pattern used by Vercel v0, every Expo monorepo reference. |
| Migration | Big bang PR | Move web SPA into `apps/web/` in one PR. Clean break. `git mv` preserves blame. |
| Expo config | `app.config.ts` (not `app.json`) | Dynamic config for build variants — switch bundle ID / deep links per environment. Standard Expo practice. |

### Known trade-offs

| Trade-off | Mitigation |
|-----------|-----------|
| Turborepo + Bun is beta ([turbo prune unsupported](https://github.com/vercel/turborepo/discussions/7456)) | Not needed for MVP. Revisit when CI/Docker optimization matters. |
| Metro + Bun transitive dep issue ([metro#1636](https://github.com/facebook/metro/issues/1636)) | `bunfig.toml` with `linker = "hoisted"`. Disables Bun isolation mode but Metro resolves correctly. |
| Big bang migration PR is large | No feature changes in the PR — purely structural. Full test suite validates nothing broke. |

---

## 2. Directory Structure

```
claude-view/
  ├── crates/                          # Rust workspace (UNCHANGED)
  │     ├── core/                      # Shared types, JSONL parser
  │     ├── db/                        # SQLite via sqlx
  │     ├── search/                    # Tantivy full-text indexer
  │     ├── server/                    # Axum HTTP routes
  │     └── relay/                     # Fly.io relay server
  │
  ├── apps/
  │     ├── web/                       # Existing Vite React SPA (moved from root)
  │     │     ├── src/                 # All existing React code
  │     │     ├── public/
  │     │     ├── index.html
  │     │     ├── package.json         # Web-specific deps (Radix, recharts, shiki, etc.)
  │     │     ├── vite.config.ts
  │     │     ├── vitest.config.ts
  │     │     ├── tsconfig.json        # Extends ../../tsconfig.base.json
  │     │     ├── eslint.config.js
  │     │     ├── playwright.config.ts
  │     │     ├── e2e/
  │     │     └── tests/
  │     │
  │     ├── mobile/                    # NEW: Expo/React Native app
  │     │     ├── app/                 # Expo Router file-based routes
  │     │     │     ├── (tabs)/        # Tab navigation
  │     │     │     ├── pair.tsx       # QR pairing deep link handler
  │     │     │     └── _layout.tsx    # Root layout
  │     │     ├── components/
  │     │     ├── hooks/
  │     │     ├── lib/
  │     │     │     ├── supabase.ts    # Supabase auth client
  │     │     │     ├── relay.ts       # Uses @claude-view/shared/relay
  │     │     │     └── crypto.ts      # Uses @claude-view/shared/crypto
  │     │     ├── app.config.ts        # Dynamic Expo config
  │     │     ├── metro.config.js      # Monorepo-aware Metro config
  │     │     ├── nativewind-env.d.ts
  │     │     ├── tailwind.config.ts   # NativeWind + shared theme
  │     │     ├── package.json
  │     │     └── tsconfig.json
  │     │
  │     └── landing/                   # NEW: Static HTML marketing site
  │           ├── public/
  │           │     └── .well-known/
  │           │           └── apple-app-site-association
  │           ├── src/
  │           │     └── index.html     # Landing page
  │           ├── package.json
  │           └── wrangler.toml        # Cloudflare Pages config
  │
  ├── packages/
  │     ├── shared/                    # Shared TS business logic
  │     │     ├── src/
  │     │     │     ├── types/         # Relay protocol types, session types
  │     │     │     ├── crypto/        # NaCl box encrypt/decrypt (tweetnacl)
  │     │     │     ├── relay/         # WS client protocol helpers
  │     │     │     ├── theme.ts       # Color palette, semantic tokens
  │     │     │     ├── utils/         # Formatting, validation, constants
  │     │     │     └── index.ts
  │     │     ├── package.json
  │     │     └── tsconfig.json
  │     │
  │     └── design-tokens/             # Shared visual constants
  │           ├── src/
  │           │     ├── colors.ts      # Palette + semantic tokens
  │           │     ├── spacing.ts     # Scale
  │           │     ├── typography.ts  # Font families, sizes, weights
  │           │     └── index.ts
  │           ├── package.json
  │           └── tsconfig.json
  │
  ├── Cargo.toml                       # Rust workspace root (unchanged)
  ├── Cargo.lock
  ├── turbo.json                       # Turborepo task config
  ├── bunfig.toml                      # linker = "hoisted" (Metro compat)
  ├── package.json                     # Bun workspace root
  ├── bun.lock
  ├── package-lock.json                # Keep for npx distribution
  ├── tsconfig.base.json               # Shared TS config all packages extend
  ├── npx-cli/                         # npm distribution wrapper (unchanged)
  ├── scripts/                         # Release scripts (unchanged)
  ├── docs/                            # Documentation (unchanged)
  ├── supabase/                        # Supabase config (unchanged)
  └── CLAUDE.md
```

---

## 3. Migration Table

What moves vs what stays at root:

| Item | Current location | New location |
|------|-----------------|-------------|
| `src/` | root | `apps/web/src/` |
| `public/` | root | `apps/web/public/` |
| `index.html` | root | `apps/web/index.html` |
| `vite.config.ts` | root | `apps/web/vite.config.ts` |
| `vitest.config.ts` | root | `apps/web/vitest.config.ts` |
| `tsconfig.app.json` | root | `apps/web/tsconfig.json` (extends base) |
| `tsconfig.node.json` | root | `apps/web/tsconfig.node.json` |
| `eslint.config.js` | root | `apps/web/eslint.config.js` |
| `playwright.config.ts` | root | `apps/web/playwright.config.ts` |
| `e2e/` | root | `apps/web/e2e/` |
| `tests/` | root | `apps/web/tests/` |
| `design-system/` | root | `packages/design-tokens/` (absorb) |
| `tsconfig.json` | root | `tsconfig.base.json` (shared base) |

**Stays at root (unchanged):**
- `crates/` — Rust workspace
- `Cargo.toml`, `Cargo.lock`
- `npx-cli/` — npm distribution wrapper
- `scripts/` — release scripts
- `docs/` — documentation
- `supabase/` — Supabase config
- `CLAUDE.md`
- `bun.lock`, `package-lock.json`
- `LICENSE`, `README*.md`

---

## 4. Workspace Configuration

### Root `package.json`

```jsonc
{
  "name": "claude-view",
  "private": true,
  "workspaces": ["apps/*", "packages/*"],
  "scripts": {
    // Turborepo-managed (JS apps + packages)
    "dev": "turbo dev",
    "build": "turbo build",
    "lint": "turbo lint",
    "typecheck": "turbo typecheck",
    "test": "turbo test",
    // Rust scripts (not managed by Turborepo)
    "dev:server": "unset CLAUDECODE CLAUDE_CODE_SSE_PORT CLAUDE_CODE_ENTRYPOINT && RUST_LOG=warn,claude_view_server=info,claude_view_core=info VITE_PORT=5173 cargo watch -w crates -x 'run -p claude-view-server'",
    "test:rust": "cargo test --workspace",
    "lint:rust": "cargo clippy --workspace -- -D warnings",
    "fmt": "cargo fmt --all"
  }
}
```

### `turbo.json`

```jsonc
{
  "$schema": "https://turbo.build/schema.json",
  "tasks": {
    "build": {
      "dependsOn": ["^build"],
      "outputs": ["dist/**", ".expo/**"]
    },
    "dev": {
      "cache": false,
      "persistent": true
    },
    "lint": {},
    "typecheck": {
      "dependsOn": ["^build"]
    },
    "test": {
      "dependsOn": ["^build"]
    }
  }
}
```

### `bunfig.toml`

```toml
[install]
linker = "hoisted"
```

Required for Metro bundler compatibility. Without this, Metro can't resolve transitive dependencies stored in `node_modules/.bun/`.

### `tsconfig.base.json`

```jsonc
{
  "compilerOptions": {
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true
  }
}
```

All `apps/*/tsconfig.json` and `packages/*/tsconfig.json` extend this.

---

## 5. Package Dependency Graph

```
apps/web         → packages/shared, packages/design-tokens
apps/mobile      → packages/shared, packages/design-tokens
apps/landing     → (standalone, no deps on packages/)

packages/shared        → (standalone, pure TS, no framework deps)
packages/design-tokens → (standalone, pure TS, no framework deps)
```

### Package references

```jsonc
// packages/shared/package.json
{
  "name": "@claude-view/shared",
  "version": "0.0.0",
  "private": true,
  "main": "./src/index.ts",
  "types": "./src/index.ts",
  "exports": { ".": "./src/index.ts" }
}

// packages/design-tokens/package.json
{
  "name": "@claude-view/design-tokens",
  "version": "0.0.0",
  "private": true,
  "main": "./src/index.ts",
  "types": "./src/index.ts",
  "exports": { ".": "./src/index.ts" }
}

// apps/web/package.json
{
  "dependencies": {
    "@claude-view/shared": "workspace:*",
    "@claude-view/design-tokens": "workspace:*"
  }
}

// apps/mobile/package.json
{
  "dependencies": {
    "@claude-view/shared": "workspace:*",
    "@claude-view/design-tokens": "workspace:*"
  }
}
```

### What goes in `packages/shared/src/`

| Module | What | Consumed by |
|--------|------|-------------|
| `types/session.ts` | Session, SessionStatus, etc. (mirror ts-rs output) | web, mobile |
| `types/relay.ts` | Command protocol types (sessions, output, command) | web, mobile |
| `crypto/nacl.ts` | NaCl box encrypt/decrypt (tweetnacl) | web, mobile |
| `relay/protocol.ts` | WS message framing, heartbeat, reconnect logic | web, mobile |
| `utils/format.ts` | Cost formatting, time formatting | web, mobile |

### What goes in `packages/design-tokens/src/`

| Module | What | Consumed by |
|--------|------|-------------|
| `colors.ts` | Palette + semantic tokens (light/dark) | web (Tailwind config), mobile (NativeWind config) |
| `spacing.ts` | Spacing scale | web, mobile |
| `typography.ts` | Font families, sizes, weights | web, mobile |

---

## 6. Apps Detail

### `apps/web/` — Existing Vite React SPA

Moves from root into `apps/web/` with minimal changes:

- All `src/` imports stay relative (no change to component code)
- New imports from shared: `import { SessionStatus } from '@claude-view/shared'`
- `vite.config.ts` — may need path updates for monorepo root
- `tsconfig.json` — extends `../../tsconfig.base.json`
- `package.json` — web-specific deps only (Radix, recharts, shiki, react-router-dom, react-virtuoso, etc.)

**Rust backend serves `apps/web/dist/`** — update static file path in `crates/server/` from `dist/` to `apps/web/dist/`.

### `apps/mobile/` — Expo/React Native

New Expo app with:

- **Routing:** Expo Router (file-based, wraps React Navigation)
- **Styling:** NativeWind v4 (Tailwind classes on RN)
- **Auth:** Supabase (magic link / Google OAuth) via `@supabase/supabase-js`
- **Crypto:** tweetnacl via `@claude-view/shared/crypto`
- **Relay:** WebSocket client via `@claude-view/shared/relay`
- **Key storage:** `expo-secure-store` (replaces IndexedDB from PWA design)
- **Config:** `app.config.ts` for dynamic build variants

Key deps: `expo`, `expo-router`, `expo-secure-store`, `expo-notifications`, `nativewind`, `@supabase/supabase-js`, `tweetnacl`, `@claude-view/shared`, `@claude-view/design-tokens`

**`metro.config.js`** must be monorepo-aware:

```js
const { getDefaultConfig } = require('expo/metro-config');
const path = require('path');

const projectRoot = __dirname;
const monorepoRoot = path.resolve(projectRoot, '../..');

const config = getDefaultConfig(projectRoot);

// Watch all files in the monorepo
config.watchFolders = [monorepoRoot];

// Resolve packages from monorepo root
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, 'node_modules'),
  path.resolve(monorepoRoot, 'node_modules'),
];

module.exports = config;
```

### `apps/landing/` — Static HTML on Cloudflare Pages

Minimal static site deployed to `m.claudeview.ai`:

- `src/index.html` — Hero section, App Store / Play Store badges, screenshots
- `public/.well-known/apple-app-site-association` — Universal links for iOS deep linking
- `wrangler.toml` — Cloudflare Pages config
- Deep link redirect: `claude-view://pair?...` → App Store if app not installed

No framework. No build step (or a trivial copy-to-dist script). Add Astro later if the marketing site grows beyond a single page.

---

## 7. What Does NOT Change

- **All Rust code** (`crates/`) — untouched
- **All React component code** — same files, just under `apps/web/src/` now
- **Git history** — `git mv` preserves blame
- **npx-cli/** — stays at root
- **scripts/** — stays at root
- **docs/** — stays at root
- **supabase/** — stays at root
- **Relay architecture** — unchanged from mobile-remote design

---

## 8. Migration Plan (Big Bang PR)

One PR that restructures the repo. No feature changes, no code modifications beyond path updates.

1. Create directory structure: `apps/web/`, `apps/mobile/`, `apps/landing/`, `packages/shared/`, `packages/design-tokens/`
2. `git mv` web SPA files into `apps/web/` (src, public, index.html, configs, e2e, tests)
3. Split root `package.json` — web-specific deps → `apps/web/package.json`, workspace config → root `package.json`
4. Create `packages/shared/` — extract relay types + crypto from existing code
5. Create `packages/design-tokens/` — extract from `design-system/` directory
6. Add `turbo.json`, `tsconfig.base.json`, `bunfig.toml`
7. Update `crates/server/` static file path: `dist/` → `apps/web/dist/`
8. Update dev scripts for new paths
9. Scaffold empty `apps/mobile/` (`npx create-expo-app`) and `apps/landing/` (static HTML)
10. Run full test suite (Rust + frontend) to verify nothing broke
11. Update CLAUDE.md with new paths and conventions

---

## 9. Dev Workflow After Migration

```bash
# Full stack dev (web + Rust backend)
bun run dev:server        # Rust backend (cargo watch)
bunx turbo dev --filter=web  # Vite dev server for web

# Mobile dev
bunx turbo dev --filter=mobile  # Expo dev server
# or: cd apps/mobile && bunx expo start

# Landing page
cd apps/landing && open src/index.html  # It's static HTML

# Build all JS apps
bunx turbo build

# Run all JS tests
bunx turbo test

# Rust (unchanged)
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

---

## 10. Proven By

| Pattern | Who uses it at scale |
|---------|---------------------|
| Turborepo + workspaces monorepo | Vercel, Nhost, shadcn-ui |
| Separate web SPA + Expo mobile app sharing types | Vercel v0 ([blog post](https://vercel.com/blog/how-we-built-the-v0-ios-app)) |
| Expo monorepo with shared packages | byCedric's [expo-monorepo-example](https://github.com/byCedric/expo-monorepo-example) (semi-official Expo reference) |
| NativeWind for RN styling | NativeWind v4 production-stable, used by Gluestack UI under the hood |
| Static landing page on Cloudflare Pages | Standard pattern for app landing pages (universal links + store redirects) |
| `app.config.ts` for Expo | Happy, standard Expo practice for build variants |

---

## Cross-references

- [`docs/plans/mobile-remote/design.md`](mobile-remote/design.md) — Mobile remote zero-setup design (architecture, command protocol, security model)
- [`docs/plans/backlog/2026-02-12-mobile-pwa-design.md`](backlog/2026-02-12-mobile-pwa-design.md) — Original PWA design (superseded by Expo pivot)
- [`docs/plans/mission-control/design.md`](mission-control/design.md) — Mission Control architecture (relay depends on Phase A)
