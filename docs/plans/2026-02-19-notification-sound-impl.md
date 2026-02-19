# Notification Sound Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Play an audible ding when any Mission Control session transitions to the "needs you" state (Stop, PermissionRequest events).

**Architecture:** Frontend-only. A new `useNotificationSound` hook detects `agentState.group` transitions in the sessions array from `useLiveSessions()`, plays Web Audio API synthesized tones, persists settings in localStorage. A Radix Popover in the header provides enable/disable toggle, volume slider, sound picker, and preview.

**Tech Stack:** React, Web Audio API, Radix Popover (`@radix-ui/react-popover` — already installed), Lucide icons, localStorage.

**Design doc:** `docs/plans/2026-02-19-notification-sound-design.md`

---

### Task 1: Create the notification sound hook

**Files:**
- Create: `src/hooks/use-notification-sound.ts`

**Step 1: Create the hook with settings management and sound synthesis**

Create `src/hooks/use-notification-sound.ts` with these responsibilities:

1. **Settings persistence** — Read/write `NotificationSoundSettings` from localStorage key `claude-view:notification-sound`. Defaults: `{ enabled: true, volume: 0.7, sound: 'ding' }`.

2. **Transition detection** — Maintain a `useRef<Map<string, string>>` mapping `sessionId → previousGroup`. On each `sessions` change, compare current `agentState.group` against previous. If transition is `previous !== 'needs_you' && current === 'needs_you'` → trigger ding. Skip sessions not in the previous map (initial discovery — no ding).

3. **Sound engine** — Three Web Audio API presets:

   | Preset | Waveform | Frequency | Duration | Decay |
   |--------|----------|-----------|----------|-------|
   | `ding` | sine | 800Hz | 150ms | exponentialRampToValueAtTime(0.001) |
   | `chime` | triangle | 600Hz, then 900Hz at +100ms | 250ms total | same |
   | `bell` | sine | 1200Hz | 300ms | same |

   Each preset creates an `OscillatorNode` → `GainNode` → `audioContext.destination`. Gain starts at `settings.volume`, ramps to 0.001 over the duration.

4. **AudioContext lifecycle** — Create lazily. Check `audioContext.state === 'suspended'` and call `resume()` on play. Expose `audioUnlocked` boolean.

5. **Debounce** — If multiple sessions transition simultaneously (e.g., 3 agents finish at once), play only one ding. Use a simple `lastPlayTime` ref with a 500ms cooldown.

```typescript
// Types
export type SoundPreset = 'ding' | 'chime' | 'bell'

export interface NotificationSoundSettings {
  enabled: boolean
  volume: number  // 0.0 - 1.0
  sound: SoundPreset
}

const STORAGE_KEY = 'claude-view:notification-sound'

const DEFAULT_SETTINGS: NotificationSoundSettings = {
  enabled: true,
  volume: 0.7,
  sound: 'ding',
}

export interface UseNotificationSoundResult {
  settings: NotificationSoundSettings
  updateSettings: (patch: Partial<NotificationSoundSettings>) => void
  previewSound: () => void
  audioUnlocked: boolean
}

// Hook signature:
export function useNotificationSound(sessions: LiveSession[]): UseNotificationSoundResult
```

Import `LiveSession` from `../components/live/use-live-sessions`.

**Step 2: Verify it compiles**

Run: `npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: No errors in `use-notification-sound.ts`.

**Step 3: Commit**

```bash
git add src/hooks/use-notification-sound.ts
git commit -m "feat: add useNotificationSound hook with Web Audio synth and transition detection"
```

---

### Task 2: Create the NotificationSoundPopover component

**Files:**
- Create: `src/components/live/NotificationSoundPopover.tsx`

**Step 1: Build the popover component**

Create `src/components/live/NotificationSoundPopover.tsx` using the existing Radix Popover pattern from `src/components/ui/DateRangePicker.tsx`:

```typescript
import * as Popover from '@radix-ui/react-popover'
import { BellRing, BellOff, Play } from 'lucide-react'
import type { NotificationSoundSettings, SoundPreset } from '../../hooks/use-notification-sound'
```

**Props:**

```typescript
interface NotificationSoundPopoverProps {
  settings: NotificationSoundSettings
  onSettingsChange: (patch: Partial<NotificationSoundSettings>) => void
  onPreview: () => void
  audioUnlocked: boolean
}
```

**Structure:**

```
Popover.Root
  Popover.Trigger → bell icon button (BellRing if enabled, BellOff if disabled)
  Popover.Portal
    Popover.Content (align="end", sideOffset={8})
      ├── Header: "Notification Sound"
      ├── Toggle row: label "Sound" + toggle switch (simple button with on/off styling)
      ├── Volume row: label "Volume" + <input type="range" min=0 max=100 step=1>
      │   (disabled when sound is off)
      ├── Sound picker: 3 radio-style buttons for ding/chime/bell
      │   (disabled when sound is off)
      └── Preview button: Play icon + "Preview"
          (always enabled — also serves as AudioContext unlock gesture)
