---
status: draft
date: 2026-02-15
---

# Paid Feature: LLM-Powered Classification & Analysis

## Context

Session classification via `claude -p` works but has fundamental issues that make it unsuitable as a free, always-on feature:

1. **Slow** — Each classification takes 15-20s (CLI process spawn + API round-trip)
2. **Expensive** — Uses the user's own Claude Code subscription quota (~$0.025/call with Haiku)
3. **Fragile** — Depends on Claude CLI env var stripping, stdin nulling, nested session avoidance
4. **Cannibalistic** — Burns the user's own Claude Code usage to analyze their Claude Code usage

This is a natural **paid feature boundary**. Classification adds real value (categorize 1200+ sessions, surface patterns) but costs real money to run.

## Proposed Tiers

```
┌─────────────────────────────────────────────────────────┐
│  Free Tier                                               │
│  - All local analytics (metrics, trends, patterns)       │
│  - Session browsing, search, export                      │
│  - Mission Control (live monitoring)                     │
│  - Manual classification (user clicks per-session)       │
│  - BYO provider: user's own Claude CLI (current impl)   │
│                                                          │
│  ════════════════════════════════════════════════════════│
│                                                          │
│  Pro Tier (Paid)                                         │
│  - Hosted classification API (we pay for LLM calls)     │
│  - Bulk classify all sessions (1-click)                  │
│  - Auto-classify new sessions on ingest                  │
│  - Advanced pattern analysis (cross-session insights)    │
│  - Priority models (Sonnet/Opus for higher accuracy)     │
│  - BYO API key: plug any LLM provider                   │
└─────────────────────────────────────────────────────────┘
```

## Provider Strategy (3 options, not mutually exclusive)

| Option | Who Pays | UX | Effort |
|--------|----------|-----|--------|
| **A. User's Claude CLI** (current) | User | Click per session, 15-20s wait, eats their quota | Done |
| **B. Hosted API** | Us (metered) | 1-click bulk, ~2s/session via direct API, auth via license key | Medium |
| **C. BYO API Key** | User (direct) | User pastes Anthropic/OpenAI key in Settings, we call API directly (no CLI spawn) | Low |

**Phase 1 (free trial):** Option A is already shipped. Users can classify sessions using their own CLI.

**Phase 2 (monetization):** Options B and C. Hosted API is the primary paid path. BYO key is the power-user escape hatch.

## Free Trial Design

- First N classifications free (e.g., 50 sessions) via hosted API
- After that: bring your own key OR subscribe to Pro tier
- Free tier always allows BYO Claude CLI (Option A) with no limit

## Technical Requirements (Future)

### Option B: Hosted API
- Lightweight proxy service (Cloudflare Worker or similar)
- License key validation (tied to GitHub account or email)
- Rate limiting per user (prevent abuse)
- Anthropic API key on our side (Haiku for cost efficiency)
- Usage metering for billing

### Option C: BYO API Key
- Settings UI: paste API key (encrypted at rest in local SQLite)
- Direct Anthropic API calls from Rust (no CLI spawn)
- Support multiple providers: Anthropic, OpenAI, Google, local (Ollama)
- Trait already exists: `LlmProvider` in `crates/core/src/llm/provider.rs`

## Current State

- `ClaudeCliProvider` (Option A) is shipped and working
- `LlmProvider` trait exists — new providers just implement `classify()` and `complete()`
- Classification prompt, parsing, DB persistence all working
- Frontend: ClassifyButton, useClassifySingle hook, CategoryBadge all shipped

## What NOT to Build Yet

- No hosted API infrastructure until we have paying users interested
- No BYO key UI until we validate demand
- No auto-classify — keep it user-initiated (click per session or bulk button)
- No free trial metering — just ship the CLI option and gauge interest

## Dependencies

- GTM launch (need users first)
- Pricing research (what would users pay for this?)
- Provider trait is already extensible — adding new providers is ~100 lines each
