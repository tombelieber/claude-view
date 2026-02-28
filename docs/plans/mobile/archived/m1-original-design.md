---
status: approved
date: 2026-02-23
purpose: Mobile Remote Connection M1 — Full end-to-end design for remote session monitoring from phone
depends_on:
  - docs/plans/backlog/2026-02-12-mobile-pwa-design.md
  - docs/plans/mission-control/phase-a-monitoring.md
  - docs/architecture/live-monitor.md
---

# Mobile Remote Connection — M1: Status Monitor

> **Goal:** Monitor Claude Code sessions from your phone via E2E encrypted relay. Full M1 end-to-end: relay server, NaCl crypto, QR pairing, WSS client, and PWA status monitor.

---

## Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Scope | Full M1 end-to-end | Relay + crypto + QR pairing + PWA |
| Relay hosting | Build locally first, Fly.io later | Faster iteration, no deploy cycles |
| Crypto | Full NaCl from day 1 | Protocol designed around it — retrofitting is harder |
| Daemon model | Same process | Relay WSS client inside existing server, like file watcher |
| PWA structure | `/mobile` route in same SPA | Shares hooks/types, mobile-only page component |
| QR placement | Header phone icon + slide-over panel | Discoverable, non-disruptive, dual-purpose (QR + device list) |
| Wire format | JSON | Already used everywhere, native `JSON.parse()` on phone, swappable later inside encrypted envelope |
| Implementation order | Bottom-up | Relay → crypto → WSS client → QR pairing → PWA UI |

---

## 1. System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Mac (same process)                          │
│                                                                 │
│  Mission Control Phase A          Relay WSS Client              │
│  ┌──────────────────────┐    ┌─────────────────────────┐       │
│  │ JSONL watcher        │    │ Subscribes to broadcast  │       │
│  │ Hook handler         │───►│ NaCl box encrypts        │       │
│  │ Broadcast channel    │    │ Sends via WSS            │       │
│  └──────────────────────┘    └──────────┬──────────────┘       │
│                                         │ outbound WSS only     │
│  QR Pairing UI (desktop)                │                       │
│  ┌──────────────────────┐               │                       │
│  │ Header phone icon    │               │                       │
│  │ Slide-over panel     │               │                       │
│  │ QR code generation   │               │                       │
│  │ Paired devices list  │               │                       │
│  └──────────────────────┘               │                       │
└─────────────────────────────────────────┼───────────────────────┘
                                          │
                              ┌───────────▼───────────┐
                              │    Relay Server        │
                              │    (localhost / Fly.io) │
                              │                        │
                              │ • Zero-knowledge       │
                              │ • Forward opaque blobs │
                              │ • Device registry      │
                              │ • Ed25519 auth         │
                              └───────────┬───────────┘
                                          │
                              ┌───────────▼───────────┐
                              │  Phone (PWA)           │
                              │  /mobile route         │
                              │                        │
                              │ • WSS to relay         │
                              │ • NaCl box decrypts    │
                              │ • Session list UI      │
                              │ • Status dots + cost   │
                              └───────────────────────┘
```

Key properties:
- **Zero-knowledge relay** — relay cannot decrypt. Protocol identical local vs production.
- **Outbound-only** — Mac never listens on additional ports. WSS client connects out.
- **Same process** — relay client runs as background task alongside file watcher and process detector.
- **Broadcast channel reuse** — relay client is another consumer of existing `broadcast::channel`.

---

## 2. Relay Server (`crates/relay/`)

Minimal Axum binary. Forwards encrypted blobs between paired devices.

### Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/ws` | WebSocket upgrade. Auth via Ed25519 signed challenge in first message. |
| `POST` | `/pair` | Mac posts pairing offer: `{ device_id, pubkey, one_time_token }`. Stored in-memory, 5-min TTL. |
| `POST` | `/pair/claim` | Phone claims offer: `{ one_time_token, device_id, pubkey_encrypted_blob }`. Forwarded to Mac, offer deleted. |
| `GET` | `/health` | Health check. Returns 200. |

### In-memory state

```rust
struct RelayState {
    // Active WebSocket connections, keyed by device_id
    connections: RwLock<HashMap<String, DeviceConnection>>,

    // Pending pairing offers, keyed by one_time_token
    pairing_offers: RwLock<HashMap<String, PairingOffer>>,

    // Device pairs: which devices are paired together
    device_pairs: RwLock<HashMap<String, HashSet<String>>>,
}
```

No database. Stateless across restarts — devices reconnect and re-authenticate. Pairing state persisted on Mac (Keychain) and phone (IndexedDB).

