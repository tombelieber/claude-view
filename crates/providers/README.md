# claude-view-providers

Session ingestion for foreign AI coding agents. Each provider module discovers
on-disk session transcripts and normalizes them into the shared
`ConversationBlock` model (full tool-call structure preserved — no lossy
text flattening).

## Layout

- `kind.rs` — `ProviderKind` enum + per-provider metadata (display name, id
  prefix, env override, default session roots)
- `model.rs` — normalized output: `ForeignSession`, `ForeignSessionMeta`,
  `ForeignUsage` (Anthropic-shape token keys; model string is load-bearing
  for pricing)
- `discover.rs` — `Provider` trait, `DiscoveredSession`, registry
- `catalog.rs` — `ForeignCatalog`: discover-all + mtime-keyed stats cache
- `util/` — resilient JSONL reader, timestamp parsing, block builders
- `parsers/` — one module per provider

## Adding a provider

1. Add a `ProviderKind` variant + metadata in `kind.rs`.
2. Implement `Provider` (discover + parse) in `parsers/<name>.rs` with fixture
   tests under `tests/fixtures/<name>/`.
3. Register it in `discover.rs::registry()`.

## Attribution

On-disk format knowledge (session directory layouts, JSON schemas, edge-case
semantics such as streaming-snapshot merging and token-accounting quirks) was
derived from studying [agentsview](https://github.com/kenn-io/agentsview)
(MIT License, © 2026 Kenn Software LLC). The Rust implementations here are
original. Trust-gated exclusions (providers whose on-disk data cannot be
rendered faithfully) are documented in the provider matrix in the main repo
docs.
