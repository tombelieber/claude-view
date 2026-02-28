# Monorepo Infra Hardening — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Harden monorepo infrastructure to production-grade by satisfying all contracts defined in the design doc.

**Architecture:** Contract-based hardening — each task fixes one or more contract violations, verified by a concrete command. No app code changes.

**Tech Stack:** Rust (main.rs path resolution), TypeScript configs (tsconfig, vite, playwright), Turborepo, Bun workspaces, shell scripts.

**Design doc:** `docs/plans/2026-02-26-monorepo-infra-hardening-design.md`

---

### Task 1: Remove stale root `src/` directory

**Contracts:** B4 (no stale artifacts)

**Files:**
- Delete: `src/` (entire directory, 141 generated type files — stale pre-monorepo leftover)

**Step 1: Verify root `src/` is untracked and not imported anywhere**

Run: `git status src/ && grep -r '../../src/' apps/ packages/`
Expected: `src/` shows as `??` (untracked), grep finds zero matches.

**Step 2: Delete the directory**

```bash
rm -rf src/
```

**Step 3: Verify deletion**

Run: `test ! -d src/ && echo "PASS: src/ deleted"`
Expected: PASS

**Step 4: Commit**

```bash
git add -A src/
git commit -m "chore: remove stale root src/ directory (pre-monorepo leftover)"
```

Note: Since `src/` is untracked, `git add -A src/` may be a no-op. If so, this is just a local cleanup with no git action needed.

---

### Task 2: Update doc references from `src/` to `apps/web/src/`

**Contracts:** B4 (no stale artifacts pointing to wrong paths)

**Files:**
- Modify: `docs/claude-rules-reference.md:152`
- Modify: `docs/architecture/message-types.md:211,344-347`

**Step 1: Fix claude-rules-reference.md**

Change line 152 from:
```
| TS type | Field typed | Check `src/types/generated/` |
```
to:
```
| TS type | Field typed | Check `apps/web/src/types/generated/` |
```

**Step 2: Fix message-types.md**

Change line 211 from:
```
**TypeScript type** (`src/types/generated/Role.ts`):
```
to:
```
**TypeScript type** (`apps/web/src/types/generated/Role.ts`):
```

Change lines 344-347 from:
```
| **TS Types** | `src/types/generated/Role.ts` | Generated TypeScript Role type |
| **Rich Pane** | `src/components/live/RichPane.tsx` | RichMessage type + renderer dispatch |
| **Hook→Message** | `src/lib/hook-events-to-messages.ts` | Converts SQLite hook events to timeline items |
| **Action Types** | `src/components/live/action-log/types.ts` | ActionCategory type + HookEventItem |
```
to:
```
| **TS Types** | `apps/web/src/types/generated/Role.ts` | Generated TypeScript Role type |
| **Rich Pane** | `apps/web/src/components/live/RichPane.tsx` | RichMessage type + renderer dispatch |
| **Hook→Message** | `apps/web/src/lib/hook-events-to-messages.ts` | Converts SQLite hook events to timeline items |
| **Action Types** | `apps/web/src/components/live/action-log/types.ts` | ActionCategory type + HookEventItem |
```

Note: Old plan files in `docs/plans/` are historical records — they document what was done at the time and should NOT be updated.

**Step 3: Verify no active docs still reference root `src/`**

Run: `grep -rn '"src/' docs/claude-rules-reference.md docs/architecture/message-types.md | grep -v apps/web`
Expected: Zero matches.

**Step 4: Commit**

```bash
git add docs/claude-rules-reference.md docs/architecture/message-types.md
git commit -m "docs: update src/ references to apps/web/src/ for monorepo layout"
```

---

### Task 3: Add no-op `build` scripts to workspace packages

**Contracts:** B5, W2 (Turbo `^build` dependency chain resolves)

**Files:**
- Modify: `packages/shared/package.json`
- Modify: `packages/design-tokens/package.json`

**Step 1: Add build script to packages/shared/package.json**

