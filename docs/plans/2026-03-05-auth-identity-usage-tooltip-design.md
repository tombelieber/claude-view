# Auth Identity in Usage Tooltip — Design

**Date:** 2026-03-05
**Status:** Done (implemented 2026-03-06)

## Problem

1. `~/.claude/.credentials.json` no longer exists — Claude Code moved credentials to macOS Keychain
2. `cli.rs` auth detection is broken (only reads the file)
3. The `OAuthUsagePill` tooltip shows quota tiers but no user identity (email, org name)
4. `claude auth status` provides email, orgName, subscriptionType — but requires a subprocess

## Data Source Analysis

| Source | Has token | Has email | Has orgName | Has subscriptionType | Latency |
|--------|-----------|-----------|-------------|---------------------|---------|
| `~/.claude/.credentials.json` | Yes | No | No | Yes | ~0ms |
| macOS Keychain | Yes | No | No | Yes | ~50ms |
| `claude auth status` CLI | No | Yes | Yes | Yes | ~200ms |
| Anthropic OAuth API | N/A (needs token) | No | No | No | ~300ms |

**Decision:** Hybrid approach — Keychain for auth detection + token, `claude auth status` for identity (cached).

## Design

### Section 1: Backend — Fix `cli.rs` auth detection

Extract `read_keychain_credentials()` from `crates/server/src/routes/oauth.rs` into a shared location (e.g., `crates/core/src/credentials.rs`). Update `cli.rs::check_auth_from_credentials()` to use file → Keychain fallback.

### Section 2: Backend — New `/api/oauth/identity` endpoint

New endpoint in `crates/server/src/routes/oauth.rs`:

```
GET /api/oauth/identity → { email, orgName, subscriptionType, authMethod }
```

- Calls `claude auth status` on first request
- Caches result in `tokio::sync::OnceCell<AuthIdentity>` (never refreshed — restart server to re-auth)
- Returns cached identity on subsequent requests (~0ms)
- On failure (CLI missing, SIGKILL'd): returns `{ hasAuth: false }` — no error surfaced to UI

Response type:
```rust
struct AuthIdentityResponse {
    has_auth: bool,
    email: Option<String>,
    org_name: Option<String>,
    subscription_type: Option<String>,
    auth_method: Option<String>,
}
```

### Section 3: Frontend — Enrich `OAuthUsagePill` tooltip

Add identity info to the tooltip header:

```
┌───────────────────────────────────┐
│  Max Plan                         │
│  user@gmail.com                   │  ← from /api/oauth/identity
│  Acme Corp                        │  ← only if orgName is meaningful
├───────────────────────────────────┤
│  Session (5hr)  ██████░░░  42%    │
│  Weekly (7 day) ███░░░░░░  18%    │
│  Extra usage    $12.50/$50        │
└───────────────────────────────────┘
```

- New `useAuthIdentity()` hook: `GET /api/oauth/identity` with `staleTime: Infinity`
- Fetched lazily on first tooltip open (same pattern as usage refetch)
- OrgName suppressed if it's just `"<email>'s Organization"` (redundant)
- Graceful degradation: if identity unavailable, tooltip shows without it (no error)

## Files to Change

| File | Change |
|------|--------|
| `crates/core/src/credentials.rs` | NEW — shared Keychain reader |
| `crates/core/src/cli.rs` | Add Keychain fallback to auth check |
| `crates/core/src/lib.rs` | Export credentials module |
| `crates/server/src/routes/oauth.rs` | Add `/api/oauth/identity` endpoint, refactor to use shared credentials |
| `crates/server/src/state.rs` | Add `OnceCell<AuthIdentity>` to AppState |
| `apps/web/src/hooks/use-auth-identity.ts` | NEW — React Query hook for identity |
| `apps/web/src/components/live/OAuthUsagePill.tsx` | Add identity header to tooltip |

## Non-Goals

- Persisting refreshed tokens back to Keychain (TODO already exists in oauth.rs)
- Showing identity outside the tooltip (AuthPill badge already works via Keychain)
- Supporting non-macOS Keychain (Linux Secret Service, Windows Credential Manager) — deferred to v2.1+
