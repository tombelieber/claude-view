# Sidecar Agent SDK Upgrade — Implementation Plan

> **Status:** DONE (2026-03-02) — all 11 tasks implemented, shippable audit passed (SHIP IT)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade the sidecar's Agent SDK from v0.1.0 → v0.2.63 and wire the `canUseTool` callback for permissions, AskUserQuestion, and ExitPlanMode interactive flows — completing the bidirectional control circuit between frontend and Claude.

## Completion Summary

| Task | Commit | Description |
|------|--------|-------------|
| 0 | `f5cd14c5` | chore(sidecar): bump SDK ^0.1.0 → ^0.2.63 |
| 1 | `11321012` | feat(protocol): add interactive card message types |
| 2-5 | `eedf206b` | feat(sidecar): wire canUseTool callback with routing + resolve methods |
| 6,9,10 | `e4cbbab0` | feat(sidecar): WS handler + resolvePermission fix + drain pending on close |
| 7 | `6f096230` | feat(web): handle interactive card messages in useControlSession |
| 8 | — | Verification pass (type-check + build + tests all green) |

Shippable audit: 4/4 passes green. Plan compliance 29/29 items, wiring integrity 11/11 paths, 0 blockers, 0 warnings. Build & tests pass (1140/1142, 2 pre-existing failures in SubAgentDrillDown.test.tsx).

**Architecture:** The sidecar uses SDK V2 (`unstable_v2_resumeSession`). v0.2.63 adds `canUseTool` to `SDKSessionOptions`, unblocking interactive permission routing. Each interactive request creates a Promise stored in a pending map; the frontend resolves it via WebSocket response message. Same event-driven pattern as the existing (but unwired) `pendingPermissions` map. The `AbortSignal` from the SDK handles timeout/cancellation.

**Tech Stack:** Node.js sidecar (Hono + ws), `@anthropic-ai/claude-agent-sdk` 0.2.63 (V2 API), React frontend hook

**Design references:**
- Research doc: `docs/plans/mission-control/2026-03-01-agent-sdk-upgrade-research.md`
- WebSocket protocol: `docs/plans/web/2026-02-28-chat-input-bar-design.md` § 7.6
- Frontend companion plan: `docs/plans/web/2026-02-28-chat-input-bar-impl.md`

**Relationship to frontend plan:** This plan completes the **sidecar pipe** — the backend half of the interactive control system. The frontend companion plan (`chat-input-bar-impl.md`) builds the UI components (ChatInputBar, PermissionCard, AskUserQuestionCard, etc.) that render on the other end of this pipe. After both plans execute, the full circuit works: Claude requests permission → sidecar emits WS message → frontend renders card → user responds → frontend sends WS message → sidecar resolves Promise → Claude continues.

---

## Task 0: Bump SDK Dependency

**Files:**
- Modify: `sidecar/package.json`
- Modify: `bun.lock` (auto-updated)

**Step 1: Install the new version**

Run: `cd sidecar && bun add @anthropic-ai/claude-agent-sdk@^0.2.63`
Expected: `package.json` updates from `^0.1.0` to `^0.2.63`

**Step 2: Verify the install resolved correctly**

Run: `cd sidecar && bun pm ls | grep claude-agent-sdk`
Expected: Shows `@anthropic-ai/claude-agent-sdk@0.2.63` (or higher patch)

**Step 3: Verify existing code still compiles**

Run: `cd sidecar && bunx tsc --noEmit 2>&1 | head -20`
Expected: 0 new errors. The existing `unstable_v2_resumeSession` API is preserved in v0.2.63 (backward-compatible). If there ARE type errors, they indicate breaking changes — stop and investigate the changelog.

**Step 3.5: CRITICAL — Verify `canUseTool` exists on V2 `SDKSessionOptions`**

The entire plan hinges on `canUseTool` being available in the V2 API (`SDKSessionOptions`). The installed v0.1.x only has it on V1's `Options` type. Verify it exists in v0.2.63 before proceeding:

Run:
```bash
cd sidecar && node -e "
const fs = require('fs');
const types = fs.readFileSync('node_modules/@anthropic-ai/claude-agent-sdk/entrypoints/sdk/runtimeTypes.d.ts', 'utf8');
const idx = types.indexOf('SDKSessionOptions');
if (idx === -1) { console.error('SDKSessionOptions not found'); process.exit(1); }
console.log(types.slice(idx, idx + 600));
"
```

Expected: `canUseTool` appears as a property of `SDKSessionOptions`.

**If `canUseTool` is NOT on `SDKSessionOptions`:** STOP. The V2 API may not support it yet. Check if it's on the `unstable_v2_resumeSession` options parameter type instead. If it's only on V1's `Options`, Tasks 3-6 need a different approach.

Also verify the `canUseTool` callback signature — the plan assumes:
```ts
canUseTool: async (toolName: string, input: Record<string, unknown>, options: { signal: AbortSignal }) => PermissionResult
```
Confirm the actual type matches. If the SDK uses a different return shape (e.g., `toolUseID` in the return, `updatedPermissions`, or different discriminant fields), update Task 3's `handleCanUseTool` return type accordingly.

**Step 4: Commit**

```bash
git add sidecar/package.json bun.lock
git commit -m "chore(sidecar): bump @anthropic-ai/claude-agent-sdk ^0.1.0 → ^0.2.63"
```

---

## Task 1: Extend IPC Types — New Message Types (Both Sides)

**Files:**
- Modify: `sidecar/src/types.ts` (lines 1-127)
- Modify: `apps/web/src/types/control.ts` (lines 1-109)