Add `"build": "echo 'No build step — consumed as TypeScript source'"` to scripts:

```json
{
  "scripts": {
    "build": "echo 'No build step — consumed as TypeScript source'",
    "typecheck": "tsc --noEmit",
    "lint": "echo 'no lint configured yet'"
  }
}
```

**Step 2: Add build script to packages/design-tokens/package.json**

Add `"build": "echo 'No build step — consumed as TypeScript source'"` to scripts:

```json
{
  "scripts": {
    "build": "echo 'No build step — consumed as TypeScript source'",
    "typecheck": "tsc --noEmit"
  }
}
```

**Step 3: Verify Turbo resolves the dependency chain**

Run: `bunx turbo build --dry-run`
Expected: Output shows `@claude-view/design-tokens#build`, `@claude-view/shared#build`, and `@claude-view/web#build` in dependency order. No warnings about missing tasks.

**Step 4: Commit**

```bash
git add packages/shared/package.json packages/design-tokens/package.json
git commit -m "chore: add no-op build scripts to workspace packages for Turbo ^build"
```

---

### Task 4: Harden `get_static_dir()` with binary-relative path probe

**Contracts:** P1, P3 (binary-relative resolution, works from any CWD)

**Files:**
- Modify: `crates/server/src/main.rs:32-53`

**Step 1: Replace `get_static_dir()` function**

Replace lines 32-53 in `crates/server/src/main.rs` with:

```rust
/// Get the static directory for serving frontend files.
///
/// Priority:
/// 1. STATIC_DIR environment variable (explicit override)
/// 2. Binary-relative ./dist (npx distribution: binary + dist/ are siblings)
/// 3. CWD-relative ./apps/web/dist (monorepo dev layout via cargo run)
/// 4. CWD-relative ./dist (legacy flat layout)
/// 5. None (API-only mode)
fn get_static_dir() -> Option<PathBuf> {
    // 1. Explicit override always wins
    if let Ok(dir) = std::env::var("STATIC_DIR") {
        let p = PathBuf::from(&dir);
        if p.exists() {
            return Some(p);
        }
        tracing::warn!(static_dir = %dir, "STATIC_DIR set but directory does not exist");
        return None;
    }

    // 2. Binary-relative: resolves symlinks (Homebrew), works regardless of CWD
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(canonical) = exe.canonicalize() {
            if let Some(exe_dir) = canonical.parent() {
                let bin_dist = exe_dir.join("dist");
                if bin_dist.exists() {
                    return Some(bin_dist);
                }
            }
        }
    }

    // 3. CWD-relative: monorepo layout (cargo run from repo root)
    let monorepo_dist = PathBuf::from("apps/web/dist");
    if monorepo_dist.exists() {
        return Some(monorepo_dist);
    }

    // 4. CWD-relative: flat layout fallback
    let dist = PathBuf::from("dist");
    dist.exists().then_some(dist)
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p claude-view-server 2>&1 | tail -5`
Expected: `Finished` with no errors.

**Step 3: Run existing server tests**

Run: `cargo test -p claude-view-server 2>&1 | tail -10`
Expected: All existing tests pass (no regressions).

**Step 4: Commit**

```bash
git add crates/server/src/main.rs
git commit -m "fix: add binary-relative path probe to get_static_dir()

Resolves symlinks via canonicalize() for Homebrew compatibility.
Logs warning when STATIC_DIR is set but missing.
Keeps CWD-relative fallback for cargo run dev workflow."
```

---

### Task 5: Fix Playwright config to use absolute paths

**Contracts:** P4 (no fragile relative paths)

**Files:**
- Modify: `apps/web/playwright.config.ts:24`

**Step 1: Replace the webServer command**

Change line 24 from:
```typescript
    command: 'cd ../.. && cargo run -p claude-view-server',
```
to:
```typescript
    command: `cd ${path.resolve(__dirname, '../..')} && cargo run -p claude-view-server`,
```

**Step 2: Verify syntax**

