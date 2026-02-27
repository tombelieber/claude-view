# ChatInputBar + Interactive Message Cards — Design

> **Date:** 2026-02-28
> **Status:** Approved
> **Scope:** Shared chat input component + interactive message cards for Mission Control
> **Approach:** Hybrid — plain textarea + Radix UI popovers (matches Claude Code VSCode extension architecture)

---

## 1. Motivation

Phase F shipped a basic `DashboardChat.tsx` with a plain `<textarea>` and send button. The Claude Code VSCode extension has a significantly richer input experience that our target users already know and love. This design upgrades the chat input to match (and exceed) the VSCode extension, while also adding interactive message cards that were missing.

**Vision alignment:** The chat input IS the product for Stage 3 (Mission Control) and Stage 4 (clawmini). The "What do you want to build?" prompt in the vision doc is literally this component.

---

## 2. Scope

### In scope (V1)

**ChatInputBar** — shared input component:
- Auto-growing textarea (plain `<textarea>`, auto-grow via `scrollHeight`)
- Slash command popover (Radix UI, full Claude Code command list)
- File/image attachments (file picker + clipboard paste)
- Mode switch badge (plan/code/ask)
- Model selector chip (Opus/Sonnet/Haiku) — **claude-view addition**
- Context gauge (% used, color-coded bar) — **claude-view addition**
- Cost estimate preview (~$X cached) — **claude-view addition**

**Interactive message cards** — inline in the chat stream:
- `AskUserQuestionCard` — multiple choice with options, optional markdown preview
- `PermissionCard` — refactored from existing `PermissionDialog.tsx` to also render inline
- `PlanApprovalCard` — rendered plan markdown + Approve/Reject
- `ElicitationCard` — generic dialog prompt (text input + submit)

### Out of scope (V1)
- Current file reference toggle (no open file concept in web dashboard)
- Project/session picker (future)
- Todo list rendering from `todos` field (optional, deferred)
- Full message stream redesign (keep existing rendering, just add input bar + cards)
- Mobile version (shared types via `@claude-view/shared` enable this later)

---

## 3. Component Architecture

### 3.1 ChatInputBar

```
<ChatInputBar>
├── <InputChrome>                 ← border/container, rounded, dark theme
│   ├── <TopBar>                  ← flex row
│   │   ├── <ModeSwitch>          ← plan/code/ask badge (left)
│   │   └── <ModelSelector>       ← model chip (right) [NEW]
│   ├── <TextArea>                ← auto-growing plain textarea (center)
│   ├── <BottomBar>               ← flex row
│   │   ├── <AttachButton>        ← paperclip icon, file picker
│   │   ├── <ContextGauge>        ← % bar [NEW]
│   │   ├── <CostPreview>         ← "~$0.15" [NEW]
│   │   └── <SendButton>          ← send/stop toggle
│   └── <SlashCommandPopover>     ← Radix Popover, above textarea
└── (data) commands.ts            ← slash command definitions
```

**File location:** `apps/web/src/components/chat/ChatInputBar.tsx`

### 3.2 Interactive Message Cards

```
apps/web/src/components/chat/cards/
├── AskUserQuestionCard.tsx       ← multiple choice question
├── PermissionCard.tsx            ← tool approval (refactored from PermissionDialog)
├── PlanApprovalCard.tsx          ← plan approval
└── ElicitationCard.tsx           ← generic dialog prompt
```

### 3.3 Full File Structure

```
apps/web/src/components/chat/
├── ChatInputBar.tsx              ← main shared input component
├── ModeSwitch.tsx                ← plan/code/ask dropdown badge
├── ModelSelector.tsx             ← model picker chip
├── SlashCommandPopover.tsx       ← "/" command menu
├── ContextGauge.tsx              ← context % bar
├── CostPreview.tsx               ← estimated cost chip
├── AttachButton.tsx              ← file/image picker
├── commands.ts                   ← slash command definitions
└── cards/
    ├── AskUserQuestionCard.tsx
    ├── PermissionCard.tsx
    ├── PlanApprovalCard.tsx
    └── ElicitationCard.tsx
```

---

## 4. Props Interface

