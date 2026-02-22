# Mobile Remote Zero-Setup Design

**Date:** 2026-02-23
**Branch:** `worktree-mobile-remote`
**Status:** Approved

## Problem

Mobile remote monitoring exists (~95% built) but:
1. Three bugs prevent end-to-end pairing flow
2. Users must self-host relay on Fly.io and configure `.env`
3. No auth — can't track users, rate limit, or prevent abuse
4. No path to remote control (only read-only monitoring)

## Goal

`npx claude-view` → click phone icon → scan QR → sign in → it works. No relay setup, no `.env`, no Fly.io account. Zero configuration.

## Architecture

```
User's Phone                    Cloud                         User's Mac
─────────────                   ─────                         ──────────

Safari/Chrome        ┌─── m.claudeview.ai ───┐
    │                │   Cloudflare Pages     │
    │  loads SPA ──► │   React mobile app     │
    │                │   + Supabase Auth SDK  │
    │                └────────────────────────┘
    │
    │                ┌── relay.claudeview.ai ──┐
    │   WSS ────────►│   Fly.io               │◄──── WSS ────  claude-view
    │  (encrypted)   │   WebSocket relay      │  (encrypted)    (Rust server)
    │                │   + JWT validation     │
    │                └────────────────────────┘
    │
    │                ┌── Supabase ────────────┐
    │   auth ───────►│   Auth (magic link)    │
    │                │   Postgres (usage log) │
    │                └────────────────────────┘
```

**Phone** = mobile-optimized React SPA on Cloudflare Pages. Own UI, sends commands through relay.
**Relay** = dumb WebSocket message broker on Fly.io. Forwards encrypted blobs. Validates JWTs.
**Mac** = execution engine. Runs claude-view + Claude CLI. Receives commands, sends results.

The phone does NOT tunnel into the Mac's web UI. It has its own mobile client that communicates via an extensible command protocol — same pattern as Slack mobile, GitHub mobile, Happy Coder.

## Infrastructure

| Service | Subdomain | Host | Purpose | Cost |
|---------|-----------|------|---------|------|
| Mobile SPA | `m.claudeview.ai` | Cloudflare Pages | React mobile app + Supabase Auth | Free |
| Relay | `relay.claudeview.ai` | Fly.io | WebSocket relay + JWT validation | Free tier |
| Auth + DB | — | Supabase | Magic link / Google OAuth + usage tracking | Free (50k MAU) |
| DNS | `claudeview.ai` | Cloudflare | Domain management | Owned |

**Total monthly cost: $0** (all within free tiers for 10-50 users).

## User Journey

```
1. $ npx claude-view
   → Binary ships with RELAY_URL=wss://relay.claudeview.ai/ws baked in
   → Rust server starts, relay_client auto-connects to relay (even with 0 paired devices)

2. Click phone icon in header → QR code appears
   → QR encodes: https://m.claudeview.ai?k=<mac_x25519_pubkey>&t=<token>

3. Phone scans QR → opens m.claudeview.ai
   → First time: "Sign in to connect"
     ├─ "Continue with Google" (one tap)
     └─ "Sign in with email" (magic link)
   → Supabase creates user, issues JWT

4. Pairing completes
   → Phone generates X25519 + Ed25519 keypairs
   → POSTs /pair/claim with: token, x25519_pubkey (cleartext), JWT
   → Relay validates JWT, stores pairing, sends pair_complete to Mac
   → Mac stores phone pubkey in Keychain

5. Sessions flow
   → Mac encrypts session data per paired device, sends via relay
   → Phone decrypts, renders session list
   → Bookmark m.claudeview.ai → next time, already signed in, sessions appear
```

## Security Model

```
Layer 1: Supabase Auth (identity)
├─ Phone signs in via magic link / Google
├─ JWT issued by Supabase (RS256, 1hr expiry)
└─ Relay validates JWT on /pair/claim + WS connect

Layer 2: QR one-time token (pairing authorization)
├─ Mac generates 32-byte random token, 5-min TTL
├─ Token embedded in QR code
└─ Consumed on claim (single use)

Layer 3: E2E encryption (data privacy)
├─ NaCl box: X25519 + XSalsa20-Poly1305
├─ Relay sees only opaque encrypted blobs
└─ Keys stored: Mac=Keychain, Phone=IndexedDB

Layer 4: Ed25519 auth (WebSocket sessions)
├─ Each WS connect: sign timestamp with Ed25519
└─ 60-second freshness window
```

**Relay can see:** device IDs, connection timestamps, message sizes.
**Relay cannot see:** session content, commands, user code, prompts.

## Command Protocol

All messages are E2E encrypted NaCl box. The relay only sees opaque blobs. Inside the encrypted envelope:

### Phase 1 (M1): Read-only monitoring

```json
// Mac → Phone: session snapshot
{
  "type": "sessions",
  "sessions": [
    {
      "id": "abc123",
      "project": "/Users/dev/myapp",
      "model": "opus",
      "status": "active",
      "cost_usd": 1.42,
      "tokens": { "input": 12000, "output": 3400 },
      "last_message": "Implementing the auth module...",
      "updated_at": 1708700000
    }
  ]
}

// Mac → Phone: live output stream
{
  "type": "output",
  "session_id": "abc123",
  "chunks": [
    { "role": "assistant", "text": "I'll create the login component..." },
    { "role": "tool", "name": "Write", "path": "src/Login.tsx" }
  ]
}
```

