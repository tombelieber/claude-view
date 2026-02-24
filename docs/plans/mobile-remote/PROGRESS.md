# Mobile Remote — Epic Progress

**Epic:** Zero-setup mobile remote monitoring and control for claude-view
**Branch:** `worktree-mobile-remote`
**Design:** [design.md](./design.md)

## Milestones

### M1: "It connects" — Scan QR → sign in → see sessions (current)

| Phase | Plan | Status | Summary |
|-------|------|--------|---------|
| **A** | [m1-phase-a-bug-fixes.md](./m1-phase-a-bug-fixes.md) | TODO | Fix 3 pairing bugs, redeploy relay, local E2E test |
| **B** | [m1-phase-b-auth-deploy.md](./m1-phase-b-auth-deploy.md) | TODO | Supabase auth, JWT validation, Expo app build + TestFlight, custom domains |

**Phase A tasks:**
- [ ] Task 1: Add `x25519_pubkey` to relay ClaimRequest
- [ ] Task 2: Send `x25519_pubkey` from phone side
- [ ] Task 3: Always connect relay_client (fix chicken-and-egg)
- [ ] Task 4: Implement `pair_complete` handler
- [ ] Task 5: Redeploy relay to Fly.io
- [ ] Task 6: Local E2E test

**Phase B tasks:**
- [ ] Task 7: Set up Supabase project (manual)
- [ ] Task 8: Add Supabase auth gate to mobile pages
- [ ] Task 9: Add JWT validation to relay
- [ ] Task 10: Configure custom domains (manual DNS)
- [ ] Task 11: Build Expo app + submit to TestFlight
- [ ] Task 12: Update QR URL + bake default RELAY_URL
- [ ] Task 13: Final redeploy + E2E test

### M2: "Remote control" — Phone sends commands, Mac executes

| Phase | Plan | Status | Summary |
|-------|------|--------|---------|
| **A** | — | NOT STARTED | Command protocol design + Mac command handler |
| **B** | — | NOT STARTED | Mobile UI for chat, approve/deny, spawn session |
| **C** | — | NOT STARTED | Push notifications via expo-notifications |

### M3: "Full parity" — Phone can do everything desktop can

| Phase | Plan | Status | Summary |
|-------|------|--------|---------|
| **A** | — | NOT STARTED | Search + analytics from phone |
| **B** | — | NOT STARTED | Multi-session management, full conversation history |

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
| `packages/mobile/` (or standalone repo) | Expo/React Native app (TBD) |
| `src/components/PairingPanel.tsx` | Desktop QR popover |

## Reference Docs

| Doc | What |
|-----|------|
| [design.md](./design.md) | Zero-setup architecture, security model, command protocol |
| [analysis-pairing-bugs.md](./analysis-pairing-bugs.md) | Original bug analysis (3 root causes) |
| [archived/](./archived/) | Earlier M1 design and impl plans (superseded) |
