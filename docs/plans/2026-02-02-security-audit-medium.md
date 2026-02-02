---
status: done
date: 2026-02-02
---

# Security Audit — Medium Fixes

> Security vulnerabilities and documentation inconsistencies that should be fixed before wider adoption.

**Source:** Full security + README audit performed 2026-02-02 across 5 parallel scans (secrets, dependencies, unsafe code, file exposure, README accuracy).

---

## Task 1: Fix path traversal in session route

**File:** `crates/server/src/routes/sessions.rs:288-302`

**Problem:** The `get_session()` handler joins user-controlled URL path segments (`project_dir`, `session_id`) into a filesystem path without sanitization. After URL-decoding, `../` sequences can escape `~/.claude/projects/`:

```
GET /api/session/..%2F..%2F..%2Fetc/passwd
```

While limited by the `.jsonl` extension (only `.jsonl` files can be read), this still allows reading arbitrary `.jsonl` files anywhere on the filesystem.

**Fix:** After constructing `session_path`, canonicalize it and verify it starts with `projects_dir`:

```rust
let canonical = session_path.canonicalize()
    .map_err(|_| ApiError::SessionNotFound(session_id.clone()))?;
if !canonical.starts_with(&projects_dir) {
    return Err(ApiError::SessionNotFound(session_id.clone()));
}
```

Apply the same pattern to any other route that builds paths from URL parameters (check `get_project_sessions`, export routes, etc.).

---

## Task 2: Restrict CORS to localhost origins

**File:** `crates/server/src/lib.rs:71-74`

**Problem:** `allow_origin(Any)` lets any website make cross-origin requests to `localhost:47892`. While the server binds to `127.0.0.1`, a malicious website visited in the user's browser can silently exfiltrate all Claude Code session data, trigger git sync, and modify settings. The code comment says "for development" but this ships in production builds.

**Fix:** Replace `Any` with explicit localhost origins:

```rust
use tower_http::cors::AllowOrigin;

let cors = CorsLayer::new()
    .allow_origin(AllowOrigin::predicate(|origin, _| {
        if let Ok(origin) = origin.to_str() {
            origin.starts_with("http://localhost:")
                || origin.starts_with("http://127.0.0.1:")
        } else {
            false
        }
    }))
    .allow_methods(Any)
    .allow_headers(Any);
```

---

## Task 3: Untrack Playwright screenshots exposing personal data

**Files:** `.playwright-mcp/fluffy-session-detail.png`, `.playwright-mcp/fluffy-sessions-test.png`, `.playwright-mcp/session-view-test.png`

**Problem:** These 3 PNG files were committed before the `.gitignore` rule was added. They expose:
- macOS username (`TBGor`)
- Notion page IDs and URLs
- Internal project names (`@vicky-ai/fluffy`, `Famatch.io`, `taipofire-donations`)
- Working directory structure

**Fix:**
```bash
git rm --cached .playwright-mcp/*.png
git commit -m "chore: untrack playwright screenshots exposing personal data"
```

The existing `.gitignore` rule will prevent future commits.

**Note:** These files will remain in git history. If the data is sensitive enough to warrant full removal, run `git filter-branch` or use BFG Repo-Cleaner. For most cases, untracking is sufficient.

---

## Task 4: Pin GitHub Actions to SHA hashes

**File:** `.github/workflows/release.yml`

**Problem:** All 10 action references use floating version tags (`@v4`, `@v2`, `@stable`). The `publish-npm` job has `id-token: write` permission — a compromised upstream action could inject code into the npm publish pipeline.

**Fix:** Pin each action to its current commit SHA. Example:

```yaml
# Before
- uses: actions/checkout@v4
# After (pin to the current v4.x.x release SHA)
- uses: actions/checkout@b4ffde65f46336ab88eb53be808477a3936bae11  # v4.2.2
```

Do this for all 10 action references. Add a comment with the version number for readability.

Also: Move `id-token: write` from the top-level `permissions` block (line 10) down into only the `publish-npm` job's `permissions` block, following least-privilege.

---

## Task 5: Drop unused `rsa` crate via sqlx feature flags

**File:** `crates/db/Cargo.toml` (sqlx dependency)

**Problem:** `rsa` 0.9.10 has RUSTSEC-2023-0071 (Marvin Attack timing sidechannel). It's pulled in by `sqlx-mysql`, but this project uses SQLite. The vulnerable code never runs, but it inflates the attack surface and binary size.

**Fix:** Ensure sqlx uses `default-features = false` and only enables the features you need:

```toml
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "sqlite"] }
```

Verify with `cargo audit` after the change.

---

## Task 6: Fix version mismatch across manifests

**Files:** `Cargo.toml:7`, `package.json:3`, `npx-cli/package.json:3`

**Problem:** Workspace `Cargo.toml` says `0.1.0`, root `package.json` says `0.1.0`, but the actually published npm package (`npx-cli/package.json`) is at `0.2.1`. Contributors see conflicting version numbers.

**Fix:** Update `Cargo.toml` and root `package.json` version to match `0.2.1`. Consider updating `scripts/release.sh` to bump all three files together.

---

## Task 7: Fix Cargo.toml repository URL

**File:** `Cargo.toml:10`

**Problem:** `repository = "https://github.com/user/vibe-recall"` — this is a placeholder that was never updated. The actual repo is `https://github.com/tombelieber/claude-view`.

**Fix:**
```toml
repository = "https://github.com/tombelieber/claude-view"
```

---

## Task 8: Add Development section to Chinese READMEs

**Files:** `README.zh-TW.md`, `README.zh-CN.md`

**Problem:** The English README has a full Development section (commands, testing, releasing). Both Chinese READMEs end after the Platform Roadmap with no development instructions.

**Fix:** Translate the Development section from `README.md` into both Chinese README files.

---

## Task 9: Correct "Rich previews" feature description

**Files:** `README.md:48`, `README.zh-TW.md`, `README.zh-CN.md`

**Problem:** "See files touched, tools used, skills invoked — at a glance" — but files touched is NOT on the session card. It requires clicking into the session. The card shows tool counts and skills only.

**Fix:** Change to: "See tools used, skills invoked — at a glance. Drill into sessions for files touched."

---

## Verification

After all fixes:
- [ ] Path traversal test: `curl localhost:47892/api/session/..%2F..%2F..%2Fetc/passwd` returns 404, not a file read error
- [ ] CORS test: `curl -H "Origin: https://evil.com" -I localhost:47892/api/projects` does NOT include `Access-Control-Allow-Origin: https://evil.com`
- [ ] `git status` shows `.playwright-mcp/*.png` removed from tracking
- [ ] `cargo audit` shows no medium+ vulnerabilities
- [ ] All three manifest files show the same version
- [ ] `Cargo.toml` repository URL points to `tombelieber/claude-view`
- [ ] Chinese READMEs have Development sections
- [ ] `cargo test --workspace` passes
- [ ] `bun test` passes
