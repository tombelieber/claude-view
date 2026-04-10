# CLAUDE.md

Project-wide instructions for Claude/Codex-style agents working in this repo.

## CI Rules

- Do not add or rely on remote CI for quality gates in this repo.
- All CI checks must be local-bounded and enforced through local scripts or
  local git hooks such as `./scripts/ci-local.sh` and `lefthook` pre-push.
- GitHub Actions are reserved for release builds and release publishing only.
- Do not recreate push or pull-request GitHub Actions workflows for lint,
  typecheck, tests, contract checks, or drift checks.

## Cargo Rules

- Use `bun dev` for normal local development. It already routes the Rust server
  through `./scripts/cq`.
- Do not run raw CPU-heavy Cargo commands directly in this repo. For
  `run`, `build`, `test`, `check`, `clippy`, `bench`, and `doc`, always use
  `./scripts/cq ...` instead.
- Examples:
  - `./scripts/cq run -p claude-view-server`
  - `./scripts/cq test --workspace`
  - `./scripts/cq check`
  - `./scripts/cq clippy --workspace -- -D warnings`
- Why: `./scripts/cq` serializes heavy Cargo work across worktrees, shares the
  repo-common Rust `target/`, and prevents per-worktree build-cache bloat.
