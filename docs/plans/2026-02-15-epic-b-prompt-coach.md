---
status: draft
date: 2026-02-15
epic: coaching-automation
---

# Epic B: Prompt Coach (Deferred)

> Help users optimize prompts BEFORE they hit Claude — at the point of action, not after the fact.

## Problem

Users write vague or suboptimal prompts. The app can see this in the data (re-edit rates, follow-up counts, session fatigue), but only tells them after the session is over. By then the damage (wasted tokens, frustration) is done.

## Vision

A prompt optimization system that intercepts at the moment of action — before the prompt reaches Claude.

## Phases

### Phase 2a: `/coach` Skill (Ship First)

A Claude Code skill file that users invoke manually before starting a task.

**User flow:**
```
$ claude
> /coach I want to refactor the auth system

Coach: Let me help you write a better prompt. A few questions:

1. Scope: Which files/modules specifically?
   (a) The entire src/auth/ directory
   (b) Just the token refresh flow
   (c) Let me describe...

2. What's wrong with the current code?
   (a) It's hard to test
   (b) It has security issues
   (c) It's too complex
   (d) Let me describe...

3. Do you have tests for this code?
   (a) Yes, full coverage
   (b) Some tests exist
   (c) No tests yet

Based on your answers, here's an optimized prompt:

---
Refactor the token refresh flow in src/auth/tokenRefresh.ts.
The current code is hard to test because [reason].
Look at the existing tests in src/auth/__tests__/ first.
After refactoring, ensure all existing tests still pass.
Write new tests for any new functions.
---

Copy this and paste it into a new session, or I can run it now.
```

**Implementation:** Single `.claude/skills/coach/SKILL.md` file. ~100 lines. Ships in 30 min.

**What it teaches us:** Which interview questions actually improve outcomes. Data feeds back into the pattern engine.

### Phase 2b: Prompt Lab (Dashboard Page)

A page in the claude-view UI that generates optimized prompts based on the user's weakness patterns.

**User flow:**
1. Open claude-view → "Prompt Lab" tab
2. See cards for each pattern weakness: "Your debug sessions have high friction"
3. Click card → get a battle-tested prompt template specific to that weakness
4. Copy to clipboard → paste into Claude Code

**Example cards:**
- "Debug Starter Kit" — structured debugging prompt (symptoms → hypothesis → verification)
- "Refactor Blueprint" — scoped refactoring prompt with test-first approach
- "Feature Spec Interview" — prompt that asks Claude to interview you before coding

**Technical scope:** New React page + static template library. No backend changes. ~1 day.

### Phase 2c: Pre-Prompt Hook (Magic Mode)

Transparent prompt enhancement. User types normally; a hook rewrites the prompt before Claude processes it.

**Technical approach:**
- Claude Code supports [hooks](https://code.claude.com/docs/en/hooks-guide) that run at specific lifecycle points
- A `PreToolUse` or custom hook could intercept prompts
- Hook adds context from the user's pattern data (e.g., "Remember: keep this session under 45 minutes")

**Open questions:**
- Does Claude Code expose a hook for user prompt submission? (Need to verify)
- How much latency does a hook add? (Must be <100ms to feel invisible)
- Privacy: should the hook send prompt data to our backend for analysis?

**Status:** Needs Claude Code hooks API research. Deferred until Phase 2a validates the concept.

### Phase 2d: Autocomplete Suggestions (Long-Term)

Google-search-style autocomplete as the user types in Claude Code.

**Why it's hard:**
- Requires CLI integration we don't control (Claude Code would need to expose a completion API)
- Real-time suggestion engine with <50ms latency
- Context-aware (knows the current project, recent patterns)

**Status:** Idea only. Depends on Claude Code adding extension points for input suggestion.

## Dependencies

- Phase 2a: None (standalone skill file)
- Phase 2b: Pattern engine data (already exists)
- Phase 2c: Claude Code hooks API (external dependency)
- Phase 2d: Claude Code completion API (doesn't exist)

## When to Start

After Epic A (Smart Rules Engine) ships. Phase 2a can be done in parallel as a quick win.
