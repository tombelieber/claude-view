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

## JS / Tooling Rules

- Prefer explicit `esbuild` scripts over `tsup` for small repo-owned runtime
  bundles when stability matters. If a wrapper build is being SIGKILLed in
  `bun preview` or local release flows, replace the wrapper rather than
  debugging around opaque tool behavior.
- For workspace packages that are consumed as source and do not need a real
  compiled artifact, the `build` script must still emit a deterministic stamp
  file under `dist/` so Turbo task outputs stay clean. Do not rely on
  per-package Turbo config to silence `no output files found` warnings.
- In `happy-dom` / Vitest UI tests, provide a default `fetch` stub for local
  `/api/*` requests in shared test setup. Do not let tests attempt real
  localhost requests just because a component mounted without a focused mock.