The WebSocket protocol needs 3 new server→client message types (sidecar sends to frontend when Claude requests user input) and 3 new client→server message types (frontend sends back with user's response).

### Step 1: Add new server→client types to `sidecar/src/types.ts`

Add after `PongMessage` (line 82), before the `ServerMessage` union (line 84):

```ts
// Interactive card messages — sidecar → frontend
// Emitted when canUseTool intercepts AskUserQuestion, ExitPlanMode, or Elicitation

export interface AskUserQuestionMessage {
  type: 'ask_user_question'
  requestId: string
  questions: {
    question: string
    header: string
    options: { label: string; description: string; markdown?: string }[]
    multiSelect: boolean
  }[]
}

export interface PlanApprovalMessage {
  type: 'plan_approval'
  requestId: string
  planData: Record<string, unknown> // ExitPlanMode tool input (allowedPrompts, etc.)
}

export interface ElicitationMessage {
  type: 'elicitation'
  requestId: string
  prompt: string
}
```

### Step 2: Update `ServerMessage` union in `sidecar/src/types.ts`

Current (line 84-92):
```ts
export type ServerMessage =
  | AssistantChunk
  | AssistantDone
  | ToolUseStart
  | ToolUseResult
  | PermissionRequest
  | SessionStatusMessage
  | ErrorMessage
  | PongMessage
```

Change to:
```ts
export type ServerMessage =
  | AssistantChunk
  | AssistantDone
  | ToolUseStart
  | ToolUseResult
  | PermissionRequest
  | AskUserQuestionMessage
  | PlanApprovalMessage
  | ElicitationMessage
  | SessionStatusMessage
  | ErrorMessage
  | PongMessage
```

### Step 3: Add new client→server types to `sidecar/src/types.ts`

Add after `PingMessage` (line 19), before `ClientMessage` union (line 21):

```ts
export interface QuestionResponse {
  type: 'question_response'
  requestId: string
  answers: Record<string, string> // { "question text": "selected option label" }
}

export interface PlanResponse {
  type: 'plan_response'
  requestId: string
  approved: boolean
  feedback?: string
}

export interface ElicitationResponse {
  type: 'elicitation_response'
  requestId: string
  response: string
}
```

### Step 4: Update `ClientMessage` union in `sidecar/src/types.ts`

Current (line 21):
```ts
export type ClientMessage = UserMessage | PermissionResponse | PingMessage
```

Change to:
```ts
export type ClientMessage =
  | UserMessage
  | PermissionResponse
  | QuestionResponse
  | PlanResponse
  | ElicitationResponse
  | PingMessage
```

### Step 5: Mirror new server→client types in `apps/web/src/types/control.ts`

Add after `PongMsg` (line 85), before the `ServerMessage` union (line 87):

```ts
// Interactive card messages — sidecar → frontend
export interface AskUserQuestionMsg {
  type: 'ask_user_question'
  requestId: string
  questions: {
    question: string
    header: string
    options: { label: string; description: string; markdown?: string }[]
    multiSelect: boolean
  }[]
}

export interface PlanApprovalMsg {
  type: 'plan_approval'
  requestId: string
  planData: Record<string, unknown>
}

export interface ElicitationMsg {
  type: 'elicitation'
  requestId: string
  prompt: string
}
```

### Step 6: Update `ServerMessage` union in `apps/web/src/types/control.ts`

Current (lines 87-95):
```ts
export type ServerMessage =
  | AssistantChunk
  | AssistantDone
  | ToolUseStartMsg
  | ToolUseResultMsg
  | PermissionRequestMsg
  | SessionStatusMsg
  | ErrorMsg
  | PongMsg
```

Change to:
```ts
export type ServerMessage =
  | AssistantChunk
  | AssistantDone
  | ToolUseStartMsg
  | ToolUseResultMsg
  | PermissionRequestMsg
  | AskUserQuestionMsg
  | PlanApprovalMsg
  | ElicitationMsg
  | SessionStatusMsg
  | ErrorMsg
  | PongMsg
```

### Step 7: Type-check both workspaces

Run: `cd sidecar && bunx tsc --noEmit 2>&1 | head -10`
Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -10`
Expected: 0 errors (type additions are non-breaking)

### Step 8: Commit

```bash
git add sidecar/src/types.ts apps/web/src/types/control.ts
git commit -m "feat(protocol): add interactive card message types for question, plan, elicitation"
```

---

## Task 2: Add Pending Callback Infrastructure to ControlSession

**Files:**
- Modify: `sidecar/src/session-manager.ts` (lines 17-35 — ControlSession interface)

The existing `pendingPermissions` map stores resolve callbacks for permission requests. Add parallel maps for question, plan, and elicitation responses. Same pattern — store a `resolve` callback, caller awaits the Promise.

### Step 1: Extend `ControlSession` interface

Current `pendingPermissions` field (lines 28-34):
```ts
  pendingPermissions: Map<
    string,
    {
      resolve: (allowed: boolean) => void
      timer: ReturnType<typeof setTimeout>
    }
  >
```

Add after `pendingPermissions`, before the closing `}`:

```ts
  pendingQuestions: Map<
    string,
    { resolve: (answers: Record<string, string>) => void }
  >
  pendingPlans: Map<
    string,
    { resolve: (result: { approved: boolean; feedback?: string }) => void }
  >
  pendingElicitations: Map<
    string,
    { resolve: (response: string) => void }
  >
```

**Why no `timer` field?** Permissions use a manual timer for auto-deny behavior (safety default). Questions, plans, and elicitations have no auto-timeout — the user responds when ready. The SDK's `AbortSignal` handles edge cases (session close, server shutdown).

### Step 2: Initialize new maps in `resume()` method

**Note:** This step modifies the CURRENT code (before Task 3 restructures it). Task 3 will replace the entire `resume()` block with a version that already includes these maps. If you are executing sequentially, this step ensures Task 2's type-check passes before moving to Task 3. Alternatively, you may skip this step and proceed directly to Task 3 which includes all 4 maps in its replacement block.

In the `resume()` method (around line 104-116), find the ControlSession initialization object. The current code uses shorthand property syntax: `pendingPermissions,` (a local variable reference, NOT `pendingPermissions: new Map()`). After `pendingPermissions,` (line 115), add these fields using explicit initialization (no local variables needed — unlike `pendingPermissions` which has a pre-existing local, the new maps are created inline):

```ts
      pendingQuestions: new Map(),
      pendingPlans: new Map(),
      pendingElicitations: new Map(),
