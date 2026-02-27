# ChatInputBar + Interactive Message Cards — Design

> **Date:** 2026-02-28
> **Status:** Approved (v2 — monitor-integrated)
> **Scope:** Chat input + interactive cards integrated into existing monitor panels
> **Approach:** Hybrid — plain textarea + Radix UI popovers, embedded in MonitorPane

---

## 1. Motivation

The monitor view already has dockview panels with RichPane rendering messages for every session. Users can arrange panels however they want (tabs, splits, stacks). Instead of building a separate chat page, we add input and interactivity directly into these existing panels.

**Key insight:** No new pages. No new chat UI. Just add input + interactive cards to the panels users already use. Dockview gives them workspace mode for free.

**What gets deleted:** `DashboardChat.tsx` (261 lines, duplicate renderers), `ControlPage.tsx` route.

**Vision alignment:** The chat input IS the product for Stage 3 (Mission Control) and Stage 4 (clawmini). The "What do you want to build?" prompt in the vision doc is literally this component.

---

## 2. Scope

### In scope (V1)

**ChatInputBar** — added to existing MonitorPane panels:

- Auto-growing textarea (plain `<textarea>`, auto-grow via `scrollHeight`)
- Slash command popover (Radix UI, full Claude Code command list)
- File/image attachments (file picker + clipboard paste)
- Mode switch badge (plan/code/ask)
- Model selector chip (Opus/Sonnet/Haiku) — **claude-view addition**
- Context gauge (% used, color-coded bar) — **claude-view addition**
- Cost estimate preview (~$X cached) — **claude-view addition**

**Interactive message cards** — inline in RichPane message stream:

- `InteractiveCardShell` — shared wrapper (header + content + action bar + resolved state)
- Evolve existing `AskUserQuestionDisplay` → interactive (add click handlers + submit)
- `PermissionCard` — inline version (refactored from existing `PermissionDialog.tsx`)
- `PlanApprovalCard` — plan markdown + Approve/Reject
- `ElicitationCard` — generic dialog prompt (text input + submit)

**Cleanup** — delete duplicate code:

- Delete `DashboardChat.tsx` (261 lines — duplicate renderers)
- Delete or redirect `ControlPage.tsx` route

### Out of scope (V1)

- Current file reference toggle (no open file concept in web dashboard)
- New "Start Session" page (future — same ChatInputBar, different wiring)
- Todo list rendering from `todos` field (optional, deferred)
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

```text
apps/web/src/components/chat/cards/
├── InteractiveCardShell.tsx      ← shared wrapper (header + actions + resolved state)
├── AskUserQuestionCard.tsx       ← evolve existing AskUserQuestionDisplay → interactive
├── PermissionCard.tsx            ← inline version (refactored from PermissionDialog)
├── PlanApprovalCard.tsx          ← plan approval
└── ElicitationCard.tsx           ← generic dialog prompt
```

### 3.3 Full File Structure