```ts
interface ChatInputBarProps {
  // Core
  onSend: (message: string, options?: SendOptions) => void
  onStop?: () => void
  isStreaming?: boolean
  disabled?: boolean
  placeholder?: string

  // Mode
  mode?: 'plan' | 'code' | 'ask'
  onModeChange?: (mode: string) => void

  // Model [NEW]
  model?: string
  onModelChange?: (model: string) => void
  availableModels?: { id: string; label: string }[]

  // Context gauge [NEW]
  contextPercent?: number  // 0-100

  // Cost preview [NEW]
  estimatedCost?: { cached: number; uncached: number } | null

  // Slash commands
  commands?: SlashCommand[]
  onCommand?: (command: string) => void

  // Attachments
  onAttach?: (files: File[]) => void
}

interface SendOptions {
  mode?: string
  model?: string
  attachments?: File[]
}

interface SlashCommand {
  name: string           // e.g. "plan"
  description: string    // e.g. "Enter plan mode"
  category?: string      // grouping for the popover
}
```

---

## 5. Sub-component Specifications

### 5.1 ModeSwitch

- **Library:** Radix `DropdownMenu`
- **Options:** plan (amber), code (green), ask (blue)
- **Visual:** pill badge with label + chevron: `[plan ▾]`
- **Behavior:** Click opens dropdown, selection changes mode
- **Keyboard:** Enter/Space opens, arrow keys navigate, Escape closes

### 5.2 ModelSelector [NEW]

- **Library:** Radix `DropdownMenu`
- **Data source:** `availableModels` prop (populated from API or hardcoded initial list)
- **Visual:** chip showing current model: `[opus 4.6 ▾]`
- **Behavior:** Disabled when streaming. Selection calls `onModelChange`
- **Default models:** opus 4.6, sonnet 4.6, haiku 4.5

### 5.3 TextArea (auto-grow)

- **Element:** Plain `<textarea>` with `rows={1}`
- **Auto-grow:** On `input` event, set `style.height = 'auto'` then `style.height = scrollHeight + 'px'`
- **Min height:** 1 line (~40px)
- **Max height:** ~200px (then overflow scrolls)
- **Keybindings:**
  - `Enter` → send message
  - `Shift+Enter` → newline
  - `Escape` → stop streaming (if active)
  - `/` at position 0 → open slash command popover
- **Placeholder:** "Send a message... (or type / for commands)"
- **Accessibility:** `aria-label="Chat message"`, `focus:ring-2 focus:ring-blue-500`

### 5.4 SlashCommandPopover

- **Library:** Radix `Popover`
- **Trigger:** Input starts with `/`
- **Position:** Above textarea (side="top", align="start")
- **Filtering:** Fuzzy match as user types (e.g., `/com` → `/commit`, `/compact`)
- **Navigation:** Up/Down arrow keys, Enter to select, Escape to close
- **Items:** Icon + name + description (mirrors Claude Code VSCode extension list)
- **Max visible:** ~8 items, scrollable
- **Selection behavior:** Replaces input with command, optionally triggers `onCommand`

### 5.5 AttachButton

- **Icon:** Lucide `Paperclip`
- **Accept:** `image/*,.txt,.md,.json,.ts,.tsx,.js,.jsx,.rs,.py,.css,.html`
- **Clipboard:** Handle paste event for images (DataTransfer API)
- **Display:** Attachment chips appear below the textarea when files are added
- **Chip:** filename + size + remove button

### 5.6 ContextGauge [NEW]

- **Visual:** Horizontal bar with label: "42%"
- **Colors:** blue < 60%, amber 60-80%, red > 80%
- **Width:** ~100px
- **Reuses:** Color thresholds from existing `ChatStatusBar.tsx`
- **Tooltip:** "Context window: 42% used (85K / 200K tokens)"

### 5.7 CostPreview [NEW]

- **Visual:** Text chip: "~$0.15"
- **Tooltip:** "~$0.15 cached / ~$1.50 uncached"
- **Data source:** `estimatedCost` prop (from `/api/control/estimate` API)
- **Updates:** When model changes (re-calls estimate API)
- **Visibility:** Hidden when `estimatedCost` is null

### 5.8 SendButton

- **Idle state:** Arrow-up icon, active when input non-empty
- **Streaming state:** Square (stop) icon, always active
- **Disabled:** When input empty and not streaming
- **Keyboard:** Enter triggers send (handled by textarea keybinding)

---

## 6. Interactive Message Cards

### 6.1 AskUserQuestionCard

Renders when Claude uses the `AskUserQuestion` tool. Displayed inline in the message stream.