```

### Step 3: Type-check

Run: `cd sidecar && bunx tsc --noEmit 2>&1 | head -10`
Expected: 0 errors

### Step 4: Commit

```bash
git add sidecar/src/session-manager.ts
git commit -m "feat(sidecar): add pending callback maps for question, plan, elicitation"
```

---

## Task 3: Wire `canUseTool` Callback — Permission Routing

**Files:**
- Modify: `sidecar/src/session-manager.ts` (lines 77-98 — resume method SDK call)
- Modify: `sidecar/src/types.ts` (import if needed)

This is the core change. The existing `resume()` creates an SDK session with NO `canUseTool` (v0.1.0 didn't support it). Now we add the callback and route permission requests through the existing `pendingPermissions` map → EventEmitter → WebSocket → frontend.

### Step 1: Replace the SDK session creation in `resume()`

Current code (lines 93-98):
```ts
    const sdkSession = unstable_v2_resumeSession(sessionId, {
      model: model ?? 'claude-sonnet-4-20250514',
      env: {
        ...(projectPath ? { CLAUDE_CWD: projectPath } : {}),
      },
    })
```

Replace with:
```ts
    const sdkSession = unstable_v2_resumeSession(sessionId, {
      model: model ?? 'claude-sonnet-4-20250514',
      env: {
        ...(projectPath ? { CLAUDE_CWD: projectPath } : {}),
      },
      canUseTool: async (toolName, input, { signal }) => {
        return this.handleCanUseTool(cs, toolName, input, signal)
      },
    })
```

**Note:** `cs` is the `ControlSession` object. It doesn't exist yet at the point where we create `sdkSession` — there's a chicken-and-egg problem. The `cs` object is created AFTER the SDK session. Fix: create `cs` first with a placeholder `sdkSession`, then assign the real one.

Restructure the resume() flow (lines 93-119).

**Why `let cs!`?** The closure in `canUseTool` captures `cs` by reference. `canUseTool` only fires asynchronously (after at least one SDK event loop turn), so `cs` is guaranteed to be assigned before the callback executes. Using `let cs!: ControlSession` (definite assignment assertion) is type-safe and avoids the fragile `null as unknown as SDKSession` cast. This pattern is used by TypeORM's `DataSource` and similar async-init APIs.

```ts
    // canUseTool closure captures `cs` by reference — safe because
    // canUseTool fires asynchronously, never during synchronous init.
    let cs!: ControlSession

    const sdkSession = unstable_v2_resumeSession(sessionId, {
      model: model ?? 'claude-sonnet-4-20250514',
      env: {
        ...(projectPath ? { CLAUDE_CWD: projectPath } : {}),
      },
      canUseTool: async (toolName, input, { signal }) => {
        return this.handleCanUseTool(cs, toolName, input, signal)
      },
    })

    cs = {
      controlId,
      sessionId,
      sdkSession,
      status: 'waiting_input',
      totalCost: 0,
      turnCount: 0,
      contextUsage: 0,
      startedAt: Date.now(),
      emitter,
      isStreaming: false,
      pendingPermissions: new Map(),
      pendingQuestions: new Map(),
      pendingPlans: new Map(),
      pendingElicitations: new Map(),
    }

    this.sessions.set(controlId, cs)
    return cs
```

**Note:** The return type remains `Promise<ControlSession>` (unchanged from the method signature at line 59). The caller in `control.ts` (line 28) accesses `cs.controlId` and `cs.sessionId` which are both present on `ControlSession`.

### Step 2: Remove the SDK V2 LIMITATION comment block

Delete lines 77-92 (the long comment block about SDK V2 limitations). Replace with a brief note:

```ts
    // SDK v0.2.63: canUseTool available — permissions, AskUserQuestion, ExitPlanMode
    // routed through handleCanUseTool → pendingPermissions/Questions/Plans maps
```

### Step 3: Add `handleCanUseTool` method — permissions only (first pass)

Add as a new private method on `SessionManager`, after `resume()` and before `sendMessage()`.

**IMPORTANT — Return type depends on Task 0 Step 3.5 findings:** The return type below assumes the SDK's `canUseTool` callback expects `{ behavior: 'allow'; updatedInput: Record<string, unknown> } | { behavior: 'deny'; message: string }`. If Task 0 Step 3.5 reveals a DIFFERENT return shape (e.g., `PermissionResult` with different discriminant fields, `toolUseID` in the return, or a flat `boolean`), update this return type AND all `resolve()` calls in Tasks 3-5 to match. The SDK's type definition is the source of truth — never fight it.

```ts
  /**
   * Central callback for all tool permission decisions.
   * Routes to specific handlers based on toolName.
   * Returns a Promise that resolves when the user responds via WebSocket.
   */
  private async handleCanUseTool(
    cs: ControlSession,
    toolName: string,
    input: Record<string, unknown>,
    signal: AbortSignal,
  ): Promise<{ behavior: 'allow'; updatedInput: Record<string, unknown> } | { behavior: 'deny'; message: string }> {
    // --- Generic permission request (all tools except AskUserQuestion / ExitPlanMode) ---
    const requestId = crypto.randomUUID()

    return new Promise((resolve) => {
      // Store resolve callback
      cs.pendingPermissions.set(requestId, {
        resolve: (allowed: boolean) => {
          resolve(
            allowed
              ? { behavior: 'allow', updatedInput: input }
              : { behavior: 'deny', message: `User denied ${toolName}` },
          )
        },
        timer: setTimeout(() => {
          // Auto-deny after 60s if frontend doesn't respond
          if (cs.pendingPermissions.has(requestId)) {
            cs.pendingPermissions.delete(requestId)
            resolve({ behavior: 'deny', message: 'Permission request timed out' })
          }
        }, 60_000),
      })

      // Handle SDK abort (session close, server shutdown)
      signal.addEventListener(
        'abort',
        () => {
          const pending = cs.pendingPermissions.get(requestId)
          if (pending) {
            clearTimeout(pending.timer)
            cs.pendingPermissions.delete(requestId)
            resolve({ behavior: 'deny', message: 'Request aborted' })
          }
        },
        { once: true },
      )

      // Emit to frontend via EventEmitter → WebSocket
      cs.status = 'waiting_permission'
      cs.emitter.emit('message', {
        type: 'permission_request',
        requestId,
        toolName,
        toolInput: input,
        description: `${toolName} requires permission`,
        timeoutMs: 60_000,
      } satisfies ServerMessage)
      cs.emitter.emit('message', {
        type: 'session_status',
        status: cs.status,
        contextUsage: cs.contextUsage,
        turnCount: cs.turnCount,
      } satisfies ServerMessage)
    })
  }
