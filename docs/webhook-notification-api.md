# Webhook Notification API

Push real-time session events (start, end, error, update) to external services via HMAC-signed HTTP webhooks. Supports Lark, Slack, and raw JSON formats.

## Quick Start

```bash
# 1. Generate an API key (localhost only)
curl -X POST http://localhost:4173/api/auth/generate-key
# → { "key": "cv_live_ABC...XYZ_1a2b3c4d", "id": "key_abc123", "prefix": "cv_live_ABC." }
#   ⚠ Save the key — it's shown ONCE and never again.

# 2. Create a webhook
curl -X POST http://localhost:4173/api/webhooks \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My Lark Bot",
    "url": "https://open.larksuite.com/open-apis/bot/v2/hook/xxxxx",
    "format": "lark",
    "events": ["session.started", "session.ended", "session.error"]
  }'
# → { "webhook": { "id": "wh_abc123...", ... }, "signingSecret": "whsec_..." }
#   ⚠ Save the signing secret — it's shown ONCE.

# 3. Test it
curl -X POST http://localhost:4173/api/webhooks/wh_abc123.../test
# → { "success": true, "statusCode": 200 }
```

That's it. The engine is always running — as soon as you create a webhook, events start flowing.

---

## Authentication

All `/api/*` endpoints require auth when accessed remotely.

| Caller | Auth |
|--------|------|
| Localhost (`127.0.0.1`, `::1`) | No auth needed (backward compat) |
| Remote | `Authorization: Bearer cv_live_...` header |

### Key Management

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/auth/generate-key` | Generate a new API key |
| `DELETE` | `/api/auth/keys/{id}` | Revoke an API key |

**Key format:** `cv_live_{32_base62_random}_{8_hex_crc32}`

Keys are hashed with SHA-256 before storage. The raw key is returned exactly once on creation.

**Storage:** `~/.claude-view/api-keys.json`

### Generate Key

```bash
curl -X POST http://localhost:4173/api/auth/generate-key
```

```json
{
  "key": "cv_live_7kPmN2xRvQ4sJ8bD1fW9tY3hL6cA5gE0_a1b2c3d4",
  "id": "key_Xk9mP2nR",
  "prefix": "cv_live_7kPm"
}
```

### Revoke Key

```bash
curl -X DELETE http://localhost:4173/api/auth/keys/key_Xk9mP2nR
```

---

## Webhook CRUD

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/webhooks` | List all webhooks |
| `POST` | `/api/webhooks` | Create a webhook |
| `GET` | `/api/webhooks/{id}` | Get a single webhook |
| `PUT` | `/api/webhooks/{id}` | Update a webhook |
| `DELETE` | `/api/webhooks/{id}` | Delete a webhook |
| `POST` | `/api/webhooks/{id}/test` | Send a test payload |

**Storage:** `~/.claude-view/notifications.json` (config) + `~/.claude-view/webhook-secrets.json` (signing keys)

### Create Webhook

```bash
curl -X POST http://localhost:4173/api/webhooks \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Slack Notifications",
    "url": "https://hooks.slack.com/services/T.../B.../xxx",
    "format": "slack",
    "events": ["session.started", "session.ended"]
  }'
```

**Validation rules:**
- `url` must start with `https://`
- `name` must be 1–64 characters
- `events` must contain at least one event type

**Response:**

```json
{
  "webhook": {
    "id": "wh_a1b2c3d4e5f6",
    "name": "Slack Notifications",
    "url": "https://hooks.slack.com/services/T.../B.../xxx",
    "format": "slack",
    "events": ["session.started", "session.ended"],
    "enabled": true,
    "createdAt": "2026-04-10T10:00:00Z"
  },
  "signingSecret": "whsec_dGVzdC13ZWJob29rLXNpZ25pbmcta2V5..."
}
```

The `signingSecret` is shown **once** on creation. Store it securely — you'll need it to verify webhook signatures on the receiving end.

### Update Webhook

Partial update — only include fields you want to change:

```bash
curl -X PUT http://localhost:4173/api/webhooks/wh_a1b2c3d4e5f6 \
  -H "Content-Type: application/json" \
  -d '{ "enabled": false }'
```

### Delete Webhook

Removes the webhook config and its signing secret:

```bash
curl -X DELETE http://localhost:4173/api/webhooks/wh_a1b2c3d4e5f6
```

### Test Send

Sends a synthetic `session.started` event to verify your endpoint works:

```bash
curl -X POST http://localhost:4173/api/webhooks/wh_a1b2c3d4e5f6/test
```

```json
{
  "success": true,
  "statusCode": 200,
  "error": null
}
```

---

## Event Types

| Event | Trigger | Use case |
|-------|---------|----------|
| `session.started` | New Claude Code session discovered | Track developer activity |
| `session.ended` | Session process exited | Log duration + cost summaries |
| `session.error` | First error detected in session | Alert on failures |
| `session.updated` | Session state changed (debounced 10s) | Live dashboards |

