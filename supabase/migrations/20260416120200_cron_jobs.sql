-- =========================================================================
-- pg_cron jobs for mobile connect revamp cleanup
--
-- This migration installs two scheduled jobs:
--   - pairing-offers-gc: every minute, deletes expired/consumed offers
--   - devices-inactivity-gc: daily 03:00 UTC, marks 30-day-stale devices
--   - cron-job-run-details-gc: weekly, prunes pg_cron's own audit table
--
-- All three are idempotent (unschedule-then-schedule) so this migration
-- can be re-run without side effects. Scheduled jobs survive migration
-- rollback on Supabase — operators must manually unschedule if they
-- revert the feature.
-- =========================================================================

BEGIN;

-- Ensure the extension is enabled (should already be from the first
-- migration, but CREATE EXTENSION IF NOT EXISTS is idempotent).
CREATE EXTENSION IF NOT EXISTS pg_cron;

-- Job 1: delete expired pairing offers every minute.
SELECT cron.unschedule('pairing-offers-gc') WHERE EXISTS (
  SELECT 1 FROM cron.job WHERE jobname = 'pairing-offers-gc'
);
SELECT cron.schedule(
  'pairing-offers-gc',
  '* * * * *',
  $$ DELETE FROM public.pairing_offers
     WHERE expires_at < now() OR consumed_at IS NOT NULL; $$
);

-- Job 2: mark 30-day-inactive devices as revoked.
-- Short statement_timeout prevents a cascading trigger chain from wedging
-- the table; lock_timeout keeps us from stacking behind other writers.
SELECT cron.unschedule('devices-inactivity-gc') WHERE EXISTS (
  SELECT 1 FROM cron.job WHERE jobname = 'devices-inactivity-gc'
);
SELECT cron.schedule(
  'devices-inactivity-gc',
  '0 3 * * *',
  $$ SET LOCAL lock_timeout = '5s';
     SET LOCAL statement_timeout = '60s';
     UPDATE public.devices
        SET revoked_at = now(),
            revoked_reason = 'inactivity_gc'
      WHERE revoked_at IS NULL
        AND last_seen_at < now() - interval '30 days'; $$
);

-- Job 3: prune pg_cron's own audit table weekly. Without this, cron.job_run_details
-- grows unbounded (one row per job run, ~1440 rows/day for pairing-offers-gc alone).
SELECT cron.unschedule('cron-job-run-details-gc') WHERE EXISTS (
  SELECT 1 FROM cron.job WHERE jobname = 'cron-job-run-details-gc'
);
SELECT cron.schedule(
  'cron-job-run-details-gc',
  '0 4 * * 0',  -- Sunday 04:00 UTC
  $$ DELETE FROM cron.job_run_details
     WHERE end_time < now() - interval '7 days'; $$
);

COMMIT;