```

### Step 4: Type-check

Run: `cd sidecar && bunx tsc --noEmit 2>&1 | head -10`
Expected: 0 errors

### Step 5: Commit

```bash
git add sidecar/src/session-manager.ts
git commit -m "feat(sidecar): wire canUseTool callback for permission routing via pendingPermissions"
```

---

## Task 4: Wire `canUseTool` — AskUserQuestion + ExitPlanMode

**Files:**
- Modify: `sidecar/src/session-manager.ts` (the `handleCanUseTool` method from Task 3)

Now extend the `handleCanUseTool` method to intercept `AskUserQuestion` and `ExitPlanMode` before falling through to generic permission handling.

### Step 1: Add imports for new types (MUST come before Steps 2-3)

At the top of `sidecar/src/session-manager.ts`, update the import from `'./types.js'` (line 15) to include the new types. **CRITICAL: Retain `ServerMessage`** — it is used in 4 `satisfies ServerMessage` assertions throughout the file (lines 151, 166, 188, 201).

Current (line 15):
```ts
import type { ActiveSession, ServerMessage } from './types.js'
```

Change to:
```ts
import type { ActiveSession, AskUserQuestionMessage, PlanApprovalMessage, ServerMessage } from './types.js'
```

### Step 2: Add AskUserQuestion routing

At the TOP of `handleCanUseTool`, BEFORE the generic permission block, add:

```ts
    // --- AskUserQuestion: route to frontend for user selection ---
    if (toolName === 'AskUserQuestion') {
      const requestId = crypto.randomUUID()
      const questions = input.questions as AskUserQuestionMessage['questions']

      return new Promise((resolve) => {
        cs.pendingQuestions.set(requestId, {
          resolve: (answers: Record<string, string>) => {
            resolve({
              behavior: 'allow',
              updatedInput: { ...input, answers },
            })
          },
        })

        signal.addEventListener(
          'abort',
          () => {
            if (cs.pendingQuestions.has(requestId)) {
              cs.pendingQuestions.delete(requestId)
              resolve({ behavior: 'deny', message: 'Question aborted' })
            }
          },
          { once: true },
        )

        // Update status and emit to frontend
        cs.status = 'waiting_permission'
        cs.emitter.emit('message', {
          type: 'ask_user_question',
          requestId,
          questions,
        } satisfies AskUserQuestionMessage)
        cs.emitter.emit('message', {
          type: 'session_status',
          status: cs.status,
          contextUsage: cs.contextUsage,
          turnCount: cs.turnCount,
        } satisfies ServerMessage)
      })
    }
```

**Note:** We reuse `'waiting_permission'` for all blocked states (permission, question, plan) because the status union in `types.ts` only has `'active' | 'waiting_input' | 'waiting_permission' | 'completed' | 'error'`. The frontend distinguishes blocked-for-what by checking which state field is non-null (`permissionRequest`, `askQuestion`, `planApproval`). If finer-grained status is needed later, add `'waiting_question' | 'waiting_plan'` to the status union.

### Step 2b: Add ExitPlanMode routing

After the AskUserQuestion block, BEFORE the generic permission block, add:

```ts
    // --- ExitPlanMode: route to frontend for plan approval ---
    if (toolName === 'ExitPlanMode') {
      const requestId = crypto.randomUUID()

      return new Promise((resolve) => {
        cs.pendingPlans.set(requestId, {
          resolve: (result: { approved: boolean; feedback?: string }) => {
            resolve(
              result.approved
                ? { behavior: 'allow', updatedInput: input }
                : { behavior: 'deny', message: result.feedback ?? 'Plan rejected by user' },
            )
          },
        })

        signal.addEventListener(
          'abort',
          () => {
            if (cs.pendingPlans.has(requestId)) {
              cs.pendingPlans.delete(requestId)
              resolve({ behavior: 'deny', message: 'Plan approval aborted' })
            }
          },
          { once: true },
        )

        // Update status and emit plan data to frontend
        cs.status = 'waiting_permission'
        cs.emitter.emit('message', {
          type: 'plan_approval',
          requestId,
          planData: input,
        } satisfies PlanApprovalMessage)
        cs.emitter.emit('message', {
          type: 'session_status',
          status: cs.status,
          contextUsage: cs.contextUsage,
          turnCount: cs.turnCount,
        } satisfies ServerMessage)
      })
    }
```

### Step 3: Type-check

Run: `cd sidecar && bunx tsc --noEmit 2>&1 | head -10`
Expected: 0 errors

### Step 4: Commit

```bash
git add sidecar/src/session-manager.ts
git commit -m "feat(sidecar): route AskUserQuestion + ExitPlanMode through canUseTool to frontend"
```

---

## Task 5: Add Resolve Methods for All Interactive Types

**Files:**
- Modify: `sidecar/src/session-manager.ts` (after existing `resolvePermission` method, lines 206-215)

Add methods that the WebSocket handler calls when the frontend responds to interactive cards.

### Step 1: Add `resolveQuestion` method

Add after `resolvePermission()`:

```ts
  resolveQuestion(
    controlId: string,
    requestId: string,
    answers: Record<string, string>,
  ): boolean {
    const cs = this.sessions.get(controlId)
    if (!cs) return false
    const pending = cs.pendingQuestions.get(requestId)
    if (!pending) return false
    cs.pendingQuestions.delete(requestId)
    cs.status = 'active'
    cs.emitter.emit('message', {
      type: 'session_status',
      status: cs.status,
      contextUsage: cs.contextUsage,
      turnCount: cs.turnCount,
    } satisfies ServerMessage)
    pending.resolve(answers)
    return true
  }
```

### Step 2: Add `resolvePlan` method

```ts
  resolvePlan(
    controlId: string,
    requestId: string,
    approved: boolean,
    feedback?: string,
  ): boolean {
    const cs = this.sessions.get(controlId)
    if (!cs) return false
    const pending = cs.pendingPlans.get(requestId)
    if (!pending) return false
    cs.pendingPlans.delete(requestId)
    cs.status = 'active'
    cs.emitter.emit('message', {
      type: 'session_status',
      status: cs.status,
      contextUsage: cs.contextUsage,
      turnCount: cs.turnCount,
    } satisfies ServerMessage)
    pending.resolve({ approved, feedback })
    return true
  }
