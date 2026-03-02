---
title: Session Intelligence — Classification, Modes & Plan Runner
status: draft-rfc
feature: session-intelligence
created: 2026-03-01
depends_on: [plugin-system, mission-control]
---

# Session Intelligence — Design RFC (DRAFT)

> Full design document lives in the private GTM repo:
> `claude-view-gtm/plans/backlog/2026-03-01-session-intelligence-design.md`

## Summary

Four-layer architecture for session work-type intelligence:

1. **Classification Engine** — Hybrid heuristic rules + Haiku observer (zero context pollution)
2. **Session Modes** — Human-chosen work modes that auto-load skills
3. **Plan Runner** — Workflow pipeline with 3-layer verification gates
4. **Plan Dashboard** — Kanban view of plan files, dependencies, session links

## Status

- Pain points: **DECIDED** (parallel session confusion, unmanaged plans, manual skills, no workflow visibility)
- Architecture: **HYPOTHESIS** (all 4 layers need validation before implementation)
- Classification approach: Heuristic Tier 1 + Haiku Tier 2 (replaced HDBSCAN v0.1)

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Classification method | Heuristic rules + Haiku fallback | Zero context pollution, no cold start, no ML pipeline |
| Mode activation | Human-chosen only, never auto | Modes change behavior; behavior changes must be intentional |
| Plan file format | YAML frontmatter (SDD-inspired) | Machine-parseable, backward-compatible, OpenSpec/spec-kit aligned |
| Priority assignment | Never auto-assigned | User decides priority; system only suggests |

## Next Steps

See Validation Plan (V1-V5) in the full RFC for pre-implementation validation steps.
