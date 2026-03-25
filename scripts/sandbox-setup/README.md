# Enterprise Sandbox (DACS) Setup

Deploy claude-view into an enterprise sandbox where `~/.claude/`, `~/.cache/`, and `~/.claude-view/` are **read-only**.

## How it works

| Step | Where | Script | What it does |
|------|-------|--------|-------------|
| 1 | **Outside** sandbox | `./setup.sh` | Build, wire hooks, package into `install-claude-view/` |
| 2 | Copy | — | `cp -r install-claude-view/ <sandbox-mount>/` |
| 3 | **Inside** sandbox | `./start.sh` | Set env vars, run binary (zero writes outside app dir) |

## Quick start

```bash
# 1. On your normal machine (repo root)
./scripts/sandbox-setup/setup.sh

# 2. Copy into sandbox
cp -r install-claude-view/ /your-dacs-mount/

# 3. Inside sandbox
cd /your-dacs-mount/install-claude-view
./start.sh
# Open http://localhost:47892
```

## Custom ports

```bash
./scripts/sandbox-setup/setup.sh --port 8080 --sidecar-port 4001
```

## What `setup.sh` does (outside sandbox)

1. Builds the Rust binary, frontend, and sidecar
2. Writes 14 Claude Code hooks to `~/.claude/settings.json`
3. Creates a statusline wrapper at `~/.cache/claude-view/statusline-wrapper.sh`
4. Packages everything into `install-claude-view/` with a generated `start.sh`

## What `start.sh` does (inside sandbox)

Runs the binary with these env vars — **zero writes outside the app directory**:

| Variable | Value | Purpose |
|----------|-------|---------|
| `CLAUDE_VIEW_SKIP_HOOKS=1` | Skip hook registration | `~/.claude/settings.json` is read-only |
| `CLAUDE_VIEW_DATA_DIR=./data` | SQLite + search index | Writes to app dir, not `~/.claude-view/` |
| `CLAUDE_VIEW_TELEMETRY=0` | Disable telemetry | No telemetry config writes |
| `STATIC_DIR=./dist` | Frontend assets | Explicit path, no search needed |
| `SIDECAR_DIR=./sidecar` | Node.js sidecar | Explicit path, no search needed |

## Directory layout (inside sandbox)

```
install-claude-view/
  claude-view          # Rust binary
  start.sh             # Generated start script
  dist/                # Compiled frontend (HTML/JS/CSS)
  sidecar/             # Node.js sidecar (Agent SDK wrapper)
    dist/index.js
    package.json
    node_modules/
  data/                # Runtime data (created on first run)
    claude-view.db
    search-index/
    telemetry.json
```

## Requirements

- **Outside**: Rust toolchain, Node.js, bun
- **Inside**: Node.js (for sidecar), `curl` + `jq` (for hooks)