```text
apps/web/src/components/chat/
├── ChatInputBar.tsx              ← main input component (embedded in MonitorPane)
├── ModeSwitch.tsx                ← plan/code/ask dropdown badge
├── ModelSelector.tsx             ← model picker chip
├── SlashCommandPopover.tsx       ← "/" command menu
├── ContextGauge.tsx              ← context % bar
├── CostPreview.tsx               ← estimated cost chip
├── AttachButton.tsx              ← file/image picker
├── commands.ts                   ← slash command definitions
└── cards/
    ├── InteractiveCardShell.tsx   ← shared card wrapper
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

### 6.0 InteractiveCardShell (shared wrapper)

All 4 card types share a common shell for consistent visual language.

**Layout:**

```text
┌─ [icon] TYPE BADGE ──── color-coded header ──────────────┐
│                                                           │
│  [card-specific content area — children]                  │
│                                                           │
│  ─────────────────────── action bar ──────────────────── │
│                              [Secondary]  [Primary]       │
└───────────────────────────────────────────────────────────┘
```

**After resolution:**

```text
┌─ [icon] TYPE BADGE ──── grayed header ── [✓ Approved] ──┐
│                                                           │
│  [content, opacity-60, non-interactive]                   │
│                                                           │
└───────────────────────────────────────────────────────────┘
```

**Props:**

```ts
interface InteractiveCardShellProps {
  variant: 'question' | 'permission' | 'plan' | 'elicitation'
  header: string
  icon?: React.ReactNode
  resolved?: { label: string; variant: 'success' | 'denied' | 'neutral' }
  children: React.ReactNode
  actions?: React.ReactNode
}
```

**Color system** (extends `AskUserQuestionDisplay.COLORS` pattern):

| Variant | Border | Header BG | Icon color |
| --- | --- | --- | --- |
| question | purple | purple-900/20 | purple-400 |
| permission | amber | amber-900/20 | amber-400 |
| plan | blue | blue-900/20 | blue-400 |
| elicitation | gray | gray-800/30 | gray-400 |

**Shared behaviors:**

- Resolved state: `opacity-60` on entire card, `pointer-events-none` on content + actions, result badge in header
- Transition: `transition-opacity duration-200`
- Border radius: `rounded-lg`
- `prefers-reduced-motion`: respects via `motion-reduce:transition-none`

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

### 7.1 Monitor-Integrated: MonitorPane + RichPane + ChatInputBar

**Key decision:** No new pages. Add ChatInputBar to existing MonitorPane. Interactive cards render inline in existing RichPane. Users arrange panels however they want via dockview.

**Current architecture (read-only monitoring):**

```text
MonitorPane
├── Header (project name, branch, cost, context %, status)
├── Content: RichTerminalPane → RichPane (message stream)
└── Footer (activity, turn count, sub-agents)
```

**New architecture (monitoring + interaction):**

```text
MonitorPane
├── Header (unchanged)
├── Content: RichTerminalPane → RichPane (messages + interactive cards inline)
├── ChatInputBar (conditional — only when session has control connection)
└── Footer (unchanged)
```

**How messages flow:**

- **Reading (existing, no changes):** SSE stream → `useTerminalSocket` → `RichPane` renders messages
- **Writing (new):** `ChatInputBar` → `useControlSession.sendMessage()` → control WebSocket → sidecar → Claude
- **Response appears naturally:** sidecar writes to JSONL → SSE picks up new messages → RichPane renders them
- **Interactive cards (new):** RichPane detects interactive message types → renders card instead of normal bubble

**When does ChatInputBar appear?**

A session panel shows the input bar when:

1. Session has a control connection (sidecar is running, `controlId` exists)
2. Session status is not `completed` or `error`

Most sessions are read-only (monitoring). Input appears only for sessions the user is actively controlling.

**Interactive card rendering in RichPane:**

RichPane already special-cases `AskUserQuestion` (lines 905-909). Extend this pattern:

- `AskUserQuestion` tool_use → render interactive `AskUserQuestionCard` (evolve existing display-only component)
- `PermissionRequestMsg` → render inline `PermissionCard`
- `ExitPlanMode` tool_use → render `PlanApprovalCard`
- `elicitation_dialog` notification → render `ElicitationCard`

**What gets deleted:**

- `DashboardChat.tsx` (261 lines — all duplicate renderers: `HistoryMessage`, `ControlMessage`, `ToolCallBlock`, plain textarea)
- `ControlPage.tsx` route (redirect to monitor view, or remove entirely)

**What stays unchanged:**

- `MonitorPane` layout (header, content, footer)
- `RichPane` message rendering (1,042 lines, battle-tested)
- `DockLayout` panel management
- `useControlSession` hook (WebSocket state management)
- `PermissionDialog` modal (kept as fallback for urgent interrupts)

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

### 7.3 Dormant Input Bar — Every Session is Resumable

**Key design decision:** There is no "read-only vs interactive" distinction. Every session is viewable, every session is resumable. The input bar handles the transition seamlessly.

**Philosophy:** The input bar IS the affordance. Users see it, they understand they can type. No hidden "take control" button to discover. The pre-flight gate (cost estimate) prevents accidental billing.

**State machine:**

```text
┌─────────┐  user focuses   ┌──────────┐  first send   ┌───────────┐
│ DORMANT │ ──────────────→ │ DORMANT  │ ────────────→ │ RESUMING  │
│ (muted) │  (still muted)  │ (typing) │  (pre-flight) │ (spinner) │
└─────────┘                  └──────────┘               └─────┬─────┘
                                                              │
                              ┌────────────────────────────────┘
                              │ resume success
                              ▼
                        ┌──────────┐  send   ┌───────────┐
                        │  ACTIVE  │ ──────→ │ STREAMING │
                        │ (ready)  │ ←────── │ (waiting) │
                        └──────────┘  done   └───────────┘
                              │
                              │ session ends
                              ▼
                        ┌───────────┐
                        │ COMPLETED │
                        │ (disabled)│
                        └───────────┘
