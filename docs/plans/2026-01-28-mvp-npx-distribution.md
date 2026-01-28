---
status: pending
date: 2026-01-28
---

# MVP Distribution: `npx claude-view`

## Summary

Ship `claude-view` as an npm package that downloads a pre-built Rust binary from GitHub Releases on first run. Supports macOS (ARM64 + Intel), Linux x64, and Windows x64.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Package name | `claude-view` | Self-describing, maximizes LLM discoverability |
| Frontend assets | Separate files (not embedded) | Open-source friendly, inspectable, swappable |
| Binary hosting | GitHub Releases | Free, proven, what vibe-kanban does |
| Platform scope | macOS ARM64/x64, Linux x64, Windows x64 | GitHub Actions has runners for all four |
| Version strategy | Pinned to npm package version | No network check on launch, users update via `@latest` |

## Architecture

### npx wrapper (`npx-cli/`)

When a user runs `npx claude-view`, npm downloads and executes a small JS package (~80 lines). It does:

1. **Detect platform** — `process.platform` + `process.arch` → map to download artifact name. Unsupported combos get a clear error.
2. **Check cache** — Look for binary at `~/.cache/claude-view/bin/vibe-recall`. Compare `~/.cache/claude-view/version` to wrapper version. If match, skip download.
3. **Download if needed** — Fetch tarball from `https://github.com/<org>/claude-view/releases/download/v<version>/claude-view-<platform>-<arch>.tar.gz`. Extract to cache dir. `chmod +x`.
4. **Run** — `execFileSync` the cached binary with `stdio: 'inherit'`. Set `STATIC_DIR` env var pointing to extracted `dist/` folder.

```
npx-cli/
├── package.json    # name: "claude-view", bin: "./index.js"
├── index.js        # platform detect → cache check → download → run
└── README.md       # npm page content
```

### Platform matrix

| Platform | Artifact name | Runner |
|----------|--------------|--------|
| macOS ARM64 | `claude-view-darwin-arm64.tar.gz` | `macos-14` |
| macOS Intel | `claude-view-darwin-x64.tar.gz` | `macos-13` |
| Linux x64 | `claude-view-linux-x64.tar.gz` | `ubuntu-latest` |
| Windows x64 | `claude-view-win32-x64.zip` | `windows-latest` |

Each tarball contains:
- `vibe-recall` binary (or `vibe-recall.exe` on Windows)
- `dist/` folder (built React SPA)

### GitHub Actions release pipeline

File: `.github/workflows/release.yml`

**Trigger:** Push git tag `v*` (e.g., `v0.1.0`)

**Matrix build (per platform):**
1. Checkout code
2. Install Rust toolchain
3. Install Node + Bun
4. `bun install && bun run build` → `dist/`
5. `cargo build --release` → binary
6. Package binary + `dist/` into tarball/zip
7. Upload as GitHub Release asset

**Post-build:**
- Create GitHub Release with all artifacts attached

### Version management

- Single source of truth: `npx-cli/package.json` version field
- Git tag matches: `v0.1.0`
- Cache stores version at `~/.cache/claude-view/version`
- On run: if cached version != wrapper version → re-download
- No "check for updates" network call. Users update via `npx claude-view@latest`

## Changes to existing code

**None.** The existing Rust server and React frontend require zero modifications:

- `STATIC_DIR` env var already supported in `crates/server/src/main.rs`
- `get_static_dir()` already resolves static file paths
- Release profile (LTO + strip) already configured in `Cargo.toml`

## New files

| File | Purpose |
|------|---------|
| `npx-cli/package.json` | npm package manifest |
| `npx-cli/index.js` | Download + cache + run logic |
| `npx-cli/README.md` | npm page |
| `.github/workflows/release.yml` | CI build + release pipeline |

## Not included (deferred)

- Linux ARM64 (Graviton/Raspberry Pi)
- Homebrew formula
- Auto-publish npm on release
- Docker
- Tauri desktop app
