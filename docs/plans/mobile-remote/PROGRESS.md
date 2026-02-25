# Mobile Remote — Epic Progress

**Epic:** Zero-setup mobile remote monitoring and control for claude-view
**Branch:** `worktree-mobile-remote`
**Status:** Deferred to next release cycle
**Last updated:** 2026-02-25

---

## Execution Order (Next Release Cycle)

These two plans must be executed **in this order**. Plan 2's Phase 1 overlaps with Plan 1 — skip it and start at Phase 2.

| Order | Plan | Tasks | Description |
|-------|------|-------|-------------|
| **1** | [Monorepo Restructure](../2026-02-25-monorepo-restructure-impl.md) | 12 | Move web SPA to `apps/web/`, Bun workspaces, Turborepo, scaffold `apps/mobile/`, `apps/landing/`, `packages/shared/`, `packages/design-tokens/` |
| **2** | [clawmini Mobile M1](./2026-02-25-clawmini-mobile-m1-impl.md) | 12 (skip Phase 1) | Relay bug fixes, Expo app (pair screen, dashboard, bottom sheet), deploy pipeline, push notifications |

**Why this order:**
- Monorepo restructure creates the directory layout, workspace config, and shared packages that the mobile app depends on
- M1 Phase 1 (Tasks 1-4) is a subset of the monorepo plan — after executing Plan 1, start M1 at **Phase 2, Task 5** (relay bug fixes)
- Adjust M1 package names from `@clawmini/*` to `@claude-view/*` to match monorepo plan conventions

---

## Milestones

### M1: "It connects" — Scan QR → see live sessions

| Phase | Plan | Status | Summary |
|-------|------|--------|---------|
| **1** | Monorepo restructure (separate plan) | DEFERRED | Infra: `apps/web/`, workspaces, shared packages, Expo scaffold |
| **2** | M1 impl Tasks 5 | DEFERRED | Fix 3 relay pairing bugs |
| **3** | M1 impl Tasks 6-9 | DEFERRED | Expo app: scaffold, pair screen, dashboard, bottom sheet |
| **4** | M1 impl Tasks 10-12 | DEFERRED | EAS build, push notifications, docs |

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

## Key Files

| File | What |
|------|------|
| `crates/relay/` | Relay server (Fly.io) |
| `crates/server/src/live/relay_client.rs` | Mac WSS client |
| `crates/server/src/crypto.rs` | NaCl + Keychain |
| `crates/server/src/routes/pairing.rs` | Desktop pairing API |
| `apps/mobile/` (after monorepo restructure) | Expo/React Native app |
| `src/components/PairingPanel.tsx` | Desktop QR popover |

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
