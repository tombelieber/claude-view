# Monorepo Restructure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restructure claude-view from a single Vite SPA + Rust backend into a Turborepo monorepo with `apps/web/`, `apps/mobile/`, `apps/landing/`, and `packages/shared/`, `packages/design-tokens/`.

**Architecture:** Big bang migration in one branch. Move web SPA into `apps/web/` via `git mv`, create workspace config at root, scaffold empty mobile and landing apps, create shared packages. Rust `crates/` untouched. One PR.

**Tech Stack:** Bun workspaces, Turborepo, Expo SDK 55 (stable, `next` tag), Tamagui v2 RC (`2.0.0-rc.17`, `latest` on npm), Cloudflare Pages

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
- `.env.example` — stays at root (unchanged, shared env template for all services)

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
- Rename + rewrite: `tsconfig.json` → `tsconfig.base.json` (shared base config all packages extend)
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
    "@claude-view/shared": "workspace:*",
    "@claude-view/design-tokens": "workspace:*",
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
    "zustand": "^5.0.10",
    "@radix-ui/react-dialog": "^1.1.11",
    "jsqr": "^1.4.0",
    "qrcode.react": "^4.2.0",
    "tweetnacl": "^1.0.3",
    "tweetnacl-util": "^0.15.1"
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
  "type": "module",
  "workspaces": ["apps/*", "packages/*"],
  "scripts": {
    "dev": "turbo dev",
    "build": "turbo build",
    "lint": "turbo lint",
    "typecheck": "turbo typecheck",
    "test": "turbo test",
    "start": "cargo run -p claude-view-server --release",
    "dev:server": "unset CLAUDECODE CLAUDE_CODE_SSE_PORT CLAUDE_CODE_ENTRYPOINT && RUST_LOG=warn,claude_view_server=info,claude_view_core=info VITE_PORT=5173 cargo watch -w crates -x 'run -p claude-view-server'",
    "dev:full": "concurrently -n rust,web -c red,cyan \"bun run dev:server\" \"turbo dev --filter=@claude-view/web\"",
    "test:rust": "cargo test --workspace",
    "lint:rust": "cargo clippy --workspace -- -D warnings",
    "lint:all": "turbo lint && cargo clippy --workspace -- -D warnings",
    "fmt": "cargo fmt --all",
    "clean:db": "rm -f ~/Library/Caches/claude-view/claude-view.db*",
    "cleanupport": "PORTS='47892 5173'; for port in $PORTS; do pids=\"$(lsof -ti tcp:$port)\"; if [ -n \"$pids\" ]; then echo \"Killing $pids listening on port $port\"; kill -9 $pids; else echo \"No process found on port $port\"; fi; done",
    "dist:pack": "mkdir -p /tmp/claude-view-staging && cp target/release/claude-view /tmp/claude-view-staging/ && cp -r apps/web/dist /tmp/claude-view-staging/ && tar -czf /tmp/claude-view-darwin-arm64.tar.gz -C /tmp/claude-view-staging . && echo 'Packed: /tmp/claude-view-darwin-arm64.tar.gz'",
    "dist:install": "rm -rf ~/.cache/claude-view && mkdir -p ~/.cache/claude-view/bin && tar -xzf /tmp/claude-view-darwin-arm64.tar.gz -C ~/.cache/claude-view/bin && echo '0.1.0' > ~/.cache/claude-view/version && echo 'Installed to ~/.cache/claude-view/'",
    "dist:run": "node npx-cli/index.js",
    "dist:test": "cd apps/web && bun run build && cd ../.. && cargo build --release -p claude-view-server && bun run dist:pack && bun run dist:install && bun run dist:run",
    "dist:clean": "rm -rf ~/.cache/claude-view /tmp/claude-view-staging /tmp/claude-view-darwin-arm64.tar.gz && echo 'Cleaned dist cache'",
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

> **Script migration note:** `bun run preview` from root is replaced by `bunx turbo preview --filter=@claude-view/web` (or `cd apps/web && bun run preview`). The `bench` script, if present, moves to `apps/web/package.json`.

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

**Step 5: Rename and rewrite `tsconfig.json` → `tsconfig.base.json`**

The old root `tsconfig.json` had project references (`tsconfig.app.json`, `tsconfig.node.json`) and a path alias. Now it becomes a base config that all packages extend.

```bash
git mv tsconfig.json tsconfig.base.json
```

```jsonc
// tsconfig.base.json (root — shared base config all packages extend)
// NOTE: No "DOM" in lib — packages don't need DOM types. apps/web adds its own.
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

**Step 6: Update `apps/web/tsconfig.json`**

The file was moved from `tsconfig.app.json`. Update it to extend the root base and fix paths.

```jsonc
// apps/web/tsconfig.json (was tsconfig.app.json)
{
  "extends": "../../tsconfig.base.json",
  "compilerOptions": {
    "tsBuildInfoFile": "./node_modules/.tmp/tsconfig.app.tsbuildinfo",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
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
  "extends": "../../tsconfig.base.json",
  "compilerOptions": {
    "tsBuildInfoFile": "./node_modules/.tmp/tsconfig.node.tsbuildinfo",
    "lib": ["ES2023"],
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "erasableSyntaxOnly": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedSideEffectImports": true
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
import { readFileSync } from 'fs'

const rootPkg = JSON.parse(readFileSync(path.resolve(__dirname, '../../package.json'), 'utf-8'))

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(
      process.env.npm_package_version || rootPkg.version
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
    host: true,
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
import path from 'path'

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
    env: { STATIC_DIR: path.resolve(__dirname, 'dist') },
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
  "extends": "../../tsconfig.base.json",
  "compilerOptions": {
    "rootDir": "src",
    "declaration": true,
    "declarationMap": true
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
/**
 * Shared theme tokens. These inline values are the initial placeholder —
 * Task 7 Step 8 replaces this file with re-exports from @claude-view/design-tokens.
 * IMPORTANT: These shapes must match design-tokens exports exactly (arrays for
 * fontFamily, numeric keys for spacing) to avoid breaking changes at replacement.
 */
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
} as const;

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

**Step 5: Create `packages/shared/src/index.ts`**

```ts
export * from './types/relay';
export * from './theme';
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
ls -la design-system/
ls -la design-system/claude-view-7-type-conversation-ui/
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
  "extends": "../../tsconfig.base.json",
  "compilerOptions": {
    "rootDir": "src",
    "declaration": true,
    "declarationMap": true
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
export { colors } from './colors';
export { spacing } from './spacing';
export { fontFamily, fontSize } from './typography';
```

**Step 8: Remove `.gitkeep` and clean up shared/theme.ts**

Update `packages/shared/src/theme.ts` to re-export from design-tokens instead of duplicating:

```ts
// packages/shared/src/theme.ts — replaces inline values from Task 6
export { colors, spacing } from '@claude-view/design-tokens';
export { fontFamily, fontSize } from '@claude-view/design-tokens';
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

**Step 9: Remove orphaned `design-system/` directory**

The old `design-system/` directory (contains only `claude-view-7-type-conversation-ui/MASTER.md` — a markdown design spec, not TS tokens) is now superseded by `packages/design-tokens/`. Remove it to avoid confusion.

```bash
git rm -r design-system/
```

**Step 10: Commit**

```bash
# Note: design-system/ deletion is already staged from Step 9 (git rm). Only add new/modified files here.
git add packages/design-tokens/ packages/shared/
git commit -m "feat: create @claude-view/design-tokens package"
```

---

## Task 8: Scaffold `apps/mobile/` (Expo SDK 55 + Tamagui v2)

**Files:**
- Create: `apps/mobile/app.config.ts`
- Create: `apps/mobile/package.json`
- Create: `apps/mobile/metro.config.js`
- Create: `apps/mobile/babel.config.js`
- Create: `apps/mobile/tamagui.config.ts`
- Create: `apps/mobile/tsconfig.json`
- Create: `apps/mobile/app/_layout.tsx`
- Create: `apps/mobile/app/(tabs)/_layout.tsx`
- Create: `apps/mobile/app/(tabs)/index.tsx`

> **Ground truth:** All core versions below are from `npx create-expo-app@latest --template tabs@sdk-55` (scaffolded Feb 25, 2026). Do NOT substitute with guessed versions.

**Step 1: Create `apps/mobile/package.json`**

```jsonc
{
  "name": "@claude-view/mobile",
  "version": "0.0.0",
  "private": true,
  "main": "expo-router/entry",
  "scripts": {
    "dev": "expo start",
    "dev:clear": "expo start -c",
    "build:ios": "eas build --platform ios",
    "build:android": "eas build --platform android",
    "lint": "echo 'no lint configured yet'",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "@claude-view/shared": "workspace:*",
    "@claude-view/design-tokens": "workspace:*",
    // --- Core SDK 55 (from template — pinned) ---
    "@expo/vector-icons": "^15.0.2",
    "@react-navigation/native": "^7.1.28",
    "expo": "~55.0.0",
    "expo-constants": "~55.0.7",
    "expo-font": "~55.0.4",
    "expo-linking": "~55.0.7",
    "expo-router": "~55.0.0",
    "expo-splash-screen": "~55.0.9",
    "expo-status-bar": "~55.0.4",
    "react": "19.2.0",
    "react-dom": "19.2.0",
    "react-native": "0.83.2",
    "react-native-reanimated": "~4.2.1",
    "react-native-safe-area-context": "~5.6.2",
    "react-native-screens": "~4.24.0",
    "react-native-web": "~0.21.0",
    "react-native-worklets": "0.7.2",
    // --- Tamagui v2 RC (latest on npm) ---
    "tamagui": "^2.0.0-rc.17",
    "@tamagui/config": "^2.0.0-rc.17",
    "@tamagui/ui": "^2.0.0-rc.17",
    "@tamagui/font-inter": "^2.0.0-rc.17",
    // --- Additional Expo first-party (install with npx expo install) ---
    "expo-secure-store": "~55.0.0",
    "expo-image": "~55.0.0"
  },
  "devDependencies": {
    "@tamagui/babel-plugin": "^2.0.0-rc.17",
    "@types/react": "~19.2.2",
    "typescript": "~5.9.2"
  }
}
```

Note: SDK 55 unifies all first-party package majors to 55 (e.g., `expo-router@~55.0.0`, not v6). `react-native-reanimated@~4.2.1` and `react-native-worklets@0.7.2` are SDK 55 requirements — do not downgrade. `react-native-gesture-handler` is no longer in the SDK 55 template. After initial creation, run `npx expo install` to auto-correct any version mismatches. See design doc §6 for the full dependency list (flash-list, mmkv, bottom-sheet, etc.) — add those when implementing each feature area.

**Step 2: Create `apps/mobile/app.config.ts`**

```ts
import { ExpoConfig, ConfigContext } from 'expo/config';

export default ({ config }: ConfigContext): ExpoConfig => ({
  ...config,
  name: 'Claude View',
  slug: 'claude-view',
  version: '0.1.0',
  orientation: 'portrait',
  icon: './assets/images/icon.png',
  scheme: 'claude-view',  // Deep link scheme: claude-view://
  userInterfaceStyle: 'automatic',
  splash: {
    image: './assets/images/splash-icon.png',
    resizeMode: 'contain',
    backgroundColor: '#ffffff',
  },
  ios: {
    supportsTablet: true,
    bundleIdentifier: 'ai.claudeview.mobile',
    associatedDomains: ['applinks:m.claudeview.ai'],
  },
  android: {
    package: 'ai.claudeview.mobile',
    adaptiveIcon: {
      foregroundImage: './assets/images/adaptive-icon.png',
      backgroundColor: '#ffffff',
    },
    intentFilters: [
      {
        action: 'VIEW',
        autoVerify: true,
        data: [{ scheme: 'https', host: 'm.claudeview.ai', pathPrefix: '/' }],
        category: ['BROWSABLE', 'DEFAULT'],
      },
    ],
  },
  web: {
    bundler: 'metro',
    output: 'static',
    favicon: './assets/images/favicon.png',
  },
  plugins: ['expo-router', 'expo-secure-store'],
  experiments: {
    typedRoutes: true,
  },
});
```

Note: No `newArchEnabled` flag — New Architecture is mandatory in SDK 55 (no opt-out). The SDK 55 template also uses `assets/images/` (not `assets/`).

**Step 3: Create `apps/mobile/metro.config.js`**

```js
const { getDefaultConfig } = require('expo/metro-config');
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

module.exports = config;
```

Note: Tamagui v2 does NOT require a custom `resolveRequest` hook — Metro's default platform-specific resolution works correctly with v2's updated package exports. No `withNativeWind` wrapper.

**Step 4: Create `apps/mobile/babel.config.js`**

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

Note: The Tamagui babel plugin is optional (app works without it, just slower). `disableExtraction: true` in dev prevents compiler-related crashes during hot reload. Reanimated plugin must always be last.

**Step 5: Create `apps/mobile/tamagui.config.ts`**

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

Note: v2 uses `@tamagui/config/v5` (not `/v3`). The `declare module` augmentation gives type-safe `$token` props throughout the app.

**Step 6: Create `apps/mobile/tsconfig.json`**

```jsonc
// Intentionally extends expo/tsconfig.base (not monorepo root) because
// Expo's base handles React Native specifics (JSX transform, metro resolver).
{
  "extends": "expo/tsconfig.base",
  "compilerOptions": {
    "strict": true,
    "paths": {
      "@/*": ["./*"]
    }
  },
  "include": ["**/*.ts", "**/*.tsx", ".expo/types/**/*.ts", "expo-env.d.ts"]
}
```

Note: No `nativewind-env.d.ts` reference — we're using Tamagui, not NativeWind.

**Step 6b: Create `apps/mobile/expo-env.d.ts` stub**

The tsconfig.json above includes `"expo-env.d.ts"` but this file is auto-generated only after the first `expo start`. Create the stub now so TypeScript doesn't error:

```bash
echo '/// <reference types="expo-router/types" />' > apps/mobile/expo-env.d.ts
```

**Step 7: Create `apps/mobile/app/_layout.tsx`**

```tsx
import { useEffect } from 'react';
import { useColorScheme } from 'react-native';
import { Stack } from 'expo-router';
import { StatusBar } from 'expo-status-bar';
import { TamaguiProvider } from 'tamagui';
import { useFonts } from 'expo-font';
import * as SplashScreen from 'expo-splash-screen';
import config from '../tamagui.config';

SplashScreen.preventAutoHideAsync();

export default function RootLayout() {
  const colorScheme = useColorScheme();
  const [fontsLoaded] = useFonts({
    Inter: require('@tamagui/font-inter/otf/Inter-Medium.otf'),
    InterBold: require('@tamagui/font-inter/otf/Inter-Bold.otf'),
  });

  useEffect(() => {
    if (fontsLoaded) {
      SplashScreen.hideAsync();
    }
  }, [fontsLoaded]);

  if (!fontsLoaded) return null;

  return (
    <TamaguiProvider config={config} defaultTheme={colorScheme === 'dark' ? 'dark' : 'light'}>
      <StatusBar style="auto" />
      <Stack>
        <Stack.Screen name="(tabs)" options={{ headerShown: false }} />
      </Stack>
    </TamaguiProvider>
  );
}
```

Note: `TamaguiProvider` must wrap the entire app. Font loading with `useFonts` + splash screen prevents FOUT. The `defaultTheme` prop on TamaguiProvider sets the initial theme before React renders.

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
import { YStack, Text, H1 } from 'tamagui';

export default function SessionsScreen() {
  return (
    <YStack flex={1} alignItems="center" justifyContent="center" backgroundColor="$background">
      <H1>Claude View Mobile</H1>
      <Text color="$colorSubtle" marginTop="$2">
        Session monitoring coming soon
      </Text>
    </YStack>
  );
}
```

Note: Uses Tamagui components (`YStack`, `H1`, `Text`) with `$token` props instead of `className` strings. This is the Tamagui way — all styling speaks the design token language.

**Step 10: Create `apps/mobile/assets/images/` with placeholder icons**

The SDK 55 template uses `assets/images/` (not `assets/`). `app.config.ts` references `icon.png`, `splash-icon.png`, and `adaptive-icon.png`.

```bash
mkdir -p apps/mobile/assets/images
# Copy icons from the SDK 55 template probe (or download defaults)
curl -sL "https://raw.githubusercontent.com/expo/expo/sdk-55/templates/expo-template-tabs/assets/images/icon.png" -o apps/mobile/assets/images/icon.png || echo "⚠️  Download placeholder icons manually."
# Create minimal splash and adaptive icons as copies for now
cp apps/mobile/assets/images/icon.png apps/mobile/assets/images/splash-icon.png 2>/dev/null || true
cp apps/mobile/assets/images/icon.png apps/mobile/assets/images/adaptive-icon.png 2>/dev/null || true
cp apps/mobile/assets/images/icon.png apps/mobile/assets/images/favicon.png 2>/dev/null || true
```

**Step 11: Add `@tamagui/font-inter` for font loading**

The `_layout.tsx` requires Inter font files. Install:

```bash
cd apps/mobile && bun add @tamagui/font-inter  # v1.x — verify against current Tamagui v2 RC release; pin to @tamagui/font-inter version that matches your tamagui core
```

**Step 12: Remove `.gitkeep` and commit**

```bash
rm apps/mobile/.gitkeep
git add apps/mobile/
git commit -m "feat: scaffold Expo SDK 55 mobile app with Tamagui v2 and shared packages"
```

**Step 13: Verify scaffold boots**

```bash
cd apps/mobile && npx expo start -c
```

Expected: Metro bundler starts, app loads with "Claude View Mobile" text using Tamagui components. Press `i` for iOS simulator or `a` for Android emulator to verify rendering. `Ctrl+C` when satisfied.

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

## Task 13: Update CI release workflow for monorepo paths

**Files:**
- Modify: `.github/workflows/release.yml`

**Step 1: Update frontend build path**

Line 79 (`bun run build`) now triggers `turbo build` which calls `apps/web`'s build. This should work automatically. Verify by checking that `apps/web/dist/` is created.

**Step 2: Update unix packaging step (line 89)**

```yaml
      - name: Package (unix)
        if: runner.os != 'Windows'
        run: |
          mkdir -p staging
          cp target/release/${{ matrix.binary }} staging/
          cp -r apps/web/dist staging/
          tar -czf ${{ matrix.artifact }} -C staging .
```

**Step 3: Update windows packaging step (line 98)**

```yaml
      - name: Package (windows)
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          New-Item -ItemType Directory -Force -Path staging
          Copy-Item "target\release\${{ matrix.binary }}" -Destination staging\
          Copy-Item -Recurse apps\web\dist staging\dist
          Compress-Archive -Path staging\* -DestinationPath ${{ matrix.artifact }}
```

**Step 4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "fix: update CI release workflow for monorepo dist path"
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
| 8 | Scaffold Expo mobile app | `feat: scaffold Expo SDK 55 mobile app with Tamagui v2 and shared packages` |
| 9 | Scaffold landing page | `feat: scaffold landing page with universal links and deep link redirect` |
| 10 | Install deps and verify workspace | `chore: reinstall deps with Bun workspace resolution` |
| 11 | Run full test suite | `fix: resolve path issues from monorepo restructure` (if needed) |
| 12 | Update docs | `docs: update CLAUDE.md and PROGRESS.md for monorepo structure` |
| 13 | Update CI release workflow | `fix: update CI release workflow for monorepo dist path` |

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `allowImportingTsExtensions: true` missing from root base tsconfig — `.ts` extension imports in `packages/shared/src/index.ts` and `packages/design-tokens/src/index.ts` would fail | Blocker | Added to root `tsconfig.json` base config (Task 3 Step 5). Also switched package index files to extensionless imports for consistency. |
| 2 | `require('../../package.json')` in ESM `vite.config.ts` — `require()` not defined in ESM context under Vite 7 | Blocker | Replaced with `readFileSync` + `JSON.parse` (Task 4 Step 1) |
| 3 | `host: true` dropped from `vite.config.ts` server config — breaks network dev access | Blocker | Restored `host: true` (Task 4 Step 1) |
| 4 | CI `release.yml` copies `dist/` at lines 89, 98 — path becomes `apps/web/dist/` after move. Every release ships binary with no UI. | Blocker | Added Task 13 to update CI workflow paths |
| 5 | Expo SDK version triple internally inconsistent: `expo ~54.0.0` + `expo-router ~4.0.0` + `react-native ~0.77.0` — none match any real SDK release | Blocker | Updated to SDK 55 per user decision: `expo ~55.0.0`, `expo-router ~55.0.0`, `react-native ~0.83.1` (Task 8 Step 1) |
| 6 | NativeWind v4 + Tailwind v3 → user decision to go SOTA with NativeWind v5 (preview) + Tailwind CSS v4 | Blocker | Updated header, Task 8 package.json (`nativewind ^5.0.0-preview`, `tailwindcss ^4.1.18`), removed `tailwind.config.ts` (CSS-first in TW v4), updated `global.css` to `@import "tailwindcss"` |
| 7 | `react-native-reanimated` and `react-native-gesture-handler` missing from mobile deps — required by Expo Router tab navigator, scaffold crashes without them | Blocker | Added both to Task 8 Step 1 package.json |
| 8 | `packages/shared/tsconfig.json` and `packages/design-tokens/tsconfig.json` have `outDir: "dist"` conflicting with inherited `noEmit: true` — silently produces nothing | Warning | Removed `outDir`, added `declaration`/`declarationMap` (moved from base config where they don't belong for apps) |
| 9 | `declaration: true` and `declarationMap: true` in base config inherited by apps — wrong for non-library packages | Warning | Moved to `packages/*/tsconfig.json` only |
| 10 | `apps/web/tsconfig.node.json` inherits `lib: ["ES2022", "DOM", "DOM.Iterable"]` — DOM types wrong for Node/Vite config context | Warning | Added `lib: ["ES2023"]` override and restored 5 linting flags from current `tsconfig.node.json` |
| 11 | `dist:pack`, `dist:install`, `dist:run`, `dist:test`, `dist:clean` scripts missing from root — npx distribution workflow broken | Warning | Added all 5 `dist:*` scripts to root `package.json` with updated `apps/web/dist` paths |
| 12 | `type: "module"` missing from root `package.json` | Warning | Added `"type": "module"` |
| 13 | `start` script missing from root — `bun run start` no longer works | Warning | Added `"start": "cargo run -p claude-view-server --release"` |
| 14 | Theme.ts type shapes (Task 6) incompatible with design-tokens re-export (Task 7) — `spacing` used named keys vs numeric, `fontFamily` used strings vs arrays | Warning | Rewrote Task 6 theme.ts to match design-tokens shapes exactly (numeric spacing keys, array fontFamily) |
| 15 | `apps/mobile/tailwind.config.ts` mixed ESM `import type` with CJS `require()` — non-standard, breaks if `type: module` added | Warning | Removed entire file — NativeWind v5 + TW v4 uses CSS-first config, no JS config needed |
| 16 | `global.css` created in Step 10 but imported in Step 7 `_layout.tsx` — misleading order for implementer | Minor | Moved `global.css` creation to Step 6, before `_layout.tsx` at Step 8 |
| 17 | Mobile tsconfig `@/*` path alias points to non-existent `./src/*` directory | Minor | Removed path alias from mobile tsconfig |
| 18 | Playwright `webServer.command` uses fragile `cd ../..` without controlling static dir | Minor | Added `env: { STATIC_DIR: path.resolve(__dirname, 'dist') }` for robustness |
| 19 | Design doc said `tsconfig.base.json`, impl plan kept it as `tsconfig.json` — inconsistent naming | Minor | **Resolved: renamed to `tsconfig.base.json`** (Turborepo convention, matches byCedric expo-monorepo-example). All package `extends` fields updated to `../../tsconfig.base.json`. `git mv tsconfig.json tsconfig.base.json` added as first command in Task 3 Step 5. |

> **Note:** Entries 20–23 below are superseded. They were written during an earlier NativeWind iteration before the Tamagui pivot. The current plan uses Tamagui — these steps/deps do not apply.

| ~~20~~ | ~~`global.css` for mobile used Tailwind v3 directives (`@tailwind base/components/utilities`)~~ | ~~Minor~~ | ~~Updated to Tailwind v4 syntax: `@import "tailwindcss"`~~ |
| ~~21~~ | ~~NativeWind v5 `global.css` used single `@import "tailwindcss"` shorthand — causes deserialization error on iOS/Android ([nativewind#1631](https://github.com/nativewind/nativewind/issues/1631))~~ | ~~Blocker~~ | ~~Replaced with explicit 4-line import: `tailwindcss/theme.css`, `tailwindcss/preflight.css`, `tailwindcss/utilities.css`, `nativewind/theme` (Task 8 Step 6)~~ |
| ~~22~~ | ~~Missing `postcss.config.mjs` — NativeWind v5 requires PostCSS with `@tailwindcss/postcss` plugin for CSS processing. Without it, no styles are applied.~~ | ~~Blocker~~ | ~~Added new Step 7 to Task 8 with `postcss.config.mjs` file~~ |
| ~~23~~ | ~~`react-native-css`, `@tailwindcss/postcss`, `postcss` missing from mobile deps — required by NativeWind v5 CSS pipeline~~ | ~~Blocker~~ | ~~Added `react-native-css ^0.2.0` to dependencies, `@tailwindcss/postcss ^4.1.18` and `postcss ^8.5.0` to devDependencies (Task 8 Step 1)~~ |
| 24 | `nativewind-env.d.ts` referenced `nativewind/types` (v4 API) — NativeWind v5 uses `react-native-css` for its type system | Blocker | Changed to `/// <reference types="react-native-css/types" />` (Task 8 Step 8) |
| 25 | DOM types (`DOM`, `DOM.Iterable`) in base `tsconfig.json` leak to `packages/shared` and `packages/design-tokens` — pure TS utility packages shouldn't have DOM globals | Warning | Removed DOM from base `lib` (now `["ES2022"]`), added `"lib": ["ES2022", "DOM", "DOM.Iterable"]` to `apps/web/tsconfig.json` (Task 3 Steps 5-6) |
| 26 | Duplicate Step 9 numbering in Task 8 — two steps both numbered 9 (root layout + tabs layout) | Warning | Renumbered: postcss.config.mjs=7, nativewind-env.d.ts=8, root layout=9, tabs layout=10, index screen=11, assets=12, commit=13 |
| 27 | Orphaned `design-system/` directory never deleted — plan absorbs contents into `packages/design-tokens/` but leaves old directory on disk | Warning | Added Step 9 to Task 7: `git rm -r design-system/` before commit |
| 28 | `app.config.ts` references `./assets/icon.png` but assets step only runs `mkdir -p` with a TODO comment — `expo start` warns on every reload | Warning | Updated Task 8 Step 12 to download Expo's template placeholder icon (1024x1024 PNG) with fallback |
| 29 | `apps/web/package.json` missing `@claude-view/shared` and `@claude-view/design-tokens` workspace deps — contradicts design doc's dependency graph | Warning | Added `"@claude-view/shared": "workspace:*"` and `"@claude-view/design-tokens": "workspace:*"` to web dependencies (Task 3 Step 1) |
| 30 | `esModuleInterop: true` in base tsconfig redundant with `verbatimModuleSyntax: true` and absent from current config — divergence from codebase | Warning | Removed `esModuleInterop` from root `tsconfig.json` (Task 3 Step 5) |
| 31 | `cat design-system/claude-view-7-type-conversation-ui` fails because it's a directory, not a file | Minor | Changed to `ls -la design-system/claude-view-7-type-conversation-ui/` (Task 7 Step 1) |
| 32 | Task 7 Step 9 describes `design-system/` as containing "a UI concept image" — actually contains `MASTER.md` (a markdown design spec) | Minor | Fixed description to "a markdown design spec, not TS tokens" |

### Round 2: NativeWind → Tamagui v2 RC Migration (SDK 55 Ground Truth)

Entries #5–7, #15, #20–24, #26 above are from the NativeWind era and are now **superseded** by this round.

| # | Issue | Severity | Fix Applied |
| --- | ----- | -------- | ----------- |
| 33 | **Design doc ↔ impl doc styling contradiction** — design doc specified Tamagui v1, impl doc specified NativeWind v5 (preview) + Tailwind CSS v4. Two mutually exclusive architectures in the same plan. | Blocker | Unified on Tamagui v2 RC (`2.0.0-rc.17`, `latest` on npm). Rewrote both docs. |
| 34 | **Reanimated version contradiction** — design doc said v4, impl doc said `~3.17.0` (v3). SDK 55 scaffold ground truth: `~4.2.1` (v4). | Blocker | Updated all deps to ground-truth `react-native-reanimated ~4.2.1` |
| 35 | **Complete Task 8 rewrite** — replaced all NativeWind v5 files (global.css, postcss.config.mjs, nativewind-env.d.ts, TW v4 CSS-first config) with Tamagui v2 equivalents (babel.config.js with `@tamagui/babel-plugin`, tamagui.config.ts with `@tamagui/config/v5`, TamaguiProvider in _layout.tsx) | Blocker | Full rewrite of Task 8: 8 files replaced, 3 files removed, new Tamagui provider pattern |
| 36 | **SDK 55 versions not from ground truth** — original deps were guesses. Scaffolded `npx create-expo-app@latest --template tabs@sdk-55` to get real versions. | Blocker | All SDK 55 deps now match scaffold: `expo ~55.0.0`, `react 19.2.0`, `react-native 0.83.2`, `expo-router ~55.0.0`, `react-native-reanimated ~4.2.1` |
| 37 | **Missing `react-native-worklets`** — new SDK 55 required dep (`0.7.2`) not in either doc | Blocker | Added `react-native-worklets 0.7.2` to deps |
| 38 | **`react-native-gesture-handler` removed in SDK 55** — no longer in template, listed as dep | Warning | Removed from deps list |
| 39 | **`moti v0.30` unmaintained** — 12+ months without update, risk for SDK 55 compat | Warning | Removed from deps. Noted raw Reanimated API as alternative for shared animations. |
| 40 | **`react-native-mmkv v4` missing peer dep** — requires `react-native-nitro-modules` | Warning | Added `react-native-nitro-modules` to deps |
| 41 | **Tamagui config import path** — design doc used `@tamagui/config/v3` (v1 API), should be `@tamagui/config/v5` (v2 API) | Warning | Fixed to `@tamagui/config/v5` in both docs |
| 42 | **SDK 55 tagged `next` not `latest`** — design doc said "stable Jan 2026", actually beta Jan 22, stable early Feb, npm tag is `next` | Minor | Updated to "stable Feb 2026, tagged next on npm". Scaffold command uses `--template tabs@sdk-55`. |
| 43 | **`react-native` version** — design doc said `0.83.1`, scaffold shows `0.83.2` | Minor | Fixed to `0.83.2` |