**Layout:**
```
┌─ [header badge] ──────────────────────────────────────────┐
│  Question text here?                                       │
│                                                            │
│  [● Option A] Label                                        │
│     Description text explaining this option                │
│                                                            │
│  [○ Option B] Label                                        │
│     Description text explaining this option                │
│                                                            │
│  [○ Option C] Label                                        │
│     Description text explaining this option                │
│                                                            │
│                                            [Submit]        │
└────────────────────────────────────────────────────────────┘
```

**With markdown preview (side-by-side):**
```
┌────────────────────────────────────────────────────────────┐
│  Question text?                                            │
│  ┌──────────────┐  ┌────────────────────────────────────┐ │
│  │ [●] Option A │  │ ```                                │ │
│  │ [○] Option B │  │ Preview content for                │ │
│  │ [○] Option C │  │ selected option                    │ │
│  └──────────────┘  │ ```                                │ │
│                     └────────────────────────────────────┘ │
│                                            [Submit]        │
└────────────────────────────────────────────────────────────┘
```

**Props:**
```ts
interface AskUserQuestionCardProps {
  questions: {
    question: string
    header: string
    options: { label: string; description: string; markdown?: string }[]
    multiSelect: boolean
  }[]
  onAnswer: (answers: Record<string, string>) => void
  answered?: boolean  // grays out after answering
  selectedAnswers?: Record<string, string>
}
```

**Behavior:**
- Single-select: radio buttons, click to select
- Multi-select: checkboxes
- Markdown preview: shown when option has `markdown` field, updates on focus
- After submission: card grays out, shows selected answer(s)
- Always shows "Other" option (free text input)

### 6.2 PermissionCard

Refactored from existing `PermissionDialog.tsx`. Renders both as modal (existing) AND inline in stream.

**Layout:**
```
┌─ Permission Request ───────────────────────────────────────┐
│  [Bash] rm -rf node_modules && npm install                 │
│                                                            │
│  ┌──────────────────────────────────────────────────────┐ │
│  │ $ rm -rf node_modules && npm install                  │ │
│  └──────────────────────────────────────────────────────┘ │
│                                                            │
│  [████████░░] 45s                   [Deny]  [Allow]        │
└────────────────────────────────────────────────────────────┘
```

**Behavior:**
- Countdown timer (configurable timeout, default 60s)
- Auto-deny on timeout
- Tool-specific previews: Bash → command, Edit → file diff, Write → file path
- After decision: card shows "Allowed" (green) or "Denied" (red) badge
- Calls `respondPermission(requestId, allowed)` via WebSocket

### 6.3 PlanApprovalCard

Renders when Claude calls `ExitPlanMode`. Shows the plan and asks for approval.

**Layout:**
```
┌─ Plan Ready ───────────────────────────────────────────────┐
│  [Rendered plan markdown content...]                       │
│                                                            │
│  ## Implementation Steps                                   │
│  1. Create the component...                                │
│  2. Add the route...                                       │
│  3. Wire up the store...                                   │
│                                                            │
│                        [Request Changes]  [Approve Plan]   │
└────────────────────────────────────────────────────────────┘
```

**Behavior:**
- Plan content rendered as markdown (using existing markdown renderer)
- "Approve Plan" sends approval, allowing Claude to proceed
- "Request Changes" opens a text input for feedback
- After decision: card shows "Approved" or "Changes Requested" badge

### 6.4 ElicitationCard

Renders for `elicitation_dialog` notification type. Generic dialog prompt.

**Layout:**
```
┌─ Input Needed ─────────────────────────────────────────────┐
│  [Dialog prompt text]                                      │
│                                                            │
│  ┌──────────────────────────────────────────────────────┐ │
│  │ Your response...                                      │ │
│  └──────────────────────────────────────────────────────┘ │
│                                                [Submit]    │
└────────────────────────────────────────────────────────────┘
```

**Behavior:**
- Text input for user response
- Submit sends response via WebSocket
- After submission: card shows submitted text, grayed out

---

## 7. Integration Points

### 7.1 In DashboardChat.tsx (existing resume flow)

Replace the plain `<textarea>` + send button at the bottom with `<ChatInputBar>`:
- `onSend` → existing `sendMessage()` from `useControlSession`
- `contextPercent` → from `SessionStatusMsg.contextUsage`
- `estimatedCost` → from `/api/control/estimate` API (already exists)
- `mode` / `onModeChange` → new state (or forward to session via `/` command)
- `model` / `onModelChange` → new state (may require backend changes for model switching)

