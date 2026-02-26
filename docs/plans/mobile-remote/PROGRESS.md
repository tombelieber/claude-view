# Mobile Remote — Epic Progress

**Epic:** Zero-setup mobile remote monitoring and control for claude-view
**Branch:** `worktree-monorepo-expo`
**Status:** M1 impl plan audited, 22 gaps fixed — ready to execute
**Last updated:** 2026-02-27

---

## Execution Order

| Order | Plan | Tasks | Status |
|-------|------|-------|--------|
| **1** | [Monorepo Restructure](../2026-02-25-monorepo-restructure-impl.md) | 12 | **DONE** |
| **2** | [clawmini Mobile M1](./2026-02-25-clawmini-mobile-m1-impl.md) | 10 | **Ready to execute** (audited, 22 gaps fixed in plan) |

Monorepo restructure is complete. M1 impl plan starts at Task 1 (all 10 tasks, 4 phases). Package names use `@claude-view/*`.

---

## Milestones

### M1: "It connects" — Scan QR → see live sessions

| Phase | Plan | Status | Summary |
|-------|------|--------|---------|
| **1** | Monorepo restructure (separate plan) | **DONE** | Infra: `apps/web/`, workspaces, shared packages, Expo scaffold |
| **2** | M1 impl Tasks 1-3 | **READY** | Shared pkg, relay fixes (push_hint + pair_complete gap-fixed) |
| **3** | M1 impl Tasks 4-7 | **READY** | Expo app: pair screen, dashboard, bottom sheet |
| **4** | M1 impl Tasks 8-10 | **READY** | Push notifications, EAS build, docs |

### M2: "Remote control" — Phone sends commands, Mac executes

| Phase | Status | Summary |
|-------|--------|---------|
| **A** | NOT STARTED | Command protocol design + Mac command handler |
| **B** | NOT STARTED | Mobile UI for chat, approve/deny, spawn session |
| **C** | NOT STARTED | Push notifications via expo-notifications |

### M3: "Full parity" — Phone can do everything desktop can

| Phase | Status | Summary |
|-------|--------|---------|
| **A** | NOT STARTED | Search + analytics from phone |
| **B** | NOT STARTED | Multi-session management, full conversation history |

---

## Infrastructure

| Service | Domain | Host | Status |
|---------|--------|------|--------|
| Relay | `relay.claudeview.ai` | Fly.io | Deployed (as `claude-view-relay.fly.dev`, custom domain TODO) |
| Mobile App | App Store / Play Store | Expo | TODO |
| App Landing | `m.claudeview.ai` | Cloudflare | Redirect page |
| Auth | — | Supabase | TODO |
| DNS | `claudeview.ai` | Cloudflare | Owned |

## Known Blockers (must fix before activating relay)

Found during shippable audit (2026-02-25). All 3 are in dormant code paths gated behind `RELAY_URL` env var — no impact on shipped functionality.

| # | File | Issue | Status |
|---|------|-------|--------|
| 1 | `relay_client.rs` | `pair_complete` handler was a TODO stub | **FIXED** (2026-02-27) — decrypts blob, verifies X25519 key, stores via `add_paired_device()` |
| 2 | `routes/pairing.rs` | `let _ =` on relay registration POST | Open — propagate error, return 502/503 |
| 3 | `crypto.rs` | Identity key file written world-readable | Open — set `0o600` permissions after write |

**Audit gap fixes (2026-02-27):** 3 gaps found in impl plan and fixed in code before execution:

- `relay_client.rs`: Added `build_envelope()` helper with `push_hint`/`push_title` for NeedsYou sessions (push notifications would never have fired)
- `relay_client.rs`: Implemented full `pair_complete` handler — decrypts phone blob, verifies X25519 key match, stores `PairedDevice`
- `pairing.rs`: Added `x25519_pubkey` to `ClaimRequest`, forwarded in `pair_complete` message
- `crypto.rs`: Added `decrypt_from_device()` function (symmetric counterpart to `encrypt_for_device`)
- `ws.rs`: Added `push_hint`/`push_title` optional fields to `RelayEnvelope`
- Impl plan: Added `projectId` param to `getExpoPushTokenAsync` (Expo SDK 55 requires it)

**Additional hardening (warnings, not blockers):**
- Relay needs rate limiting on `/pair` and `/pair/claim` endpoints
- Relay needs input length validation on all string fields
- Relay needs WebSocket message size limits (`.max_message_size()`)
- Relay CORS is maximally permissive — restrict for production
- Pairing token logged at INFO level — truncate or omit
- TOCTOU race in `create_pair` — use `DashMap::entry()` for atomic check-and-insert
- 5 silent `let _ =` error swallows in relay_client.rs and backfill.rs — should log warnings
- `.env.example` has uncommented production relay URL — comment out for local dev

## Key Files

| File | What |
|------|------|
| `crates/relay/` | Relay server (Fly.io) |
| `crates/server/src/live/relay_client.rs` | Mac WSS client |
| `crates/server/src/crypto.rs` | NaCl + Keychain |
| `crates/server/src/routes/pairing.rs` | Desktop pairing API |
| `apps/mobile/` (after monorepo restructure) | Expo/React Native app |

## Reference Docs

| Doc | What |
|-----|------|
| [design.md](./design.md) | Zero-setup architecture, security model, command protocol |
| [2026-02-25-clawmini-mobile-m1-design.md](./2026-02-25-clawmini-mobile-m1-design.md) | M1 design: live dashboard, Expo native, keypair auth |
| [2026-02-25-clawmini-mobile-m1-impl.md](./2026-02-25-clawmini-mobile-m1-impl.md) | M1 impl: 12 tasks across 4 phases |
| [../2026-02-25-monorepo-restructure-design.md](../2026-02-25-monorepo-restructure-design.md) | Monorepo design |
| [../2026-02-25-monorepo-restructure-impl.md](../2026-02-25-monorepo-restructure-impl.md) | Monorepo impl: 12 tasks |
| [analysis-pairing-bugs.md](./analysis-pairing-bugs.md) | Original bug analysis (3 root causes) |
| [archived/](./archived/) | Earlier M1 design and impl plans (superseded) |
