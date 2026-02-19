---
status: approved
date: 2026-02-12
purpose: GTM pivot — Remote Session Monitoring via PWA as primary product wedge
---

# Mobile PWA: Remote Session Monitoring

> **Goal:** Let users monitor and control their Claude Code sessions from their phone. This replaces the AI Fluency Score as the primary product wedge. "Monitor your Claude Code sessions from your phone" is more visceral than "measure your AI fluency."

---

## 1. Problem Statement

### Who

Senior engineers on Claude Code Max ($200/mo) who run long-running sessions and step away from their desk. They want ROI on every token and need visibility when sessions stall.

### What hurts

1. **No remote visibility.** Claude Code sessions are tied to the terminal where they started. Walk away from your laptop and you are blind. There is no way to check session status from your phone.

2. **No push notifications.** When a session enters `WAITING_FOR_USER` state, there is no alert. The session sits idle burning time until the user happens to check. For users paying $200/mo, idle time is wasted money.

3. **Existing tools require LAN access.** The current Mission Control design assumes `localhost:47892` with optional user-provided tunnels (Tailscale, Cloudflare). This works for home-office setups but fails for coffee shops, travel, or when the user forgets to configure a tunnel.

### Why this is the product wedge

Everything else -- fluency score, analytics, pattern discovery -- is nice-to-have. Remote monitoring is a **pain point** that senior Claude Code users feel multiple times per day. It is the hook that gets them to install the tool. Once installed, the analytics and coaching features drive retention.

---

## 2. Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| First product | Remote session monitoring (PWA) | Biggest pain point: can't see Claude Code sessions from mobile |
| Repo structure | Monorepo (same repo as claude-view) | Solo dev velocity, shared types, one PR = one feature |
| Relay server | Rust (Axum), new `crates/relay/` crate | Zero type duplication, one build, existing Axum knowledge |
| Hosting | Fly.io | Predictable pricing for long-lived WebSockets. Rust handles 50K+ connections per VM. ~$2-7/mo to start. CF Workers charges per-duration on Durable Objects which gets expensive for long-lived WS |
| Mobile | PWA first, native later | iOS 16.4+ supports PWA push notifications. Zero App Store friction |
| Push notifications | OneSignal (trigger only, no session content in payload) | Free tier 10K subscribers. Phone fetches encrypted data from relay after receiving push |
| Security | E2E NaCl box (X25519 + XSalsa20-Poly1305), zero-knowledge relay, QR code pairing | Relay cannot decrypt. Even compromised Fly.io sees only opaque blobs |
| Local daemon | macOS `launchd` UserAgent, always-on, outbound WSS only, Keychain for keys | No root, no inbound ports, auto-restart on crash |
| Relay connection | Always connected, heartbeat 30s, auto-reconnect with exponential backoff (1s to 30s max) | Simpler than auto-connect/disconnect. One idle WS is ~50KB memory, trivial |
| Target user | Senior engineers on Claude Code Max ($200/mo) | Natural upsell to API tier when $200/mo ceiling isn't enough for 24/7 agents |
| PWA tech stack | Same React SPA, mobile-first responsive views | Shares 90% of stack with existing app (React, Vite, Tailwind, Radix, Lucide, React Router). Add BottomNav + mobile layout (pattern from project-a). No separate framework. |
| Daemon UX | Silent launchd registration on first `npx claude-view` run + toast notification | "Every prompt = friction." No install step, no permission dialog. Server just never stops. Settings toggle for the 1% who care. |
| QR pairing UX | QR code always visible on dashboard (WhatsApp Web model). Scan = paired. No confirmation prompt. | Zero decisions for user. One-time token expires after scan or 5 min. New QR on each page load. Paired devices list for revocation. |
| Repo structure | 2 repos: `claude-view` (all code, open source) + `marketing-site` (marketing, private) | Engine UI is a tab in the same dashboard, not a separate app. Engine backend is `crates/engine/` in same repo. |
| Open source strategy | Everything open source. Cloud is the paid product. | GitLab/Supabase/Vercel model. Moat is infrastructure (API credits, orchestration), not code. Open source = viral distribution. |
| Monetization | Free: self-host everything. Paid: hosted relay + engine cloud (API tier for 24/7 agents) | Users who can self-host were never going to pay. Paying users want "scan QR and it works." |

