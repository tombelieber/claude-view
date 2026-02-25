# Monorepo Restructure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restructure claude-view from a single Vite SPA + Rust backend into a Turborepo monorepo with `apps/web/`, `apps/mobile/`, `apps/landing/`, and `packages/shared/`, `packages/design-tokens/`.

**Architecture:** Big bang migration in one branch. Move web SPA into `apps/web/` via `git mv`, create workspace config at root, scaffold empty mobile and landing apps, create shared packages. Rust `crates/` untouched. One PR.

**Tech Stack:** Bun workspaces, Turborepo, Expo SDK 54, NativeWind v4, Astro (deferred), Cloudflare Pages

**Design doc:** `docs/plans/2026-02-25-monorepo-restructure-design.md`

---

## Task 1: Create directory scaffold

**Files:**
- Create: `apps/web/.gitkeep` (temporary, removed after Task 2)
- Create: `apps/mobile/.gitkeep` (temporary, removed after Task 8)
- Create: `apps/landing/.gitkeep` (temporary, removed after Task 9)
- Create: `packages/shared/.gitkeep` (temporary, removed after Task 6)
- Create: `packages/design-tokens/.gitkeep` (temporary, removed after Task 7)

**Step 1: Create all directories**

```bash
mkdir -p apps/web apps/mobile apps/landing packages/shared packages/design-tokens
touch apps/web/.gitkeep apps/mobile/.gitkeep apps/landing/.gitkeep packages/shared/.gitkeep packages/design-tokens/.gitkeep
```

**Step 2: Verify structure**

```bash
find apps packages -type d
```

Expected:
```
apps
apps/web
apps/mobile
apps/landing
packages
packages/shared
packages/design-tokens
```

**Step 3: Commit**

```bash
git add apps/ packages/
git commit -m "chore: create monorepo directory scaffold"
```

---

## Task 2: Move web SPA files into `apps/web/`

**Files:**
- Move: `src/` → `apps/web/src/`
- Move: `public/` → `apps/web/public/`
- Move: `index.html` → `apps/web/index.html`
- Move: `vite.config.ts` → `apps/web/vite.config.ts`
- Move: `vitest.config.ts` → `apps/web/vitest.config.ts`
- Move: `eslint.config.js` → `apps/web/eslint.config.js`
- Move: `playwright.config.ts` → `apps/web/playwright.config.ts`
- Move: `e2e/` → `apps/web/e2e/`
- Move: `tests/` → `apps/web/tests/`
- Move: `tsconfig.app.json` → `apps/web/tsconfig.json` (rename)
- Move: `tsconfig.node.json` → `apps/web/tsconfig.node.json`

**Step 1: git mv all web files**

Use `git mv` to preserve blame history.

```bash
git mv src apps/web/src
git mv public apps/web/public
git mv index.html apps/web/index.html
git mv vite.config.ts apps/web/vite.config.ts
git mv vitest.config.ts apps/web/vitest.config.ts
git mv eslint.config.js apps/web/eslint.config.js
git mv playwright.config.ts apps/web/playwright.config.ts
git mv e2e apps/web/e2e
git mv tests apps/web/tests
git mv tsconfig.app.json apps/web/tsconfig.json
git mv tsconfig.node.json apps/web/tsconfig.node.json
```

**Step 2: Remove .gitkeep**

```bash
rm apps/web/.gitkeep
```

**Step 3: Verify the move**

```bash
ls apps/web/
```

Expected: `e2e  eslint.config.js  index.html  playwright.config.ts  public  src  tests  tsconfig.json  tsconfig.node.json  vite.config.ts  vitest.config.ts`

**Step 4: Commit**

```bash
git add -A
git commit -m "refactor: move web SPA into apps/web/"
```

---

## Task 3: Create root workspace config

**Files:**
- Modify: `package.json` (root — becomes workspace root)
- Create: `apps/web/package.json` (web-specific deps)
- Modify: `tsconfig.json` → rewrite as `tsconfig.base.json`
- Create: `turbo.json`
- Create: `bunfig.toml`

**Step 1: Create `apps/web/package.json`**

Extract web-specific deps from root `package.json`. The web app keeps ALL existing dependencies and devDependencies that are specific to the web SPA. Root keeps only workspace-level config.

