# Landing Page — Production Deployment Checklist

> Pre-launch checklist for going live on `claudeview.ai`. Every item must be DONE before announcing publicly.

## Status: PENDING

## Current State (2026-03-02)

Waitlist API is **functional** on `claude-view-landing.pages.dev` with test Turnstile key. Supabase migration applied. All endpoints verified working. Not yet on custom domain.

## Checklist

### 1. Create Real Turnstile Widget

- [ ] Create widget via CF API (or Dashboard > Turnstile > Add Widget)
  - Domain: `claudeview.ai`
  - Mode: `managed` (invisible)
  - Name: `claudeview-waitlist`
- [ ] Save the returned `sitekey` and `secret`

```bash
# Via CF API (replace YOUR_CF_API_TOKEN):
curl -s -X POST "https://api.cloudflare.com/client/v4/accounts/96887e7bf8b696172bc5cbed241ed409/challenges/widgets" \
  -H "Authorization: Bearer YOUR_CF_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"domains":["claudeview.ai"],"mode":"managed","name":"claudeview-waitlist"}' \
  | jq '.result | {sitekey, secret}'
```

### 2. Update Turnstile Sitekey in Code

- [ ] Replace test key in `apps/landing/src/data/site.ts:155`
  - FROM: `1x00000000000000000000AA` (test — always passes, shows visible banner)
  - TO: real `sitekey` from Step 1 (invisible, actual bot protection)

### 3. Update CF Pages Secret

- [ ] Set real Turnstile secret (replace test secret):

```bash
cd apps/landing
printf '%s' 'REAL_TURNSTILE_SECRET' | npx wrangler pages secret put TURNSTILE_SECRET_KEY --project-name claude-view-landing
```

> **IMPORTANT:** Use `printf '%s'`, NOT `echo`. `echo` adds trailing newline that silently breaks the secret.

### 4. Connect Custom Domain

- [ ] Add `claudeview.ai` as custom domain to CF Pages project:

```bash
npx wrangler pages project update claude-view-landing --production-branch main
# Then in CF Dashboard: Pages > claude-view-landing > Custom domains > Add > claudeview.ai
```

> Note: Custom domain requires DNS to be on Cloudflare (already is). CF auto-provisions SSL.

### 5. Clean Test Data

- [ ] Truncate test entries from waitlist table (3 test rows from smoke testing):

```sql
-- Run via Supabase Dashboard > SQL Editor:
TRUNCATE TABLE public.waitlist RESTART IDENTITY;
```

> This resets the `position` sequence so the first real signup gets position #1.

### 6. Build & Deploy to Production

- [ ] Build and deploy with real Turnstile key baked in:

```bash
cd apps/landing
bun run build
npx wrangler pages deploy dist --project-name claude-view-landing --branch main
```

> **IMPORTANT:** Must use `--branch main` to deploy as production (has access to production secrets). Without it, current git branch name is used and may create a preview deployment instead.

### 7. E2E Verification on `claudeview.ai`

- [ ] Homepage hero — waitlist form visible, Turnstile badge invisible (managed mode)
- [ ] Submit email — get position #1 + referral link + "Join 1 developers" counter
- [ ] Copy referral link — paste in incognito, verify `?ref=` param present
- [ ] Referral signup — submit different email via referral link, verify success
- [ ] Duplicate submit — same email returns same position (not error)
- [ ] Pricing cards — "Join Waitlist" smooth-scrolls to hero form
- [ ] Mobile section — "Join the mobile waitlist" smooth-scrolls to hero form
- [ ] Mobile-setup docs — `/docs/guides/mobile-setup/` has compact waitlist form
- [ ] Stale referral — `?ref=NONEXISTENT` + signup succeeds (referred_by = null)
- [ ] GET count — `curl https://claudeview.ai/api/waitlist` returns `{"total_count": N}`

### 8. Remaining L1 Blockers (from follow-up doc)

These are separate from the waitlist but block L1 launch:

- [ ] **App Store badges** — Either link to real listings or remove badges entirely
- [ ] **Twitter/X handle** — Create `@claude_view`, set `TWITTER_HANDLE` in `site.ts`

## Deploy Gotchas (Learned 2026-03-02)

| Gotcha | Fix |
|--------|-----|
| `echo` adds `\n` to piped secrets | Use `printf '%s'` instead |
| Non-main branch → preview deploy (no prod secrets) | Always `--branch main` |
| Migration in git ≠ migration applied | Run `supabase db push` after commit |
| CF Pages reads git branch for env detection | Explicit `--branch` overrides |