---

## 3. Solution Overview

- **PWA** accessible from any mobile browser (Safari, Chrome). Add to home screen for native-feel experience.
- **E2E encrypted relay** on Fly.io. The relay is zero-knowledge -- it forwards opaque blobs between paired devices.
- **QR code pairing** from the claude-view desktop UI. No accounts, no passwords, no email signup.
- **Push notifications** via OneSignal. The push payload contains only a trigger signal ("session needs attention"), never session content. The phone fetches encrypted data from the relay after receiving the push.

---

## 4. Architecture

### Data flow

```
Claude Code writes JSONL
    |
    v
 launchd daemon (vibe-recall, always running)
    |
    v  notify crate detects file change
Mission Control MONITOR layer (existing Phase A)
    |
    v  broadcast channel
Relay WebSocket client (NEW, in crates/relay/)
    |
    v  NaCl box encrypted
Fly.io relay (zero-knowledge, forwards blobs)
    |
    v  WSS
PWA on phone (decrypts with device key)
    + OneSignal push (trigger only, no content)
```

### Crate layout (addition to existing workspace)

```
crates/
├── core/         # Shared types, JSONL parser (existing)
├── db/           # SQLite (existing)
├── search/       # Tantivy (existing)
├── server/       # Axum HTTP routes (existing, adds relay WS client)
└── relay/        # NEW — Fly.io relay server binary
    ├── src/
    │   ├── main.rs       # Axum server entrypoint
    │   ├── ws.rs         # WebSocket handler
    │   ├── pairing.rs    # Device pairing registry (in-memory)
    │   ├── auth.rs       # Ed25519 challenge verification
    │   └── rate_limit.rs # Per-device rate limiting
    └── Cargo.toml
```

The relay is a separate binary (`cargo build -p vibe-recall-relay`) deployed to Fly.io. The local daemon runs the existing `vibe-recall` binary with a new relay client module that connects outbound to the relay.

---

## 5. Security Architecture (5 Layers)

| Layer | What | How |
|-------|------|-----|
| 1. Transport | TLS everywhere | Mac <--WSS--> Fly.io <--WSS--> Phone |
| 2. Payload encryption | NaCl box (X25519 + XSalsa20-Poly1305) | Mac encrypts with phone's pubkey. Phone decrypts with own privkey. Relay sees only opaque ciphertext. |
| 3. Device authentication | Ed25519 signed challenges | Each connection sends `sign(timestamp + device_id)`. Relay verifies against registered pubkey. No passwords. |
| 4. Push isolation | Trigger only, no content | OneSignal push says "check your sessions." Phone fetches encrypted data from relay via WSS. A compromised push provider learns nothing. |
| 5. Local security | Keychain + no inbound ports | Private keys stored in macOS Keychain, not filesystem. Daemon makes outbound WSS connections only. No listening ports, no firewall rules needed. |

### Threat model

| Threat | Mitigation |
|--------|-----------|
| Fly.io compromised | Layer 2: relay only sees ciphertext. Attacker gets encrypted blobs with no keys. |
| OneSignal compromised | Layer 4: push contains no session data. Attacker can trigger spurious "check sessions" alerts but learns nothing. |
| MITM on network | Layer 1: TLS certificate pinning in WSS. Standard browser/OS cert validation. |
| Stolen phone | Phone keypair can be revoked from desktop UI. Re-pair generates new keys. |
| Daemon crash | launchd auto-restarts. Relay sees device go offline, queues messages (bounded buffer, 100 messages max, 5 min TTL). |

---

## 6. QR Pairing Flow