### WebSocket protocol

Every WS message is an envelope:

```json
{
    "to": "device_id_of_recipient",
    "payload": "<base64 NaCl box ciphertext>"
}
```

Relay reads `to`, forwards raw message to recipient. Never touches `payload`. If recipient offline, message dropped (no queuing for M1).

### Auth flow (first WS message)

```json
{
    "type": "auth",
    "device_id": "mac-abc123",
    "timestamp": 1708700000,
    "signature": "<Ed25519 sign(timestamp + device_id)>"
}
```

Relay verifies signature against pubkey registered during pairing. Rejects if timestamp >60s stale.

### Constraints

- Max 10 paired devices per Mac
- Max 100 concurrent WS connections (local dev)
- Pairing offers expire after 5 minutes
- Rate limit: 60 messages/min per device

---

## 3. Crypto & Key Management

### Key types

| Key | Algorithm | Created when | Stored where | Purpose |
|-----|-----------|-------------|-------------|---------|
| Mac identity keypair | Ed25519 | First server start | macOS Keychain | Authenticate WS connections |
| Mac encryption keypair | X25519 | First server start | macOS Keychain | Encrypt/decrypt session data |
| Phone encryption keypair | X25519 | QR scan | IndexedDB | Encrypt/decrypt session data |
| Phone identity keypair | Ed25519 | QR scan | IndexedDB | Authenticate WS connections |
| One-time pairing token | Random 32 bytes | QR generation | In-memory (relay, 5-min TTL) | Single-use handshake |

### Encryption flow (Mac → Phone)

```
LiveSession JSON → serde_json::to_vec (~1KB)
    → crypto_box::seal(plaintext, nonce, phone_pubkey, mac_privkey)
    → base64(nonce ++ ciphertext) (~1.4KB)
    → WS message payload
```

### Rust dependencies

- `crypto_box` — NaCl box (X25519 + XSalsa20-Poly1305), pure Rust
- `ed25519-dalek` — Ed25519 signing
- `security-framework` — macOS Keychain access

### Phone-side JS

- `tweetnacl` (7KB) — NaCl box open, key generation

### Keychain storage

```
Keychain item: "com.claude-view.identity"
├── Ed25519 private key
├── X25519 private key
└── Paired devices: [{ device_id, x25519_pubkey, name, paired_at }]
```

Keys created on first server start. No user interaction — default access control.

### Startup behavior

| Scenario | Behavior |
|----------|----------|
| First start ever | Generate keypairs, store in Keychain. Relay client idle (no paired devices). |
| Keys exist, no paired device | Relay client connects, authenticates. No data sent. |
| Keys exist, device paired | Relay client connects, authenticates, subscribes to broadcast, encrypts and sends updates. |

---

## 4. QR Pairing Flow

### QR code payload

```json
{
  "r": "ws://localhost:47892/relay",
  "k": "<mac X25519 pubkey, base64>",
  "t": "<one-time token, base64>",
  "v": 1
}
```

Encoded as URL: `claude-view://pair?r=...&k=...&t=...&v=1`

### Pairing sequence

```
Mac                          Relay                      Phone
 │  1. POST /pair              │                          │
 │  { device_id, pubkey,       │                          │
 │    one_time_token }         │                          │
 │────────────────────────────►│  stores, 5-min TTL       │
 │                             │                          │
 │  2. Display QR code         │                          │
 │                             │                          │
 │                             │  3. Phone scans QR       │
 │                             │     generates keypair    │
 │                             │                          │
 │                             │  4. POST /pair/claim     │
 │                             │◄─────────────────────────│
 │                             │  { token, device_id,     │
 │                             │    pubkey_encrypted }     │
 │                             │                          │
 │  5. Relay forwards          │                          │
 │◄────────────────────────────│                          │
 │                             │                          │
 │  6. Mac decrypts, stores    │                          │
 │     phone pubkey in Keychain│                          │
 │                             │                          │
 │  7. Both open WSS           │                          │
 │─────────────────────────────┼──────────────────────────│
```

### Desktop UI — Header icon + slide-over panel

**Unpaired state:** Phone icon in header nav (green dot badge). Panel shows QR code, 5-min countdown, "No devices paired."

**Paired state:** Phone icon (no badge). Panel shows paired devices list with name, date, remove button. "Pair another device" expands QR.

### Phone-side scanning

`/mobile` route shows `MobilePairingPage` when no keys in IndexedDB. Uses `navigator.mediaDevices.getUserMedia` + `jsQR` (~15KB).

### Security properties