```

### Step 3: Add `resolveElicitation` method

```ts
  resolveElicitation(
    controlId: string,
    requestId: string,
    response: string,
  ): boolean {
    const cs = this.sessions.get(controlId)
    if (!cs) return false
    const pending = cs.pendingElicitations.get(requestId)
    if (!pending) return false
    cs.pendingElicitations.delete(requestId)
    cs.status = 'active'
    cs.emitter.emit('message', {
      type: 'session_status',
      status: cs.status,
      contextUsage: cs.contextUsage,
      turnCount: cs.turnCount,
    } satisfies ServerMessage)
    pending.resolve(response)
    return true
  }
```

**Pattern:** All three resolve methods follow the same pattern as `resolvePermission` (Task 9): delete map entry → set status to `'active'` → emit `session_status` → resolve Promise. This ensures the frontend always receives a status update when any blocked state clears.

### Step 4: Type-check

Run: `cd sidecar && bunx tsc --noEmit 2>&1 | head -10`
Expected: 0 errors

### Step 5: Commit

```bash
git add sidecar/src/session-manager.ts
git commit -m "feat(sidecar): add resolveQuestion, resolvePlan, resolveElicitation methods"
```

---

## Task 6: Extend WebSocket Handler for New Response Types

**Files:**
- Modify: `sidecar/src/ws-handler.ts` (lines 36-48 — switch on msg.type)

The WebSocket handler already processes `user_message`, `permission_response`, and `ping`. Add cases for the 3 new client→server message types.

### Step 1: Add new cases to the switch block

Current (lines 36-48) — note: the actual code uses `ws.send(JSON.stringify(...))` directly, NOT a `send()` helper:
```ts
      switch (msg.type) {
        case 'user_message':
          sessions.sendMessage(controlId, msg.content).catch((err) => {
            ws.send(JSON.stringify({ type: 'error', message: String(err), fatal: false }))
          })
          break
        case 'permission_response':
          sessions.resolvePermission(controlId, msg.requestId, msg.allowed)
          break
        case 'ping':
          ws.send(JSON.stringify({ type: 'pong' }))
          break
      }
```

Change to:
```ts
      switch (msg.type) {
        case 'user_message':
          sessions.sendMessage(controlId, msg.content).catch((err) => {
            ws.send(JSON.stringify({ type: 'error', message: String(err), fatal: false }))
          })
          break
        case 'permission_response':
          sessions.resolvePermission(controlId, msg.requestId, msg.allowed)
          break
        case 'question_response': {
          const ok = sessions.resolveQuestion(controlId, msg.requestId, msg.answers)
          if (!ok) ws.send(JSON.stringify({ type: 'error', message: 'Unknown question requestId', fatal: false }))
          break
        }
        case 'plan_response': {
          const ok = sessions.resolvePlan(controlId, msg.requestId, msg.approved, msg.feedback)
          if (!ok) ws.send(JSON.stringify({ type: 'error', message: 'Unknown plan requestId', fatal: false }))
          break
        }
        case 'elicitation_response': {
          const ok = sessions.resolveElicitation(controlId, msg.requestId, msg.response)
          if (!ok) ws.send(JSON.stringify({ type: 'error', message: 'Unknown elicitation requestId', fatal: false }))
          break
        }
        case 'ping':
          ws.send(JSON.stringify({ type: 'pong' }))
          break
      }
```

### Step 2: Drain interactive pending maps on WS disconnect (DO NOT close the session)

**CRITICAL: Do NOT call `sessions.close()` on WS disconnect.** The frontend's reconnect logic (`use-control-session.ts` lines 167-177) implements exponential backoff with up to 10 retries. If we destroy the SDK session every time the WebSocket disconnects, a page refresh or 5-second network hiccup permanently kills the active Claude session. The user would lose all in-progress work. This is the exact anti-pattern that reconnect logic exists to prevent.

**What we DO need:** When the WebSocket disconnects while interactive Promises are pending (questions, plans, elicitations), those Promises will hang forever because there's no frontend to respond to them. We need to drain those maps — but NOT destroy the SDK session.

**Exception: `pendingPermissions` is NOT drained here** because it already has its own 60-second auto-deny timer (set in `handleCanUseTool`). The timer fires regardless of WebSocket state.

Current (lines 55-57):
```ts
  ws.on('close', () => {
    session.emitter.removeListener('message', onMessage)
  })
```

Change to:
```ts
  ws.on('close', () => {
    session.emitter.removeListener('message', onMessage)

    // Drain interactive pending maps (question/plan/elicitation) — these have
    // no auto-timeout timer, so they'd hang forever without a connected frontend.
    // Do NOT call sessions.close() — that destroys the SDK session and defeats
    // the frontend's reconnect logic (exponential backoff, up to 10 retries).
    // pendingPermissions is NOT drained here because it has its own 60s auto-deny timer.
    // IMPORTANT — Semantic behavior on WS disconnect:
    // - pendingQuestions: resolve({}) → SDK gets `allow` with empty answers (proceeds, not denied)
    // - pendingPlans: resolve({ approved: false }) → SDK gets `deny` (plan rejected)
    // - pendingElicitations: resolve('') → SDK gets `allow` with empty response (proceeds)
    //
    // For questions and elicitations, "proceed with empty input" is the safest behavior
    // because the WS may reconnect and the user can retry. Denying would abort Claude's
    // turn entirely. Plans are rejected because auto-approving an implementation plan
    // without user review is a security/correctness risk.
    for (const [, pending] of session.pendingQuestions) {
      pending.resolve({}) // empty answers → allow with no selections (safe default)
    }
    session.pendingQuestions.clear()

    for (const [, pending] of session.pendingPlans) {
      pending.resolve({ approved: false }) // reject plan (never auto-approve)
    }
    session.pendingPlans.clear()

    for (const [, pending] of session.pendingElicitations) {
      pending.resolve('') // empty response → allow with no input (safe default)
    }
    session.pendingElicitations.clear()
  })
