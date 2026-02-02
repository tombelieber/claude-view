# X/Twitter Thread
# Copy each numbered block as a separate tweet

---

1/

<!-- ATTACH: images/ci-failure-publish-to-npm.png to this tweet -->

npm trusted publishing (OIDC, no tokens) kept failing in my GitHub Actions CI.

The error: "Access token expired or revoked"

I didn't HAVE a token. That was the whole point.

3 days of debugging. Fix was 1 line. Let me save you the pain.

🧵

---

2/

My setup: textbook GitHub Actions workflow.

- id-token: write ✅
- setup-node with registry-url ✅
- npm publish --provenance ✅
- trusted publisher configured on npmjs.com ✅

CI logs:
✅ Provenance signed
✅ Published to Sigstore
❌ "Access token expired"
❌ 404

???

---

3/

The maddening part: provenance signing WORKED.

The OIDC token was valid enough to sign a Sigstore attestation. Then npm immediately said "token expired" trying to authenticate with the registry.

Same token. One works. One doesn't. How??

---

4/

Answer buried in the docs:

"This feature requires npm CLI v11.5.1 or later. You need Node 24.X."

My CI: Node 22 / npm 10.9.4

npm 10 can sign provenance (existed since 9.5). But the OIDC-to-registry auth exchange? Added in 11.5.1.

---

5/

<!-- ATTACH: images/cover-node-22-to-24.png to this tweet -->

The fix:

```diff
- node-version: "22"
+ node-version: "24"
```

That's it. No NPM_TOKEN. No secrets. --provenance flag not even needed anymore (automatic in 11.5.1+).

---

6/

Why this is a trap:

- Every GH Actions tutorial defaults to node "22" or "lts/*"
- The error says "expired" not "unsupported" — sends you checking configs
- Provenance signing works fine on Node 22 — you think OIDC is working

The version req is easy to miss and hard to diagnose.

---

7/

Quick checklist for npm trusted publishing:

☐ node-version: "24"
☐ registry-url in setup-node
☐ id-token: write permission
☐ Public repo
☐ Manual first publish (then configure trusted publisher)
☐ NO token env vars

Full writeup: [link]

---

8/

<!-- ATTACH: images/ci-success-node-24.png — the payoff shot, all green -->

If you remember one thing:

node-version: "24"

That's it. That's the tweet.
