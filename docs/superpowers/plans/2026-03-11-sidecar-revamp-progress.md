# Sidecar Revamp — Execution Queue

**Plan:** `docs/superpowers/plans/2026-03-11-sidecar-revamp.md`
**Spec:** `docs/superpowers/specs/2026-03-11-sidecar-revamp-design.md`
**Method:** Subagent-driven development (fresh subagent per task, two-stage review)

---

## Chunk 1: Protocol Types & Event Mapper

- [ ] **Task 1:** Create `sidecar/src/protocol.ts` — 27 server events + 7 client messages, fully typed discriminated unions
- [ ] **Task 2:** Create `sidecar/src/event-mapper.ts` + tests — pure SDK→protocol translation for all 22 message types

## Chunk 2: Permission Handler, Session Registry & SDK Session

- [ ] **Task 3:** Create `sidecar/src/permission-handler.ts` + tests — canUseTool routing (AskUserQuestion, ExitPlanMode, elicitation, generic), timeout, abort, drain
- [ ] **Task 4:** Create `sidecar/src/session-registry.ts` — typed session map with sequenced event emission
- [ ] **Task 5:** Create `sidecar/src/sdk-session.ts` — create/resume with long-lived stream loop, sendMessage, listAvailableSessions

## Chunk 3: Routes, WS Handler & Server Wiring

- [ ] **Task 6:** Create `sidecar/src/routes.ts` — HTTP endpoints: create, resume, list, list-available, send, terminate
- [ ] **Task 7:** Rewrite `sidecar/src/ws-handler.ts` — delegates to PermissionHandler, handles all 7 client message types
- [ ] **Task 8:** Rewrite `sidecar/src/index.ts` + delete old files (`types.ts`, `session-manager.ts`, `control.ts`, `control.test.ts`)

## Chunk 4: Frontend Types & Hooks

- [ ] **Task 9:** Expand `apps/web/src/types/control.ts` — 11→27 server event types, new PermissionRequest fields
- [ ] **Task 10:** Update `apps/web/src/hooks/use-control-session.ts` — handle all 27 events, add new state fields (sessionInit, rateLimitStatus, activeTasks, modelUsage, etc.)
- [ ] **Task 11:** Create `apps/web/src/hooks/use-available-sessions.ts` — session picker hook

## Chunk 5: Wiring & Verification

- [ ] **Task 12:** Update `crates/server/src/routes/control.rs` — add proxy routes for create-session and available-sessions
- [ ] **Task 13:** Build & typecheck everything (sidecar tsc, sidecar tests, web tsc, turbo build, cargo check)
- [ ] **Task 14:** Manual end-to-end verification (dev stack, WS connect, send message, permission request)