```jsonc
// apps/web/package.json
{
  "name": "@claude-view/web",
  "version": "0.8.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite --port 5173",
    "build": "vite build",
    "preview": "vite preview",
    "lint": "eslint .",
    "typecheck": "tsc --noEmit",
    "test": "vitest run",
    "test:watch": "vitest",
    "test:ui": "vitest --ui",
    "test:e2e": "playwright test",
    "test:e2e:ui": "playwright test --ui"
  },
  "dependencies": {
    "@radix-ui/react-popover": "^1.1.15",
    "@radix-ui/react-tabs": "^1.1.13",
    "@radix-ui/react-tooltip": "^1.2.8",
    "@tailwindcss/typography": "^0.5.19",
    "@tanstack/react-query": "^5.90.18",
    "@tanstack/react-table": "^8.21.3",
    "clsx": "^2.1.1",
    "dompurify": "^3.3.1",
    "lucide-react": "^0.562.0",
    "react": "^19.2.0",
    "react-day-picker": "^9.13.0",
    "react-dom": "^19.2.0",
    "react-markdown": "^9.0.1",
    "react-router-dom": "^7.13.0",
    "react-virtuoso": "^4.18.1",
    "recharts": "^3.7.0",
    "rehype-raw": "^7.0.0",
    "remark-gfm": "^4.0.0",
    "shiki": "^3.22.0",
    "sonner": "^2.0.7",
    "tailwind-merge": "^3.4.0",
    "zustand": "^5.0.10"
  },
  "devDependencies": {
    "@eslint/js": "^9.39.1",
    "@playwright/test": "^1.58.0",
    "@tailwindcss/vite": "^4.1.18",
    "@testing-library/dom": "^10.4.1",
    "@testing-library/jest-dom": "^6.9.1",
    "@testing-library/react": "^16.3.2",
    "@testing-library/user-event": "^14.6.1",
    "@types/node": "^22.0.0",
    "@types/react": "^19.2.5",
    "@types/react-dom": "^19.2.3",
    "@vitejs/plugin-react": "^5.1.1",
    "@vitest/ui": "^4.0.18",
    "eslint": "^9.39.1",
    "eslint-plugin-react": "^7.37.5",
    "eslint-plugin-react-hooks": "^7.0.1",
    "eslint-plugin-react-refresh": "^0.4.24",
    "globals": "^17.0.0",
    "happy-dom": "^20.4.0",
    "tailwindcss": "^4.1.18",
    "typescript": "~5.9.3",
    "typescript-eslint": "^8.46.4",
    "vite": "^7.2.4",
    "vitest": "^4.0.18"
  }
}
```

**Step 2: Rewrite root `package.json`**

Root becomes the Bun workspace root. Remove all web-specific deps. Keep Rust scripts, release scripts, and workspace config.

```jsonc
// package.json (root)
{
  "name": "claude-view",
  "version": "0.8.0",
  "private": true,
  "workspaces": ["apps/*", "packages/*"],
  "scripts": {
    "dev": "turbo dev",
    "build": "turbo build",
    "lint": "turbo lint",
    "typecheck": "turbo typecheck",
    "test": "turbo test",
    "dev:server": "unset CLAUDECODE CLAUDE_CODE_SSE_PORT CLAUDE_CODE_ENTRYPOINT && RUST_LOG=warn,claude_view_server=info,claude_view_core=info VITE_PORT=5173 cargo watch -w crates -x 'run -p claude-view-server'",
    "dev:full": "concurrently -n rust,web -c red,cyan \"bun run dev:server\" \"turbo dev --filter=@claude-view/web\"",
    "test:rust": "cargo test --workspace",
    "lint:rust": "cargo clippy --workspace -- -D warnings",
    "lint:all": "turbo lint && cargo clippy --workspace -- -D warnings",
    "fmt": "cargo fmt --all",
    "clean:db": "rm -f ~/Library/Caches/claude-view/claude-view.db*",
    "cleanupport": "PORTS='47892 5173'; for port in $PORTS; do pids=\"$(lsof -ti tcp:$port)\"; if [ -n \"$pids\" ]; then echo \"Killing $pids listening on port $port\"; kill -9 $pids; else echo \"No process found on port $port\"; fi; done",
    "release": "./scripts/release.sh",
    "release:minor": "./scripts/release.sh minor",
    "release:major": "./scripts/release.sh major",
    "release:bump": "node -e \"const v=process.argv[1]; const fs=require('fs'); const p=JSON.parse(fs.readFileSync('npx-cli/package.json','utf8')); p.version=v; fs.writeFileSync('npx-cli/package.json', JSON.stringify(p,null,2)+'\\n'); console.log('npx-cli/package.json -> '+v);\"",
    "release:tag": "node -e \"const v=require('./npx-cli/package.json').version; require('child_process').execFileSync('git',['tag','v'+v],{stdio:'inherit'}); console.log('Tagged v'+v);\"",
    "release:push": "git push && git push --tags"
  },
  "devDependencies": {
    "concurrently": "^9.1.2",
    "turbo": "^2"
  }
}
```

