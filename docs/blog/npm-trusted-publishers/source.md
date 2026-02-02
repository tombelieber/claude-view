# npm Trusted Publishers Broke My CI and I Mass-Blamed OIDC For 3 Days

> TL;DR: npm trusted publishing requires **Node 24 / npm >= 11.5.1**. If you're on Node 22, OIDC auth silently fails and npm gives you the most misleading error message of all time.

---

## The Setup

I'm building [claude-view](https://github.com/tombelieber/claude-view) — a tool that lets you browse your Claude Code sessions in a pretty web UI. It ships as a Rust binary via `npx`:

```bash
npx claude-view
```

The release pipeline is straightforward:
1. Push a git tag (`v0.2.1`)
2. GitHub Actions builds for 4 platforms (macOS ARM/Intel, Linux, Windows)
3. Creates a GitHub Release with SHA256 checksums
4. Publishes the npx wrapper to npm

Steps 1-3 worked first try. Step 4? That's where I lost 3 days of my life.

---

## The Crime Scene

Here's the npm publish job. Clean. Simple. By the book.

```yaml
publish-npm:
  name: Publish to npm
  needs: release
  runs-on: ubuntu-latest
  permissions:
    contents: read
    id-token: write  # <-- for OIDC
  steps:
    - uses: actions/checkout@v4
    - uses: actions/setup-node@v4
      with:
        node-version: "22"
        registry-url: "https://registry.npmjs.org"
    - run: npm publish --provenance --access public
      working-directory: npx-cli
```

I configured trusted publishers on npmjs.com. Repository owner, repo name, workflow filename — triple-checked everything. Hit publish.

CI output:

```
npm notice publish Signed provenance statement with source and build information from GitHub Actions
npm notice publish Provenance statement published to transparency log: https://search.sigstore.dev/?logIndex=904858389
npm notice Access token expired or revoked. Please try logging in again.
npm error 404 Not Found - PUT https://registry.npmjs.org/claude-view
```

Read that again. **Provenance signing succeeded.** The OIDC token worked well enough to sign and publish an attestation to Sigstore. Then — in the very next line — npm says "Access token expired or revoked."

What token? I didn't give you a token. That was the whole point.

---

## The Five Stages of Grief

### Stage 1: Denial

*"My trusted publisher config must be wrong."*

I deleted and re-created it. Checked the workflow filename. Checked the repo owner. Checked the case sensitivity. Everything matched. Failed again.

### Stage 2: Anger

*"OIDC doesn't actually work. The docs are lying."*

I Googled "npm trusted publishers not working" and found exactly zero helpful results because the feature was too new. Every Stack Overflow answer was about `NPM_TOKEN`. Every blog post was about `NODE_AUTH_TOKEN`.

### Stage 3: Bargaining

*"Fine, I'll just use a token."*

I generated a granular access token, added it as a GitHub secret, wired up `NODE_AUTH_TOKEN`. This would have worked. But it felt wrong — like putting a key under the doormat when you have a fingerprint scanner.

### Stage 4: Depression

*"Maybe I should just `npm publish` manually for every release like a caveman."*

I actually did this for v0.2.0. Manually. From my laptop. Like it's 2019.

### Stage 5: Acceptance (aka Actually Reading The Docs)

