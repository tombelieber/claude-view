# Post title:
# npm trusted publishing silently fails on Node 22 — needs Node 24 (npm >= 11.5.1)

# Subreddits: r/node, r/javascript, r/github

---

Spent 3 days debugging why npm trusted publishers (the new OIDC auth — no tokens needed) kept failing in GitHub Actions. Sharing so nobody else has to.

<!-- Reddit: Drag images/ci-failure-publish-to-npm.png into the Reddit editor.
     ONE image only. Or skip it — text-only posts do well in r/node. -->

**The error:**

```
npm notice publish Signed provenance statement ✅
npm notice Access token expired or revoked ❌
npm error 404 Not Found
```

Provenance signing works. Auth doesn't. Config was correct. What gives?

**The fix:**

```diff
- node-version: "22"
+ node-version: "24"
```

That's it. npm trusted publishing requires npm >= 11.5.1, which only ships with Node 24. Node 22 has npm 10, which can sign Sigstore provenance (that code existed since npm 9.5) but **cannot do the OIDC-to-registry-auth exchange** — that was added in 11.5.1.

So npm 10 signs your provenance, then falls back to traditional token auth, finds nothing, and says "token expired." Not "missing." Not "unsupported." Expired. Sends you on a wild goose chase through your npmjs.com settings.

**Minimal working workflow:**

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

No `NPM_TOKEN`. No `NODE_AUTH_TOKEN`. No secrets. `--provenance` is automatic.

**Quick checklist:**
- node-version: "24" (NOT 22, NOT lts/*)
- registry-url set in setup-node
- id-token: write permission
- Repo must be public
- Publish manually once first (trusted publishers need an existing package)
- Don't set any token env vars

Full writeup with the debugging journey: [link to your dev.to/blog post]

Hope this saves someone a few days.
