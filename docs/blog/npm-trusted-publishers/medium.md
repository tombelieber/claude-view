# npm Trusted Publishers Broke My CI and I Blamed OIDC For 3 Days

![Cover: Node 22→24](./images/cover-node-22-to-24.png)
<!-- Medium: Upload via the editor's image tool as the hero/featured image -->

## The 2-line fix nobody told me about

npm recently shipped trusted publishing — a way to publish packages from GitHub Actions without storing secrets. No tokens. No rotation. Just OIDC magic.

I was hyped. I configured everything by the book. Then I spent three days staring at this error:

> Access token expired or revoked. Please try logging in again.

What token? I was using trusted publishing. There was no token to expire.

- - -

## What I built

I'm working on a tool called claude-view that lets you browse your Claude Code session history in a web UI. It ships as a Rust binary through npx — you run `npx claude-view` and it downloads the right binary for your platform.

The release pipeline is simple: push a git tag, GitHub Actions builds for four platforms, creates a GitHub Release, and publishes the npm wrapper. The first three steps worked on the first try. The npm publish? That's where things went sideways.

- - -

## The confusing part

Here's what the CI logs showed:

![CI run #7 — all builds pass, Publish to npm fails](./images/ci-failure-publish-to-npm.png)
*All 4 builds green. Create Release green. Publish to npm? Dead.*

```
Signed provenance statement ✅
Provenance published to Sigstore ✅
Access token expired or revoked ❌
404 Not Found ❌
```

Look at that sequence. The OIDC token worked well enough to sign a provenance attestation and publish it to Sigstore's transparency log. Then, in the very next line, npm says my token is expired.

Provenance signing uses OIDC. Authentication uses OIDC. They're the same token. How does one work and the other fail?

- - -

## The debugging spiral

I went through the classics.

I re-created the trusted publisher configuration on npmjs.com. Checked the repository owner, name, and workflow filename for case sensitivity. Everything matched. Failed again.

I googled "npm trusted publishers not working" and found nothing useful. The feature was too new. Every answer online was about NPM_TOKEN and NODE_AUTH_TOKEN — the exact thing trusted publishing was supposed to replace.

I briefly gave in and created a granular access token. Added it as a GitHub secret. It would have worked. But it felt wrong, like buying a combination lock for a door that has a fingerprint scanner.

I even published v0.2.0 manually from my laptop. Like it was 2019.

- - -

## The answer was in the version number

Then I found this line in the npm trusted publishers documentation:

**This feature requires npm CLI v11.5.1 or later. You need Node 24.X.**

I checked my CI. It was running npm 10.9.4 on Node 22.

That was it. Node 22 ships with npm 10. Trusted publishing needs npm 11.5.1, which ships with Node 24.

Here's the subtle part: npm 10 already knows how to sign Sigstore provenance attestations. That code path has existed since npm 9.5. But the ability to exchange an OIDC token for npm registry authentication — the actual trusted publishing feature — was added in 11.5.1.

So npm 10 signs the provenance successfully, then tries to authenticate with the registry using a traditional token, finds none, and reports "Access token expired or revoked." Not "missing." Not "unsupported." Expired.

![CI run #8 — everything green after switching to Node 24](./images/ci-success-node-24.png)
*Same workflow. Same config. Different Node version.*

- - -

## The fix was two lines

```diff
-  node-version: "22"
+  node-version: "24"
```

And removing the `--provenance` flag, since it's automatic with trusted publishing in npm 11.5.1+.

That's the whole fix. No NPM_TOKEN. No NODE_AUTH_TOKEN. No secrets at all.

- - -

## Why this matters

Every GitHub Actions tutorial I've seen defaults to `node-version: "22"` or `"lts/*"`. If you're setting up trusted publishing for the first time, you'll almost certainly start with one of those. The npm docs mention the version requirement, but it's buried among setup instructions.

The real problem is the error message. "Access token expired or revoked" sends you on a wild goose chase through your npmjs.com settings, your GitHub secrets, your workflow permissions. It never occurs to you that the problem is your Node version, because the error has nothing to do with Node versions.

- - -

## The quick reference

If you're setting up npm trusted publishing, here's your checklist:

**Node version must be 24.** Not 22. Not lts. Not "latest" unless that resolves to 24+.

**registry-url must be set** in the setup-node action, even though it's the default registry.

**id-token: write permission** must be on the job.

**No token env vars.** Don't set NODE_AUTH_TOKEN or NPM_TOKEN. Let OIDC handle it.

**Repo must be public.** Provenance requires public source repos.

**Publish manually first.** Trusted publishers can only be configured on existing npm packages.

- - -

## The complete workflow

For anyone who just wants to copy and paste:

```yaml
publish-npm:
  runs-on: ubuntu-latest
  permissions:
    contents: read
    id-token: write
  steps:
    - uses: actions/checkout@v4
    - uses: actions/setup-node@v4
      with:
        node-version: "24"
        registry-url: "https://registry.npmjs.org"
    - run: npm publish --access public
```

- - -

If you remember one thing from this post: `node-version: "24"`.

*I'm building [claude-view](https://github.com/tombelieber/claude-view) — a local web UI for browsing Claude Code sessions. Try it: `npx claude-view`.*
