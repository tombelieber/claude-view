---
status: approved
date: 2026-02-25
type: design
supersedes: m1-phase-a-bug-fixes.md, m1-phase-b-auth-deploy.md
---

# clawmini Mobile — M1 Design: Live Dashboard

> **One line:** Scan QR on Mac → see all active AI agent sessions on your phone. Native, encrypted, real-time.

## Context

clawmini mobile is the phone surface of the clawmini product — an agentic engineering command center. It is NOT a remote viewer for Claude Code sessions. The architecture, screens, and data model are designed from day 0 as the foundation for autonomous agent dispatch, approval workflows, and plan execution from your phone.

M1 ships the first shippable milestone: a read-only live dashboard. M1.5 adds approve/deny (the first thing Tailscale can't do). M2+ adds the plan runner and multi-agent dispatch that justify charging.

### Why Not PWA

Zero successful PWA-only mobile products exist. Every "PWA success story" (Twitter, Starbucks, Pinterest, Uber) maintains native apps as primary. iOS PWA has broken background sync, unreliable push notifications, and storage eviction after 7 days. All competing dev tools (Happy Coder, Replit, Vercel v0) chose Expo/React Native.

### Reference Implementation

Happy Coder (`github.com/slopus/happy`) — 5-package monorepo, Expo app, relay server, E2E encryption, keypair auth. Analyzed in detail as architecture reference.

## M1 Scope

### Ships

- Scan QR from Mac → paired (keypair auth, zero accounts, zero PII)
- See all active sessions grouped by agent state (needs you / autonomous)
- Per-session: project, status, cost, context %, model, sub-agents, progress
- Push notifications when agent state changes
- "Mac offline" state when relay disconnects

### Explicitly Deferred

| Feature | When | Why |
|---------|------|-----|
| Approve/deny tool calls | M1.5 | Requires bidirectional command channel |
| Plan runner / multi-agent dispatch | M2+ | Core clawmini value, needs thick server |
| Billing / RevenueCat | When agentic features exist | A remote viewer isn't worth charging for |
| Web login / Supabase | When team features exist | Keypair auth is sufficient for individual |
| Voice input | M3+ | Defer indefinitely |
| Artifacts system | M3+ | Encrypted blob storage, not M1 |
| Social graph | Never | No need |
| Conversation view with syntax highlighting | M2 | Pre-tokenize on Mac, render native spans |

## Architecture

### Data Flow

```
Mac (source of truth)
  └─ LiveSessionManager (in-memory state)
     └─ relay_client.rs (WSS outbound)
        └─ Relay (Fly.io, dumb pipe, no storage)
           └─ Phone (Expo app, WSS inbound)
              └─ Decrypt → render
```

- Mac must be online for phone to see anything. Mac offline = no sessions running = nothing to show.
- Relay is stateless. No caching. No database. Forwards encrypted blobs.
- Phone shows "Mac offline" when relay disconnects — that's correct, not a bug.

### Auth Model: Keypair (No Supabase)

Copied from Happy Coder's proven pattern. Identity = cryptographic keypair.

| Step | What happens |
|------|-------------|
| 1. Mac generates QR | Ed25519 signing key + X25519 encryption key, stored in macOS Keychain |
| 2. Phone scans QR | Extracts Mac's X25519 pubkey + one-time token |
| 3. Phone generates keypair | Ed25519 + X25519, stored in Expo SecureStore (Keychain-backed) |
| 4. Phone claims pairing | POST `/pair/claim` with encrypted phone pubkey |
| 5. Relay forwards to Mac | Mac stores phone pubkey in Keychain |
| 6. Both sides authenticated | Ed25519 signature on every WS connection (60s freshness) |

No email. No password. No account. No Supabase. No third-party dependency.

Bot defense: pairing requires physical QR scan from a running Mac. Stronger gate than email verification. IP rate limiting on relay endpoints. Add account layer later when paid features exist.

### Encryption

- **Key exchange:** X25519 (Curve25519 Diffie-Hellman)
- **Message encryption:** NaCl secretbox (XSalsa20-Poly1305)
- **Auth signatures:** Ed25519 (60s freshness window)
- **Phone key storage:** Expo SecureStore (iOS Keychain, Android Keystore)
- **Mac key storage:** macOS Keychain (`com.claude-view`)
- **Relay sees:** Only encrypted blobs. Zero-knowledge.

### Wire Protocol

Rust structs are the single source of truth. `ts-rs` (already in `Cargo.toml`) generates TypeScript types.

Add `#[derive(TS)]` to: `LiveSession`, `SessionEvent`, `AgentState`, `CostBreakdown`, `TokenUsage`, `SubAgentInfo`, `ProgressItem`, `ToolUsed`.

`cargo test` generates `.ts` files → `packages/shared/types/generated/`. Both web and mobile import from here. No manual type duplication, no drift.

## Monorepo Restructure

Full restructure from flat layout to `apps/` + `packages/` structure.

### New Structure

```
claude-view/
├── crates/                          # Rust workspace (UNCHANGED)
│   ├── core/                        # Shared types, JSONL parser
│   ├── db/                          # SQLite via sqlx
│   ├── search/                      # Tantivy full-text indexer
│   ├── server/                      # Axum HTTP routes
│   └── relay/                       # Fly.io relay server
│
├── apps/
│   ├── web/                         # Existing Vite React SPA (moved from root)
│   │   ├── src/
│   │   ├── public/
│   │   ├── index.html
│   │   ├── package.json
│   │   ├── vite.config.ts
│   │   ├── vitest.config.ts
│   │   ├── tailwind.config.ts
│   │   └── tsconfig.json
│   │
│   ├── mobile/                      # NEW: Expo/React Native app
│   │   ├── app/                     # Expo Router file-based routes
│   │   ├── components/              # React Native components
│   │   ├── hooks/                   # App-specific hooks
│   │   ├── lib/                     # App-specific utilities
│   │   ├── app.config.ts            # Expo config (3 build variants)
│   │   ├── package.json
│   │   ├── metro.config.js
│   │   ├── tailwind.config.ts       # NativeWind config
│   │   └── tsconfig.json
│   │
│   └── landing/                     # NEW: Static landing page
│       ├── index.html               # App Store badges, hero, screenshots
│       ├── .well-known/             # apple-app-site-association
│       └── _redirects               # Cloudflare Pages config
│
├── packages/
│   └── shared/                      # Shared TS business logic
│       ├── src/
│       │   ├── types/               # ts-rs generated types + manual types
│       │   ├── crypto/              # tweetnacl encrypt/decrypt
│       │   ├── relay/               # WS client protocol, useMobileRelay
│       │   └── utils/               # formatCost, groupSessions, time formatting
│       ├── package.json
│       └── tsconfig.json
│
├── Cargo.toml                       # Rust workspace root
├── turbo.json                       # Turborepo task config
├── package.json                     # Bun workspace root
├── bun.lock
├── package-lock.json                # For npx distribution
├── tsconfig.base.json               # Shared TS config
└── CLAUDE.md
```

### What Moves

| Item | From | To |
|------|------|----|
| `src/` | root | `apps/web/src/` |
| `public/` | root | `apps/web/public/` |
| `index.html` | root | `apps/web/index.html` |
| `vite.config.ts` | root | `apps/web/vite.config.ts` |
| `vitest.config.ts` | root | `apps/web/vitest.config.ts` |
| `tailwind.config.ts` | root | `apps/web/tailwind.config.ts` |
| `tsconfig*.json` | root | `apps/web/` (app-specific) + root (base) |
| `e2e/`, `tests/` | root | `apps/web/e2e/`, `apps/web/tests/` |
| Relay WS logic from `src/hooks/` | `apps/web/` | `packages/shared/relay/` |
| Crypto utils from `src/` | `apps/web/` | `packages/shared/crypto/` |

### What Stays

| Item | Why |
|------|-----|
| `crates/` | Rust workspace, orthogonal to JS |
| `npx-cli/` | npm distribution wrapper |
| `scripts/` | Build/release scripts |
| `docs/` | Documentation |
| `supabase/` | Deferred, stays at root |

### Tooling Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Package manager | **Bun** (stays) | CLAUDE.md says Bun for dev. Don't add a third PM. |
| Monorepo orchestration | **Turborepo** | Industry standard, works with Bun (`bunx turbo`) |
| No pnpm | — | Project already has bun.lock + package-lock.json. Don't add a third. |
| No design-tokens package | — | Both apps use Tailwind/NativeWind. Share palette via shared/theme.ts if needed. |
| Landing page | **Static HTML** | Not Astro. Single page + well-known files. Add framework when marketing grows. |

## Expo App Design

### Build Variants (from Happy)

| Variant | Bundle ID | App Name | Deep Links |
|---------|-----------|----------|------------|
| development | `com.clawmini.dev` | clawmini (dev) | None |
| preview | `com.clawmini.preview` | clawmini (preview) | None |
| production | `com.clawmini.app` | clawmini | `https://m.claudeview.ai/*` |

Deep links only on production to avoid dev/preview builds intercepting prod links.

### Dependencies

| Package | Purpose |
|---------|---------|
| `expo-router` | File-based navigation |
| `expo-camera` | QR scanning |
| `expo-secure-store` | Keypair storage (Keychain-backed) |
| `expo-notifications` | Push alerts |
| `expo-haptics` | Tactile feedback on scan, pull-to-refresh |
| `nativewind` | Tailwind CSS for React Native |
| `tweetnacl` | NaCl crypto (matches Rust side) |
| `react-native-reanimated` | Bottom sheet, transitions |
| `@gorhom/bottom-sheet` | Session detail sheet |
| `@storybook/react-native` | Component isolation (optional, wire up on setup) |

### Rendering Strategy

| Content | Approach | Why |
|---------|----------|-----|
| Dashboard, cards, status | **Native + NativeWind** | Simple UI, must feel native |
| Conversation view (M2+) | **Native + pre-tokenized from Mac** | Mac runs Shiki, sends colored tokens via relay, phone renders `<Text>` spans. Shiki quality, native performance. |
| Mermaid diagrams (M3+) | **DOM component (`'use dom'`)** | Only justified WebView case — requires JS execution for SVG |

Happy Coder validates this: they built a custom native syntax highlighter (regex tokenizer → `<Text>` components). No WebView for code. Expo DOM components are an escape hatch, not a foundation.

## Visual Design

### Design Tokens

| Token | Value | Usage |
|-------|-------|-------|
| `bg-base` | `#0F172A` (slate-900) | App background |
| `bg-surface` | `#1E293B` (slate-800) | Card backgrounds |
| `bg-border` | `#334155` (slate-700) | Borders, dividers |
| `text-primary` | `#F8FAFC` (slate-50) | Primary text |
| `text-muted` | `#94A3B8` (slate-400) | Secondary text |
| `status-green` | `#22C55E` | Autonomous / success |
| `status-amber` | `#F59E0B` | Needs attention |
| `status-red` | `#EF4444` | Error / stuck |
| `accent` | `#6366F1` (indigo) | Brand / AI accent |
| `font-mono` | Fira Code | Data, costs, code |
| `font-sans` | Fira Sans | UI labels, body |

### Screen 1: Pair (first-time only)

```
┌─────────────────────────────┐
│                         [×] │
│                              │
│      ┌────────────────┐     │
│      │                │     │
│      │   [ Camera ]   │     │
│      │   viewfinder   │     │
│      │                │     │
│      └────────────────┘     │
│                              │
│   Scan the QR code from     │
│   your Mac's claude-view    │
│                              │
│   One scan. No account.     │
│   No password. Ever.        │
│                              │
└─────────────────────────────┘
```

- Full-screen camera with rounded viewfinder cutout
- Subtle pulse animation on viewfinder border (indigo glow, `150-300ms`)
- On successful scan: haptic feedback + viewfinder turns green + auto-navigate
- No forms, no onboarding, no tutorial

### Screen 2: Dashboard (main screen)

```
┌─────────────────────────────┐
│  clawmini        ● Connected│
│─────────────────────────────│
│                              │
│  ┌─ NEEDS YOU ─────────────┐│
│  │  ┌────────────────────┐ ││
│  │  │ auth-service        │ ││
│  │  │ ⏳ Awaiting input   │ ││
│  │  │ $0.31  ████████░ 78%│ ││
│  │  └────────────────────┘ ││
│  └──────────────────────────┘│
│                              │
│  ┌─ AUTONOMOUS ────────────┐│
│  │  ┌────────────────────┐ ││
│  │  │ api-tests           │ ││
│  │  │ ⚡ Writing tests     │ ││
│  │  │ $0.09  ████░░░░ 42% │ ││
│  │  └────────────────────┘ ││
│  │  ┌────────────────────┐ ││
│  │  │ db-migration        │ ││
│  │  │ ⚡ Editing files     │ ││
│  │  │ $0.18  ██████░░ 61% │ ││
│  │  └────────────────────┘ ││
│  └──────────────────────────┘│
│                              │
│─────────────────────────────│
│  1 needs you · 2 auto · $0.58│
└─────────────────────────────┘
```

- Cards grouped by agent state: "Needs You" (amber left accent) at top, "Autonomous" (green) below
- Each card: project name, agent state icon + label, cost (mono font), context % bar
- Summary bar pinned at bottom: glanceable totals
- Connection indicator top-right: green dot "Connected" / red dot "Mac offline"
- Pull-to-refresh with haptic feedback
- Empty state: "No active sessions" with subtle breathing animation on clawmini logo
- Mac offline: cards grey out, summary shows "Mac offline" in red
- Tap card → opens session detail (bottom sheet)

### Screen 3: Session Detail (bottom sheet)

```
┌─────────────────────────────┐
│  (dimmed dashboard behind)  │
├─────────────────────────────┤
│  ─── (drag handle)          │
│                              │
│  auth-service                │
│  ~/dev/myapp                 │
│  branch: feat/auth           │
│                              │
│  Status    Awaiting input    │
│  Model     Sonnet 4.6        │
│  Turns     14                │
│  Time      12m 34s           │
│                              │
│  ── Cost ──────────────────  │
│  Input     $0.22             │
│  Output    $0.09             │
│  Total     $0.31             │
│                              │
│  ── Context ───────────────  │
│  ██████████████████░░░  78%  │
│  156k / 200k tokens          │
│                              │
│  ── Activity ──────────────  │
│  "Implement JWT middleware"  │
│                              │
│  ── Sub-agents (2) ────────  │
│  ⚡ test-writer   writing..  │
│  ✓  schema-gen    done       │
│                              │
│  ── Progress ──────────────  │
│  ✓ Create auth middleware    │
│  ✓ Add JWT validation        │
│  ○ Write integration tests   │
│  ○ Update API docs           │
│                              │
│  ┌─────────────────────────┐│
│  │  🔒 Approve / Deny       ││
│  │     coming in M1.5       ││
│  └─────────────────────────┘│
└─────────────────────────────┘
```

- Bottom sheet via `@gorhom/bottom-sheet` — swipe up to expand, down to dismiss
- Half-height default, full-screen on drag up
- All data from existing `LiveSession` struct — no new API calls
- M1.5 teaser: approve/deny area visible but locked (subtle, not annoying)
- Sub-agent list and progress items from existing `LiveSession.subAgents` and `LiveSession.progressItems`

## Push Notifications

| Trigger | Notification | Timing |
|---------|-------------|--------|
| Agent state → `needs_you` | "[project] needs your input" | Immediate |
| Agent state → error/stuck | "[project] encountered an error" | Immediate |
| All agents complete | "All N sessions complete — $X.XX total" | 30s debounce |
| Mac goes offline | "Mac disconnected" | After 60s of no heartbeat |

Tap notification → opens app → navigates to that session's detail sheet.

Implementation: `expo-notifications` + server-side Expo Push API from relay (when Mac sends state change, relay also fires push to registered phone token).

## Relay Server Changes (Minimal)

The existing relay at `crates/relay/` needs 3 bug fixes + 1 new feature:

| Change | What |
|--------|------|
| Fix: `x25519_pubkey` in ClaimRequest | Relay must forward phone's encryption pubkey to Mac |
| Fix: `pair_complete` handler on Mac | Mac must process incoming phone pubkey from relay |
| Fix: relay_client always connects | Remove chicken-and-egg (connect on startup, not only when paired) |
| New: Push token registration | `POST /push-tokens` endpoint, store per device, forward via Expo Push API |

No protocol changes. No database. No caching. Same dumb pipe.

## Landing Page (`m.claudeview.ai`)

Static HTML deployed to Cloudflare Pages:

- App Store / Play Store badges with download links
- Hero section: "Your AI agents, in your pocket"
- Screenshot of dashboard
- `.well-known/apple-app-site-association` for universal links
- QR deep link handler: `claude-view://pair?k=...&t=...` redirects to App Store if app not installed

Not a framework. Not Astro. Single `index.html` + well-known files. Add framework when marketing requires it.

## Code Sharing Strategy

```
packages/shared/              ← Reused by BOTH apps
├── types/generated/          ← ts-rs output (LiveSession, etc.)
├── crypto/                   ← tweetnacl encrypt/decrypt, key management
├── relay/                    ← useMobileRelay hook, WS protocol
└── utils/                    ← formatCost, groupSessions, formatDuration

apps/web/src/                 ← Web UI (existing, uses <div>)
                                 Imports from @clawmini/shared

apps/mobile/components/       ← Native UI (new, uses <View>)
                                 Imports from @clawmini/shared
                                 THIN — just rendering. All logic is shared.
```

## Competitive Context

| Product | Mobile story | Our advantage |
|---------|-------------|---------------|
| Happy Coder | Expo app, full relay, E2E encrypted | Same architecture. We add Mission Control analytics. |
| Replit | #1 on App Store, full IDE | We're agent-focused, not IDE. Different product. |
| Cursor | Desktop only, no mobile | We own mobile. |
| Claude Code + Tailscale | DIY remote access | Commodity. We add UX, push notifications, agent grouping. |
| Kiro | Web IDE, no mobile | We're the mobile command center for their users. |

## Success Criteria

M1 is done when:
1. Scan QR on Mac → phone shows all active sessions within 2 seconds
2. Session state changes on Mac → phone updates within 1 second
3. Push notification fires when agent state → needs_you
4. "Mac offline" shows correctly when Mac sleeps
5. App is on TestFlight (iOS) and internal testing (Android)

## What Comes After M1

| Milestone | What | Trigger to start |
|-----------|------|-----------------|
| **M1.5** | Approve/deny from phone. Bidirectional command channel. | M1 shipped + daily usage |
| **M2** | Conversation view (pre-tokenized Shiki). Full session history. | M1.5 validated |
| **M2.5** | Plan runner from phone (dispatch, monitor, steer). | Desktop plan runner working |
| **M3** | RevenueCat billing. Thick server. Multi-Mac. | Enough agentic value to charge |

## Key Decisions Summary

| Decision | Choice | Reference |
|----------|--------|-----------|
| Mobile framework | Expo/React Native (not PWA) | All competitors chose native |
| Auth | Keypair (not Supabase) | Happy Coder proves it works |
| Relay | Dumb pipe (not thick server) | Mac is source of truth for M1 |
| Monorepo | Full restructure (`apps/` + `packages/`) | Industry standard |
| Package manager | Bun (not pnpm) | Existing decision in CLAUDE.md |
| Rendering | Native + NativeWind (not DOM components) | Expo recommends native-first |
| Code highlighting (M2) | Pre-tokenize on Mac, render native spans | Shiki quality + native performance |
| Styling | NativeWind (Tailwind for RN) | Same classes as web |
| Type sync | ts-rs (Rust → TS auto-generation) | Already in Cargo.toml |
| Storybook | Wire up on setup, don't block M1 | Storybook 9 + Expo works |
