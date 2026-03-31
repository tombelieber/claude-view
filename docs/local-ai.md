# On-Device AI

Run a local LLM on your Mac for offline session classification and enrichment. Zero cloud dependency, zero API keys — everything stays on your machine.

## What It Does

claude-view uses a local LLM to classify coding sessions into phases (planning, implementation, debugging, testing, review) and enrich session metadata. This powers the phase badges, timeline views, and insights in the Live Monitor.

## Requirements

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| macOS | Apple Silicon (M1+) | M2 Pro or later |
| RAM | 4 GB free | 8 GB free |
| Disk | 2.5 GB (default model) | 5 GB (larger model) |
| Python | 3.10+ | 3.11+ |

## Quick Start

### 1. Install oMLX

[oMLX](https://github.com/nicholascpark/omlx) is a lightweight MLX inference server for Apple Silicon.

```bash
pip install omlx
```

Verify the install:

```bash
omlx --help
```

### 2. Enable in Settings

Open claude-view → **Settings** → **On-Device AI** card → click **Enable Local AI**.

That's it. claude-view will:
1. Download the default model (~2.5 GB, one-time)
2. Spawn oMLX as a managed child process
3. Start classifying sessions automatically

### 3. Verify It's Running

The On-Device AI card shows:
- **Status:** Running (green)
- **Mode:** Managed (blue badge) or External (gray badge)
- **Port:** 10710

## Models

Two curated models are available, both quantized for Apple Silicon via MLX:

| Model | Size | Min RAM | Use Case |
|-------|------|---------|----------|
| **Qwen 3.5 4B** (default) | 2.5 GB | 4 GB | Fast classification, works on all Apple Silicon |
| Qwen 3 8B | 5 GB | 8 GB | Higher accuracy, needs M1 Pro+ or 16 GB RAM |

Switch models in the On-Device AI card dropdown. If the new model isn't downloaded yet, it downloads automatically with progress UI (speed, ETA, cancel button).

## How It Works

### Managed Mode (default)

claude-view owns the oMLX process lifecycle:

```
Enable → detect omlx binary → download model → spawn omlx serve → health check → ready
Disable → SIGTERM → 3s grace → SIGKILL → port freed
```

- **Auto-restart:** If oMLX crashes, claude-view restarts it (up to 3 times)
- **Graceful shutdown:** On app exit, oMLX is terminated cleanly via SIGTERM
- **No orphans:** The process is tied to claude-view's lifecycle

### External Mode

If oMLX is already running on port 10710 when claude-view starts, it uses the existing process instead of spawning a new one. The mode badge shows "External".

This is useful if you:
- Run oMLX with custom flags
- Share one oMLX instance across multiple tools
- Manage oMLX via a process supervisor

```bash
# Start oMLX manually with custom settings
omlx serve --model-dir ~/.claude-view/local-llm/models/ --port 10710 --host 127.0.0.1
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OMLX_PORT` | `10710` | Port for the oMLX server |
| `OMLX_PATH` | _(auto-detect)_ | Explicit path to the omlx binary |

### Config File

Persisted at `~/.claude-view/local-llm.json`:

```json
{
  "enabled": true,
  "active_model": "qwen3.5-4b-mlx-4bit"
}
```

### Data Directory

All model files live under `~/.claude-view/local-llm/`:

```
~/.claude-view/local-llm/
├── local-llm.json          # config (enabled, active model)
├── inventory.json           # downloaded model tracking
└── models/
    ├── qwen3.5-4b-mlx-4bit/ # ~2.5 GB
    └── qwen3-8b-mlx-4bit/   # ~5 GB (if downloaded)
```

## API Endpoints

For automation or scripting:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/local-llm/status` | GET | Current status, mode, model info |
| `/api/local-llm/enable` | POST | Enable + download + spawn (SSE progress) |
| `/api/local-llm/disable` | POST | Disable + kill managed process |
| `/api/local-llm/models` | GET | List available models with install status |
| `/api/local-llm/switch` | POST | Switch active model `{"model_id": "..."}` |
| `/api/local-llm/cancel-download` | POST | Cancel in-progress download |

## Troubleshooting

### "omlx not found"

oMLX isn't on your PATH:

```bash
# Check if installed
which omlx

# Install if missing
pip install omlx

# Or set explicit path
export OMLX_PATH=/path/to/omlx
```

### Status shows "Starting..." but never "Running"

The model may still be loading. Qwen 3.5 4B takes ~10s on M2 Pro, longer on M1. Check the oMLX logs:

```bash
# If managed mode, logs go to claude-view debug output
# If external mode, check the terminal where you started omlx
```

### Port 10710 already in use

Another process is using the port:

```bash
# Find what's using it
lsof -ti :10710 -sTCP:LISTEN

# Kill it if it's a stale oMLX
kill $(lsof -ti :10710 -sTCP:LISTEN)

# Or use a different port
OMLX_PORT=10711 ./target/debug/claude-view
```

### Model download stuck or failed

```bash
# Check disk space
df -h ~/.claude-view

# Clear partial download and retry
rm -rf ~/.claude-view/local-llm/models/qwen3.5-4b-mlx-4bit/
# Then click Enable again in the UI
```

### Disable doesn't free the port

If you see the port still in use after disabling:

```bash
# Force kill any orphaned oMLX
kill $(lsof -ti :10710 -sTCP:LISTEN) 2>/dev/null
```

## Uninstall

To completely remove local AI data:

```bash
rm -rf ~/.claude-view/local-llm/
```

This removes all downloaded models (~2.5-7.5 GB) and config. The feature can be re-enabled anytime — models will re-download.
