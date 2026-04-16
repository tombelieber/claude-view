#!/usr/bin/env bash
# Phase 0 teardown — removes the synthetic test fixtures inserted by
# Tasks 6-9 so the plan can be re-run on the same dev project.
#
# Does NOT drop the schema (use the rollback migration for that).

set -euo pipefail

: "${SUPABASE_DEV_DB_URL:?must be set}"

psql "$SUPABASE_DEV_DB_URL" <<'SQL'
-- Remove all test devices inserted during Phase 0.
DELETE FROM public.devices
 WHERE device_id IN (
   'mac-1111222233334444',
   'mac-9999888877776666',
   'mac-stale00000000',
   'ios-2222222222222222',
   'ios-5555666677778888',
   'ios-9999000011112222',
   'ios-9999999999999999',
   'android-a1a1a1a1a1a1a1a1'
 );

-- Remove any test pairing offers.
DELETE FROM public.pairing_offers
 WHERE issuing_device_id LIKE 'mac-11112222%' OR issuing_device_id LIKE 'mac-99998888%';

-- Remove test audit log entries (the FK uses ON DELETE CASCADE from auth.users,
-- but we're not deleting users — just their test rows).
DELETE FROM public.device_events
 WHERE device_id IN (
   'mac-1111222233334444',
   'mac-9999888877776666',
   'mac-stale00000000',
   'ios-2222222222222222',
   'ios-5555666677778888',
   'ios-9999000011112222',
   'ios-9999999999999999',
   'android-a1a1a1a1a1a1a1a1'
 );

SELECT 'Teardown complete. Devices and events for synthetic fixtures removed.' AS result;
SQL
