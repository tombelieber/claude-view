# Custom Skill Registry Fix — Design

**Date:** 2026-02-21
**Status:** Approved

## Problem

Skills invoked via the Skill tool (`/prove-it`, `/auditing-plans`, `/shippable`) don't appear in the "Top Skills" analytics dashboard. The registry builder only scans `~/.claude/plugins/installed_plugins.json` and misses two other skill sources that Claude Code natively supports:

| Source | Path | Scanned? |
|--------|------|----------|
| Installed plugins | `~/.claude/plugins/installed_plugins.json` | Yes |
| User-level custom skills | `~/.claude/skills/*/SKILL.md` | **No** |
| Project-level custom skills | `<project>/.claude/skills/*/SKILL.md` | **No** |

When the indexer encounters a Skill tool_use with `{"skill": "prove-it"}`, it calls `registry.lookup("prove-it")`, gets `None`, classifies as `Rejected { reason: "not_in_registry" }`, and discards it. The invocation is never inserted into the `invocations` table and never appears in stats.

## Solution

Extend `build_registry()` in `crates/core/src/registry.rs` to also scan:

1. `{claude_dir}/skills/*/SKILL.md` — user-level custom skills
2. Project-level `.claude/skills/*/SKILL.md` — for each known project path

### Naming Convention

| Source | Qualified ID | Example |
|--------|-------------|---------|
| Plugin skill | `{plugin_name}:{skill_name}` | `superpowers:brainstorming` |
| User custom skill | `user:{skill_name}` | `user:prove-it` |
| Project custom skill | `project:{skill_name}` | `project:ui-ux-pro-max` |

`user:` and `project:` act as synthetic plugin names, following the existing `{prefix}:{name}` convention without collision risk.

### Registry Changes

In `build_registry()`, after scanning installed plugins (step 2) and before registering built-in tools (step 3):

**Step 2a:** Scan `{claude_dir}/skills/` for direct subdirectories containing `SKILL.md`. Register each as `InvocableInfo { id: "user:{name}", kind: Skill, plugin_name: None }`.

**Step 2b:** Accept an optional list of project paths. For each, scan `{project_path}/.claude/skills/*/SKILL.md`. Register each as `InvocableInfo { id: "project:{name}", kind: Skill, plugin_name: None }`.

### No Changes Needed

- **`classify_tool_use()`** — `Registry::lookup()` already tries qualified then bare name. `lookup("prove-it")` will now find `user:prove-it` via bare name map.
- **`SkillStat` type** — stays `{ name, count }`. No source field needed.
- **Frontend** — skills appear in existing Top Skills card with no UI changes.
- **Dashboard query** — already queries all invocations with `kind = 'skill'`.

### Dedup Rules

- `global_seen_ids` already prevents duplicate qualified IDs
- If a skill exists at both user-level and project-level, user-level wins (scanned first)
- If a skill name collides with a plugin skill, plugin wins (scanned first)

### Re-indexing

Existing sessions won't pick up newly-registered skills until re-indexed. This happens naturally on schema changes or `--reindex`. No forced re-index needed — new sessions will track correctly immediately.

### No Signature Change Needed for User-Level

User-level skills live at `{claude_dir}/skills/` which is already reachable from the existing `claude_dir` parameter. No callers need updating.

Project-level skills (at `<project>/.claude/skills/`) would require passing project paths — deferred to a follow-up since only 1 of 11 missing skills is project-level.

## Approach Rejected

- **Source badges in UI** — adds visual noise without solving the core problem
- **Grouped sections by source** — over-engineered for a simple leaderboard
