# Conversation Sharing — Design Doc

**Date:** 2026-02-28
**Status:** Approved

## Problem

Claude-view users want to share conversations with others — for education, demos, or showing "look what Claude did for me." Currently the only export is a local JSON/CSV download with metadata only (no conversation content).

## Target User

$200/month Claude power users. Non-tech or tech-adjacent (PMs, founders, consultants). Share via WhatsApp/Slack/email — not publishing to the web.

## UX Model

**ChatGPT's sharing model:** Private by default. Explicit "Share" action. Unguessable link. No expiry. Revoke anytime. No password, no permissions UI.

### Share Flow

1. User clicks **Share** button on session detail header
2. Link is instantly generated (no dialog/form)
3. Auto-copied to clipboard, toast: "Link copied!"
4. Button changes to "Shared" state with dropdown: Copy Link | Revoke

### Viewer Experience

Recipient opens link → sees the **full rich session viewer** — same chat/debug modes, tool call details, agent breakdown, rich/JSON toggle. Read-only. No login required.

### Revocation

- From session detail: "Shared" dropdown → Revoke
- From shared links list in Settings: see all active shares, view count, bulk revoke

## Architecture

**All-Cloudflare** — Worker + R2 + D1. No relay involvement.

```
User's Mac (claude-view)          Cloudflare (all free tier)
┌──────────────────────┐    ┌──────────────────────────────────┐
│ Click "Share"        │    │  Worker: share.claude-view.app   │
│   ↓                  │    │                                  │
│ Local server         │    │  POST /api/share                 │
│ serializes session   │───▶│    → generate 22-char token      │
│ as gzipped JSON      │    │    → store metadata in D1        │
│                      │    │    → return presigned upload URL  │
│ Upload blob to R2    │───▶│                                  │
│ (presigned PUT)      │    │  R2: shares/{token}.json.gz      │
│   ↓                  │    │                                  │
│ Copy link to         │    │  GET /s/{token}                  │
│ clipboard            │    │    → serve viewer SPA            │
│                      │    │    → SPA fetches blob from R2    │
│                      │    │    → renders rich session viewer  │
│ "Shared links" list  │    │                                  │
│   revoke ──────────────▶  │  DELETE /api/share/{token}       │
│                      │    │    → delete from R2 + D1         │
└──────────────────────┘    └──────────────────────────────────┘
```

### Why All-Cloudflare

- **R2 free tier:** 10 GB storage, 10M reads/mo, $0 egress
- **Workers free tier:** 100K requests/day
- **D1 free tier:** 5 GB database
- **Pages:** Unlimited free static hosting
- Same pattern as fluffy project (proven, team has experience)
- Relay stays lean (WebSocket relay only, no blob storage)

## Data Flow

### Share Creation

1. User clicks Share → frontend calls `POST /api/sessions/{id}/share`
2. Local Rust server reads the session JSONL file
3. Serializes to JSON (messages + tool calls + metadata), gzip compresses
4. Calls Cloudflare Worker `POST /api/share` with session metadata
5. Worker generates 22-char base62 token (131 bits entropy)
6. Worker stores metadata in D1, returns presigned R2 upload URL
7. Local server uploads gzipped blob directly to R2 via presigned PUT
8. Worker confirms upload, returns share URL
9. Local server returns URL to frontend → auto-copy to clipboard → toast

### Share Viewing

1. Recipient opens `share.claude-view.app/s/{token}`
2. Cloudflare Pages serves the viewer SPA (static React build)
3. SPA extracts token, calls Worker `GET /api/share/{token}`
4. Worker looks up D1 metadata, increments view count
5. Worker returns R2 CDN URL for the blob
6. SPA fetches gzipped JSON from R2, decompresses in browser
7. SPA renders with same rich session components — read-only

### Share Revocation

1. User clicks Revoke → frontend calls `DELETE /api/sessions/{id}/share`
2. Local server calls Worker `DELETE /api/share/{token}`
3. Worker deletes R2 blob + D1 metadata
4. Link returns 404

