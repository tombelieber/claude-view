# Monorepo Infra Hardening — Contract-Based Design

**Date:** 2026-02-26
**Branch:** worktree-monorepo-expo
**Scope:** Monorepo infrastructure only (build pipeline, CI, release, path resolution, workspace config, test harness). No app code changes.
**Approach:** Define verifiable contracts, fix violations, leave regression guards.

## Context

The monorepo restructure (18 commits) moved the web SPA to `apps/web/`, added mobile/landing scaffolds, and introduced Turborepo workspaces. Three parallel audit agents found the restructure is architecturally sound with zero regressions on core functionality. However, several infra-level issues need hardening before this is production-grade.

## Contracts

### Category 1: Build Hermeticity

Build must not rely on implicit state. Any clean checkout produces identical results.

| ID | Contract | Current Status | Fix Required |
|----|----------|---------------|-------------|
| B1 | `bun install --frozen-lockfile` succeeds on clean checkout | PASS | None |
| B2 | `cd apps/web && bun run build` produces `apps/web/dist/index.html` | PASS | None |
| B3 | `cargo build --release -p claude-view-server` succeeds | PASS | None |
| B4 | No stale source artifacts in repo that could shadow real sources | FAIL — root `src/` (141 generated type files) exists untracked | Delete root `src/`, update 2 doc references |
| B5 | Turbo `^build` dependency chain resolves for all packages | FAIL — `packages/shared` and `packages/design-tokens` have no `build` script | Add no-op `build` scripts |

### Category 2: Path Resolution Robustness

Binary finds frontend assets regardless of launch context.

| ID | Contract | Current Status | Fix Required |
|----|----------|---------------|-------------|
| P1 | `get_static_dir()` has binary-relative probe, not just CWD-relative | FAIL — only CWD-relative probes | Add `current_exe().parent().join("dist")` probe |
| P2 | `STATIC_DIR` env var always overrides other probes | PASS | None |
| P3 | Binary serves frontend when launched from arbitrary CWD (npx scenario) | PASS (via STATIC_DIR) but fragile | P1 fix makes this robust without env var |
| P4 | Playwright config uses absolute paths | FAIL — `cd ../..` relative | Use `path.resolve(__dirname, '../..')` |

### Category 3: Version Consistency

Release flow keeps all version sources in sync.

| ID | Contract | Current Status | Fix Required |
|----|----------|---------------|-------------|
| V1 | `release.sh` syncs 4 version sources (npx-cli, root, apps/web, Cargo.toml) | PASS | None |
| V2 | CI verify step fails on version mismatch | PASS | None |
| V3 | `__APP_VERSION__` has single authoritative source | WEAK — dual source (root pkg + apps/web pkg), no assertion | Add build-time assertion that both match, or read from single source |

### Category 4: Workspace Integrity

Monorepo workspace config is correct and enforceable.

