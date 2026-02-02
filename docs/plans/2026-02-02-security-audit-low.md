---
status: done
date: 2026-02-02
---

# Security Audit — Low Priority Fixes

> Minor improvements for defense-in-depth and documentation completeness. None are urgent.

**Source:** Full security + README audit performed 2026-02-02 across 5 parallel scans (secrets, dependencies, unsafe code, file exposure, README accuracy).

---

## Task 1: Validate URL schemes in HTML export

**File:** `src/lib/export-html.ts:51-53`

**Problem:** When exporting conversations to HTML, markdown links are converted to `<a>` tags. The URL is not validated against dangerous schemes. A markdown link like `[click](javascript:alert(1))` produces a working `javascript:` href in the exported HTML file.

**Attack surface is narrow:** requires (1) a malicious link in a Claude Code session JSONL, (2) the user exports that session as HTML, (3) the user opens the exported file in a browser and clicks the link.

**Fix:** Add URL scheme validation before rendering the `<a>` tag:

```typescript
const SAFE_SCHEMES = /^https?:\/\//i;
html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, text, url) => {
    const safeUrl = url.replace(/"/g, '&quot;');
    if (!SAFE_SCHEMES.test(url)) {
        return text; // render as plain text, not a link
    }
    return `<a href="${safeUrl}" target="_blank" rel="noopener noreferrer">${text}</a>`;
});
```

---

## Task 2: Scope `id-token: write` to publish job only

**File:** `.github/workflows/release.yml:8-10`

**Problem:** `id-token: write` is declared at the workflow top-level but is only needed by the `publish-npm` job. The `build` and `release` jobs get unnecessary elevated permissions.

**Fix:** Remove `id-token: write` from the top-level `permissions` block. It's already declared in the `publish-npm` job's own `permissions` block (or add it there if missing):

```yaml
# Top-level
permissions:
  contents: write

# publish-npm job only
publish-npm:
  permissions:
    contents: read
    id-token: write
```

*Note: If Task 4 from the Medium plan (pin Actions to SHAs) is done first, this can be done in the same commit.*

---

## Task 3: Add rate limiting middleware (optional)

**Files:** `crates/server/src/lib.rs`

**Problem:** No rate limiting on any endpoint. While this is a localhost tool, it's defense-in-depth against cross-origin abuse (especially if CORS is not yet restricted).

**Fix (optional):** Add `tower_governor` or a simple token-bucket middleware. A generous limit (e.g., 100 req/s) is sufficient — the goal is to prevent automated exfiltration, not throttle normal usage.

**Skip if:** CORS is already restricted to localhost origins (Task 2 of Medium plan).

---

## Task 4: Reduce information in error responses

**File:** `crates/server/src/error.rs:86-113`

**Problem:** Some error variants include file paths in the response body (e.g., `ParseError::NotFound { path }`). While acceptable for a localhost tool, this could aid path traversal exploitation.

**Fix:** Review error responses and ensure file paths are logged server-side but not included in HTTP response bodies. Return generic messages like `"Session not found"` instead of `"File not found: /Users/.../.claude/projects/.../session.jsonl"`.

---

## Task 5: Update platform badge in all READMEs

**Files:** `README.md:19`, `README.zh-TW.md:19`, `README.zh-CN.md:19`

**Problem:** Badge says "Platform-macOS" but CI builds for macOS, Linux, and Windows.

**Fix:** Update the badge URL in all three files:
```html
<img src="https://img.shields.io/badge/Platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg">
```

*Note: If Task 3 from the Critical plan (sync platform tables) is done first, this should be done in the same commit.*

---

## Task 6: Document `CLAUDE_VIEW_PORT` env var in main README

**Files:** `README.md`

**Problem:** The npx-cli README documents `CLAUDE_VIEW_PORT` and `PORT` env vars, but the main README does not mention them. Users who find the repo on GitHub won't know they can customize the port.

**Fix:** Add a "Configuration" section or add to the existing Quick Start:

```markdown
### Configuration

| Env Variable | Default | Description |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` | `47892` | Override the default port |
| `PORT` | `47892` | Alternative port override |
```

---

## Task 7: Document all export formats in README

**Files:** `README.md:50`, `README.zh-TW.md`, `README.zh-CN.md`

**Problem:** The feature table only mentions "Export to HTML" but the app also supports PDF and Markdown export.

**Fix:** Update the feature description:
```
Export conversations | Share or archive as HTML, PDF, or Markdown
```

---

## Task 8: Add `test:client` command to README dev table

**File:** `README.md` (Development section)

**Problem:** The development commands table lists `bun test` (cargo test) but not `test:client` (vitest for frontend tests). Also, describing `bun test` as "Run Rust test suite" is confusing since `bun test` normally invokes Bun's test runner.

**Fix:** Add to the commands table:
```
| bun test         | Run Rust test suite (cargo test --workspace) |
| bun test:client  | Run frontend tests (vitest)                  |
```

---

## Task 9: Monitor transitive dependency updates

**No code change needed.** Track these for future dependency updates:

| Crate | Issue | Advisory | Fix |
|-------|-------|----------|-----|
| `instant` 0.1.13 | Unmaintained | RUSTSEC-2024-0384 | Wait for tantivy upgrade replacing with `web-time` |
| `number_prefix` 0.4.0 | Unmaintained | RUSTSEC-2025-0119 | Wait for indicatif upgrade |
| `lru` 0.12.5 | Unsound `IterMut` | RUSTSEC-2026-0002 | Wait for tantivy upgrade |

**Recommended:** Add `cargo audit` to CI pipeline to catch new advisories automatically.

---

## Verification

After all fixes:
- [ ] HTML export with `[test](javascript:alert(1))` renders as plain text, not a link
- [ ] GitHub Actions `id-token: write` only appears in `publish-npm` job permissions
- [ ] Error responses from invalid session requests don't include file paths
- [ ] Platform badge shows all three platforms
- [ ] `CLAUDE_VIEW_PORT` documented in main README
- [ ] Export feature mentions HTML, PDF, and Markdown
- [ ] `test:client` command appears in dev table
