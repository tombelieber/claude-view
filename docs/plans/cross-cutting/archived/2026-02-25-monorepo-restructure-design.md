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
| Mobile framework | Expo SDK 55 + React Native 0.83.2 | SDK 55 stable (Feb 2026, tagged `next` on npm). New Architecture mandatory (Legacy gone). RN 0.83.2 + React 19.2.0 + Reanimated 4.2.1 + Hermes v1 opt-in. All competing dev tools (Happy, Replit, v0) use Expo. |
| Mobile styling | Tamagui v2 RC (`2.0.0-rc.17`, `latest` on npm) | Brand-first design system: token props in JSX (`bg="$surface"`, `p="$md"`), optimizing compiler flattens views at build time, `@tamagui/ui` provides 30+ reskinnable components. v2 RC is the `latest` tag on npm — Tamagui team considers it production-ready. v2 targets RN 0.81+ / React 19+ / New Architecture — exact match for SDK 55. Chosen over NativeWind v5 (still preview/pre-release, Reanimated v4 dep conflicts) and Unistyles (no built-in token system or components). Proven at scale, ~24KB core, zero external deps. [Verified working: Expo SDK 54 + Tamagui v2 + monorepo](https://github.com/expo/expo/discussions/42767). |
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
| Tamagui v2 RC — not final stable | v2 RC is the `latest` npm tag (team considers it production-ready). 17 RCs with continuous canary releases = bugs getting fixed rapidly. v1.144.3 available as fallback if v2 hits a wall. Disable compiler extraction in dev (`disableExtraction: true`). Compiler is optional — app works without it, just slower. |
| Tamagui has its own DSL (Stack, XStack, $tokens) — learning curve | DSL is small (~10 primitives). Team already knows React Native. Token props map 1:1 to design-tokens package. |

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
  │     ├── mobile/                    # NEW: Expo/React Native app (SDK 55)
  │     │     ├── app/                 # Expo Router v6 file-based routes
  │     │     │     ├── (tabs)/        # Tab navigation (native tabs API)
  │     │     │     ├── pair.tsx       # QR pairing deep link handler
  │     │     │     └── _layout.tsx    # Root layout (wraps TamaguiProvider)
  │     │     ├── components/
  │     │     │     └── ui/            # Custom brand components (using @tamagui/ui primitives)
  │     │     ├── hooks/
  │     │     ├── lib/
  │     │     │     ├── supabase.ts    # Supabase auth client
  │     │     │     ├── relay.ts       # Uses @claude-view/shared/relay
  │     │     │     ├── crypto.ts      # Uses @claude-view/shared/crypto
  │     │     │     └── storage.ts     # MMKV (cache) + expo-secure-store (tokens)
  │     │     ├── tamagui.config.ts    # createTamagui() — imports @claude-view/design-tokens
  │     │     ├── app.config.ts        # Dynamic Expo config
  │     │     ├── metro.config.js      # Monorepo-aware Metro config
  │     │     ├── babel.config.js      # Tamagui Babel plugin for compiler optimization
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
  ├── tsconfig.base.json               # Shared TS base config all packages extend
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
| `tsconfig.json` | root | `tsconfig.base.json` (rename + rewrite as shared base) |

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
    "target": "ES2022",
    "lib": ["ES2022"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "skipLibCheck": true,
    "allowImportingTsExtensions": true,
    "sourceMap": true,
    "verbatimModuleSyntax": true,
    "moduleDetection": "force",
    "noEmit": true
  }
}
```

No `DOM` in lib — packages don't need DOM types (`apps/web` adds its own). No `declaration`/`declarationMap` — only `packages/*/tsconfig.json` add those. All `apps/*/tsconfig.json` and `packages/*/tsconfig.json` extend this.

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
| `colors.ts` | Palette + semantic tokens (light/dark) | web (Tailwind config), mobile (`createTamagui()` tokens) |
| `spacing.ts` | Spacing scale | web, mobile |
| `typography.ts` | Font families, sizes, weights | web, mobile |

> **Token consumption split:** `apps/web` uses Tailwind CSS v4 (CSS-first `@theme` blocks). `apps/mobile` uses Tamagui v2 (`createTamagui({ ...defaultConfig, tokens: { ...overrides } })` in `tamagui.config.ts`, importing `defaultConfig` from `@tamagui/config/v5`). These are different config formats — do NOT try to share a config file. Instead, both apps import the raw TS token values from `packages/design-tokens/` and adapt them to their own format. Web: `@theme { --color-surface: ...}`. Mobile: `createTamagui({ tokens: { color: { surface: tokens.colors.surface } } })`.

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

New Expo SDK 55 app with:

- **Routing:** Expo Router (file-based, native tabs API). SDK 55 unifies all first-party packages to major 55 — `expo-router@~55.0.0`.
- **Styling + Design System:** Tamagui v2 RC (`2.0.0-rc.17`, `latest` on npm) — token-first styling with `$token` props in JSX, optimizing compiler flattens components at build time. `@tamagui/ui` provides 30+ reskinnable components (Button, Input, Sheet, Dialog, etc.). v2 targets RN 0.81+ / React 19+ / New Architecture — exact match for SDK 55. Import config from `@tamagui/config/v5`.
- **Icons:** Lucide React Native (same icon names as web's `lucide-react`)
- **Lists:** `@shopify/flash-list` v2.2 — New Arch only, ~10x faster than FlatList (Shopify production)
- **Animations:** `react-native-reanimated` ~4.2.1 (UI thread, 120fps) — bundled with SDK 55. `react-native-worklets` 0.7.2 is a new required peer dep in SDK 55.
- **Data fetching:** `@tanstack/react-query` v5 — same mental model as web, handles cache/background refetch
- **Auth:** Supabase (magic link / Google OAuth) via `@supabase/supabase-js`
- **Crypto:** tweetnacl via `@claude-view/shared/crypto`
- **Relay:** WebSocket client via `@claude-view/shared/relay`
- **Key storage:** `expo-secure-store` (tokens/keys only — iOS Keychain / Android Keystore)
- **Fast cache:** `react-native-mmkv` **v4.1** (Nitro Module — fastest KV store, synchronous, ~30x faster than AsyncStorage). Requires `react-native-nitro-modules` as peer dep. API: `createMMKV()` (not `new MMKV()`).
- **Images:** `expo-image` — built into SDK 55, better caching + progressive loading than bare `<Image>`
- **Bottom sheets:** `@gorhom/bottom-sheet` v5 — NArch compatible, used by Discord/Shopify. Note: [known layout shift bug with React Navigation + NArch](https://github.com/gorhom/react-native-bottom-sheet/issues/1944) — monitor.
- **Polish:** `expo-haptics` (tactile feedback), `expo-blur` (native blur views, RenderNode API on Android 12+), `expo-linear-gradient`
- **Analytics:** PostHog (`posthog-react-native`) — self-hostable, open source, GDPR-friendly
- **Push notifications:** OneSignal (`@onesignal/react-native-onesignal`) — cross-platform, has Expo config plugin
- **i18n:** `i18next` + `react-i18next` + `expo-localization` — defacto standard
- **OTA updates:** EAS Update — Hermes bytecode diffing in SDK 55 = smaller updates. Channels: `preview`, `production`
- **Architecture:** New Architecture mandatory (SDK 55, no legacy). No `newArchEnabled` flag needed in app.json — it's the only option.
- **Config:** `app.config.ts` for dynamic build variants (EAS channels, OneSignal App ID, PostHog key, bundle IDs)

**Key deps (runtime) — verified against `npx create-expo-app --template tabs@sdk-55`:**

```
# Core SDK 55 (from template — DO NOT change these versions)
expo@~55.0.0
expo-router@~55.0.0                 # Unified to SDK major version in SDK 55
react@19.2.0
react-native@0.83.2
react-native-reanimated@~4.2.1      # v4, NOT v3 — bundled with SDK 55
react-native-worklets@0.7.2         # NEW required peer dep in SDK 55
react-native-safe-area-context@~5.6.2
react-native-screens@~4.24.0
@react-navigation/native@^7.1.28

# Expo first-party (all aligned to ~55.0.x)
expo-image@~55.0.x
expo-secure-store@~55.0.x
expo-localization@~55.0.x
expo-haptics@~55.0.x
expo-blur@~55.0.x
expo-linear-gradient@~55.0.x
expo-status-bar@~55.0.x
expo-font@~55.0.x

# Tamagui v2 RC (latest on npm)
tamagui@^2.0.0-rc.17                # v2 RC — targets RN 0.81+/React 19+/NArch
@tamagui/config@^2.0.0-rc.17        # Import from @tamagui/config/v5
@tamagui/ui@^2.0.0-rc.17            # 30+ reskinnable components
@tamagui/font-inter                  # Inter font files for useFonts() in _layout.tsx

# UI + interaction
lucide-react-native
react-native-svg
react-native-mmkv@^4.1.x            # Nitro Module — requires react-native-nitro-modules
react-native-nitro-modules           # Peer dep for mmkv v4
@shopify/flash-list@^2.2.x          # New Arch only
@tanstack/react-query@^5.x
@gorhom/bottom-sheet@^5.x
zustand@5.0.7                       # Pin to known-good (v5.0.9 had Metro regression)

# Services
posthog-react-native
@onesignal/react-native-onesignal
i18next
react-i18next
@supabase/supabase-js
tweetnacl
tweetnacl-util
@claude-view/shared
@claude-view/design-tokens
```

**Dev deps:**

```
@tamagui/babel-plugin@^2.0.0-rc.17  # Compiler — flattens styled() calls at build time
maestro                             # npx maestro — no install, E2E testing
```

**NOT included (removed from SDK 55 template):**
- `react-native-gesture-handler` — no longer in SDK 55 tabs template
- `moti` — unmaintained (12+ months without update), use raw Reanimated API or `@alloc/moti` fork if needed

**`metro.config.js`** — monorepo-aware:

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

> **Note:** Tamagui v2 no longer requires a custom `resolveRequest` hook for `.native.js` resolution — Metro's default platform-specific resolution handles it correctly with v2's updated package exports.

**`babel.config.js`** — Tamagui v2 compiler (optional, improves perf):

```js
module.exports = function (api) {
  api.cache(true);
  return {
    presets: ['babel-preset-expo'],
    plugins: [
      [
        '@tamagui/babel-plugin',
        {
          components: ['tamagui', '@tamagui/ui'],
          config: './tamagui.config.ts',
          logTimings: true,
          disableExtraction: process.env.NODE_ENV === 'development',
        },
      ],
      'react-native-worklets/plugin', // Must be last (Reanimated v4 moved the plugin here)
    ],
  };
};
```

**`tamagui.config.ts`** — brand design system (v2 uses `@tamagui/config/v5`):

```ts
import { defaultConfig } from '@tamagui/config/v5';
import { createTamagui } from 'tamagui';
import { colors, spacing } from '@claude-view/design-tokens';

const config = createTamagui({
  ...defaultConfig,
  tokens: {
    ...defaultConfig.tokens,
    color: {
      ...defaultConfig.tokens.color,
      ...colors,       // Brand palette from packages/design-tokens
    },
    space: {
      ...defaultConfig.tokens.space,
      ...spacing,      // Brand spacing scale
    },
  },
});

export default config;
export type Conf = typeof config;

declare module 'tamagui' {
  interface TamaguiCustomConfig extends Conf {}
}
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
6. Rename `tsconfig.json` → `tsconfig.base.json` and rewrite as shared base; add `turbo.json`, `bunfig.toml`
7. Update `crates/server/` static file path: `dist/` → `apps/web/dist/`
8. Update dev scripts for new paths
9. Scaffold `apps/mobile/` with SDK 55 and `apps/landing/` (static HTML):

   ```bash
   # SDK 55 template (native tabs layout)
   npx create-expo-app@latest apps/mobile --template tabs@sdk-55
   cd apps/mobile

   # Add Tamagui v2 RC (styling + design system + components)
   bun add tamagui @tamagui/config @tamagui/ui
   bun add -D @tamagui/babel-plugin

   # Add interaction layer
   bun add @gorhom/bottom-sheet

   # Add icons + storage + state
   bun add lucide-react-native react-native-svg
   bun add react-native-mmkv react-native-nitro-modules
   bun add zustand@5.0.7

   # Add polish layer (use npx expo install for SDK-aligned versions)
   npx expo install expo-haptics expo-blur expo-linear-gradient expo-image expo-secure-store

   # Add data + lists
   bun add @shopify/flash-list @tanstack/react-query

   # Create tamagui.config.ts, babel.config.js, metro.config.js (see §6)
   ```

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
| Tamagui token-first design system | Tamagui v2 RC is the `latest` npm tag (team considers production-ready). Targets RN 0.81+/React 19+/NArch. 17 RCs + continuous canary = active development. ~24KB core, zero external deps, optimizing compiler. [Tamagui Expo guide](https://tamagui.dev/docs/guides/expo). Verified working: [Expo SDK 54 + Tamagui v2 + monorepo](https://github.com/expo/expo/discussions/42767). v1.144.3 available as stable fallback. |
| Static landing page on Cloudflare Pages | Standard pattern for app landing pages (universal links + store redirects) |
| `app.config.ts` for Expo | Happy, standard Expo practice for build variants |

---

## Cross-references

- [`docs/plans/mobile-remote/design.md`](mobile-remote/design.md) — Mobile remote zero-setup design (architecture, command protocol, security model)
- [`docs/plans/backlog/2026-02-12-mobile-pwa-design.md`](backlog/2026-02-12-mobile-pwa-design.md) — Original PWA design (superseded by Expo pivot)
- [`docs/plans/mission-control/design.md`](mission-control/design.md) — Mission Control architecture (relay depends on Phase A)