**Step 3: Create `turbo.json`**

```jsonc
// turbo.json
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

**Step 4: Create `bunfig.toml`**

```toml
[install]
linker = "hoisted"
```

**Step 5: Rewrite `tsconfig.json` as base config**

The old root `tsconfig.json` had project references (`tsconfig.app.json`, `tsconfig.node.json`) and a path alias. Now it becomes a base config that all packages extend.

```jsonc
// tsconfig.json (root — base config, replaces old project-references file)
{
  "compilerOptions": {
    "strict": true,
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "skipLibCheck": true,
    "esModuleInterop": true,
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true,
    "verbatimModuleSyntax": true,
    "moduleDetection": "force",
    "noEmit": true
  }
}
```

**Step 6: Update `apps/web/tsconfig.json`**

The file was moved from `tsconfig.app.json`. Update it to extend the root base and fix paths.

```jsonc
// apps/web/tsconfig.json (was tsconfig.app.json)
{
  "extends": "../../tsconfig.json",
  "compilerOptions": {
    "tsBuildInfoFile": "./node_modules/.tmp/tsconfig.app.tsbuildinfo",
    "useDefineForClassFields": true,
    "types": ["vite/client"],
    "jsx": "react-jsx",
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "erasableSyntaxOnly": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedSideEffectImports": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"]
    }
  },
  "include": ["src"],
  "exclude": ["src/server"]
}
```

**Step 7: Update `apps/web/tsconfig.node.json`**

```jsonc
// apps/web/tsconfig.node.json
{
  "extends": "../../tsconfig.json",
  "compilerOptions": {
    "tsBuildInfoFile": "./node_modules/.tmp/tsconfig.node.tsbuildinfo"
  },
  "include": ["vite.config.ts"]
}
```

**Step 8: Commit**

```bash
git add -A
git commit -m "refactor: create workspace root, turbo.json, split package.json"
```

---

## Task 4: Update Vite, Vitest, and Playwright configs for new paths

**Files:**
- Modify: `apps/web/vite.config.ts`
- Modify: `apps/web/vitest.config.ts`
- Modify: `apps/web/playwright.config.ts`

**Step 1: Update `apps/web/vite.config.ts`**

The `@` alias and `__APP_VERSION__` need path updates. The `__dirname` is now `apps/web/`, so the alias `./src` still works. But `package.json` version read needs to go up two levels to root.

```ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(
      process.env.npm_package_version ||
      require('../../package.json').version
    ),
  },
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 5173,
    proxy: {
      '/api/live/sessions': {
        target: 'http://localhost:47892',
        ws: true,
      },
      '/api': 'http://localhost:47892',
    },
  },
  build: {
    outDir: 'dist',
  },
})
```

**Step 2: Update `apps/web/vitest.config.ts`**

Path alias `@` still resolves to `./src` relative to `__dirname`, so this should work as-is. Verify `setupFiles` path.

```ts
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  test: {
    globals: true,
    environment: 'happy-dom',
    setupFiles: ['./src/test-setup.ts'],
    exclude: ['**/node_modules/**', '**/e2e/**', '**/.claude/**'],
  },
})
```

**Step 3: Update `apps/web/playwright.config.ts`**

The `webServer.command` runs Cargo from repo root. Since Playwright will be invoked from `apps/web/`, update the command to reference the correct CWD or use an absolute path.

```ts
import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  timeout: 180000,
  use: {
    baseURL: 'http://localhost:47892',
    trace: 'on-first-retry',
    actionTimeout: 15000,
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'cd ../.. && cargo run -p claude-view-server',
    url: 'http://localhost:47892/api/health',
    reuseExistingServer: !process.env.CI,
    timeout: 120000,
  },
})
```

**Step 4: Commit**

```bash
git add apps/web/vite.config.ts apps/web/vitest.config.ts apps/web/playwright.config.ts
git commit -m "refactor: update web app configs for monorepo paths"
```

---

## Task 5: Update Rust server static file path

**Files:**
- Modify: `crates/server/src/main.rs:38-46` (`get_static_dir` function)

**Step 1: Update `get_static_dir` to look for `apps/web/dist/`**

The function currently falls back to `./dist`. After the move, frontend build output is at `./apps/web/dist/`. Update the fallback chain:

1. `STATIC_DIR` env var (explicit override — unchanged)
2. `./apps/web/dist` (monorepo layout)
3. `./dist` (backwards compat for npx distribution, where dist is bundled alongside binary)

```rust
fn get_static_dir() -> Option<PathBuf> {
    std::env::var("STATIC_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            // Monorepo layout: apps/web/dist
            let monorepo_dist = PathBuf::from("apps/web/dist");
            if monorepo_dist.exists() {
                return Some(monorepo_dist);
            }
            // Fallback: flat layout (npx distribution)
            let dist = PathBuf::from("dist");
            dist.exists().then_some(dist)
        })
}
```

**Step 2: Run Rust tests to verify nothing broke**

```bash
cargo test -p claude-view-server
```

Expected: All tests pass. The static dir tests use `STATIC_DIR` env var or mock, so they're unaffected by the path change.

**Step 3: Commit**

```bash
git add crates/server/src/main.rs
git commit -m "feat: support apps/web/dist/ static file path for monorepo layout"
```

---

## Task 6: Create `packages/shared/`

**Files:**
- Create: `packages/shared/package.json`
- Create: `packages/shared/tsconfig.json`
- Create: `packages/shared/src/index.ts`
- Create: `packages/shared/src/types/relay.ts`
- Create: `packages/shared/src/theme.ts`

**Step 1: Create `packages/shared/package.json`**

```jsonc
{
  "name": "@claude-view/shared",
  "version": "0.0.0",
  "private": true,
  "type": "module",
  "main": "./src/index.ts",
  "types": "./src/index.ts",
  "exports": {
    ".": "./src/index.ts",
    "./*": "./src/*.ts"
  },
  "scripts": {
    "typecheck": "tsc --noEmit",
    "lint": "echo 'no lint configured yet'"
  }
}
```

**Step 2: Create `packages/shared/tsconfig.json`**

```jsonc
{
  "extends": "../../tsconfig.json",
  "compilerOptions": {
    "rootDir": "src",
    "outDir": "dist"
  },
  "include": ["src"]
}
```

**Step 3: Create `packages/shared/src/types/relay.ts`**

Stub the relay command protocol types from the mobile-remote design doc. These will be fleshed out when the relay is implemented.

```ts
/** Session status as reported by the Mac daemon */
export type SessionStatus = 'active' | 'waiting' | 'idle' | 'done';

