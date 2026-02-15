---
status: draft
date: 2026-02-15
epic: coaching-automation
---

# Epic C: Trusted Marketplace (Deferred)

> Curated skill/tool recommendations with security trust signals — because the ecosystem is a minefield.

## Problem

Claude Code's skill/plugin ecosystem is growing fast, but there's no trusted curation layer. Users see patterns like "you rarely test first" but don't know which TDD skill to install. Meanwhile, malicious skills can inject harmful instructions, exfiltrate data, or run arbitrary code.

The user's insight: "there're SO FREAKING MANY DANGEROUS MALICIOUS Hack / virus / injection in these tools. so security is a big problem."

## Vision

A "Recommended Tools" section in claude-view that:
1. Maps pattern weaknesses to specific skills/tools that address them
2. Shows trust signals (verified author, source repo, star count, security audit status)
3. Provides 1-click install with a safety preview (what files will be written, what permissions required)

## Trust Badge System

| Badge | Meaning | Criteria |
|-------|---------|----------|
| **Verified** (blue tick) | Reviewed by claude-view team or Anthropic | Manual security audit, known author, no suspicious hooks |
| **Community** (gray tick) | Popular and well-maintained | 50+ stars, active maintenance, no reported issues |
| **Unvetted** (no badge) | Not yet reviewed | New or low-usage, no security audit |
| **Flagged** (red warning) | Known issues reported | Security vulnerability, data exfiltration, malicious behavior |

## Phases

### Phase 3a: Static Curated List (Ship First)

A "Recommended" section on the Insights page with a hand-curated list of ~10-15 skills.

**Each recommendation card shows:**
- Skill name + one-line description
- Trust badge (Verified / Community)
- Which pattern weakness it addresses (e.g., "Addresses: W04 — Test-First Correlation")
- GitHub stars + last updated date
- Author name + link to source
- "View on GitHub" button (no auto-install yet)

**Curation criteria for v1:**
- Only skills from known authors (Anthropic, well-known community members)
- Must have public source code
- Must not register any hooks that run arbitrary code
- Must not require network access
- We manually review each one before adding to the list

**Data source:** Static JSON file in the repo. Updated manually via PR.

**Technical scope:** New React component + static data file. No backend changes. ~2-3 hours.

### Phase 3b: Dynamic Data + Pattern Mapping

- Fetch real-time GitHub star counts + last commit date
- Map recommendations to user's specific pattern weaknesses
- Sort by relevance: skills that address the user's worst patterns appear first
- Show: "Recommended because your debug sessions have high friction"

**Technical scope:** Backend endpoint that fetches GitHub API data. Cache in SQLite. ~1 day.

### Phase 3c: 1-Click Install with Safety Preview

Before installing any skill, show a preview of:
- What files will be created (e.g., `.claude/skills/tdd/SKILL.md`)
- What hooks will be registered (if any)
- What permissions are requested
- What directories it will read/write

User must explicitly approve after seeing the preview.

**Technical scope:** Backend skill installer + frontend safety preview modal. ~2-3 days.

### Phase 3d: Community Submissions + Security Pipeline

- Community can submit skills for review via GitHub PR
- Automated checks: scan for suspicious patterns (eval, exec, fetch to external URLs)
- Manual review queue for verified badge
- Rating/review system in the UI
- Report mechanism for flagged skills

**Technical scope:** Large — community platform features. Defer to v2+.

## Security Principles

1. **No auto-install ever.** User must click AND confirm after safety preview.
2. **Filesystem-only.** Skills are `.md` files. If a skill requires running code (hooks, scripts), that's a higher trust bar.
3. **Source-verifiable.** Every recommended skill links to its public source. Users can inspect before installing.
4. **Separation of concerns.** The marketplace recommends; the user decides. We never install without consent.
5. **Conservative defaults.** Start with 10 hand-curated, verified skills. Grow slowly.

## Pattern → Skill Mapping (Draft)

| Pattern Weakness | Recommended Skill | Why |
|-----------------|-------------------|-----|
| W04: Low test-first correlation | TDD skill | Guides test-before-code workflow |
| P01: Suboptimal prompt length | Prompt coach skill (Epic B) | Interview-based prompt optimization |
| S01: Sessions too long | Session timer reminder | Notification after 45 min |
| W03: Not enough planning | Plan-first skill | Enforces explore → plan → implement |
| B01: High retry patterns | Debugging skill | Structured debug methodology |
| W06: No branch discipline | Git workflow skill | Enforces feature branch creation |

## Dependencies

- Phase 3a: None (static data + frontend component)
- Phase 3b: GitHub API access
- Phase 3c: File system installer (similar to Epic A's rule writer)
- Phase 3d: Community infrastructure (large scope)

## When to Start

After Epic A ships. Phase 3a can be done quickly as a static page.
