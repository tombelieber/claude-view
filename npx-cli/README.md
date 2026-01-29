# claude-view

Browse and search your Claude Code sessions in a beautiful local web UI.

## Quick Start

```bash
npx claude-view
```

This downloads the pre-built binary for your platform, caches it locally, and starts a local web server. Open the printed URL in your browser to explore your Claude Code sessions.

## What It Does

- Reads your local `~/.claude/` session files (JSONL)
- Indexes them into a local SQLite database for fast search
- Serves a React UI at `http://localhost:47892`
- Everything stays on your machine -- no data is sent anywhere

## Configuration

| Env Variable | Default | Description |
|---|---|---|
| `CLAUDE_VIEW_PORT` | `47892` | Port for the local server |
| `PORT` | `47892` | Alternative port override |

## Supported Platforms

| OS | Architecture |
|---|---|
| macOS | Apple Silicon (arm64), Intel (x64) |
| Linux | x64 |
| Windows | x64 |

## How It Works

On first run, `npx claude-view` downloads a platform-specific tarball from GitHub Releases containing a compiled Rust binary and bundled frontend assets. The binary is cached at `~/.cache/claude-view/` so subsequent runs start instantly.

## Links

- [GitHub Repository](https://github.com/myorg/claude-view)
- [Report an Issue](https://github.com/myorg/claude-view/issues)

## License

MIT