### Phase 2 (M2): Remote control

```json
// Phone → Mac: send message to session
{ "type": "command", "action": "send_message", "session_id": "abc123",
  "text": "Also add password validation" }

// Phone → Mac: approve/deny tool use
{ "type": "command", "action": "approve_tool", "session_id": "abc123",
  "tool_use_id": "tu_789", "approved": true }

// Phone → Mac: spawn new session
{ "type": "command", "action": "spawn_session",
  "project": "/Users/dev/myapp", "prompt": "Fix the login bug" }
```

### Phase 3 (M3): Full parity

```json
// Phone → Mac: search, analytics, notifications
{ "type": "command", "action": "search", "query": "auth bug" }
{ "type": "command", "action": "analytics", "range": "7d" }
{ "type": "notification", "title": "Claude needs approval",
  "session_id": "abc123", "tool": "Bash: rm -rf node_modules" }
```

Protocol is extensible — add new `action` types without breaking existing clients. Unknown actions are ignored.

## Supabase Schema

```sql
-- usage_log: track relay usage per user
create table usage_log (
  id         bigint generated always as identity primary key,
  user_id    uuid references auth.users(id),
  event      text not null,          -- 'pair', 'connect', 'message'
  metadata   jsonb default '{}',
  created_at timestamptz default now()
);

-- paired_devices: server-side pairing record
create table paired_devices (
  id          bigint generated always as identity primary key,
  user_id     uuid references auth.users(id),
  device_id   text not null,
  device_type text not null,         -- 'mac' or 'phone'
  paired_at   timestamptz default now(),
  last_seen   timestamptz
);
```

## Rate Limits (relay, per authenticated user)

- Max 3 concurrent WebSocket connections
- Max 100 messages/minute
- Max 5 paired devices

## Competitive Positioning vs Happy

| | Happy | claude-view mobile |
|---|---|---|
| What it is | CLI wrapper (replaces `claude`) | Dashboard (works alongside `claude`) |
| Breakage risk | High — wraps CLI internals | Low — reads session files |
| Mobile | Native iOS + Android (Expo) | PWA (no app store) |
| Control | Full remote control + voice | M1: monitoring, M2: full control |
| Analytics | None — live view only | Deep — history, cost, patterns, search |
| Auth | None (open source, free) | Supabase (tracks users) |
| Price | Free forever (MIT) | Part of claude-view product |

Edge: zero CLI change, analytics moat, no app store dependency.

## Changes to Existing Code

### Relay (`crates/relay/`)

- Add `x25519_pubkey: String` to ClaimRequest, forward in pair_complete
- Add Supabase JWT validation (RS256, fetch JWKS from Supabase)
- Accept `Authorization: Bearer <jwt>` on WS handshake
- Add per-user rate limiting
- Custom domain: `relay.claudeview.ai`

### Mac relay client (`crates/server/src/live/relay_client.rs`)

- Always connect when RELAY_URL is set (fix chicken-and-egg)
- Implement `pair_complete` handler (extract x25519_pubkey, store in Keychain)
- Hardcode default RELAY_URL (`wss://relay.claudeview.ai/ws`)

### Mobile pages (`src/pages/Mobile*.tsx`)

- Add Supabase auth screen before pairing
- Send JWT with claim request
- Send `x25519_pubkey` in claim POST
- Deploy to Cloudflare Pages as separate build

### Desktop (`src/components/PairingPanel.tsx`)

- QR encodes `https://m.claudeview.ai?k=...&t=...`
- Remove `.env` RELAY_URL requirement (bake in default, allow override)

## Phased Roadmap

### M1: "It connects" (current sprint)

- Fix relay_client chicken-and-egg (always connect)
- Send X25519 pubkey in clear (fix circular crypto)
- Implement `pair_complete` handler (store pairing)
- Add Supabase auth to mobile pages
- Add JWT validation to relay
- Deploy mobile SPA to Cloudflare Pages (`m.claudeview.ai`)
- Point relay to `relay.claudeview.ai`
- Bake default RELAY_URL into binary
- **Result:** Scan QR → sign in → see sessions live

### M2: "Remote control"

- Phone → Mac command protocol (send_message, approve_tool, spawn_session)
- Mac command handler (execute commands from phone)
- Mobile UI for session interaction (chat input, approve/deny buttons)
- Push notifications via Web Push API
- **Result:** Full remote control from phone

### M3: "Full parity"

- Search from phone
- Analytics from phone
- Multi-session management
- Full conversation history view
- **Result:** Phone can do everything desktop can

## One-Time Setup (you, not users)

| Task | Time |
|------|------|
| Create Supabase project, enable magic link + Google OAuth | 15 min |
| Create Cloudflare Pages project for `m.claudeview.ai` | 10 min |
| DNS: CNAME `m.claudeview.ai` → Cloudflare Pages | 2 min |
| DNS: CNAME `relay.claudeview.ai` → `claude-view-relay.fly.dev` | 2 min |
| Fly.io: `fly certs add relay.claudeview.ai` | 2 min |
