# Server (crates/server/ + crates/core/) Plans

Rust backend — Axum HTTP, JSONL parsing, search, live monitoring, pricing, sessions.

## Active

| File | Status | Description |
|------|--------|-------------|
| `2026-02-28-plugin-skill-mcp-design.md` | approved | Claude Code plugin + skill + MCP server |
| `2026-02-28-plugin-skill-mcp-impl.md` | approved | Implementation plan for plugin/skill/MCP |
| `2026-02-24-session-backup-design.md` | done (standalone); integration deferred | Session backup tool at `claude-backup` repo |

## Recently Completed

| File | Description |
|------|-------------|
| `archived/2026-02-28-clean-user-message-design.md` | Strip noise tags from lastUserMessage, IDE file chip |
| `archived/2026-02-28-clean-user-message-impl.md` | Rust parser + accumulator + types + web/mobile SessionCard |
| `archived/2026-02-24-reliability-release-design.md` | Reliability release design |
| `archived/2026-02-24-reliability-release-impl.md` | Reliability release implementation |
| `archived/2026-02-24-reliability-release-issues.md` | 4 foundation bugs (PR #14) |
| `archived/2026-02-22-unified-search-design.md` | Unified search design |
| `archived/2026-02-22-unified-search.md` | Unified search implementation |
| `archived/2026-02-22-hook-events-in-conversation-history.md` | Hook events in conversation history |
| `archived/2026-02-21-work-reports-design.md` | Work reports design |
| `archived/2026-02-21-work-reports-impl.md` | Work reports implementation |
| `archived/2026-02-21-unified-llm-settings-design.md` | Unified LLM settings |
| `archived/2026-02-21-unified-llm-settings-impl.md` | Unified LLM settings implementation |
| `archived/2026-02-21-session-liveness-final-fix.md` | Session liveness final fix |
| `archived/2026-02-21-lossless-rich-messages-design.md` | Lossless rich messages design |
| `archived/2026-02-21-lossless-rich-messages.md` | Lossless rich messages implementation |
| `archived/2026-02-21-live-state-recovery-design.md` | Live state recovery design |
| `archived/2026-02-21-live-state-recovery-impl.md` | Live state recovery implementation |
| `archived/2026-02-21-jsonl-ground-truth-recovery-design.md` | JSONL ground truth recovery design |
| `archived/2026-02-21-jsonl-ground-truth-recovery.md` | JSONL ground truth recovery implementation |
| `archived/2026-02-21-hook-event-group-accuracy.md` | Hook event group accuracy |
| `archived/2026-02-21-fix-token-deduplication.md` | Token deduplication fix |
| `archived/2026-02-21-custom-skill-registry-design.md` | Custom skill registry design |
| `archived/2026-02-21-custom-skill-registry-impl.md` | Custom skill registry implementation |
| `archived/2026-02-21-costusd-parity-design.md` | Cost USD parity design |
| `archived/2026-02-21-costusd-parity-impl.md` | Cost USD parity implementation |
| `archived/2026-02-20-session-card-question-compacting-design.md` | Session card question compacting |
| `archived/2026-02-20-session-card-question-compacting-impl.md` | Session card compacting implementation |
| `archived/2026-02-20-ppid-session-liveness-design.md` | PPID session liveness design |
| `archived/2026-02-20-ppid-session-liveness-impl.md` | PPID session liveness implementation |
| `archived/2026-02-20-live-monitor-rich-history-design.md` | Live monitor rich history design |
| `archived/2026-02-20-live-monitor-rich-history-impl.md` | Live monitor rich history implementation |
| `archived/2026-02-20-hook-events-log-design.md` | Hook events log design |
| `archived/2026-02-20-hook-events-log-impl.md` | Hook events log implementation |
| `archived/2026-02-18-full-text-search-design.md` | Phase 6: Tantivy search |

## Backlog

(Area-specific future work)