Run: `cd apps/web && npx tsc --noEmit playwright.config.ts 2>&1 || echo "Skip typecheck — Playwright config not in tsconfig scope"`

Note: Playwright config may not be in tsconfig scope. Visual inspection of the template literal is sufficient — `path.resolve(__dirname, '../..')` always resolves to the monorepo root regardless of CWD.

**Step 3: Commit**

```bash
git add apps/web/playwright.config.ts
git commit -m "fix: use absolute path in Playwright webServer command"
```

---

### Task 6: Scope root `dev` script to web-only

**Contracts:** W3 (default dev doesn't start Expo)

**Files:**
- Modify: `package.json:9` (root)

**Step 1: Change dev script and add dev:all**

In root `package.json`, change:
```json
"dev": "turbo dev",
```
to:
```json
"dev": "turbo dev --filter=@claude-view/web",
"dev:all": "turbo dev",
```

**Step 2: Verify the script resolves**

Run: `bun run dev --dry-run 2>&1 | head -5 || bunx turbo dev --filter=@claude-view/web --dry-run 2>&1 | head -5`
Expected: Shows only `@claude-view/web#dev` in the task graph, not `@claude-view/mobile#dev`.

**Step 3: Commit**

```bash
git add package.json
git commit -m "chore: scope default dev script to web-only, add dev:all for full monorepo"
```

---

### Task 7: Simplify `__APP_VERSION__` to single source

**Contracts:** V3 (no dual version source)

**Files:**
- Modify: `apps/web/vite.config.ts:7,11-13`

**Step 1: Remove dual source, use only root package.json**

Replace lines 7 and 10-14 in `apps/web/vite.config.ts`:

From:
```typescript
const rootPkg = JSON.parse(readFileSync(path.resolve(__dirname, '../../package.json'), 'utf-8'))

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(
      process.env.npm_package_version || rootPkg.version
    ),
  },
```

To:
```typescript
const rootPkg = JSON.parse(readFileSync(path.resolve(__dirname, '../../package.json'), 'utf-8'))

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(rootPkg.version),
  },
```

This removes the `npm_package_version` env var fallback. The root `package.json` version is the single source of truth, kept in sync by `release.sh`.

**Step 2: Verify build still injects version**

Run: `cd apps/web && bun run build 2>&1 | tail -3 && grep -o '__APP_VERSION__\|"0\.8\.0"' dist/assets/*.js | head -3`
Expected: Build succeeds. Version string `"0.8.0"` appears in bundled JS (it's inlined by Vite define).

**Step 3: Commit**

```bash
git add apps/web/vite.config.ts
git commit -m "fix: use single version source for __APP_VERSION__ (root package.json only)"
```

---

### Task 8: Add explicit strict flags to mobile tsconfig

**Contracts:** W4 (consistent strict mode across monorepo)

**Files:**
- Modify: `apps/mobile/tsconfig.json`

**Step 1: Add key flags from monorepo base without breaking Expo compatibility**

Replace `apps/mobile/tsconfig.json` contents with:

```json
{
  "extends": "expo/tsconfig.base",
  "compilerOptions": {
    "strict": true,
    "moduleResolution": "bundler",
    "verbatimModuleSyntax": false,
    "paths": {
      "@/*": ["./*"]
    }
  },
  "include": ["**/*.ts", "**/*.tsx", ".expo/types/**/*.ts", "expo-env.d.ts"]
}
```

Notes:
- `strict: true` — already present, confirmed.
- `moduleResolution: "bundler"` — matches monorepo base, required for `workspace:*` imports.
- `verbatimModuleSyntax: false` — Expo/Metro can't handle `import type` syntax enforcement yet, so we explicitly disable this (monorepo base sets `true`). This is intentional divergence documented inline.
- We do NOT extend `../../tsconfig.base.json` because Expo's base includes React Native-specific settings (jsx, lib, etc.) that we need.

**Step 2: Verify typecheck passes**

Run: `cd apps/mobile && bunx tsc --noEmit 2>&1 | tail -5`
Expected: No errors (or only pre-existing ones unrelated to this change).

**Step 3: Commit**

```bash
git add apps/mobile/tsconfig.json
git commit -m "chore: add explicit strict flags to mobile tsconfig for monorepo consistency"
```

---

### Task 9: Gitignore `package-lock.json`

**Contracts:** C4 (explicit lockfile policy)

**Files:**
- Modify: `.gitignore`

**Step 1: Verify package-lock.json is already gitignored**

Run: `grep 'package-lock' .gitignore`

If the line `package-lock.json` already exists (as shown in our exploration — line 35), this task is **already done**. Verify and skip.

Expected: Line `# npm lockfile (Bun workspaces use workspace:* which npm can't resolve)` + `package-lock.json` already present.

**Step 2: Verify no staged package-lock.json**

Run: `git ls-files package-lock.json`
Expected: Empty (file is not tracked). If it returns `package-lock.json`, run `git rm --cached package-lock.json` to untrack it.

**Step 3: Commit if any change was needed**

```bash
# Only if git rm --cached was needed:
git add .gitignore
git commit -m "chore: ensure package-lock.json is untracked and gitignored"
```

---

### Task 10: Verify remaining contracts (no code changes expected)

**Contracts:** W1 (exports point to real files), W5 (no cross-boundary imports)

**Step 1: Verify all package.json exports point to existing files**

Run:
```bash
for pkg in packages/shared packages/design-tokens; do
  echo "=== $pkg ==="
  main=$(node -e "console.log(require('./$pkg/package.json').main)")
  test -f "$pkg/$main" && echo "PASS: $main exists" || echo "FAIL: $main missing"
done
```
Expected: All PASS.

**Step 2: Verify no cross-boundary imports**

Run: `grep -rn '\.\./\.\./src/' apps/ packages/`
Expected: Zero matches (already verified during planning, confirm at execution time).

**Step 3: No commit needed — verification only.**

---

### Task 11: Full contract verification sweep

Run every contract's verification command end-to-end.

**Step 1: Build hermeticity**

```bash
cd apps/web && bun run build && test -f dist/index.html && echo "B2 PASS"
cargo build -p claude-view-server && echo "B3 PASS"
test ! -d src/ && echo "B4 PASS"
bunx turbo build --dry-run 2>&1 | grep -q "design-tokens#build" && echo "B5 PASS"
```

**Step 2: Path resolution**

```bash
# Verify get_static_dir() compiles with binary-relative probe
cargo test -p claude-view-server && echo "P1 PASS"
```

**Step 3: Version consistency**

```bash
ROOT_V=$(node -e "console.log(require('./package.json').version)")
WEB_V=$(node -e "console.log(require('./apps/web/package.json').version)")
CARGO_V=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
NPX_V=$(node -e "console.log(require('./npx-cli/package.json').version)")
echo "root=$ROOT_V web=$WEB_V cargo=$CARGO_V npx=$NPX_V"
[ "$ROOT_V" = "$WEB_V" ] && [ "$ROOT_V" = "$CARGO_V" ] && [ "$ROOT_V" = "$NPX_V" ] && echo "V1 PASS" || echo "V1 FAIL"
```

**Step 4: Workspace integrity**

```bash
bunx turbo dev --filter=@claude-view/web --dry-run 2>&1 | grep -c "mobile" | grep -q "^0$" && echo "W3 PASS"
```

**Step 5: CI pipeline**

```bash
git ls-files package-lock.json | wc -l | grep -q "^0$" && echo "C4 PASS"
```

**Step 6: Report all results**

Print summary table of all contract statuses.

---

## Execution Notes

- Tasks 1-3 are pure config/cleanup — no risk of runtime regression.
- Task 4 (Rust `get_static_dir()`) is the highest-impact change — verify with `cargo test` and manual binary invocation.
- Tasks 5-8 are config tightening — low risk.
- Task 9 may be a no-op (already gitignored).
- Task 10-11 are verification only.
- Total: ~8 actual changes, ~3 verification-only steps.