/** Mac → Phone: session snapshot */
export interface RelaySessionSnapshot {
  type: 'sessions';
  sessions: RelaySession[];
}

export interface RelaySession {
  id: string;
  project: string;
  model: string;
  status: SessionStatus;
  cost_usd: number;
  tokens: { input: number; output: number };
  last_message: string;
  updated_at: number;
}

/** Mac → Phone: live output stream */
export interface RelayOutputStream {
  type: 'output';
  session_id: string;
  chunks: RelayOutputChunk[];
}

export interface RelayOutputChunk {
  role: 'assistant' | 'tool' | 'user';
  text?: string;
  name?: string;
  path?: string;
}

/** Phone → Mac: command */
export interface RelayCommand {
  type: 'command';
  action: string;
  session_id?: string;
  [key: string]: unknown;
}

/** Union of all relay message types */
export type RelayMessage =
  | RelaySessionSnapshot
  | RelayOutputStream
  | RelayCommand;
```

**Step 4: Create `packages/shared/src/theme.ts`**

```ts
/** Shared color palette consumed by both Tailwind (web) and NativeWind (mobile) configs */
export const colors = {
  primary: {
    50: '#eff6ff',
    100: '#dbeafe',
    200: '#bfdbfe',
    300: '#93c5fd',
    400: '#60a5fa',
    500: '#3b82f6',
    600: '#2563eb',
    700: '#1d4ed8',
    800: '#1e40af',
    900: '#1e3a8a',
  },
  // Session status colors
  status: {
    active: '#22c55e',    // green-500
    waiting: '#f59e0b',   // amber-500
    idle: '#3b82f6',      // blue-500
    done: '#6b7280',      // gray-500
    error: '#ef4444',     // red-500
  },
} as const;

export const spacing = {
  xs: 4,
  sm: 8,
  md: 16,
  lg: 24,
  xl: 32,
  '2xl': 48,
} as const;