```

**Session lifecycle:** Sessions are terminated via explicit HTTP `DELETE /api/control/sessions/:controlId` (which calls `sessions.close()` → drains ALL maps + calls `sdkSession.close()`). They are NOT terminated by WebSocket disconnect. An orphan session reaper can be added later if needed (e.g., reap sessions with no WS connection for > 30 minutes).

**Design note — Question/Elicitation drain semantics:** The `pendingQuestions` resolve callback always flows to the `allow` branch in `handleCanUseTool` (Task 4 Step 2, line 534-538). There is no `deny` path — the callback only accepts `answers`. Passing `{}` means "allow with empty answers", which lets Claude proceed. This is intentional for WS disconnect: if the user refreshes the page mid-question, Claude can continue rather than aborting. If denial is needed in the future, restructure the pending map to `{ resolve, reject }` and wire the `reject` path to `{ behavior: 'deny', message: '...' }`.

### Step 3: Type-check

Run: `cd sidecar && bunx tsc --noEmit 2>&1 | head -10`
Expected: 0 errors. The `ClientMessage` union (updated in Task 1) includes the new types, so the switch cases type-narrow correctly.

### Step 4: Commit

```bash
git add sidecar/src/ws-handler.ts
git commit -m "feat(sidecar): handle question, plan, elicitation responses in WS handler + drain pending on disconnect"
```

---

## Task 7: Extend Frontend Hook — Incoming Messages + `sendRaw`

**Files:**
- Modify: `apps/web/src/hooks/use-control-session.ts`

The frontend hook already handles incoming `permission_request` messages. Add handlers for the 3 new interactive message types, plus a `sendRaw` method for the frontend to send arbitrary JSON (used by `useControlCallbacks` in the companion chat-input-bar-impl plan).

### Step 1: Add state fields for new interactive types

Extend `ControlSessionState` (lines 16-27). Insert the 3 new fields **between `permissionRequest` (line 25) and `error` (line 26)** — do NOT place after `error`:

```ts
  permissionRequest: PermissionRequestMsg | null
  askQuestion: AskUserQuestionMsg | null
  planApproval: PlanApprovalMsg | null
  elicitation: ElicitationMsg | null
  error: string | null  // existing — keep in place
```

Update `initialState` (lines 29-40). Insert **between `permissionRequest: null` (line 38) and `error: null` (line 39)**:

```ts
  permissionRequest: null,
  askQuestion: null,
  planApproval: null,
  elicitation: null,
  error: null,  // existing — keep in place
```

### Step 2: Add imports for new message types

At the top (line 4), update the import from `'../types/control'`. **CRITICAL: Retain `ChatMessage` and `ServerMessage`** — `ChatMessage` is used in the `messages` state field (line 18) and `sendMessage` setState (line 224). `ServerMessage` is used to type the `ws.onmessage` parse (line 71).

Current (line 4):
```ts
import type { ChatMessage, PermissionRequestMsg, ServerMessage } from '../types/control'
```

Change to:
```ts
import type {
  AskUserQuestionMsg,
  ChatMessage,
  ElicitationMsg,
  PermissionRequestMsg,
  PlanApprovalMsg,
  ServerMessage,
} from '../types/control'
```

### Step 3: Add message handler cases

In the `ws.onmessage` handler: the switch block is inside `setState((prev) => { switch (msg.type) { ... } })` starting at line 74. The `'permission_request'` case opens at **line 130** (not 135). Add the 3 new cases after the `permission_request` case block (after line 135):

```ts
        case 'ask_user_question':
          return {
            ...prev,
            askQuestion: msg,
            status: 'waiting_permission',
          }

        case 'plan_approval':
          return {
            ...prev,
            planApproval: msg,
            status: 'waiting_permission',
          }

        case 'elicitation':
          return {
            ...prev,
            elicitation: msg,
            status: 'waiting_permission',
          }
```

**Note:** The switch is inside a `setState` updater that returns the new state. These cases use `return { ...prev, ... }` (matching the existing pattern), NOT `setState((prev) => ...)` which would be invalid inside an updater function.

### Step 4: SKIP — `sendRaw` already exists

`sendRaw` is **already implemented** at lines 229-233 and is already included in the return value at line 248. Do NOT add it again — doing so would create a duplicate `const` declaration and cause a TypeScript error.

### Step 5: Add response helper methods + clear state on response

After `respondPermission` (line 246), add methods that send responses AND clear the corresponding state:

```ts
  const answerQuestion = useCallback(
    (requestId: string, answers: Record<string, string>) => {
      sendRaw({ type: 'question_response', requestId, answers })
      setState((prev) => ({ ...prev, askQuestion: null }))
    },
    [sendRaw],
  )

  const approvePlan = useCallback(
    (requestId: string, approved: boolean, feedback?: string) => {
      sendRaw({ type: 'plan_response', requestId, approved, feedback })
      setState((prev) => ({ ...prev, planApproval: null }))
    },
    [sendRaw],
  )

  const submitElicitation = useCallback(
    (requestId: string, response: string) => {
      sendRaw({ type: 'elicitation_response', requestId, response })
      setState((prev) => ({ ...prev, elicitation: null }))
    },
    [sendRaw],
  )
```

**Architectural note:** `apps/web/src/hooks/use-control-callbacks.ts` already provides similar callbacks built on `sendRaw`. These hook-level helpers are a convenience for direct consumers of `useControlSession` who don't use `useControlCallbacks`. Both paths work; neither conflicts.

### Step 6: Update return value

Current (line 248 — **not** 242):
```ts
  return { ...state, sendMessage, sendRaw, respondPermission }
```

Change to:
```ts
  return {
    ...state,
    sendMessage,
    sendRaw,
    respondPermission,
    answerQuestion,
    approvePlan,
    submitElicitation,
  }