## Storage

### D1 Schema (Share Metadata)

```sql
CREATE TABLE shares (
    token       TEXT PRIMARY KEY,       -- 22-char base62
    session_id  TEXT NOT NULL,          -- original session ID
    title       TEXT,                   -- for listing
    size_bytes  INTEGER NOT NULL,       -- uncompressed size
    created_at  INTEGER NOT NULL,       -- unix timestamp
    view_count  INTEGER DEFAULT 0
);
```

### R2 Object Layout

```
shares/{token}.json.gz    -- gzipped session JSON blob
```

### Real Session Sizes (from actual JSONL data)

| Category | Raw size | Gzipped (~4:1) | Share-worthy? |
|----------|---------|-----------------|---------------|
| Small (<1 MB) | 4,280 files | <256 KB | Rarely |
| Medium (1-10 MB) | 521 files | 250 KB–2.5 MB | Sometimes |
| Large (10-50 MB) | 16 files | 2.5–12 MB | Yes — these are the demos |
| Huge (50-150 MB) | 1 file | 12–40 MB | Yes — epic sessions |

**Max share size:** 150 MB raw / ~40 MB compressed. Covers 100% of current sessions.

## Cloudflare Worker API

```
POST   /api/share           — Create share, get presigned upload URL
GET    /api/share/{token}    — Get metadata + R2 URL (public, no auth)
DELETE /api/share/{token}    — Revoke share (requires origin auth)
GET    /api/share/list       — List shares by origin (requires origin auth)
```

**Auth for mutations:** Local claude-view server includes a simple HMAC signature in requests. Not user accounts — just proof that the request came from a claude-view instance.

## Viewer SPA (Cloudflare Pages)

- New workspace: `apps/share/` — minimal React build
- Reuses session viewer components from `apps/web/`
- Extract shared components into `packages/session-viewer/`
- Read-only: no sidebar, no navigation, no settings
- Header: session title + "Viewed via claude-view" branding + "Get claude-view" CTA
- Deploy: `share.claude-view.app` on Cloudflare Pages

## Local Server API Additions

```
POST   /api/sessions/{id}/share    — Create share for this session
DELETE /api/sessions/{id}/share    — Revoke share for this session
GET    /api/shares                 — List all active shares
```

## Frontend Changes

### Session Detail Header
- Add Share button (link icon) next to existing controls
- States: "Share" (unshared) → generating spinner → "Shared" (with dropdown)
- Dropdown: Copy Link | Open Preview | Revoke

### Settings Page
- "Shared Links" section: list of active shares
- Columns: title, date shared, views, link
- Revoke button per share, bulk revoke

## Security Model

| Layer | Implementation |
|-------|---------------|
| Private by default | Nothing shared until explicit user action |
| Unguessable token | 22-char base62 = 131 bits entropy |
| No expiry | Same as ChatGPT — lives until revoked |
| Instant revocation | DELETE removes blob + metadata |
| Rate limiting | Worker rate-limits creation (10/hour/IP) |
| Origin auth | HMAC signature for mutations |
| Public reads | Token IS the credential |
| Size limit | 150 MB raw per share |

## Cost Projection

| Scale | Monthly cost |
|-------|-------------|
| 100 users, 5 shares/week | **$0** (free tier) |
| 1,000 users, 50 shares/week | **$0** (still free tier) |
| 10,000 users | ~$5/mo (Workers paid plan) |

## What Gets Shared

Full session content — same as JSONL:
- All human/assistant messages
- Tool calls (read, edit, bash, write) with outputs
- Thinking blocks
- Token usage metadata
- Session metadata (project, duration, timestamps)

## Non-Goals (V1)

- No user accounts / login
- No password protection
- No expiry picker (add later if needed)
- No comments / annotations on shared sessions
- No "fork" / import shared session
- No partial sharing (specific turns only)
