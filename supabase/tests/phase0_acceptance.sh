#!/usr/bin/env bash
# Phase 0 acceptance — runs every test in order, reports PASS/FAIL per item.
set -euo pipefail

: "${SUPABASE_DEV_URL:?must be set}"
: "${SUPABASE_DEV_DB_URL:?must be set}"
: "${SUPABASE_DEV_PUBLISHABLE_KEY:?must be set}"
: "${SUPABASE_DEV_PROJECT_REF:?must be set}"

RESULTS=()
pass() { RESULTS+=("PASS  $1"); echo "PASS  $1"; }
fail() { RESULTS+=("FAIL  $1"); echo "FAIL  $1" >&2; }

JWT_1=$(cat /tmp/dev_jwt.txt)

echo "=== Phase 0 acceptance run ==="

echo ""
echo "Check 1: schema exists"
if psql "$SUPABASE_DEV_DB_URL" -tAc "SELECT 1 FROM information_schema.tables WHERE table_schema='public' AND table_name='devices'" | grep -q 1; then
  pass "public.devices table exists"
else
  fail "public.devices table missing"
fi

echo ""
echo "Check 2: RLS enabled"
RLS_CHECK=$(psql "$SUPABASE_DEV_DB_URL" -tAc "
SELECT COUNT(*) FROM pg_tables
WHERE schemaname='public' AND tablename IN ('devices','device_events','pairing_offers') AND rowsecurity=true")
if [[ "$RLS_CHECK" == "3" ]]; then
  pass "RLS enabled on all three tables"
else
  fail "RLS not enabled on all tables (got $RLS_CHECK/3)"
fi

echo ""
echo "Check 3: trigger attached"
if psql "$SUPABASE_DEV_DB_URL" -tAc "SELECT tgname FROM pg_trigger WHERE tgrelid='public.devices'::regclass AND tgname='devices_audit'" | grep -q devices_audit; then
  pass "devices_audit trigger attached"
else
  fail "devices_audit trigger missing"
fi

echo ""
echo "Check 4: realtime publication includes devices"
if psql "$SUPABASE_DEV_DB_URL" -tAc "SELECT 1 FROM pg_publication_tables WHERE pubname='supabase_realtime' AND tablename='devices'" | grep -q 1; then
  pass "public.devices in supabase_realtime publication"
else
  fail "public.devices NOT in supabase_realtime publication"
fi

echo ""
echo "Check 5: pg_cron jobs scheduled"
CRON_COUNT=$(psql "$SUPABASE_DEV_DB_URL" -tAc "SELECT COUNT(*) FROM cron.job WHERE jobname IN ('pairing-offers-gc','devices-inactivity-gc','cron-job-run-details-gc')")
if [[ "$CRON_COUNT" == "3" ]]; then
  pass "all 3 pg_cron jobs scheduled"
else
  fail "pg_cron jobs missing (got $CRON_COUNT/3)"
fi

echo ""
echo "Check 6: type codegen file exists"
if [[ -f packages/shared/src/types/supabase.generated.ts ]]; then
  pass "supabase.generated.ts present"
else
  fail "supabase.generated.ts missing"
fi

echo ""
echo "Check 7: all five edge functions deployed"
for fn in pair-offer pair-claim devices-revoke devices-terminate-others devices-list; do
  CODE=$(curl -s -o /dev/null -w "%{http_code}" "$SUPABASE_DEV_URL/functions/v1/$fn" -H "Authorization: Bearer $JWT_1" -X POST)
  if [[ "$CODE" == "400" || "$CODE" == "401" || "$CODE" == "405" || "$CODE" == "500" || "$CODE" == "200" || "$CODE" == "201" ]]; then
    pass "edge function $fn responding (HTTP $CODE)"
  else
    fail "edge function $fn not reachable (HTTP $CODE)"
  fi
done

echo ""
echo "Check 8: RLS isolation (re-runs the isolation test)"
if ./supabase/tests/phase0_rls_isolation_test.sh > /tmp/rls_test.log 2>&1; then
  pass "RLS isolation test (see /tmp/rls_test.log)"
else
  fail "RLS isolation test (see /tmp/rls_test.log)"
fi

echo ""
echo "=== Phase 0 acceptance results ==="
printf '%s\n' "${RESULTS[@]}"

FAIL_COUNT=$(printf '%s\n' "${RESULTS[@]}" | grep -c '^FAIL' || true)
if [[ "$FAIL_COUNT" == "0" ]]; then
  echo ""
  echo "ALL PHASE 0 ACCEPTANCE CHECKS PASSED"
  exit 0
else
  echo ""
  echo "$FAIL_COUNT checks FAILED — fix before proceeding to Phase 1"
  exit 1
fi