```

**Visual states:**

| State | Appearance | Input bar behavior |
|-------|-----------|-------------------|
| **Dormant** | Muted bg (`bg-gray-800/30`), placeholder "Resume this session..." | First send → pre-flight dialog → resume |
| **Resuming** | Spinner in send button area, input disabled | Waiting for sidecar to start + resume |
| **Active** | Full color, placeholder "Send a message..." | Direct send via WebSocket |
| **Streaming** | Stop button visible, input disabled | Claude is responding |
| **Completed** | Muted bg, placeholder "Session ended" | No interaction possible |

**Session header indicator:**

| State | Badge |
|-------|-------|
| No control connection | (none — default viewing state) |
| Active control | Green dot + "LIVE" text in header |

**Safety rule:** Input bar is dormant when session `agentState.group === 'autonomous'` (Claude mid-turn). Activatable only when session is idle/waiting/completed. This sidesteps concurrent access issues entirely.

**Exception — new session spawn:** Input bar starts in `ACTIVE` state (no session to resume). First send calls `/api/control/start` instead of `/api/control/resume`.

### 7.4 Four Integration Points

Same `<ChatInputBar>` component, 4 different wiring contexts:

**1. Monitor panels (`MonitorPane`)**

- Session is already live — send directly via `useControlSession(controlId).sendMessage`
- ChatInputBar appears at the bottom of MonitorPane when `controlId` exists
- This is the primary interaction point

**2. Expanded session detail tabs (`SessionDetailPanel` → terminal tab)**

- Same as #1 — session is live, `controlId` already available
- ChatInputBar appears below the RichPane in the terminal tab
- User expands a session panel → sees full terminal + input bar

**3. Session history detail (`ConversationView` → `/sessions/:sessionId`)**

- Session is historical — needs resume first
- ChatInputBar shows with a "Resume & Send" button on first message
- `onSend` calls `/api/control/resume` → gets `controlId` → transitions to live mode
- Once resumed, behaves like #1 (direct WebSocket send)
- If session can't be resumed (too old, different machine), show disabled state with tooltip

**4. New session spawn**

- No session yet — user starts from scratch
- ChatInputBar appears as a "new session" panel or top-level prompt (e.g., "+" button in monitor toolbar → empty panel with ChatInputBar)
- `onSend` calls `/api/control/start` with the message → creates session → panel transitions to live monitoring
- No `contextPercent` (new session = 0%), `estimatedCost` shows first-message estimate
- Once session starts, it appears as a new panel in the monitor view automatically

### 7.6 Interaction Binding — Card Callbacks to Sidecar

**The problem:** Interactive cards (AskUserQuestion, Permission, PlanApproval, Elicitation) render inside RichPane. RichPane is a display component — it doesn't own `useControlSession`. How do button clicks in cards reach the sidecar?

**Solution: Callback context via `ControlCallbacks` prop on RichPane.**

RichPane already receives many props. Add one more — an optional `controlCallbacks` object that threads response functions from the parent component (which owns `useControlSession`) down to the cards.

```ts
// New type — passed from parent to RichPane to interactive cards
interface ControlCallbacks {
  /** Respond to AskUserQuestion card */
  answerQuestion: (requestId: string, answers: Record<string, string>) => void
  /** Respond to permission request (allow/deny) */
  respondPermission: (requestId: string, allowed: boolean) => void
  /** Respond to plan approval (approve/reject with feedback) */
  approvePlan: (requestId: string, approved: boolean, feedback?: string) => void
  /** Respond to elicitation dialog */
  submitElicitation: (requestId: string, response: string) => void
}
```

**Threading path:**

```text
MonitorPane (owns useControlSession)
  │
  │  controlCallbacks={{
  │    answerQuestion: (id, answers) => controlSession.send({ type: 'question_response', requestId: id, answers }),
  │    respondPermission: (id, allowed) => controlSession.respondPermission(id, allowed),
  │    approvePlan: (id, ok, fb) => controlSession.send({ type: 'plan_response', requestId: id, approved: ok, feedback: fb }),
  │    submitElicitation: (id, text) => controlSession.send({ type: 'elicitation_response', requestId: id, response: text }),
  │  }}
  │
  ▼
