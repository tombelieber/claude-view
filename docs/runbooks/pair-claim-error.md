# Runbook — `pair.claim.error` SEV-2 alert

**Alert trigger:** `pair.claim.error > 10/min` in PostHog over any 5-min window.

**Severity:** SEV-2 — pairing is broken for real users, but existing devices continue working.

## Triage

1. Pull the last hour of pair-claim logs from Supabase Functions dashboard and filter by `correlation_id` to identify the failure pattern. If a single user is failing repeatedly, contact them; if many users, it's systemic.

2. Check the last-deployed migration:
   ```sql
   SELECT * FROM supabase_migrations.schema_migrations
    ORDER BY version DESC LIMIT 5;
   ```

3. Run the Phase 0 acceptance script against dev Supabase:
   ```bash
   ./supabase/tests/phase0_acceptance.sh
   ```

4. If the `claim_pairing` RPC is reported missing (`PGRST202`), re-push migrations:
   ```bash
   supabase db push
   ```

## Known causes

(populated by Phase 5 post-launch; Phase 0 ships this file with placeholder content so the observability alert has somewhere to link to)

- _none yet_

## Escalation

Ping the on-call engineer with:
- correlation_id range from the alerting window
- number of affected users
- output of `supabase/tests/phase0_acceptance.sh`
- most recent migration version