```
┌──────────────────────┐         ┌──────────────────┐         ┌───────────────────┐
│     Mac (Desktop)     │         │   Fly.io Relay    │         │   Phone (PWA)     │
└──────────┬───────────┘         └────────┬─────────┘         └─────────┬─────────┘
           │                              │                             │
           │  1. Generate X25519 keypair  │                             │
           │     Store privkey in Keychain│                             │
           │                              │                             │
           │  2. Display QR code:         │                             │
           │     { pubkey, pairing_token, │                             │
           │       relay_url }            │                             │
           │                              │                             │
           │                              │         3. Scan QR code     │
           │                              │  ◄──────────────────────────│
           │                              │         Phone stores Mac's  │
           │                              │         pubkey, generates   │
           │                              │         own X25519 keypair  │
           │                              │                             │
           │                              │  4. Phone sends own pubkey  │
           │                              │     to Mac via relay        │
           │    5. Receive phone pubkey   │     (encrypted with Mac's   │
           │◄─────────────────────────────│      pubkey)                │
           │                              │                             │
           │  6. Pairing complete         │                             │
           │     Both devices have each   │                             │
           │     other's public keys      │                             │
           │                              │                             │
```

**Steps:**

1. Mac generates an X25519 keypair. Private key goes into macOS Keychain. Public key is embedded in the QR code.
2. Mac displays a QR code in the claude-view desktop UI containing: `{ device_pubkey, one_time_pairing_token, relay_url }`.
3. Phone scans the QR code via the PWA camera. Stores Mac's public key in IndexedDB. Generates its own X25519 keypair.
4. Phone sends its public key to the relay, encrypted with Mac's public key, along with the one-time pairing token.
5. Relay forwards the encrypted blob to Mac. Mac decrypts to obtain the phone's public key.
6. Pairing complete. Both devices can now encrypt/decrypt messages for each other. The one-time pairing token is invalidated.

**Re-pairing:** Desktop UI has a "Remove device" button. Revokes the phone's public key. Phone must scan a new QR code to re-pair.