```

### Step 7: Type-check

Run: `cd apps/web && bunx tsc --noEmit 2>&1 | head -20`
Expected: 0 errors

### Step 8: Commit

```bash
git add apps/web/src/hooks/use-control-session.ts
git commit -m "feat(web): handle interactive card messages in useControlSession"
```

---

## Task 8: Type-Check All Workspaces + Build

**Files:** None (verification only)

### Step 1: Type-check sidecar

Run: `cd sidecar && bunx tsc --noEmit`
Expected: 0 errors

### Step 2: Type-check web frontend

Run: `cd apps/web && bunx tsc --noEmit`
Expected: 0 errors

### Step 3: Build sidecar

Run: `cd sidecar && bun run build`
Expected: Build succeeds, `sidecar/dist/index.js` created

### Step 4: Build web frontend

Run: `bun run build`
Expected: Turbo build succeeds for all apps

### Step 5: Run sidecar tests (if any exist)

Run: `cd sidecar && bun run test 2>&1 | tail -20`
Expected: All existing tests pass (new code is additive, no behavior changes to existing flows)

### Step 6: Run web frontend tests

Run: `cd apps/web && bunx vitest run 2>&1 | tail -20`
Expected: All existing tests pass

---

## Task 9: Update `resolvePermission` to Restore Status After Response

**Dependency:** This task completes the status lifecycle started in Task 3 (which sets `cs.status = 'waiting_permission'`). Between Tasks 3-8 and this task, permission responses do NOT restore status to `'active'`. Execute this task immediately after Task 8 — do not skip or defer. If execution is interrupted between Tasks 3 and 9, permission responses will leave the session stuck in `'waiting_permission'` status.

**Files:**
- Modify: `sidecar/src/session-manager.ts`

The current `resolvePermission` (lines 206-215) resolves the pending callback but does NOT update `cs.status` back from `'waiting_permission'` to `'active'`. The `handleCanUseTool` method sets status to `'waiting_permission'` when emitting the request — we need to restore it when the response comes in.

### Step 1: Update `resolvePermission`

Current:
```ts
  resolvePermission(controlId: string, requestId: string, allowed: boolean): boolean {
    const cs = this.sessions.get(controlId)
    if (!cs) return false
    const pending = cs.pendingPermissions.get(requestId)
    if (!pending) return false
    clearTimeout(pending.timer)
    cs.pendingPermissions.delete(requestId)
    pending.resolve(allowed)
    return true
  }
```

Change to:
```ts
  resolvePermission(controlId: string, requestId: string, allowed: boolean): boolean {
    const cs = this.sessions.get(controlId)
    if (!cs) return false
    const pending = cs.pendingPermissions.get(requestId)
    if (!pending) return false
    clearTimeout(pending.timer)
    cs.pendingPermissions.delete(requestId)
    cs.status = 'active'
    cs.emitter.emit('message', {
      type: 'session_status',
      status: cs.status,
      contextUsage: cs.contextUsage,
      turnCount: cs.turnCount,
    } satisfies ServerMessage)
    pending.resolve(allowed)
    return true
  }
```

### Step 2: Commit

```bash
git add sidecar/src/session-manager.ts
git commit -m "fix(sidecar): restore session status to active after permission/question/plan response"
```

---

## Task 10: Drain Pending Maps in `close()` — Prevent Leaked Promises

**Files:**
- Modify: `sidecar/src/session-manager.ts` (lines 217-222 — `close()` method)

The current `close()` method calls `cs.sdkSession.close()` and deletes the session from the map, but does NOT explicitly drain the pending maps. If `close()` is called while no `canUseTool` invocation is active (e.g., session is `waiting_input`), the `AbortSignal` never fires for old entries. Timers from `pendingPermissions` could still fire after the session is deleted, calling `resolve()` into the void.

**Pattern:** gRPC servers drain pending calls before shutdown. Node.js HTTP servers close keep-alive connections before exit. Same principle.

### Step 1: Update `close()` to drain all pending maps

Current (lines 217-222):
```ts
  async close(controlId: string): Promise<void> {
    const cs = this.sessions.get(controlId)
    if (!cs) return
    cs.sdkSession.close() // close() is synchronous in V2, no await
    this.sessions.delete(controlId)
  }
```

Change to:
```ts
  async close(controlId: string): Promise<void> {
    const cs = this.sessions.get(controlId)
    if (!cs) return

    // Drain all pending promises before closing (prevents leaked promises)
    for (const [, pending] of cs.pendingPermissions) {
      clearTimeout(pending.timer)
      pending.resolve(false) // deny
    }
    cs.pendingPermissions.clear()

    for (const [, pending] of cs.pendingQuestions) {
      pending.resolve({}) // allow with empty answers (see Task 6 Step 2 design note)
    }
    cs.pendingQuestions.clear()

    for (const [, pending] of cs.pendingPlans) {
      pending.resolve({ approved: false }) // reject plan
    }
    cs.pendingPlans.clear()

    for (const [, pending] of cs.pendingElicitations) {
      pending.resolve('') // empty response
    }
    cs.pendingElicitations.clear()

    cs.sdkSession.close()
    this.sessions.delete(controlId)
  }