| ID | Contract | Current Status | Fix Required |
|----|----------|---------------|-------------|
| W1 | Every package.json `exports` field points to existing files | NEEDS VERIFY | Script check |
| W2 | All workspace packages have a `build` script (for Turbo `^build`) | FAIL — shared + design-tokens missing | Add no-op build scripts (same as B5) |
| W3 | `bun run dev` only starts web app (not Expo) for default dev workflow | FAIL — starts all workspaces | Change root `dev` script to `turbo dev --filter=@claude-view/web`, add `dev:all` for full monorepo |
| W4 | Mobile tsconfig strict mode matches monorepo base | WEAK — extends `expo/tsconfig.base` not monorepo base | Add explicit `"strict": true` and key flags to mobile tsconfig (don't break Expo compatibility) |
| W5 | No workspace package imports outside its own tree (isolation) | NEEDS VERIFY | Grep for `../../src` patterns |

### Category 5: CI Pipeline Correctness

Release pipeline produces correct artifacts from monorepo layout.

| ID | Contract | Current Status | Fix Required |
|----|----------|---------------|-------------|
| C1 | CI builds frontend before Rust, packages both | PASS | None |
| C2 | Tarball contains binary + flat `dist/` | PASS | None |
| C3 | npx-cli `STATIC_DIR` matches tarball layout | PASS | None |
| C4 | `package-lock.json` policy is explicit (regenerated or gitignored) | FAIL — stale, deleted but not gitignored | Add to `.gitignore` with comment explaining why (npm can't handle `workspace:*`) |

## Fixes Summary

### Must Fix (contract violations)

1. **B4** — Delete root `src/` directory, update doc references in `docs/claude-rules-reference.md:152` and `docs/architecture/message-types.md:344`
2. **B5/W2** — Add `"build": "echo 'No build step'"` to `packages/shared/package.json` and `packages/design-tokens/package.json`
3. **P1** — Add binary-relative path probe to `get_static_dir()` in `crates/server/src/main.rs`
4. **P4** — Fix Playwright config to use `path.resolve(__dirname, '../..')`
5. **W3** — Change root `dev` script to filter web-only, add `dev:all` for full monorepo
6. **C4** — Add `package-lock.json` to `.gitignore`

### Should Fix (weak contracts)

7. **V3** — Simplify `__APP_VERSION__` to read only from root `package.json` (remove `npm_package_version` fallback that could diverge)
8. **W4** — Add explicit strict-mode flags to `apps/mobile/tsconfig.json` compilerOptions

### Verify Only (no code change expected)

9. **W1** — Script-verify all package.json exports point to real files
10. **W5** — Grep-verify no cross-boundary imports exist

## Design Decisions

### P1: Binary-relative path resolution

Pattern used by: Electron (resolves resources relative to `app.getAppPath()`), Go binaries (`os.Executable()`), Tauri (`resolve_resource()`).

```rust
fn get_static_dir() -> Option<PathBuf> {
    // 1. Explicit override
    if let Ok(dir) = std::env::var("STATIC_DIR") {
        return Some(PathBuf::from(dir));
    }

    // 2. Binary-relative (npx distribution: binary + dist/ are siblings)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.canonicalize().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())) {
            let flat = exe_dir.join("dist");
            if flat.exists() {
                return Some(flat);
            }
        }
    }

    // 3. CWD-relative fallback (cargo run from repo root)
    let monorepo_dist = PathBuf::from("apps/web/dist");
    if monorepo_dist.exists() {
        return Some(monorepo_dist);
    }
    let dist = PathBuf::from("dist");
    dist.exists().then_some(dist)
}
```

`canonicalize()` resolves symlinks (important for Homebrew which symlinks binaries). CWD fallback kept for `cargo run` dev workflow.

### W3: Dev script filtering

```json
"dev": "turbo dev --filter=@claude-view/web",
"dev:all": "turbo dev",
"dev:full": "concurrently ... dev:server ... turbo dev --filter=@claude-view/web"
```

Default `dev` = web only (95% use case). `dev:all` for when you need mobile too.

### C4: package-lock.json policy

npm cannot resolve `workspace:*` protocol (Bun-specific). Rather than maintaining a stale lockfile, gitignore it. Users install via `npx` (downloads binary), not `npm install` on the repo. Dev uses Bun.

## Verification Plan

After all fixes applied:

```bash
# Build hermeticity
rm -rf node_modules apps/web/dist target/release
bun install --frozen-lockfile
cd apps/web && bun run build && test -f dist/index.html
cargo build --release -p claude-view-server

# Path resolution
STATIC_DIR=/tmp/nonexistent cargo run -p claude-view-server &  # should warn no frontend
cd /tmp && /path/to/target/release/claude-view &  # should find dist via binary-relative

# Version consistency
./scripts/release.sh patch --dry-run  # verify all 4 files bumped

# Workspace integrity
turbo build  # should complete for all packages including shared/design-tokens
bun run dev  # should only start web, not Expo

# CI pipeline
bun run dist:pack && tar -tf /tmp/claude-view-darwin-arm64.tar.gz  # verify contents
```