export const typography = {
  fontFamily: {
    sans: 'Fira Sans, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
    mono: 'Fira Code, ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace',
  },
} as const;
```

**Step 5: Create `packages/shared/src/index.ts`**

```ts
export * from './types/relay.ts';
export * from './theme.ts';
```

**Step 6: Remove `.gitkeep`**

```bash
rm packages/shared/.gitkeep
```

**Step 7: Commit**

```bash
git add packages/shared/
git commit -m "feat: create @claude-view/shared package with relay types and theme"
```

---

## Task 7: Create `packages/design-tokens/`

**Files:**
- Create: `packages/design-tokens/package.json`
- Create: `packages/design-tokens/tsconfig.json`
- Create: `packages/design-tokens/src/colors.ts`
- Create: `packages/design-tokens/src/spacing.ts`
- Create: `packages/design-tokens/src/typography.ts`
- Create: `packages/design-tokens/src/index.ts`
- Move: `design-system/` contents → absorbed into `packages/design-tokens/`

**Step 1: Check what's in `design-system/`**

```bash
ls design-system/
cat design-system/claude-view-7-type-conversation-ui
```

If `design-system/` only contains image/asset files (not TS), keep it separate. If it contains TS token definitions, absorb them.

**Step 2: Create `packages/design-tokens/package.json`**

```jsonc
{
  "name": "@claude-view/design-tokens",
  "version": "0.0.0",
  "private": true,
  "type": "module",
  "main": "./src/index.ts",
  "types": "./src/index.ts",
  "exports": {
    ".": "./src/index.ts",
    "./*": "./src/*.ts"
  },
  "scripts": {
    "typecheck": "tsc --noEmit"
  }
}
```

**Step 3: Create `packages/design-tokens/tsconfig.json`**

```jsonc
{
  "extends": "../../tsconfig.json",
  "compilerOptions": {
    "rootDir": "src",
    "outDir": "dist"
  },
  "include": ["src"]
}
```

**Step 4: Create `packages/design-tokens/src/colors.ts`**

Extract from `packages/shared/src/theme.ts` (or duplicate — tokens are the canonical source, shared re-exports if needed).

```ts
/** Design token: color palette */
export const colors = {
  primary: {
    50: '#eff6ff',
    100: '#dbeafe',
    200: '#bfdbfe',
    300: '#93c5fd',
    400: '#60a5fa',
    500: '#3b82f6',
    600: '#2563eb',
    700: '#1d4ed8',
    800: '#1e40af',
    900: '#1e3a8a',
  },
  status: {
    active: '#22c55e',
    waiting: '#f59e0b',
    idle: '#3b82f6',
    done: '#6b7280',
    error: '#ef4444',
  },
  gray: {
    50: '#f9fafb',
    100: '#f3f4f6',
    200: '#e5e7eb',
    300: '#d1d5db',
    400: '#9ca3af',
    500: '#6b7280',
    600: '#4b5563',
    700: '#374151',
    800: '#1f2937',
    900: '#111827',
  },
} as const;
```

**Step 5: Create `packages/design-tokens/src/spacing.ts`**

```ts
/** Design token: spacing scale (in pixels) */
export const spacing = {
  0: 0,
  px: 1,
  0.5: 2,
  1: 4,
  2: 8,
  3: 12,
  4: 16,
  5: 20,
  6: 24,
  8: 32,
  10: 40,
  12: 48,
  16: 64,
} as const;
```

**Step 6: Create `packages/design-tokens/src/typography.ts`**

```ts
/** Design token: typography */
export const fontFamily = {
  sans: ['Fira Sans', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'Roboto', 'sans-serif'],
  mono: ['Fira Code', 'ui-monospace', 'SFMono-Regular', 'SF Mono', 'Menlo', 'Consolas', 'monospace'],
} as const;

