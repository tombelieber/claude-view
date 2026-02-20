---
title: "npm Trusted Publishers Broke My CI and I Blamed OIDC For 3 Days"
slug: npm-trusted-publishers-node-version-trap
tags: npm, github-actions, ci-cd, nodejs, oidc
subtitle: "The 2-line fix for the most misleading error in npm history"
# canonical_url: https://your-blog.com/npm-trusted-publishers
cover_image: ./images/cover-node-22-to-24.png  # Upload to Hashnode CDN first, then paste URL here
enableToc: true
---

> npm trusted publishing requires **Node 24 / npm >= 11.5.1**. On Node 22, OIDC auth silently fails and npm says "Access token expired" — when you never gave it a token.

![Cover: Node 22→24](./images/cover-node-22-to-24.png)
<!-- Hashnode: Upload via editor, 1600x840 recommended. The cover_image frontmatter handles the hero display. -->

---

# The Setup

I'm building [claude-view](https://github.com/tombelieber/claude-view) — browse your Claude Code sessions in a web UI. Ships as a Rust binary via `npx claude-view`.

My CI pipeline: push a git tag → build 4 platforms → GitHub Release → publish to npm.

Everything worked except npm publish. For 3 days.

# The Crime Scene

The publish job. Textbook setup:

```yaml
publish-npm:
  runs-on: ubuntu-latest
  permissions:
    contents: read
    id-token: write  # OIDC
  steps:
    - uses: actions/checkout@v4
    - uses: actions/setup-node@v4
      with:
        node-version: "22"
        registry-url: "https://registry.npmjs.org"
    - run: npm publish --provenance --access public
```

Trusted publishers configured on npmjs.com. Triple-checked. Result:

![CI run #7 — all builds pass, Publish to npm fails](./images/ci-failure-publish-to-npm.png)

```
npm notice publish Signed provenance statement ✅
npm notice publish Provenance published to Sigstore ✅
npm notice Access token expired or revoked ❌
npm error 404 Not Found ❌
```

**Provenance signing succeeded.** OIDC worked for Sigstore. Then npm says my token is "expired."

I didn't have a token. That was the whole point.

# The Five Stages of Grief

**Denial:** "My config must be wrong." Deleted and re-created the trusted publisher. Checked case sensitivity. Everything matched. Failed again.

**Anger:** "OIDC doesn't work. The docs are lying." Googled "npm trusted publishers not working." Zero useful results — feature too new.

**Bargaining:** "Fine, I'll use a token." Generated a granular access token, added `NODE_AUTH_TOKEN`. Would have worked. Felt like putting a key under the doormat when you own a fingerprint scanner.

**Depression:** "I'll just publish manually." I actually did this for v0.2.0. From my laptop. Like it's 2019.

**Acceptance:** Found this in the [npm docs](https://docs.npmjs.com/trusted-publishers):

> **This feature requires npm CLI v11.5.1 or later. You need Node 24.X.**

My CI was running **npm 10.9.4** on **Node 22**. npm 10 can sign Sigstore attestations (that code path already existed) but can't exchange OIDC tokens for registry auth. That part was added in npm 11.5.1.

So npm 10 signs provenance, then falls back to traditional token auth, finds nothing, and lies to your face about what went wrong.

<!-- OPTIONAL: If you want to add a flow diagram later, Excalidraw works well here -->

# The Fix

Two lines:

```diff
     - uses: actions/setup-node@v4
       with:
-        node-version: "22"
+        node-version: "24"
         registry-url: "https://registry.npmjs.org"

-    - run: npm publish --provenance --access public
+    - run: npm publish --access public
```

1. **Node 22 → 24** — gets npm 11.5.1+ with OIDC registry auth
2. **Drop `--provenance`** — automatic with trusted publishing

No `NPM_TOKEN`. No `NODE_AUTH_TOKEN`. No secrets. Pure OIDC.

![CI run #8 — everything green, including Publish to npm](./images/ci-success-node-24.png)

# Complete Working Workflow

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
        node-version: "24"
        registry-url: "https://registry.npmjs.org"

    - name: Publish
      run: npm publish --access public
      working-directory: my-package
```

## npmjs.com Setup (One-Time)

1. Manually publish first: `npm login && npm publish --access public`
2. Go to `npmjs.com/package/YOUR-PKG/access`
3. Add trusted publisher — repo owner, repo name, workflow filename (exact, case-sensitive), environment blank

## Pre-Flight Checklist

- `node-version: "24"` (NOT 22, NOT "lts/*")
- `registry-url: "https://registry.npmjs.org"` in setup-node
- `id-token: write` permission on the job
- GitHub repo is public
- No `NODE_AUTH_TOKEN` or `NPM_TOKEN` env var set

# Why The Error Message Is So Bad

What npm 10 does in GitHub Actions:

1. Detects OIDC environment ✅
2. Gets OIDC token from GitHub ✅
3. Signs provenance with Sigstore ✅
4. Tries registry auth with... a traditional token ❌
5. Finds nothing ❌
6. Reports "Access token expired or revoked" ❌❌❌

It says "expired or revoked" — not "missing." Implies you had a token that went bad. You didn't. npm 10 just can't say "I don't understand OIDC auth yet."

# Things That Will NOT Fix This

- **Re-creating the trusted publisher config** — Config is fine, CLI is too old
- **Adding `NPM_TOKEN` secret** — Works, but defeats the purpose
- **`npm@latest` without Node 24** — npm 11 requires Node 24 runtime
- **Praying** — Tested. Ineffective.

# Takeaway

npm trusted publishing works beautifully. Zero secrets, OIDC-only, Sigstore-attested. The trap is the version requirement. Every GitHub Actions tutorial defaults to Node 22. The error message blames your config instead of your runtime.

**One thing to remember:** `node-version: "24"`.

---

*Written while shipping [claude-view](https://github.com/tombelieber/claude-view). Browse your Claude Code sessions: `npx claude-view`.*