> **For detailed pairing protocol (QR payload format, security properties, `/pair` and `/pair/claim` endpoints), see [`docs/plans/2026-02-17-flyio-relay-design.md`](2026-02-17-flyio-relay-design.md#3-pairing-protocol).**

---

## 7. PWA Mobile Phases

| Phase | Name | Scope | Timeline | Depends on |
|-------|------|-------|----------|------------|
| M1 | Status Monitor | Session list with status dots (active/waiting/idle), project name, cost. Push notifications for `WAITING_FOR_USER`. Tap to expand detail. | ~1 week | Mission Control Phase A + relay + daemon + QR pairing |
| M2 | Read-Only Dashboard | Full session cards with context gauge, cost tooltip, summary bar. Pull-to-refresh. Offline-capable (cached last state). | ~1-2 weeks | M1 |
| M3 | Interactive Control | Reply to waiting sessions from phone. Resume pre-flight (cost estimate, cache status). Inline chat. | ~2-3 weeks | M2 + Mission Control Phase F (Agent SDK sidecar) |

### Architectural rule

Build the relay protocol for M3 from day 1. The WebSocket protocol is bidirectional from the start, even though M1 and M2 only use the server-to-phone direction. M1/M2 are UI gates on a full-capability protocol, not protocol limitations.

### M1 wireframe (Status Monitor)

```
┌────────────────────────────────┐
│  Claude Sessions          ● 3  │  ← header + active count
├────────────────────────────────┤
│ 🟢  claude-view refactor       │  ← green = active
│     costs: $1.24  ctx: 42%     │
├────────────────────────────────┤
│ 🟡  api-redesign               │  ← amber = WAITING_FOR_USER
│     costs: $0.87  ctx: 68%     │
│     ⚠ Waiting for input        │
├────────────────────────────────┤
│ 🔵  docs update                │  ← blue = idle (no activity 5min+)
│     costs: $0.12  ctx: 15%     │
└────────────────────────────────┘
```

---

## 8. Relay Server Design (crates/relay/)

The relay is a minimal Rust Axum binary deployed to Fly.io. Its sole purpose is to forward encrypted blobs between paired devices. It stores no session data, logs no message content, and cannot decrypt payloads.

**For detailed relay server design (endpoints, constraints, deployment config, cost estimates), see [`docs/plans/2026-02-17-flyio-relay-design.md`](2026-02-17-flyio-relay-design.md).**

---

## 9. Daemon Design

### macOS launchd plist

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "...">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.vibe-recall.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/vibe-recall</string>
        <string>--daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/vibe-recall-daemon.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/vibe-recall-daemon.err</string>
</dict>
</plist>
```

**Location:** `~/Library/LaunchAgents/com.vibe-recall.daemon.plist` (UserAgent, no root required).

### Installation UX (invisible daemon)

There is no "install daemon" step. The user never learns the word "daemon."

1. First `npx claude-view` (npm wrapper) → starts `vibe-recall` binary, silently writes plist, runs `launchctl load`, opens browser
2. Toast notification: "claude-view is running at localhost:47892"
3. Every subsequent `npx claude-view` → server already running, just opens browser
4. Settings page has "Run in background" toggle (default: ON)
5. `claude-view stop` CLI command (npm wrapper) for power users

The server IS the daemon. Same binary, always running. No mode switch, no separate process.

### Daemon responsibilities

| Responsibility | Detail |
|---------------|--------|
| JSONL watching | Existing Mission Control Phase A `notify` watcher |
| Session state machine | Existing Phase A state machine (active/waiting/idle/done) |
| Cost calculation | Existing Phase A token-to-cost calculator |
| Relay WS client | NEW -- maintains outbound WSS to Fly.io relay |
| Encryption | NEW -- NaCl box encrypt all outbound messages with phone's pubkey |
| Keychain access | NEW -- read/write X25519 privkey + phone pubkey from macOS Keychain |
| Heartbeat | Send ping every 30s. If relay doesn't pong within 5s, reconnect with exponential backoff (1s, 2s, 4s, 8s, 16s, 30s max). |

### What the daemon does NOT do

- No inbound network connections. Outbound WSS only.
- No root privileges. Runs as the logged-in user.
- No persistent database (live session state is in-memory, historical data stays in the existing SQLite DB on `localhost:47892`).
- No file modifications. Read-only access to `~/.claude/projects/` JSONL files.

---

## 10. Dependencies on Mission Control

| Mission Control Phase | What it provides | PWA Phase that needs it |
|----------------------|-----------------|----------------------|
| Phase A (Read-Only Monitoring) | JSONL file watcher, session state machine, cost calculator, broadcast channel | M1 (Status Monitor) |
| Phase F (Interactive Control) | Agent SDK sidecar, session resume, bidirectional chat | M3 (Interactive Control) |

The relay client in `crates/server/` is an additional consumer of the `broadcast::channel` from Phase A. When the MONITOR layer emits a session state change, the relay client encrypts it and forwards to the phone via the relay.

```
Phase A broadcast channel
    ├── SSE endpoint (existing, for desktop browser)
    └── Relay WS client (NEW, for mobile via Fly.io)
```

**Implementation order:**

1. Mission Control Phase A (JSONL watcher, state machine, cost calc)
2. Relay server (`crates/relay/`) + Fly.io deployment
3. Daemon launchd integration + Keychain + relay WS client
4. QR pairing UI in desktop claude-view
5. PWA M1 (Status Monitor)
6. PWA M2 (Read-Only Dashboard)
7. Mission Control Phase F (Agent SDK sidecar) -- can be parallel with M2
8. PWA M3 (Interactive Control)

---

## 11. Relationship to GTM Strategy

### The pivot

The original GTM plan (`docs/plans/2026-02-07-gtm-launch-strategy.md`) positioned the AI Fluency Score as the launch feature. This document pivots to remote session monitoring as the primary wedge.

| | Original GTM | New GTM |
|--|-------------|---------|
| Launch feature | AI Fluency Score | Remote session monitoring (PWA) |
| Hook | "What's your Claude score?" | "Monitor your Claude Code sessions from your phone" |
| Viral mechanic | Score sharing | "I just approved a PR from my phone while walking my dog" |
| Retention driver | Score improvement over time | Daily habit of checking sessions |
| Upsell path | Pro features (team benchmarks) | API tier for 24/7 autonomous agents |

### Growth loop

```
User installs claude-score for session monitoring
    |
    v
User monitors sessions from phone daily (habit loop)
    |
    v
User sees cost/efficiency data passively (fluency score surfaces here)
    |
    v
User shares "monitoring from my phone" screenshot (viral moment)
    |
    v
Other Claude Code users try it (acquisition)
```

### Target user progression

```
Claude Code Max user ($200/mo)
    |
    v  "I can see all my sessions from my phone"
Power user (monitors 10+ concurrent sessions)
    |
    v  "I need sessions running 24/7, $200/mo cap isn't enough"
API tier user (pay-per-token, no cap)
    |
    v  "I want to write specs and throw them into an autonomous pipeline"
Spec-to-code engine customer (future product)
```

---

## 12. Future Vision (Deferred)

These features build on the remote monitoring foundation but are out of scope for this design.

| Feature | Description | Prerequisite |
|---------|-------------|-------------|
| Spec-to-code engine | Write specs, throw into autonomous pipeline, get merge-ready branches | M3 + API tier billing |
| AI SDLC IDE | Like Cursor but for PMs -- everyone with ideas can build | Spec-to-code engine |
| Team dashboard | Manager view of all team members' sessions (opt-in) | Relay multi-tenant auth |
| Cross-device sync | Monitor sessions on Mac A from Mac B | Relay protocol supports it already |

### Business model sketch (deferred)

| Tier | Price | What |
|------|-------|------|
| Free | $0 | Monitor via PWA. Uses user's Claude Code subscription. |
| Pro | TBD | 24/7 autonomous agents via API. Spec-to-code engine. Team features. |

---

## Cross-references

- [`docs/plans/2026-02-17-flyio-relay-design.md`](2026-02-17-flyio-relay-design.md) -- End-to-end relay server, daemon client, QR pairing protocol, deployment
- [`docs/plans/2026-02-17-flyio-relay-implementation.md`](2026-02-17-flyio-relay-implementation.md) -- Step-by-step implementation plan for the relay
- [`docs/plans/2026-02-16-relay-hosting-adr.md`](2026-02-16-relay-hosting-adr.md) -- Hosting provider decision (Fly.io) and re-evaluation triggers
- [`docs/plans/mission-control/design.md`](mission-control/design.md) -- Mission Control architecture (Phase A is prerequisite)
- [`docs/plans/mission-control/phase-a-monitoring.md`](mission-control/phase-a-monitoring.md) -- JSONL watcher, state machine, broadcast channel
- [`docs/plans/mission-control/phase-f-interactive.md`](mission-control/phase-f-interactive.md) -- Agent SDK sidecar (prerequisite for M3)
- `docs/plans/2026-02-07-gtm-launch-strategy.md` (on Desktop) -- Original GTM plan this pivots from

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Missing cross-reference: `2026-02-17-flyio-relay-design.md` does not exist | Blocker | Created the design document and implementation plan, removed TODO warnings |
| 2 | Binary name mismatch: plan used `vibe-recall-server` but actual binary is `vibe-recall` | Blocker | Updated lines 73 and 107 to use `vibe-recall` |
| 3 | Daemon plist used old `claude-score` naming | Blocker | Updated plist (lines 229-247) and location (line 251) to use `vibe-recall` naming consistently |
| 4 | CLI naming unclear: `npx claude-view` vs `vibe-recall` | Warning | Clarified that `npx claude-view` is npm wrapper that invokes `vibe-recall` binary (line 257) |
| 5 | Hook path in phase-a-monitoring differs from actual | Minor | Noted in changelog; phase-a-monitoring.md is a separate plan with its own audit cycle |
| 6 | Growth loop uses old `claude-score` product name | Minor | Left as-is; this is a conceptual diagram, not code reference. Product naming TBD. |