export const fontSize = {
  xs: 12,
  sm: 14,
  base: 16,
  lg: 18,
  xl: 20,
  '2xl': 24,
  '3xl': 30,
} as const;
```

**Step 7: Create `packages/design-tokens/src/index.ts`**

```ts
export { colors } from './colors.ts';
export { spacing } from './spacing.ts';
export { fontFamily, fontSize } from './typography.ts';
```

**Step 8: Remove `.gitkeep` and clean up shared/theme.ts**

Update `packages/shared/src/theme.ts` to re-export from design-tokens instead of duplicating:

```ts
// packages/shared/src/theme.ts
export { colors, spacing, fontFamily, fontSize } from '@claude-view/design-tokens';
```

Add design-tokens as a dependency of shared:

```jsonc
// packages/shared/package.json — add to dependencies
"dependencies": {
  "@claude-view/design-tokens": "workspace:*"
}
```

```bash
rm packages/design-tokens/.gitkeep
```

**Step 9: Commit**

```bash
git add packages/design-tokens/ packages/shared/
git commit -m "feat: create @claude-view/design-tokens package"
```

---

## Task 8: Scaffold `apps/mobile/` (Expo)

**Files:**
- Create: `apps/mobile/app.config.ts`
- Create: `apps/mobile/package.json`
- Create: `apps/mobile/metro.config.js`
- Create: `apps/mobile/tsconfig.json`
- Create: `apps/mobile/tailwind.config.ts`
- Create: `apps/mobile/nativewind-env.d.ts`
- Create: `apps/mobile/app/_layout.tsx`
- Create: `apps/mobile/app/(tabs)/_layout.tsx`
- Create: `apps/mobile/app/(tabs)/index.tsx`

**Step 1: Create `apps/mobile/package.json`**

```jsonc
{
  "name": "@claude-view/mobile",
  "version": "0.0.0",
  "private": true,
  "main": "expo-router/entry",
  "scripts": {
    "dev": "expo start",
    "build:ios": "eas build --platform ios",
    "build:android": "eas build --platform android",
    "lint": "echo 'no lint configured yet'",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "@claude-view/shared": "workspace:*",
    "@claude-view/design-tokens": "workspace:*",
    "expo": "~54.0.0",
    "expo-router": "~4.0.0",
    "expo-secure-store": "~14.0.0",
    "expo-status-bar": "~2.0.0",
    "nativewind": "^4.1.0",
    "react": "^19.0.0",
    "react-native": "~0.77.0",
    "react-native-safe-area-context": "~5.0.0",
    "react-native-screens": "~4.0.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "tailwindcss": "^3.4.0",
    "typescript": "~5.9.3"
  }
}
```

Note: Exact Expo SDK 54 versions should be verified at scaffold time with `npx create-expo-app@latest --template tabs`. The versions above are approximate.

**Step 2: Create `apps/mobile/app.config.ts`**

```ts
import { ExpoConfig, ConfigContext } from 'expo/config';

export default ({ config }: ConfigContext): ExpoConfig => ({
  ...config,
  name: 'Claude View',
  slug: 'claude-view',
  version: '0.1.0',
  orientation: 'portrait',
  icon: './assets/icon.png',
  scheme: 'claude-view',  // Deep link scheme: claude-view://
  userInterfaceStyle: 'automatic',
  ios: {
    supportsTablet: false,
    bundleIdentifier: 'ai.claudeview.mobile',
    associatedDomains: ['applinks:m.claudeview.ai'],
  },
  android: {
    package: 'ai.claudeview.mobile',
    intentFilters: [
      {
        action: 'VIEW',
        autoVerify: true,
        data: [{ scheme: 'https', host: 'm.claudeview.ai', pathPrefix: '/' }],
        category: ['BROWSABLE', 'DEFAULT'],
      },
    ],
  },
  plugins: ['expo-router', 'expo-secure-store'],
});
```

**Step 3: Create `apps/mobile/metro.config.js`**

```js
const { getDefaultConfig } = require('expo/metro-config');
const { withNativeWind } = require('nativewind/metro');
const path = require('path');

const projectRoot = __dirname;
const monorepoRoot = path.resolve(projectRoot, '../..');

const config = getDefaultConfig(projectRoot);

// Watch all files in the monorepo
config.watchFolders = [monorepoRoot];

// Resolve packages from both project and monorepo root
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, 'node_modules'),
  path.resolve(monorepoRoot, 'node_modules'),
];

module.exports = withNativeWind(config, { input: './global.css' });
```

**Step 4: Create `apps/mobile/tsconfig.json`**

```jsonc
{
  "extends": "expo/tsconfig.base",
  "compilerOptions": {
    "strict": true,
    "paths": {
      "@/*": ["./src/*"]
    }
  },
  "include": ["**/*.ts", "**/*.tsx", ".expo/types/**/*.ts", "expo-env.d.ts", "nativewind-env.d.ts"]
}
```

**Step 5: Create `apps/mobile/tailwind.config.ts`**

```ts
import type { Config } from 'tailwindcss';
import { colors } from '@claude-view/design-tokens';

export default {
  content: ['./app/**/*.{ts,tsx}', './components/**/*.{ts,tsx}'],
  presets: [require('nativewind/preset')],
  theme: {
    extend: {
      colors: {
        primary: colors.primary,
        status: colors.status,
      },
    },
  },
  plugins: [],
} satisfies Config;
```

**Step 6: Create `apps/mobile/nativewind-env.d.ts`**

```ts
/// <reference types="nativewind/types" />
```

**Step 7: Create `apps/mobile/app/_layout.tsx`**

```tsx
import '../global.css';
import { Stack } from 'expo-router';
import { StatusBar } from 'expo-status-bar';