Add interactive cards in the message stream:
- When `PermissionRequestMsg` arrives → render `<PermissionCard>` inline
- When `tool_use_start` with `toolName === "AskUserQuestion"` → render `<AskUserQuestionCard>`
- When `tool_use_start` with `toolName === "ExitPlanMode"` → render `<PlanApprovalCard>`
- When `session_status` with `status === "waiting_input"` from elicitation → render `<ElicitationCard>`

### 7.2 WebSocket Protocol Extensions

The current `ServerMessage` union (control.ts) needs new message types for interactive cards:

```ts
// New message types to add
interface AskUserQuestionMsg {
  type: 'ask_user_question'
  requestId: string
  questions: { question: string; header: string; options: {...}[]; multiSelect: boolean }[]
}

interface PlanApprovalMsg {
  type: 'plan_approval'
  requestId: string
  planContent: string  // markdown
}

interface ElicitationMsg {
  type: 'elicitation'
  requestId: string
  prompt: string
}
```

These require sidecar updates to forward Agent SDK events → WebSocket. Currently only `permission_request` flows through the sidecar.

### 7.3 Future "Start Session" page

Same `<ChatInputBar>` but:
- `onSend` triggers session creation flow (calls `/api/control/start` or similar)
- No `contextPercent` (new session = 0%)
- `estimatedCost` shows first-message estimate
- Mode defaults to "code"

---

## 8. UX Audit Compliance

| Rule | Implementation |
|------|---------------|
| Focus states | `focus:ring-2 focus:ring-blue-500` on textarea and all buttons |
| Keyboard nav | Tab order: mode → model → textarea → attach → send. Popover: arrow keys |
| Touch targets | All buttons min 44x44px |
| Cursor pointer | All clickable elements have `cursor-pointer` |
| Transitions | 150ms for dropdowns, 200ms for gauge color transitions |
| No emoji icons | Lucide SVGs throughout |
| prefers-reduced-motion | Auto-grow uses `motion-reduce:transition-none` |
| ARIA | `aria-label` on textarea, `aria-expanded` on popovers, `role="listbox"` on command list |
| Color contrast | 4.5:1 minimum for all text on dark background |
| Error feedback | Clear error messages near problem (cost estimate failure, attachment error) |

---

## 9. Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Input type | Plain `<textarea>` | Claude Code VSCode extension uses plain textarea. No rich text needed for LLM input |
| Overlay library | Radix UI | Already in the project. Consistent with existing overlays |
| Slash command filtering | Simple substring match | Fuzzy match is over-engineering for ~50 commands. Exact + prefix covers 99% of use |
| Auto-grow technique | `scrollHeight` on input event | Standard technique, 3 lines of code, no library needed |
| Image paste | `DataTransfer` API on paste event | Native browser API, no library needed |
| New dependencies | None | Radix + Lucide already in project. Zero new deps |
| Cards vs modals | Inline cards (primary) + modal fallback | Cards provide context in the stream. Modal for urgent interrupts (permission with countdown) |

---

## 10. Slash Commands (V1)

Mirroring the Claude Code VSCode extension command list:

| Command | Description | Category |
|---------|------------|----------|
| `/plan` | Enter plan mode | Mode |
| `/code` | Enter code mode | Mode |
| `/ask` | Enter ask mode | Mode |
| `/compact` | Compact conversation context | Session |
| `/review` | Review recent changes | Action |
| `/commit` | Create a git commit | Action |
| `/test` | Run tests | Action |
| `/help` | Get help | Info |
| `/cost` | Show session cost breakdown | Info (claude-view) |
| `/status` | Show session status | Info (claude-view) |

**Extensible:** The `commands.ts` file exports the list. Adding new commands = adding entries.

---

## 11. Migration Path

1. Build `ChatInputBar` as a new shared component (no changes to existing code)
2. Build interactive cards in `chat/cards/` (no changes to existing code)
3. Replace `DashboardChat.tsx` textarea with `<ChatInputBar>` (single integration point)
4. Add card rendering to `DashboardChat.tsx` message stream
5. Extend sidecar WebSocket protocol for new interactive message types
6. Future: Create "Start Session" page reusing `<ChatInputBar>`
