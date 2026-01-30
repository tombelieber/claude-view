---
status: pending
date: 2026-01-29
---

# Phase 4: npx Release Pipeline — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship `npx claude-view` so any user with Node.js can run the app with zero build tools.

**Architecture:** The npx wrapper (`npx-cli/index.js`) and CI pipeline (`.github/workflows/release.yml`) are already built. This plan adds SHA256 checksum verification, automated npm publish to the CI pipeline, and documents what the human must do (create npm account, set GitHub secrets).

**Tech Stack:** GitHub Actions, npm registry, Node.js crypto (SHA256), GitHub Releases

---

## What's Done vs What's Needed

| Component | Status | Action |
|-----------|--------|--------|
| `npx-cli/index.js` (download + cache + launch) | Done | Modify — add checksum verification |
| `npx-cli/package.json` | Done | No change |
| `.github/workflows/release.yml` (build 4 platforms) | Done | Modify — add checksum generation + npm publish |
| GitHub repo (`tombelieber/claude-view`) | Exists | — |
| npm account + NPM_TOKEN secret | Missing | **Human action required** |
| SHA256 checksums in release | Missing | Add to CI |
| npm publish step in CI | Missing | Add to CI |
| npm provenance (`--provenance`) | Missing | Add to CI |

---

## Human Setup Guide (Before Any Code Changes)

These are one-time setup steps that only the human can do. Complete these before running any tasks.

### Step A: Create or verify npm account

1. Go to https://www.npmjs.com/signup (or log in if you have an account)
2. Verify your email address
3. Check that the package name `claude-view` is available:
   ```bash
   npm view claude-view
   ```
   If it returns 404, the name is free. If taken, pick a scoped name like `@example-org/claude-view`.

### Step B: Generate npm access token

1. Log in to https://www.npmjs.com/
2. Click your avatar > **Access Tokens**
3. Click **Generate New Token**
4. Select **Granular Access Token** (recommended over legacy tokens)
5. Settings:
   - **Token name:** `claude-view-github-actions`
   - **Expiration:** 365 days (or no expiry)
   - **Packages and scopes:** Select **Only select packages and scopes**, then add `claude-view`
   - **Permissions:** **Read and write**
6. Click **Generate Token**
7. **Copy the token immediately** — it won't be shown again. It looks like `npm_xxxxxxxxxxxx`.

### Step C: Add NPM_TOKEN to GitHub repo secrets

1. Go to `https://github.com/tombelieber/claude-view/settings/secrets/actions`
2. Click **New repository secret**
3. Name: `NPM_TOKEN`
4. Value: paste the token from Step B
5. Click **Add secret**

### Step D: Enable npm provenance (optional but recommended)

npm provenance requires the GitHub Actions OIDC token. This is already handled by the workflow permissions we add — no extra setup needed.

### Step E: Verify GitHub repo is public

npm provenance only works with public repos. Verify:
- `https://github.com/tombelieber/claude-view` is public
- If private, either make it public or skip the `--provenance` flag

---

## Implementation Tasks

### Task 1: Add SHA256 checksum generation to CI

**Files:**
- Modify: `.github/workflows/release.yml`

**Why:** Without checksums, the npx wrapper downloads and runs a binary with no integrity verification. An attacker who compromises a GitHub Release could inject a malicious binary.

**Step 1: Add checksum generation after artifact download**

In `.github/workflows/release.yml`, modify the `release` job to generate checksums before creating the GitHub Release.

Replace the entire `release` job with:

```yaml
  release:
    name: Create Release
    needs: build
    runs-on: ubuntu-latest

    permissions:
      contents: write

    steps:
      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts
          merge-multiple: true

      - name: Generate checksums
        run: |
          cd artifacts
          sha256sum claude-view-*.tar.gz claude-view-*.zip > checksums.txt
          cat checksums.txt

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          generate_release_notes: true
          files: artifacts/*
```

**Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add SHA256 checksum generation to release pipeline"
```

---

### Task 2: Add checksum verification to npx wrapper

**Files:**
- Modify: `npx-cli/index.js`

**Why:** The wrapper must verify the downloaded binary matches the expected checksum before running it.

**Step 1: Add checksum functions after the existing `extractZip` function**

```javascript
async function downloadChecksums(version) {
  const url = `https://github.com/${REPO}/releases/download/v${version}/checksums.txt`;
  try {
    const data = await download(url);
    const lines = data.toString("utf-8").trim().split("\n");
    const map = {};
    for (const line of lines) {
      const match = line.match(/^([a-f0-9]{64})\s+(.+)$/);
      if (match) {
        map[match[2]] = match[1];
      }
    }
    return map;
  } catch {
    return null;
  }
}

function verifyChecksum(buffer, expectedHash) {
  const crypto = require("crypto");
  const actual = crypto.createHash("sha256").update(buffer).digest("hex");
  if (actual !== expectedHash) {
    console.error("\nChecksum verification FAILED!");
    console.error(`  Expected: ${expectedHash}`);
    console.error(`  Actual:   ${actual}`);
    console.error(`\nThe downloaded binary may be corrupted or tampered with.`);
    console.error(`Try again, or report at https://github.com/${REPO}/issues`);
    process.exit(1);
  }
}
```

**Step 2: Wire into the download flow in `main()`**

After `buffer = await download(url);` and before the `// Clean previous install` comment, insert:

```javascript
    // Verify integrity
    const checksums = await downloadChecksums(VERSION);
    if (checksums) {
      const expectedHash = checksums[platformInfo.artifact];
      if (expectedHash) {
        verifyChecksum(buffer, expectedHash);
      }
    }
```

**Step 3: Commit**

```bash
git add npx-cli/index.js
git commit -m "feat(npx-cli): add SHA256 checksum verification on binary download"
```

---

### Task 3: Add automated npm publish to CI

**Files:**
- Modify: `.github/workflows/release.yml`

**Why:** Currently CI builds binaries and creates a GitHub Release but never publishes to npm. This automates it.

**Step 1: Add npm publish job after the release job**

Append this job to `.github/workflows/release.yml`:

```yaml
  publish-npm:
    name: Publish to npm
    needs: release
    runs-on: ubuntu-latest

    permissions:
      contents: read
      id-token: write

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: "22"
          registry-url: "https://registry.npmjs.org"

      - name: Publish
        run: npm publish --provenance --access public
        working-directory: npx-cli
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
```

**Key details:**
- `id-token: write` enables npm provenance (Sigstore attestation)
- `registry-url` must be set in `setup-node` for auth to work
- `--provenance` proves the package was built by this CI, not a compromised laptop
- `--access public` required for unscoped packages on first publish
- `working-directory: npx-cli` publishes from the npx-cli subdirectory

**Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add automated npm publish with provenance attestation"
```

---

### Task 4: Add version sync check to CI

**Files:**
- Modify: `.github/workflows/release.yml`

**Why:** The git tag (e.g. `v0.1.0`) must match `npx-cli/package.json` version (`0.1.0`). If they drift, the npx wrapper downloads a release that doesn't exist.

**Step 1: Add version check after Checkout in the build job**

Insert this step right after the `Checkout` step in the `build` job:

```yaml
      - name: Verify version matches tag
        run: |
          TAG_VERSION="${GITHUB_REF_NAME#v}"
          PKG_VERSION=$(node -p "require('./npx-cli/package.json').version")
          if [ "$TAG_VERSION" != "$PKG_VERSION" ]; then
            echo "::error::Tag version ($TAG_VERSION) != npx-cli/package.json version ($PKG_VERSION)"
            exit 1
          fi
          echo "Version verified: $TAG_VERSION"
```

**Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add version sync check between git tag and package.json"
```

---

### Task 5: Add release convenience script

**Files:**
- Modify: root `package.json`

**Why:** Bumping version + tagging is error-prone across multiple files. A script standardizes it.

**Step 1: Read root package.json to check existing scripts**

**Step 2: Add release scripts**

Add to root `package.json` scripts section:

