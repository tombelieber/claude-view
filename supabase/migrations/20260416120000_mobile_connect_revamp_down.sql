-- =========================================================================
-- ROLLBACK script for mobile connect revamp Phase 0.
--
-- This file is NEVER run automatically by `supabase db push`. It exists
-- so an operator can manually revert the feature if dev-env experiments
-- corrupt the schema or a prod deploy reveals a P0 bug.
--
-- Usage: psql "$SUPABASE_DEV_DB_URL" -f supabase/migrations/20260416120000_mobile_connect_revamp_down.sql
--
-- WARNING: this deletes ALL device registry data for every user in the
-- database. Do NOT run in production without a fresh backup.
-- =========================================================================

BEGIN;

-- Pull the Realtime publication first so no change events fire during tear-down.
ALTER PUBLICATION supabase_realtime DROP TABLE IF EXISTS public.devices;

-- Cancel the cron jobs.
SELECT cron.unschedule('pairing-offers-gc') WHERE EXISTS (SELECT 1 FROM cron.job WHERE jobname = 'pairing-offers-gc');
SELECT cron.unschedule('devices-inactivity-gc') WHERE EXISTS (SELECT 1 FROM cron.job WHERE jobname = 'devices-inactivity-gc');
SELECT cron.unschedule('cron-job-run-details-gc') WHERE EXISTS (SELECT 1 FROM cron.job WHERE jobname = 'cron-job-run-details-gc');

-- Drop triggers before the function they depend on.
DROP TRIGGER IF EXISTS devices_audit ON public.devices;
DROP TRIGGER IF EXISTS devices_audit_update ON public.devices;
DROP FUNCTION IF EXISTS public.tg_devices_audit();

-- Drop the claim function.
DROP FUNCTION IF EXISTS public.claim_pairing(TEXT, TEXT, UUID, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT);

-- Drop tables in reverse-FK order (pairing_offers → devices → device_events
-- are all independent but ON DELETE CASCADE from auth.users covers us).
DROP TABLE IF EXISTS public.pairing_offers;
DROP TABLE IF EXISTS public.device_events;
DROP TABLE IF EXISTS public.devices;

COMMIT;
