---
status: draft
date: 2026-02-10
---

# Multi-Tool Provider Architecture

> Support Cursor, OpenCode, Aider, Copilot, Windsurf, and other AI coding agents alongside Claude Code.

## Motivation

Today the app reads exclusively from `~/.claude/projects/**/*.jsonl` (Claude Code's native session data). Users of multiple AI coding tools have no single place to see all their AI-assisted development activity. Supporting multiple tools:

1. **Expands TAM** — Not everyone uses Claude Code exclusively
2. **Strengthens "AI Fluency Score"** — Score across ALL tools, not just one
3. **Moat via aggregation** — Becomes the unified dashboard for AI-assisted development
4. **Honest attribution** — Storage donut already labels "Claude Code" vs "This app" — extending to N tools is natural

## Landscape

| Tool | Data format | Data location | Session structure |
|------|------------|---------------|-------------------|
| **Claude Code** | JSONL (one file per session) | `~/.claude/projects/{encoded-path}/{session-id}.jsonl` | Turns with role/content/usage |
| **Cursor** | SQLite | `~/Library/Application Support/Cursor/User/globalStorage/` | Composer conversations in DB |
| **OpenCode** | JSONL / JSON | `~/.opencode/sessions/` | Similar to Claude Code |
| **Aider** | Markdown chat logs | `.aider.chat.history.md` per project | Markdown with fenced code blocks |
| **GitHub Copilot** | No local logs | Telemetry only (no user-accessible files) | N/A — would need extension API |
| **Windsurf (Codeium)** | SQLite | `~/Library/Application Support/Windsurf/` | Similar to Cursor |
| **Cline** | JSON | `~/.cline/tasks/` | Task-based conversations |

## Architecture: Provider Trait

```rust
// crates/core/src/provider.rs

pub trait CodingToolProvider: Send + Sync {
    /// Human-readable name ("Claude Code", "Cursor", "Aider")
    fn name(&self) -> &str;

    /// Brand color for UI attribution (hex)
    fn brand_color(&self) -> &str;

    /// Root directory where this tool stores data
    fn data_dir(&self) -> Result<PathBuf, DiscoveryError>;

    /// Discover all projects/sessions from the data directory
    fn discover_sessions(&self) -> Result<Vec<DiscoveredSession>, DiscoveryError>;

    /// Parse a single session into the unified Session model
    fn parse_session(&self, path: &Path) -> Result<ParsedSession, ParseError>;

    /// Total bytes consumed by this tool's data (for storage stats)
    fn storage_bytes(&self) -> Result<u64, io::Error>;
}
```

### Unified Session Model

All providers normalize into the same `ParsedSession` struct:

```rust
pub struct ParsedSession {
    pub provider: String,        // "claude-code", "cursor", "aider"
    pub session_id: String,
    pub project_path: Option<PathBuf>,
    pub started_at: Option<i64>,
    pub turns: Vec<UnifiedTurn>,
    pub model: Option<String>,
    pub total_tokens: Option<u64>,
}

pub struct UnifiedTurn {
    pub role: UnifiedRole,       // User, Assistant, System, Tool
    pub content: String,
    pub model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub timestamp: Option<i64>,
}
```

## Crate Changes

| Crate | Changes |
|-------|---------|
| `crates/core` | Add `provider.rs` trait, `providers/` module with one file per tool. Move Claude Code discovery/parsing behind `ClaudeCodeProvider` |
| `crates/db` | Add `provider` column to `sessions` table. Migration adds column + index. Storage stats query groups by provider |
| `crates/search` | No change — Tantivy indexes unified content regardless of source |
| `crates/server` | Storage stats endpoint returns per-provider breakdown. New `GET /api/providers` endpoint |

## Frontend Changes

| Component | Changes |
|-----------|---------|
| **StorageOverview donut** | Dynamic slices from `GET /api/stats/storage` (one slice per provider + app slices). Colors from provider `brand_color` |
| **Session list** | Provider icon/badge per session |
| **Dashboard stats** | Filter by provider or show aggregated |
| **AI Fluency Score** | Weighted across all providers |

## Implementation Phases

### Phase A: Refactor (no new tools yet)

Extract current Claude Code logic into `ClaudeCodeProvider` implementing the trait. Everything works exactly as before, but through the provider abstraction.

- [ ] Define `CodingToolProvider` trait in `crates/core`
- [ ] Create `providers/claude_code.rs` implementing the trait
- [ ] Move discovery, parsing, storage_bytes behind the trait
- [ ] Add `provider` column to sessions (migration)
- [ ] Update indexer to call provider trait methods
- [ ] Verify all existing tests still pass

### Phase B: Second provider (pick one)

Add the easiest second provider to validate the abstraction. **Recommended: Aider** (simplest format — Markdown chat logs, well-documented, easy to parse).

- [ ] Create `providers/aider.rs`
- [ ] Implement session discovery from `.aider.chat.history.md`
- [ ] Parse Markdown into `UnifiedTurn` structs
- [ ] Storage stats includes Aider data
- [ ] Frontend donut shows Aider slice
- [ ] Session list shows provider badge

### Phase C: Popular tools

Add remaining high-demand providers:

- [ ] Cursor (SQLite parsing)
- [ ] Cline (JSON tasks)
- [ ] OpenCode (JSONL)
- [ ] Windsurf (SQLite)

### Phase D: Community providers

- [ ] Provider plugin API (WASM or dynamic library)
- [ ] Community-contributed providers via PR
- [ ] Provider auto-detection (scan known paths on startup)

## Design Principles

1. **Provider is metadata, not a silo** — All sessions live in the same DB, same search index, same dashboard. Provider is just a column/filter
2. **Zero config** — Auto-detect installed tools by scanning known paths. No manual setup
3. **Graceful degradation** — If a tool's data dir doesn't exist, skip it silently
4. **Provider-specific brand** — Each tool gets its own color/icon in the UI for clear attribution
5. **Unified metrics** — AI Fluency Score, cost tracking, and trends work across all providers

## Open Questions

- **Copilot**: No local session data. Skip, or build a VS Code extension to export?
- **Naming**: If we support multiple tools, "claude-score" name may limit perception. Consider "dev-score" or "ai-score" post-rename?
- **Privacy**: Some tools may store data differently. Need per-provider privacy review
- **Update frequency**: Cursor/Windsurf use live SQLite DBs. Do we watch for changes or poll?

## When to Build

After GTM launch and AI Fluency Score ship. This is a **growth multiplier** — increases TAM from "Claude Code users" to "all AI coding tool users."

Suggested slot: **Phase 7** (after Phase 6: Search).
