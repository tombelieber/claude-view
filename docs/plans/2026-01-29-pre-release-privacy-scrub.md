---
status: done
date: 2026-01-29
---

# Pre-Release Privacy Scrub Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove all personal identifiers from tracked files before open-source release.

**Architecture:** Systematic find-and-replace across 3 file groups (code, docs, config), followed by deletion of archived plans, then a verification sweep. Git history is left as-is.

**Tech Stack:** grep, git, manual edits

---

## Personal Identifiers Scrubbed

All personal identifiers (usernames, org names, author names, project names, email domains) were replaced with generic placeholders (`user`, `example-org`, `OWNER`, `acme.io`, `my-app`, `my-project`, `Author`). See git history for the full substitution map.

## Invariants

- **Tests must still pass** after all code changes (`cargo test -p core`, `cargo test -p server`)
- **No functional logic changes** — only string literals in comments, doc-comments, test fixtures, and config
- **Encoded path patterns must stay structurally valid** — e.g., `--` → `/@` conversion logic must still work in tests

---

### Execution Summary

All 11 tasks completed:

1. **Deleted** `docs/plans/archived/` (11 files with heavy personal info)
2. **Scrubbed** `crates/core/src/discovery.rs` — doc-comments and test fixtures (228 tests pass)
3. **Scrubbed** `crates/server/src/routes/sessions.rs` — test fixtures (107 tests pass)
4. **Scrubbed** `npx-cli/` — repo URL placeholder in index.js, package.json, README.md
5. **Scrubbed** `README.md`, `README.zh-TW.md`, `README.zh-CN.md` — badge URLs
6. **Scrubbed** 9 active plan docs — paths, org names, project names, author
7. **Verified** `.github/workflows/release.yml` and `scripts/release.sh` — already clean
8. **Verified** `CLAUDE.md` — already clean
9. **Verification sweep** — `git grep` for all patterns returned zero matches
10. **Build and test** — `cargo build`, `cargo test`, frontend build all pass
11. **Committed** with descriptive message