### Debouncing

`session.updated` fires 10+ times per second during active sessions. The engine coalesces updates to **at most 1 delivery per 10 seconds** per webhook+session pair. Other event types are delivered immediately.

---

## Payload Format

All payloads follow the [Stripe event convention](https://docs.stripe.com/api/events):

```json
{
  "id": "evt_a1b2c3d4e5f6g7h8",
  "type": "session.ended",
  "timestamp": 1775764333,
  "data": {
    "sessionId": "abc123-def456",
    "project": "claude-view",
    "projectPath": "/Users/dev/claude-view",
    "model": "claude-sonnet-4-6",
    "modelDisplayName": "Sonnet",
    "status": "Done",
    "durationSecs": 252,
    "costUsd": 0.35,
    "turnCount": 12,
    "editCount": 8,
    "gitBranch": "feature/webhooks",
    "entrypoint": "cli",
    "sessionKind": "interactive",
    "agentState": "Done",
    "webUrl": "https://your-instance.com/sessions/abc123-def456"
  }
}
```

**Field inclusion rules:**
- `durationSecs` — only present when both `startedAt` and `closedAt` are known (typically `session.ended`)
- `costUsd` — omitted when zero
- `turnCount`, `editCount` — omitted when zero
- `error`, `errorDetails` — only present on `session.error`
- `webUrl` — only present when `baseUrl` is configured in `notifications.json`

---

## Output Formats

### `raw`

The canonical payload JSON, as-is. Use this for custom integrations.

### `lark`

Lark interactive card (`msg_type: "interactive"`):

```json
{
  "msg_type": "interactive",
  "card": {
    "header": {
      "title": { "content": "Session Ended", "tag": "plain_text" }
    },
    "elements": [
      { "tag": "div", "text": { "content": "**Project:** claude-view", "tag": "lark_md" } },
      { "tag": "div", "text": { "content": "**Model:** Sonnet", "tag": "lark_md" } },
      { "tag": "div", "text": { "content": "**Status:** Done", "tag": "lark_md" } },
      { "tag": "div", "text": { "content": "**Duration:** 4m 12s  |  **Cost:** $0.35  |  **Turns:** 12", "tag": "lark_md" } },
      { "tag": "hr" },
      { "tag": "action", "actions": [
        { "tag": "button", "text": { "content": "View Session", "tag": "plain_text" }, "type": "primary", "url": "..." }
      ]}
    ]
  }
}
```

Send this directly to a Lark bot webhook URL — no transformation needed.

### `slack`

Slack Block Kit:

```json
{
  "blocks": [
    { "type": "header", "text": { "type": "plain_text", "text": "Session Ended" } },
    { "type": "section", "fields": [
      { "type": "mrkdwn", "text": "*Project:* claude-view" },
      { "type": "mrkdwn", "text": "*Model:* Sonnet" },
      { "type": "mrkdwn", "text": "*Status:* Done" },
      { "type": "mrkdwn", "text": "*Duration:* 4m 12s" },
      { "type": "mrkdwn", "text": "*Cost:* $0.35" }
    ]},
    { "type": "actions", "elements": [
      { "type": "button", "text": { "type": "plain_text", "text": "View Session" }, "url": "..." }
    ]}
  ]
}
```

Send this directly to a Slack incoming webhook URL.

---

## Signature Verification

Webhooks are signed using HMAC-SHA256 following the [Standard Webhooks](https://www.standardwebhooks.com/) spec. Every delivery includes three headers:

| Header | Example | Description |
|--------|---------|-------------|
| `webhook-id` | `evt_a1b2c3d4e5f6g7h8` | Unique event ID |
| `webhook-timestamp` | `1775764333` | Unix timestamp |
| `webhook-signature` | `v1,K5oZfzN95Z2P...` | HMAC-SHA256 signature |

### How to Verify (Node.js example)

```javascript
const crypto = require('crypto');

function verifyWebhook(body, headers, signingSecret) {
  const webhookId = headers['webhook-id'];
  const timestamp = headers['webhook-timestamp'];
  const signature = headers['webhook-signature'];

  // 1. Strip whsec_ prefix and decode the key
  const keyBase64 = signingSecret.replace('whsec_', '');
  const key = Buffer.from(keyBase64, 'base64');

  // 2. Reconstruct signed content
  const signedContent = `${webhookId}.${timestamp}.${body}`;

  // 3. Compute HMAC-SHA256
  const computed = crypto
    .createHmac('sha256', key)
    .update(signedContent)
    .digest('base64');

  // 4. Compare
  const expected = signature.replace('v1,', '');
  return crypto.timingSafeEqual(
    Buffer.from(computed),
    Buffer.from(expected)
  );
}
```

### How to Verify (Python example)

```python
import hmac, hashlib, base64

def verify_webhook(body: str, headers: dict, signing_secret: str) -> bool:
    webhook_id = headers['webhook-id']
    timestamp = headers['webhook-timestamp']
    signature = headers['webhook-signature']

    # 1. Decode the key
    key = base64.b64decode(signing_secret.removeprefix('whsec_'))

    # 2. Reconstruct signed content
    signed_content = f"{webhook_id}.{timestamp}.{body}"

    # 3. Compute HMAC-SHA256
    computed = base64.b64encode(
        hmac.new(key, signed_content.encode(), hashlib.sha256).digest()
    ).decode()

    # 4. Compare
    expected = signature.removeprefix('v1,')
    return hmac.compare_digest(computed, expected)
```

---

## Delivery & Retry

- **Method:** HTTP POST
- **Content-Type:** `application/json`
- **User-Agent:** `claude-view/1.0`
- **Retry:** Up to 3 attempts on 5xx or network errors (100ms, 500ms backoff)
- **No retry on 4xx** — client errors are terminal
- **Async:** Deliveries run in background tasks and never block the event loop

---

## Config Files

All config lives under `~/.claude-view/`:

### `notifications.json`

```json
{
  "baseUrl": "https://your-instance.com",
  "webhooks": [
    {
      "id": "wh_a1b2c3d4e5f6",
      "name": "Slack Notifications",
      "url": "https://hooks.slack.com/services/T.../B.../xxx",
      "format": "slack",
      "events": ["session.started", "session.ended"],
      "enabled": true,
      "createdAt": "2026-04-10T10:00:00Z"
    }
  ]
}
```

- `baseUrl` — optional, used to build `webUrl` links in payloads
- Edit this file directly for hot-reload — changes take effect on the next event (no restart needed)

### `webhook-secrets.json`

```json
{
  "secrets": {
    "wh_a1b2c3d4e5f6": "whsec_dGVzdC13ZWJob29rLXNpZ25pbmcta2V5..."
  }
}
```

Maps webhook IDs to HMAC signing secrets. Managed automatically by the CRUD API.

### `api-keys.json`

```json
{
  "keys": [
    {
      "id": "key_Xk9mP2nR",
      "hash": "e3b0c44298fc1c149afbf4c8996fb924...",
      "prefix": "cv_live_7kPm",
      "createdAt": "2026-04-10T10:00:00Z"
    }
  ]
}
```

Stores SHA-256 hashes of API keys. The raw key is never stored.

---

## Architecture

```
                     ┌─────────────────────────────────┐
                     │      Live Session Manager        │
                     │  (file watcher, process detector)│
                     └──────────┬──────────────────────┘
                                │ broadcast::Sender<SessionEvent>
                                │
                   ┌────────────┼────────────┐
                   │            │            │
                   ▼            ▼            ▼
              SSE Stream   Webhook Engine   (other subscribers)
              /api/live/     │
              stream         │
                             ├── Event Mapper
                             │   SessionDiscovered → session.started
                             │   SessionClosed     → session.ended
                             │   SessionUpdated    → session.error + session.updated
                             │
                             ├── Debouncer (10s per webhook+session)
                             │
                             ├── Config Loader (hot-reload from disk)
                             │
                             ├── Formatter (raw / lark / slack)
                             │
                             └── HMAC Signer + HTTP Delivery (async, retry)
```

The webhook engine subscribes to the same `broadcast::Sender<SessionEvent>` that powers the SSE live stream. It runs as a background tokio task spawned at server startup.

---

## Recipes

### Lark Bot for Session Alerts

1. Create a Lark custom bot in your group chat
2. Copy the webhook URL (starts with `https://open.larksuite.com/open-apis/bot/v2/hook/`)
3. Create the webhook:

```bash
curl -X POST http://localhost:4173/api/webhooks \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Team Lark Bot",
    "url": "https://open.larksuite.com/open-apis/bot/v2/hook/YOUR_TOKEN",
    "format": "lark",
    "events": ["session.ended", "session.error"]
  }'
```

### Slack Channel Notifications

1. Create a Slack app with an incoming webhook
2. Copy the webhook URL
3. Create the webhook:

```bash
curl -X POST http://localhost:4173/api/webhooks \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Dev Channel",
    "url": "https://hooks.slack.com/services/T.../B.../YOUR_TOKEN",
    "format": "slack",
    "events": ["session.started", "session.ended", "session.error"]
  }'
```

### Custom Integration (Raw JSON)

For building your own dashboard or piping events to a data warehouse:

```bash
curl -X POST http://localhost:4173/api/webhooks \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Data Pipeline",
    "url": "https://your-api.com/ingest/claude-events",
    "format": "raw",
    "events": ["session.started", "session.ended", "session.updated", "session.error"]
  }'
```

### Pause a Webhook Without Deleting

```bash
curl -X PUT http://localhost:4173/api/webhooks/wh_a1b2c3d4e5f6 \
  -H "Content-Type: application/json" \
  -d '{ "enabled": false }'
```

Re-enable later with `{ "enabled": true }`.