```

**Styling rules (from CLAUDE.md):**
- Use explicit Tailwind colors with `dark:` variants — NO `text-muted-foreground` or shadcn CSS variable classes
- `cursor-pointer` on all clickable elements
- Transitions: `transition-colors duration-150`
- Bell icon: `w-5 h-5`, button has `p-1.5 rounded-md hover:bg-gray-200 dark:hover:bg-gray-700`
- Popover content: `w-64 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 shadow-lg p-4`

**Toggle switch** — a styled `<button>` with `role="switch"` and `aria-checked`:
- ON: `bg-indigo-500` pill with white circle right
- OFF: `bg-gray-300 dark:bg-gray-600` pill with white circle left
- Size: `w-9 h-5`

**Volume slider** — native `<input type="range">` with accent-color styling:
- `accent-indigo-500 w-full h-1`
- Show percentage label next to it

**Sound picker** — 3 buttons, active one has `ring-2 ring-indigo-500 bg-indigo-50 dark:bg-indigo-900/30`:

```
[♪ Ding]  [♪ Chime]  [♪ Bell]
```

**Preview button** — full-width at bottom:
- `flex items-center justify-center gap-2 w-full py-1.5 rounded-md text-sm bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 cursor-pointer`

**Step 2: Verify it compiles**

Run: `npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: No errors.

**Step 3: Commit**

```bash
git add src/components/live/NotificationSoundPopover.tsx
git commit -m "feat: add NotificationSoundPopover with toggle, volume, sound picker, preview"
```

---

### Task 3: Wire into MissionControlPage

**Files:**
- Modify: `src/pages/MissionControlPage.tsx`

**Step 1: Add imports and hook call**

At the top of `MissionControlPage.tsx`, add imports:

```typescript
import { useNotificationSound } from '../hooks/use-notification-sound'
import { NotificationSoundPopover } from '../components/live/NotificationSoundPopover'
```

Inside `MissionControlPage()`, after the `useLiveSessions()` call (line 34), add:

```typescript
const { settings: soundSettings, updateSettings: updateSoundSettings, previewSound, audioUnlocked } = useNotificationSound(sessions)
```

**Step 2: Add popover to header**

In the header JSX (line 138-156), insert the popover between the `ViewModeSwitcher` and the connection status `div`. The new header structure:

```tsx
<div className="flex items-center justify-between">
  <div className="flex items-center gap-4">
    <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
      Mission Control
    </h1>
    <ViewModeSwitcher mode={viewMode} onChange={handleViewModeChange} />
  </div>
  <div className="flex items-center gap-3">
    <NotificationSoundPopover
      settings={soundSettings}
      onSettingsChange={updateSoundSettings}
      onPreview={previewSound}
      audioUnlocked={audioUnlocked}
    />
    <div className="flex items-center gap-2 text-xs text-gray-400 dark:text-gray-500">
      <span
        className={`inline-block h-2 w-2 rounded-full ${isConnected ? 'bg-green-500' : 'bg-red-500'}`}
      />
      {isConnected ? 'Live' : 'Reconnecting...'}
      {lastUpdate && (
        <span className="ml-2">
          Updated {formatRelativeTime(lastUpdate)}
        </span>
      )}
    </div>
  </div>
</div>
```

Key change: the right side of the header wraps the popover + connection status in a `flex items-center gap-3` container.

**Step 3: Verify it compiles and renders**

Run: `npx tsc --noEmit --pretty 2>&1 | head -20`
Expected: No errors.

**Step 4: Manual test**

1. Open Mission Control in browser
2. Bell icon should appear in header between view mode switcher and "Live" indicator
3. Click bell icon → popover opens with toggle (ON), volume slider, 3 sound presets, preview button
4. Click "Preview" → should hear a ding tone
5. Toggle OFF → bell icon changes to BellOff, sound disabled
6. Refresh page → settings should persist (localStorage)
7. If a session transitions to needs_you → ding should play (if sound ON)

**Step 5: Commit**

```bash
git add src/pages/MissionControlPage.tsx
git commit -m "feat: wire notification sound into Mission Control header"
```

---

### Task 4: End-to-end verification (wiring checklist)

Per CLAUDE.md "Wiring-Up Checklist (MANDATORY)":

**Step 1: Verify the hook is actually called**

Check: `useNotificationSound(sessions)` is called in `MissionControlPage` with the real `sessions` array from `useLiveSessions()`.

**Step 2: Verify settings flow end-to-end**

1. Open popover → change volume to 30% → close popover → reopen → volume should still be 30%
2. Open popover → switch to "Chime" → click Preview → should hear chime (not ding)
3. Refresh browser → settings should persist

**Step 3: Verify transition detection**

The real test: start a Claude Code session in terminal while Mission Control is open. When the agent finishes (Stop event), the ding should play. If you can't trigger a real session, verify with console:

```javascript
// In browser console, check that previousGroups ref is being populated:
// The hook should be tracking session IDs in its ref map
```

**Step 4: Verify no ding on page load**

Open Mission Control with active sessions already running. There should be NO burst of dings on initial load — only on subsequent transitions.

**Step 5: Final commit (if any fixes needed)**

```bash
git add -A
git commit -m "fix: notification sound wiring fixes from e2e verification"
```