```json
"release:bump": "node -e \"const v=process.argv[1]; const fs=require('fs'); const p=JSON.parse(fs.readFileSync('npx-cli/package.json','utf8')); p.version=v; fs.writeFileSync('npx-cli/package.json', JSON.stringify(p,null,2)+'\\n'); console.log('npx-cli/package.json -> '+v);\"",
"release:tag": "node -e \"const v=require('./npx-cli/package.json').version; require('child_process').execFileSync('git',['tag','v'+v],{stdio:'inherit'}); console.log('Tagged v'+v);\"",
"release:push": "git push && git push --tags"
```

Usage:
```bash
bun run release:bump 0.2.0
git add npx-cli/package.json && git commit -m "chore: bump to v0.2.0"
bun run release:tag
bun run release:push
# CI takes over from here
```

**Step 3: Commit**

```bash
git add package.json
git commit -m "chore: add release convenience scripts"
```

---

### Task 6: First publish — dry run validation

**Files:** No changes, validation only.

**Step 1: Verify npx-cli package contents**

```bash
cd npx-cli && npm pack --dry-run
```

Verify:
- Only `index.js`, `package.json`, `README.md` are included (no source code leaks)
- Package name is `claude-view`
- `bin` field points to `./index.js`

**Step 2: Build locally**

```bash
bun run build
cargo build --release -p vibe-recall-server
```

Both must succeed.

**Step 3: Smoke-test the binary**

```bash
mkdir -p /tmp/cv-staging
cp target/release/vibe-recall /tmp/cv-staging/
cp -r dist /tmp/cv-staging/
STATIC_DIR=/tmp/cv-staging/dist /tmp/cv-staging/vibe-recall
# Should start on http://127.0.0.1:47892
```

**Step 4: Verify GitHub secret**

Confirm `NPM_TOKEN` exists at `https://github.com/tombelieber/claude-view/settings/secrets/actions`

---

### Task 7: First release

**Step 1: Decide version** — `0.2.0` (or `0.1.0` if never published)

**Step 2: Bump**

```bash
bun run release:bump 0.2.0
```

**Step 3: Commit all changes**

```bash
git add -A
git commit -m "chore: prepare v0.2.0 release"
```

**Step 4: Tag and push**

```bash
bun run release:tag
bun run release:push
```

**Step 5: Monitor CI** at `https://github.com/tombelieber/claude-view/actions`

Verify:
1. All 4 platform builds succeed
2. GitHub Release created with 5 files (4 archives + checksums.txt)
3. npm publish succeeds

**Step 6: Verify npm**

```bash
npm view claude-view
```

**Step 7: End-to-end test**

```bash
rm -rf ~/.cache/claude-view
npx claude-view@latest
```

Should download, verify checksum, start server on :47892.

---

## Release Checklist (For Every Future Release)

```
[ ] Tests pass: cargo test -p vibe-recall-db && cargo test -p vibe-recall-server
[ ] Frontend builds: bun run build
[ ] Version bumped: bun run release:bump X.Y.Z
[ ] Committed: git commit -m "chore: prepare vX.Y.Z release"
[ ] Tagged: bun run release:tag
[ ] Pushed: bun run release:push
[ ] CI green: 4 platform builds + npm publish
[ ] Verified: npx claude-view@latest works
```

---

## What Claude Does vs What You Do

| Action | Who |
|--------|-----|
| Write checksum verification code (Task 2) | **Claude** |
| Modify CI workflow (Tasks 1, 3, 4) | **Claude** |
| Add release scripts (Task 5) | **Claude** |
| Dry run validation (Task 6) | **Claude** |
| Create npm account (Step A) | **You** |
| Generate npm token (Step B) | **You** |
| Add `NPM_TOKEN` to GitHub secrets (Step C) | **You** |
| Verify repo is public (Step E) | **You** |
| Run `release:tag` + `release:push` (Task 7) | **You** |
| Monitor CI and verify publish | **You** |

---

## Security Model

| Threat | Mitigation |
|--------|-----------|
| Compromised binary on GitHub Release | SHA256 checksum verification in npx wrapper |
| Compromised npm package | npm provenance attestation (Sigstore) |
| Stale npm token | Granular token scoped to `claude-view` only, 365-day expiry |
| Version mismatch (tag vs package.json) | CI fails fast if versions don't match |
| Supply chain (npm account takeover) | npm provenance proves CI origin regardless of account state |