RichPane (receives controlCallbacks prop, passes to card components)
  │
  ▼
InteractiveCard (calls controlCallbacks.answerQuestion/respondPermission/etc.)
```

**When `controlCallbacks` is undefined** (read-only session, no control connection), RichPane renders display-only versions of the cards (existing behavior — shows the question/permission but no action buttons).

**WebSocket message types (client → sidecar):**

```ts
// Sent by frontend when user interacts with a card
type ClientCardResponse =
  | { type: 'question_response'; requestId: string; answers: Record<string, string> }
  | { type: 'permission_response'; requestId: string; allowed: boolean }
  | { type: 'plan_response'; requestId: string; approved: boolean; feedback?: string }
  | { type: 'elicitation_response'; requestId: string; response: string }
```

**WebSocket message types (sidecar → frontend):**

These already partially exist. Extend `ServerMessage` union:

```ts
// New server → client messages (add to existing union in control.ts)
interface AskUserQuestionMsg {
  type: 'ask_user_question'
  requestId: string
  questions: { question: string; header: string; options: { label: string; description: string; markdown?: string }[]; multiSelect: boolean }[]
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

**Sidecar handling (session-manager.ts):**

The sidecar's WebSocket handler already processes `permission_response`. Extend to handle all 4 response types:

```ts
// ws-handler.ts — extend incoming message handling
case 'question_response':
  sessions.resolveQuestion(msg.requestId, msg.answers)
  break
case 'plan_response':
  sessions.resolvePlan(msg.requestId, msg.approved, msg.feedback)
  break
case 'elicitation_response':
  sessions.resolveElicitation(msg.requestId, msg.response)
  break
```

Each `resolve*` method in SessionManager calls the corresponding Agent SDK callback to send the user's response back to Claude.

**Status: Permission is the only one fully wired today.** The other 3 (question, plan, elicitation) need sidecar-side handlers + SDK callbacks. The SDK V2 `canUseTool` callback exists for permissions; the other interactive types may need different SDK hooks (or fall back to sending a user message with structured metadata).

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

1. Build `InteractiveCardShell` shared wrapper + 4 card types (no changes to existing code)
2. Build `ChatInputBar` as a new component with sub-components (no changes to existing code)
3. Wire interactive cards into `RichPane` message rendering (detect interactive types → render cards)
4. Wire `ChatInputBar` into `MonitorPane` (conditional on control connection)
5. Delete `DashboardChat.tsx` and `ControlPage.tsx`
6. Extend sidecar WebSocket protocol for new interactive message types
7. Future: "Start Session" flow reusing `<ChatInputBar>` in a new-session context
