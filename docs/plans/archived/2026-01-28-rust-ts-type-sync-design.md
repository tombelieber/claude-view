---
status: approved
date: 2026-01-28
---

# Rust → TypeScript Type Sync Design

## Problem

The Rust backend and React TypeScript frontend maintain separate type definitions for API responses. This causes:

- **Drift:** `SessionInfo` is missing 4 fields on the TS side (`summary`, `gitBranch`, `isSidechain`, `deepIndexed`)
- **Wasteful conversions:** `modifiedAt` is serialized as ISO string by Rust, then parsed back to Unix number by the frontend
- **Missing types:** 3 API response types (`InvocableWithCount`, `StatsOverview`, `ErrorResponse`) have no TS counterpart
- **Manual maintenance:** Every Rust type change requires a matching TS edit

## Solution

Use `ts-rs` to auto-generate TypeScript types from Rust structs. Rust is the single source of truth.

## Approach: ts-rs

`ts-rs` is a derive macro that reads Rust struct definitions (including serde attributes) and generates TypeScript interface files.

### Setup

Add `ts-rs` as a dev-dependency to crates that define API types:

```toml
# crates/core/Cargo.toml
[dev-dependencies]
ts-rs = "10"

# crates/db/Cargo.toml
[dev-dependencies]
ts-rs = "10"

# crates/server/Cargo.toml
[dev-dependencies]
ts-rs = "10"
```

### Annotating Types

Add `#[derive(TS)]` and `#[ts(export)]` to all API-boundary structs:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/types/generated/")]
pub struct ProjectInfo {
    pub name: String,
    pub display_name: String,
    pub sessions: Vec<SessionInfo>,
    pub active_count: usize,
}
```

`ts-rs` respects `#[serde(rename_all = "camelCase")]`, so generated TS uses camelCase field names automatically.

### Types to Annotate (11 total)

**`crates/core/src/types.rs`** (8 types):
- `ProjectInfo`
- `SessionInfo`
- `ToolCounts`
- `ParsedSession`
- `SessionMetadata`
- `Message`
- `Role`
- `ToolCall`

**`crates/db/src/queries.rs`** (2 types):
- `InvocableWithCount`
- `StatsOverview`

**`crates/server/src/routes/health.rs`** (1 type):
- `HealthResponse`

Internal-only types (parser intermediates, DB row structs) do NOT get `#[derive(TS)]`.

### modifiedAt Fix

Remove the custom ISO serializer from `SessionInfo.modified_at`:

```rust
// Before — wasteful round-trip: i64 → ISO string → number
#[serde(with = "unix_to_iso")]
pub modified_at: i64,

// After — direct: i64 → number
pub modified_at: i64,
```

This eliminates the ISO→number conversion in `fetchProjects()` and lets `ts-rs` generate `modifiedAt: number` without any `#[ts(type = "...")]` overrides.

### Output

Generated files land in `src/types/generated/`:

```
src/types/
  generated/
    ProjectInfo.ts
    SessionInfo.ts
    ToolCounts.ts
    ParsedSession.ts
    SessionMetadata.ts
    Message.ts
    Role.ts
    ToolCall.ts
    InvocableWithCount.ts
    StatsOverview.ts
    HealthResponse.ts
    index.ts          ← barrel re-export (hand-maintained)
```

Files are committed to git so frontend devs can work without running `cargo test`.

### Frontend Migration

1. Delete hand-written interfaces from:
   - `src/hooks/use-projects.ts` (`ProjectInfo`, `SessionInfo`, `ToolCounts`)
   - `src/hooks/use-session.ts` (`Message`, `ToolCall`, `SessionData`)
   - `src/components/HealthIndicator.tsx` (`HealthResponse`)

2. Replace with imports:
   ```typescript
   import type { ProjectInfo, SessionInfo } from '../types/generated'
   ```

3. Delete the `modifiedAt` ISO→number conversion in `fetchProjects()`.

## Workflow

### Generation

`ts-rs` exports types during `cargo test`. Running `cargo test -p core` regenerates all core types. No separate command needed.

### CI Staleness Guard

Add a CI step:

```bash
cargo test
git diff --exit-code src/types/generated/
```

If generated files differ from committed versions, CI fails. This catches "changed Rust type but forgot to commit regenerated TS."

## Summary

| Decision | Choice |
|----------|--------|
| Tool | `ts-rs` (v10) |
| Source of truth | Rust structs with `#[derive(TS)]` |
| Scope | 11 API-boundary types |
| Output directory | `src/types/generated/` (committed) |
| Trigger | `cargo test` auto-exports |
| CI guard | `git diff --exit-code` after generation |
| `modifiedAt` fix | Remove custom ISO serializer, send Unix number |
| Casing | `#[serde(rename_all = "camelCase")]` respected by ts-rs |