Then I found this line buried in the [npm trusted publishers docs](https://docs.npmjs.com/trusted-publishers):

> **This feature requires npm CLI v11.5.1 or later.**
> **You need to use Node 24.X when publishing.**

I checked my CI:

```
npm: 10.9.4
node: v22.22.0
```

Node 22 ships with npm 10. Trusted publishing needs npm 11.5.1+. Which only ships with Node 24.

**The OIDC signing worked** because Sigstore uses a separate code path. **The OIDC authentication failed** because npm 10 doesn't know how to exchange the OIDC token for registry auth. It fell back to looking for a traditional token, found nothing, and gave me the world's most confusing error message.

---

## The Fix

Two lines. Two lines cost me 3 days.

```diff
       - uses: actions/setup-node@v4
         with:
-          node-version: "22"
+          node-version: "24"
           registry-url: "https://registry.npmjs.org"

-      - run: npm publish --provenance --access public
+      - run: npm publish --access public
```

That's it:
1. **Node 22 → Node 24** (gets npm >= 11.5.1 with OIDC auth support)
2. **Remove `--provenance`** (automatic with trusted publishing — the flag is redundant)

No `NPM_TOKEN`. No `NODE_AUTH_TOKEN`. No secrets to rotate. Pure OIDC.

---

## The Working Workflow

For the copy-pasters (I respect you), here's the complete working `publish-npm` job:

```yaml
publish-npm:
  name: Publish to npm
  needs: release  # or whatever your build job is called
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
      working-directory: npx-cli  # adjust to your package dir
```

### npmjs.com Setup (One-Time)

1. Publish your package manually first (trusted publishers can only be configured on existing packages):
   ```bash
   cd your-package && npm login && npm publish --access public
   ```
2. Go to `https://www.npmjs.com/package/YOUR-PACKAGE/access`
3. Under **Publishing access**, add a trusted publisher:
   - **Repository owner**: your GitHub username or org
   - **Repository name**: your repo name
   - **Workflow filename**: `release.yml` (must match exactly, including extension)
   - **Environment**: leave blank (unless you use GitHub Environments)

### Checklist

- [ ] Package exists on npm (manual first publish)
- [ ] Trusted publisher configured on npmjs.com
- [ ] `node-version: "24"` in `setup-node` (NOT 22, NOT 20, NOT "lts/*")
- [ ] `registry-url: "https://registry.npmjs.org"` in `setup-node`
- [ ] `id-token: write` in job permissions
- [ ] `contents: read` in job permissions
- [ ] GitHub repo is **public** (provenance requires public repos)
- [ ] No `NODE_AUTH_TOKEN` or `NPM_TOKEN` env var set (let OIDC handle it)

---

## Why The Error Message Is So Bad

Here's what npm 10 does when you run `npm publish --provenance` in a GitHub Actions environment:

1. Detects OIDC environment ✅
2. Gets an OIDC token from GitHub ✅
3. Signs provenance with Sigstore using that token ✅
4. Tries to authenticate with the npm registry using... a traditional token ❌
5. Finds no `NODE_AUTH_TOKEN` ❌
6. Reports "Access token expired or revoked" ❌❌❌

The error says "expired or revoked" — not "missing." It implies you HAD a token that went bad. You didn't have one at all. npm just doesn't have a code path for "I see you're trying OIDC but I'm too old to understand it."

npm 11.5.1+ adds the missing step: exchange the OIDC token for a short-lived npm registry token. That's the whole feature.

---

## Things That Will NOT Fix This

Saving you some time:

| What You'll Try | Why It Won't Work |
|----------------|-------------------|
| Re-creating the trusted publisher config | Config is fine, npm CLI is too old |
| Adding `NPM_TOKEN` secret | Works, but defeats the purpose of trusted publishing |
| Adding `NODE_AUTH_TOKEN` env var | Same — you're back to managing secrets |
| Using `--provenance` flag | Already included in npm 11.5.1+ by default |
| Switching to `npm@latest` without updating Node | npm 11 needs Node 24 runtime |
| Praying | Tested. Ineffective. |

---

## The Takeaway

npm trusted publishing is real and it works. Zero secrets, OIDC-authenticated, Sigstore-attested. It's genuinely better than managing tokens.

But the version requirement is a trap. The docs mention it, but it's easy to miss when every GitHub Actions tutorial defaults to `node-version: "22"` or `"lts/*"`. And the error message actively misleads you into thinking the problem is your configuration, not your runtime.

**If you remember one thing from this post:** `node-version: "24"`.

---

*Published from the [claude-view](https://github.com/tombelieber/claude-view) trenches. If you use Claude Code, give it a spin — `npx claude-view` — and star the repo if it saves you time.*
