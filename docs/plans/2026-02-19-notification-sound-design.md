---
status: approved
date: 2026-02-19
---

# Notification Sound on "Needs You" State Change

Play an audible ding when any Mission Control session transitions to the "needs you" state, so users can multitask without watching the dashboard.

## Trigger Events

The ding fires when a session's `agentState.group` transitions from a non-`needs_you` value to `needs_you`. This is driven by these Claude Code hook events:

| Hook Event | Resulting State | Group |
|---|---|---|
| `Stop` | `idle` | NeedsYou |
| `PermissionRequest` | `needs_permission` | NeedsYou |
| `Notification(idle_prompt)` | `idle` | NeedsYou |
| `Notification(permission_prompt)` | `needs_permission` | NeedsYou |

The ding does NOT fire on:
- Initial session discovery (only transitions)
- Re-entering `needs_you` when already in `needs_you` (no duplicate dings)
- `TaskCompleted`, `SessionEnd`, or other non-trigger events

## Architecture: Frontend-Only

Zero backend changes. The frontend already receives all session state via SSE `session_updated` events with full `agentState`. Detection and playback happen entirely in a new React hook.

```
SSE session_updated event
  → useLiveSessions() updates sessions state
  → useNotificationSound(sessions) detects group transition
  → Web Audio API plays ding
```

## Sound Engine

Web Audio API synthesizes tones programmatically — no audio files needed.

### Presets

| Name | Waveform | Frequency | Duration | Character |
|---|---|---|---|---|
| Ding (default) | sine | 800Hz | 150ms | Clean, attention-getting |
| Chime | triangle | 600Hz + 900Hz | 250ms | Softer, two-tone |
| Bell | sine | 1200Hz | 300ms | Bright, sustained |

Each preset uses an `OscillatorNode` → `GainNode` with exponential decay ramp to zero.

### Browser Autoplay Policy

`AudioContext` requires a user gesture before it can play. Strategy:
- Create `AudioContext` lazily on first user interaction (click/tap)
- The settings popover "Preview" button doubles as a gesture unlock
- If context is suspended, the bell icon shows a subtle dot indicator

## Settings

Stored in `localStorage` under key `claude-view:notification-sound`.

```typescript
interface NotificationSoundSettings {
  enabled: boolean           // default: true
  volume: number             // 0.0 - 1.0, default: 0.7
  sound: 'ding' | 'chime' | 'bell'  // default: 'ding'
}
```

## UI: Header Popover

A bell icon in the Mission Control header bar, right side, between the view mode switcher and the connection status indicator.

```
┌─────────────────────────────────────────────────────────────┐
│ Mission Control  [Grid][List][Kanban][Monitor]  🔔  ● Live  │
└─────────────────────────────────────────────────────────────┘
```

Click the bell icon to open a Radix Popover:

```
┌─────────────────────┐
│  Notification Sound  │
│  [═══════ON/OFF═══] │
│                      │
│  Volume  ━━━━━━○    │
│                      │
│  Sound               │
│  ● Ding              │
│  ○ Chime             │
│  ○ Bell              │
│                      │
│  [▶ Preview]         │
└─────────────────────┘
```

Components:
- **Toggle switch** — enables/disables sound globally
- **Volume slider** — 0% to 100%
- **Sound picker** — radio buttons for preset selection
- **Preview button** — plays the selected sound at current volume (also unlocks AudioContext)

Bell icon states:
- **Filled** (sound enabled) — `BellRing` from Lucide
- **Outline with slash** (sound disabled) — `BellOff` from Lucide

## Files

| File | Action | Purpose |
|---|---|---|
| `src/hooks/use-notification-sound.ts` | Create | Transition detection + Web Audio playback + settings persistence |
| `src/components/live/NotificationSoundPopover.tsx` | Create | Bell icon + Radix popover with toggle, volume, sound picker, preview |
| `src/pages/MissionControlPage.tsx` | Modify | Wire `useNotificationSound(sessions)` + render popover in header |

## Hook API

```typescript
// use-notification-sound.ts
interface UseNotificationSoundOptions {
  sessions: LiveSession[]
}

interface UseNotificationSoundResult {
  settings: NotificationSoundSettings
  updateSettings: (patch: Partial<NotificationSoundSettings>) => void
  previewSound: () => void
  audioUnlocked: boolean
}

function useNotificationSound(options: UseNotificationSoundOptions): UseNotificationSoundResult
```

The hook:
1. Reads settings from localStorage on mount
2. Maintains a `useRef<Map<string, string>>` of `sessionId → previousGroup`
3. On each `sessions` change, iterates and compares groups
4. If any session transitioned TO `needs_you` AND `settings.enabled` → play sound
5. Exposes `previewSound()` for the Preview button
6. Exposes `updateSettings()` which writes to localStorage and updates internal state

## Design Decisions

- **Frontend-only** — server already provides all needed data via SSE, no reason to add backend complexity
- **Web Audio API** — zero asset management, works offline, customizable
- **Radix Popover** — per project convention (CLAUDE.md: prefer Radix primitives)
- **localStorage** — consistent with existing settings pattern (view mode, banner state)
- **No ding on discovery** — prevents a burst of dings when first connecting with multiple active sessions