export default function RootLayout() {
  return (
    <>
      <StatusBar style="auto" />
      <Stack>
        <Stack.Screen name="(tabs)" options={{ headerShown: false }} />
      </Stack>
    </>
  );
}
```

**Step 8: Create `apps/mobile/app/(tabs)/_layout.tsx`**

```tsx
import { Tabs } from 'expo-router';

export default function TabLayout() {
  return (
    <Tabs>
      <Tabs.Screen
        name="index"
        options={{ title: 'Sessions', headerTitle: 'Claude Sessions' }}
      />
    </Tabs>
  );
}
```

**Step 9: Create `apps/mobile/app/(tabs)/index.tsx`**

```tsx
import { View, Text } from 'react-native';

export default function SessionsScreen() {
  return (
    <View className="flex-1 items-center justify-center bg-white dark:bg-gray-900">
      <Text className="text-xl font-bold text-gray-900 dark:text-white">
        Claude View Mobile
      </Text>
      <Text className="mt-2 text-gray-500 dark:text-gray-400">
        Session monitoring coming soon
      </Text>
    </View>
  );
}
```

**Step 10: Create `apps/mobile/global.css`**

```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

**Step 11: Create `apps/mobile/assets/` placeholder**

```bash
mkdir -p apps/mobile/assets
# TODO: Add app icon + splash screen assets
```

**Step 12: Remove `.gitkeep` and commit**

```bash
rm apps/mobile/.gitkeep
git add apps/mobile/
git commit -m "feat: scaffold Expo mobile app with NativeWind and shared packages"
```

---

## Task 9: Scaffold `apps/landing/` (static HTML)

**Files:**
- Create: `apps/landing/package.json`
- Create: `apps/landing/src/index.html`
- Create: `apps/landing/public/.well-known/apple-app-site-association`
- Create: `apps/landing/wrangler.toml`

**Step 1: Create `apps/landing/package.json`**

```jsonc
{
  "name": "@claude-view/landing",
  "version": "0.0.0",
  "private": true,
  "scripts": {
    "dev": "open src/index.html",
    "build": "mkdir -p dist && cp -r src/* dist/ && cp -r public/* dist/",
    "deploy": "wrangler pages deploy dist"
  }
}
```