- One-time token: used once, then deleted
- Phone pubkey encrypted with Mac pubkey before transit
- QR expires after 5 minutes
- New QR on each panel open

---

## 5. Relay WSS Client (in existing server)

### Codebase location

```
crates/server/src/
├── live/
│   ├── manager.rs          ← existing
│   ├── state.rs            ← existing
│   └── relay_client.rs     ← NEW
├── routes/
│   ├── live.rs             ← existing
│   └── pairing.rs          ← NEW
└── crypto.rs               ← NEW
```

### Startup integration

```
LiveSessionManager::start()
├── spawn_file_watcher()        ← existing
├── spawn_process_detector()    ← existing
├── spawn_cleanup_task()        ← existing
└── spawn_relay_client()        ← NEW
```

### Broadcast channel consumption

```
Phase A broadcast channel
    ├── SSE endpoint        ← existing (desktop browser)
    └── Relay WSS client    ← NEW (encrypts → relay → phone)
```

### Reconnection

- Exponential backoff: 1s → 2s → 4s → 8s → 16s → 30s max
- Heartbeat ping every 30s, pong expected within 5s
- On reconnect: re-authenticate, re-send current state snapshot
- Events during disconnect are dropped (phone gets full state on reconnect)

### Configuration

```rust
struct RelayConfig {
    relay_url: String,             // default: "ws://localhost:47892/relay"
    enabled: bool,                 // default: true (idle if no paired devices)
    heartbeat_interval_secs: u32,  // default: 30
    max_reconnect_delay_secs: u32, // default: 30
}
```

---

## 6. Mobile PWA (`/mobile` route)

### Component tree

```
/mobile route
├── MobilePairingPage          ← shown when no keys in IndexedDB
│   ├── QR scanner (jsQR + camera API)
│   └── Pairing status indicator
│
└── MobileMonitorPage          ← shown when paired
    ├── MobileHeader           ← "Claude Sessions" + active count badge
    ├── ConnectionStatus       ← green dot / red reconnecting
    ├── MobileSessionList      ← pull-to-refresh wrapper
    │   └── MobileSessionCard[]
    │       ├── StatusDot      ← reuse from desktop
    │       ├── ProjectName
    │       ├── AgentStateLabel
    │       ├── CostBadge
    │       └── ContextBar     ← thin progress bar
    └── MobileSessionDetail    ← slide-up bottom sheet on card tap
        ├── Full agent state + label
        ├── Token breakdown
        ├── Cost breakdown
        ├── Sub-agent pills
        └── Progress items list
```

### Shared components (reused from desktop)

StatusDot, CostTooltip, SubAgentPills, ProgressItem, useMediaQuery

### Mobile-only components (new)

MobileSessionCard, MobileSessionDetail, MobileHeader, ConnectionStatus, PullToRefresh, QRScanner

### Phone-side data flow

```
Open /mobile → check IndexedDB for keys
    ├── No keys → QRScanner → pair → store keys
    └── Keys exist → WSS to relay → auth → receive encrypted messages
        → nacl.box.open() → JSON → LiveSession → React state → render
```

### PWA manifest

```json
{
  "name": "Claude View",
  "short_name": "Claude",
  "start_url": "/mobile",
  "display": "standalone",
  "background_color": "#0F172A",
  "theme_color": "#0F172A",
  "icons": [{ "src": "/icon-192.png", "sizes": "192x192" }]
}
```

Service worker deferred to M2.

### M1 does NOT include

- Push notifications (M2)
- Offline mode (M2)
- Interactive control / reply to sessions (M3)
- Service worker caching (M2)

---

## Implementation Order (Bottom-Up)

1. **crates/relay/** — relay server binary (endpoints, WS handler, in-memory state)
2. **Crypto module** — `crypto.rs` (NaCl box, Ed25519, Keychain read/write)
3. **Relay WSS client** — `relay_client.rs` (connect, auth, encrypt, send)
4. **QR pairing** — desktop slide-over panel + relay `/pair` + `/pair/claim`
5. **`/mobile` PWA route** — phone UI (scanner, session list, detail sheet)

---

## Cross-references

- [`docs/plans/backlog/2026-02-12-mobile-pwa-design.md`](backlog/2026-02-12-mobile-pwa-design.md) — Full mobile PWA vision (M1-M3)
- [`docs/architecture/live-monitor.md`](../architecture/live-monitor.md) — Live monitor architecture (hooks, SSE, broadcast channel)
- [`docs/plans/mission-control/phase-a-monitoring.md`](mission-control/phase-a-monitoring.md) — Phase A (prerequisite, done)
