# Disk Growth Investigation

## Summary

The repo kept growing for two separate reasons:

1. Active git worktrees were each compiling their own Rust `target/` and, for
   Playwright, their own `target-playwright/`.
2. Old `.worktrees/*` directories were left behind on disk after they stopped
   being tracked by `git worktree list`, so they became invisible to normal Git
   cleanup flows.

`cq` already solved queueing and one-shot incremental bloat, but the common dev
paths were still bypassing it and the repo did not have tooling to surface or
remove orphaned worktree directories.

## What We Found

### 1. The main dev scripts were bypassing `cq`

Both `scripts/dev.sh` and `scripts/dev-server.sh` claimed to use `cq`, but they
actually ran:

- `cargo watch -w crates -x 'run -p claude-view-server'`

That meant the hottest local workflow never got:

- cross-worktree build queueing
- process-group cleanup from `cq`
- any shared target-dir behavior

### 2. Rust build output was duplicated per worktree

Before this fix, `cq` did not assign a shared `CARGO_TARGET_DIR`. When invoked
from a worktree, Cargo defaulted to that worktree's local `target/`, so large
branches accumulated their own multi-GB build directories.

### 3. Playwright had the same duplication problem

`apps/web/playwright.config.ts` hardcoded:

- `CARGO_TARGET_DIR = ../../target-playwright`

Inside a git worktree that resolves to a worktree-local `target-playwright/`,
so every branch could create another large Rust cache just for Playwright.

### 4. Orphaned worktree directories were not visible to Git anymore

Some directories under `.worktrees/` still contained old build artifacts, but
their admin entries under `.git/worktrees/` were already missing. Git no longer
reported them in `git worktree list`, so they stayed on disk until removed
manually.

## Fixes In This PR

### 1. Route watched dev server builds through `cq`

- `scripts/dev.sh`
- `scripts/dev-server.sh`

Both now use:

- `cargo watch -s './scripts/cq run -p claude-view-server'`

This restores the queue and the shared target-dir behavior to the workflows
developers actually run every day.

### 2. Make `cq` default to a shared Rust `target/`

`scripts/cq` now resolves the git common dir and, unless the caller overrides
it, exports:

- `CARGO_TARGET_DIR=<repo-common-root>/target`

That keeps worktree builds in one shared cache instead of duplicating `target/`
per worktree.

### 3. Share Playwright Rust builds across worktrees

`apps/web/playwright.config.ts` now:

- resolves the git common dir
- uses `<repo-common-root>/target-playwright`
- sets `CARGO_INCREMENTAL=0` for the Playwright server bootstrap path
- routes the server startup through `./scripts/cq`

### 4. Add worktree storage tooling

`scripts/worktree-storage.sh` reports:

- active worktrees
- orphaned `.worktrees/*` directories
- per-worktree `target/`, `target-playwright/`, and `node_modules/` usage

It can also remove orphaned worktree directories explicitly.

## What This Does Not Solve

`node_modules/` is still per worktree. That duplication is structural for the
current Bun workspace layout and is not safely auto-shared in this PR.

This PR focuses on the largest avoidable duplication:

- Rust `target/`
- Playwright `target-playwright/`
- orphaned worktree directories that Git no longer tracks

## Operational Guidance

- Use `bun run disk:report` to inspect current worktree disk usage.
- Use `bun run disk:cleanup:orphans` to remove orphaned `.worktrees/*`
  directories.
- Use `bun run cargo:sweep` to prune stale incremental Rust caches.