```

### Step 2: Type-check

Run: `cd sidecar && bunx tsc --noEmit 2>&1 | head -10`
Expected: 0 errors

### Step 3: Commit

```bash
git add sidecar/src/session-manager.ts
git commit -m "fix(sidecar): drain pending promise maps in close() to prevent leaked promises"
```

---

## Companion Plan Overlap Notes

The following items in `2026-02-28-chat-input-bar-impl.md` are **partially completed** by this plan:

| Chat-Input-Bar Task | What this plan does | What remains |
|---|---|---|
| Task 15, Step 2 (add `sendRaw` to hook) | **Already existed** before this plan — `sendRaw` was at lines 229-233 of `use-control-session.ts` | Skip this step |
| Task 15, Step 1 (ControlCallbacks type) | NOT done — type lives in a separate file | Still needed |
| Task 15, Step 3 (useControlCallbacks hook) | NOT done — wraps sendRaw into callbacks | Still needed |
| Task 16 (ControlSessionInfo in control.ts) | **Already existed** — `ControlSessionInfo` is at lines 98-103 of `control.ts` | Skip this step |

When executing `2026-02-28-chat-input-bar-impl.md` after this plan, **skip Task 15 Step 2** (sendRaw already exists) and **skip Task 16** (ControlSessionInfo already exists).

---

## Elicitation via Hooks — Deferred

The research doc identifies `Elicitation` as a **hook event**, not a tool use. This means it does NOT flow through `canUseTool`. It requires the `hooks` option on `SDKSessionOptions`:

```ts
hooks: {
  Elicitation: [{ matcher: '*', hooks: [elicitationHandler] }],
}
```

The hook callback signature for `Elicitation` needs investigation against the SDK source or docs. The infrastructure is in place (types, pending map, resolve method, WS handler, frontend state), but the **sidecar trigger** (how to create the pending Promise from a hook event) is deferred until the hook callback shape is confirmed.

**Tracked as:** Future task — wire `Elicitation` hook in `resume()` once callback signature is confirmed.

**Note:** `Elicitation` is confirmed in the `HookEvent` union in SDK v0.2.63 (per research doc), but is NOT present in the currently installed v0.1.x. After Task 0 upgrades the SDK, verify `Elicitation` appears in `HOOK_EVENTS` before implementing the hook wiring.

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `canUseTool` on V2 `SDKSessionOptions` unverifiable until SDK upgraded | Blocker | Added Task 0 Step 3.5: verify `canUseTool` exists in new SDK types after install |
| 2 | `null as unknown as SDKSession` cast fragile — type lie, null dereference window | Blocker | Task 3: replaced with `let cs!: ControlSession` definite assignment pattern |
| 3 | `resume()` return changed from `ControlSession` to plain object — violates declared return type | Blocker | Task 3: changed to `return cs` (preserves `Promise<ControlSession>` signature) |
| 4 | Task 4 Step 3 import block dropped `ServerMessage` — 4 `satisfies ServerMessage` assertions would fail | Blocker | Fixed import to retain `ServerMessage` alongside new types |
| 5 | AskUserQuestion/ExitPlanMode blocks never set `cs.status` or emit `session_status` | Blocker | Task 4: added `cs.status = 'waiting_permission'` + status emission in both blocks |
| 6 | `resolveQuestion`/`resolvePlan`/`resolveElicitation` never restored status to `'active'` | Blocker | Task 5: added status restoration + `session_status` emission in all 3 resolve methods |
| 7 | WS disconnect leaves interactive Promises hanging forever (no frontend to respond) | Blocker | Task 6 Step 2: drain question/plan/elicitation maps on WS close (NOT session destroy) |
| 8 | `close()` did not drain pending maps — leaked Promises after session delete | Blocker | Added Task 10: drain all 4 pending maps in `close()` before `sdkSession.close()` |
| 9 | Task 7 Step 2 import dropped `ChatMessage` + `ServerMessage` — used elsewhere in hook | Blocker | Fixed import to retain all existing types + add new ones |
| 10 | Task 7 Step 4: `sendRaw` already existed at L229-233 — duplicate declaration error | Blocker | Marked Step 4 as SKIP |
| 11 | Task 7 Step 6: "Current" return at "line 242" wrong — actual is `{...state, sendMessage, sendRaw, respondPermission}` at L248 | Blocker | Fixed to show actual current return at correct line |
| 12 | Task 6 Step 1: ws-handler "Current" code used `send()` helper — actual uses `ws.send(JSON.stringify(...))` | Warning | Fixed code block to match actual source |
| 13 | Task 7 Step 3: line numbers off by 5 (onmessage L67 not L73, permission_request L130 not L135) | Warning | Fixed line references + clarified switch is inside setState updater (use `return` not `setState`) |
| 14 | Task 7 Step 5: helper callbacks redundant with `useControlCallbacks` | Warning | Added architectural note explaining both paths coexist |
| 15 | Task 7 Step 1: new fields insert between `permissionRequest` and `error` — boundary not named | Minor | Clarified insertion point with both boundary fields shown |
| 16 | Task 6: discarded boolean return from resolve methods — no error feedback | Minor | Added error response for failed resolve in all 3 new WS handler cases |
| 17 | Companion plan overlap: `ControlSessionInfo` already existed at L98-103 of `control.ts` | Minor | Updated overlap table to mark Task 16 as already done |
| 18 | Elicitation HookEvent: only valid for v0.2.63+, not installed v0.1.x | Warning | Added verification note in Elicitation section |
| 19 | Task 6 Step 2: `sessions.close()` on WS disconnect DESTROYS active SDK session — defeats frontend's reconnect logic (10 retries, exponential backoff) | Blocker | Rewrote Step 2: drain interactive maps only, do NOT destroy session. Sessions terminated via explicit HTTP DELETE |
| 20 | Task 3 Step 3: `handleCanUseTool` return type assumed without SDK verification | Warning | Added conditional note: return type must match SDK's actual `canUseTool` callback signature from Task 0 Step 3.5 |
| 21 | Task 4: duplicate import step — Step 1 (new) and Step 3 (old) both added same import | Blocker | Removed old Step 3, renumbered Steps 4→3, 5→4 |
| 22 | Task 7 Step 8: `git add` includes `apps/web/src/types/control.ts` — already committed in Task 1 | Minor | Removed redundant file from git add; updated commit message |
| 23 | Task 4: Step numbering — two steps both labeled "Step 2" (AskUserQuestion + ExitPlanMode) | Warning | Renumbered ExitPlanMode to "Step 2b" |
| 24 | Task 2 Step 2: anchor string `pendingPermissions: new Map()` doesn't match actual code (shorthand `pendingPermissions,`) | Blocker | Fixed anchor to reference actual shorthand syntax; added local variable declarations; noted Task 3 replaces the whole block anyway |
| 25 | Task 6 Step 2 + Task 10: `pending.resolve({})` comment says "deny" but code path is "allow with empty answers" | Warning | Fixed all comments to accurately describe behavior; added design note explaining why "allow with empty" is the correct default on disconnect |
| 26 | Task 9: no stated dependency on Tasks 3-5 — intermediate commits have broken status lifecycle | Warning | Added dependency note: Task 9 completes the status lifecycle started in Task 3; must not be skipped or deferred |
| 27 | 7 new `emitter.emit('message', {...})` calls missing `satisfies ServerMessage` — breaks codebase pattern | Warning | Added `satisfies ServerMessage` to all emit calls in Tasks 3, 5, and 9 (matching existing pattern at lines 151, 166, 188, 201) |
| 28 | Task 2 Step 2: dead local variable declarations (`const pendingQuestions = new Map()` etc.) never used — inline `new Map()` already used in cs object | Minor | Removed dead locals; clarified that new maps use inline init (no local variables needed, unlike pre-existing `pendingPermissions`) |