**Step 2: Create `apps/landing/src/index.html`**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Claude View — Monitor Claude Code from your phone</title>
  <meta name="description" content="Monitor and control your Claude Code sessions from anywhere. Native iOS and Android app." />
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0f172a; color: #f8fafc; min-height: 100vh; display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 2rem; }
    h1 { font-size: 2.5rem; font-weight: 700; margin-bottom: 1rem; text-align: center; }
    p { font-size: 1.25rem; color: #94a3b8; margin-bottom: 2rem; text-align: center; max-width: 40rem; }
    .badges { display: flex; gap: 1rem; flex-wrap: wrap; justify-content: center; }
    .badges a img { height: 48px; }
  </style>
  <script>
    // Deep link handler: if opened with ?k=...&t=..., redirect to app or store
    (function() {
      const params = new URLSearchParams(window.location.search);
      if (params.has('k') && params.has('t')) {
        const deepLink = 'claude-view://pair?' + params.toString();
        window.location.href = deepLink;
        // Fallback: if app not installed, show the page (store badges below)
      }
    })();
  </script>
</head>
<body>
  <h1>Claude View</h1>
  <p>Monitor and control your Claude Code sessions from your phone. See live status, costs, and context — approve tool calls from anywhere.</p>
  <div class="badges">
    <!-- TODO: Replace with actual App Store and Play Store URLs -->
    <a href="#"><img src="https://tools.applemediaservices.com/api/badges/download-on-the-app-store/black/en-us" alt="Download on the App Store" /></a>
    <a href="#"><img src="https://play.google.com/intl/en_us/badges/static/images/badges/en_badge_web_generic.png" alt="Get it on Google Play" style="height: 70px; margin-top: -11px;" /></a>
  </div>
</body>
</html>
```

**Step 3: Create `apps/landing/public/.well-known/apple-app-site-association`**

```json
{
  "applinks": {
    "apps": [],
    "details": [
      {
        "appID": "TEAMID.ai.claudeview.mobile",
        "paths": ["*"]
      }
    ]
  }
}
```

Note: Replace `TEAMID` with actual Apple Developer Team ID when available.

**Step 4: Create `apps/landing/wrangler.toml`**

```toml
name = "claude-view-landing"
compatibility_date = "2026-02-25"

[site]
bucket = "./dist"
```

**Step 5: Remove `.gitkeep` and commit**

```bash
rm apps/landing/.gitkeep
git add apps/landing/
git commit -m "feat: scaffold landing page with universal links and deep link redirect"
```

---

## Task 10: Install dependencies and verify workspace

**Step 1: Delete old `node_modules` and reinstall**

```bash
rm -rf node_modules apps/web/node_modules
bun install
```

Expected: Bun resolves all workspaces. No errors. `bun.lock` updated.

**Step 2: Verify workspace resolution**

```bash
bun pm ls --all 2>/dev/null | head -20
```

Should show `@claude-view/web`, `@claude-view/mobile`, `@claude-view/landing`, `@claude-view/shared`, `@claude-view/design-tokens` as workspace packages.

**Step 3: Verify Turborepo sees all packages**

```bash
bunx turbo ls
```

Expected: Lists all 5 workspace packages.

**Step 4: Commit lockfile**

```bash
git add bun.lock package.json
git commit -m "chore: reinstall deps with Bun workspace resolution"
```

---

## Task 11: Run full test suite and verify nothing broke

**Step 1: Run web frontend tests**

```bash
cd apps/web && bunx vitest run
```

Expected: All 794 tests pass.

**Step 2: Run web typecheck**

```bash
cd apps/web && bunx tsc --noEmit
```

Expected: No errors.

**Step 3: Run web lint**

```bash
cd apps/web && bunx eslint .
```

Expected: No errors.

**Step 4: Run Rust tests**

```bash
cargo test --workspace
```

Expected: All 548+ tests pass.

**Step 5: Run Rust clippy**

```bash
cargo clippy --workspace -- -D warnings
```

Expected: No warnings.

**Step 6: Verify Turborepo task execution**

```bash
bunx turbo typecheck
```

Expected: Runs typecheck across all workspace packages that have the script.

**Step 7: Build web app and verify static serving**

```bash
cd apps/web && bunx vite build
```

Expected: Build outputs to `apps/web/dist/`.

Then verify Rust server finds it:

```bash
ls apps/web/dist/index.html
```

Expected: File exists.

**Step 8: Commit any remaining fixes**

If any tests failed due to path issues, fix them and commit:

```bash
git add -A
git commit -m "fix: resolve path issues from monorepo restructure"
```

---

## Task 12: Update CLAUDE.md and documentation

**Files:**
- Modify: `CLAUDE.md`
- Modify: `docs/plans/PROGRESS.md`

**Step 1: Update CLAUDE.md**

Add the monorepo structure to the Architecture section. Update paths throughout (e.g., `src/` references become `apps/web/src/`). Add:

- New workspace layout table
- Updated dev commands (`bunx turbo dev`, `dev:full`)
- New testing rules: `cd apps/web && bunx vitest run` instead of `bun run test:client`
- Note about `bunfig.toml` and Metro compatibility

**Step 2: Update PROGRESS.md**

Add monorepo restructure to "Recently Completed" section.

**Step 3: Commit**

```bash
git add CLAUDE.md docs/plans/PROGRESS.md
git commit -m "docs: update CLAUDE.md and PROGRESS.md for monorepo structure"
```

---

## Summary

| Task | What | Commit message |
|------|------|---------------|
| 1 | Directory scaffold | `chore: create monorepo directory scaffold` |
| 2 | Move web SPA files | `refactor: move web SPA into apps/web/` |
| 3 | Workspace config (package.json, turbo, bunfig, tsconfig) | `refactor: create workspace root, turbo.json, split package.json` |
| 4 | Update Vite/Vitest/Playwright configs | `refactor: update web app configs for monorepo paths` |
| 5 | Rust static file path | `feat: support apps/web/dist/ static file path for monorepo layout` |
| 6 | `packages/shared/` | `feat: create @claude-view/shared package with relay types and theme` |
| 7 | `packages/design-tokens/` | `feat: create @claude-view/design-tokens package` |
| 8 | Scaffold Expo mobile app | `feat: scaffold Expo mobile app with NativeWind and shared packages` |
| 9 | Scaffold landing page | `feat: scaffold landing page with universal links and deep link redirect` |
| 10 | Install deps and verify workspace | `chore: reinstall deps with Bun workspace resolution` |
| 11 | Run full test suite | `fix: resolve path issues from monorepo restructure` (if needed) |
| 12 | Update docs | `docs: update CLAUDE.md and PROGRESS.md for monorepo structure` |
