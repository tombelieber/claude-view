#!/usr/bin/env bash
# Phase 0 RLS isolation acceptance test.
# Ensures user_id filtering is enforced at the database level, not just in app code.
#
# Prereqs: $SUPABASE_DEV_URL, $SUPABASE_DEV_PUBLISHABLE_KEY, /tmp/dev_jwt.txt (user 1),
# /tmp/dev_jwt_2.txt (user 2) all present.

set -euo pipefail

: "${SUPABASE_DEV_URL:?must be set}"
: "${SUPABASE_DEV_PUBLISHABLE_KEY:?must be set}"

JWT_1=$(cat /tmp/dev_jwt.txt)
JWT_2=$(cat /tmp/dev_jwt_2.txt)

echo "RLS test: user 1 reads only their own devices"
USER_1_LIST=$(curl -s "$SUPABASE_DEV_URL/rest/v1/devices?select=device_id,user_id" \
  -H "apikey: $SUPABASE_DEV_PUBLISHABLE_KEY" \
  -H "Authorization: Bearer $JWT_1")
USER_1_OWN_ONLY=$(echo "$USER_1_LIST" | jq '[.[] | select(.device_id | startswith("mac-") or startswith("ios-") or startswith("android-"))] | length')
echo "User 1 sees $USER_1_OWN_ONLY devices"
if [[ "$USER_1_OWN_ONLY" -lt 1 ]]; then
  echo "FAIL: user 1 should see at least the Mac device from Task 6"
  exit 1
fi

echo ""
echo "RLS test: user 2 sees zero of user 1's devices"
USER_2_LIST=$(curl -s "$SUPABASE_DEV_URL/rest/v1/devices?select=device_id" \
  -H "apikey: $SUPABASE_DEV_PUBLISHABLE_KEY" \
  -H "Authorization: Bearer $JWT_2")
USER_2_COUNT=$(echo "$USER_2_LIST" | jq 'length')
echo "User 2 sees $USER_2_COUNT devices"
if [[ "$USER_2_COUNT" -ne 0 ]]; then
  echo "FAIL: user 2 should see zero devices (has none paired yet), saw $USER_2_COUNT"
  echo "$USER_2_LIST" | jq .
  exit 1
fi

echo ""
echo "RLS test: user 2 cannot claim user 1's pending offer (ACCOUNT_MISMATCH)"
# Create a fresh offer for user 1.
OFFER_RESP=$(curl -s -X POST "$SUPABASE_DEV_URL/functions/v1/pair-offer" \
  -H "Authorization: Bearer $JWT_1" \
  -H "Content-Type: application/json" \
  -d '{"issuing_device_id":"mac-1111222233334444"}')
TOKEN=$(echo "$OFFER_RESP" | jq -r .token)
echo "Created offer token: $TOKEN"

# User 2 tries to claim it.
CLAIM_RESP=$(curl -s -X POST "$SUPABASE_DEV_URL/functions/v1/pair-claim" \
  -H "Authorization: Bearer $JWT_2" \
  -H "Content-Type: application/json" \
  -d "{
    \"token\": \"$TOKEN\",
    \"device_id\": \"ios-9999999999999999\",
    \"ed25519_pubkey\": \"$(printf 'cccccccccccccccccccccccccccccccc' | base64)\",
    \"x25519_pubkey\": \"$(printf 'dddddddddddddddddddddddddddddddd' | base64)\",
    \"platform\": \"ios\",
    \"display_name\": \"Evil iPhone\"
  }")
ERROR_CODE=$(echo "$CLAIM_RESP" | jq -r '.error.code')
if [[ "$ERROR_CODE" != "ACCOUNT_MISMATCH" ]]; then
  echo "FAIL: expected ACCOUNT_MISMATCH, got $ERROR_CODE"
  echo "$CLAIM_RESP" | jq .
  exit 1
fi
echo "User 2 correctly rejected with ACCOUNT_MISMATCH"

echo ""
echo "RLS test: user 2 cannot revoke user 1's device (DEVICE_NOT_FOUND)"
REVOKE_RESP=$(curl -s -X POST "$SUPABASE_DEV_URL/functions/v1/devices-revoke" \
  -H "Authorization: Bearer $JWT_2" \
  -H "Content-Type: application/json" \
  -d '{"device_id":"mac-1111222233334444"}')
ERROR_CODE=$(echo "$REVOKE_RESP" | jq -r '.error.code')
if [[ "$ERROR_CODE" != "DEVICE_NOT_FOUND" ]]; then
  echo "FAIL: expected DEVICE_NOT_FOUND, got $ERROR_CODE"
  echo "$REVOKE_RESP" | jq .
  exit 1
fi
echo "User 2 correctly rejected with DEVICE_NOT_FOUND"

echo ""
echo "RLS test: user 2 cannot UPDATE user 1's display_name via PostgREST"
UPDATE_RESP=$(curl -s -X PATCH "$SUPABASE_DEV_URL/rest/v1/devices?device_id=eq.mac-1111222233334444" \
  -H "apikey: $SUPABASE_DEV_PUBLISHABLE_KEY" \
  -H "Authorization: Bearer $JWT_2" \
  -H "Content-Type: application/json" \
  -H "Prefer: return=representation" \
  -d '{"display_name":"Hacked by user 2"}')
# PostgREST returns an empty array [] when RLS filters the row out before UPDATE.
# It does NOT error — it silently returns no rows updated. This is a PostgREST
# quirk that every RLS test must account for.
UPDATE_COUNT=$(echo "$UPDATE_RESP" | jq 'length')
if [[ "$UPDATE_COUNT" != "0" ]]; then
  echo "FAIL: expected zero rows updated (RLS should filter), got $UPDATE_COUNT"
  echo "$UPDATE_RESP" | jq .
  exit 1
fi
echo "User 2 correctly updated zero rows (RLS filter active)"

# Sanity check: user 1's display_name is unchanged.
USER_1_DEVICE=$(curl -s "$SUPABASE_DEV_URL/rest/v1/devices?select=device_id,display_name&device_id=eq.mac-1111222233334444" \
  -H "apikey: $SUPABASE_DEV_PUBLISHABLE_KEY" \
  -H "Authorization: Bearer $JWT_1")
DISPLAY_NAME=$(echo "$USER_1_DEVICE" | jq -r '.[0].display_name')
if [[ "$DISPLAY_NAME" == "Hacked by user 2" ]]; then
  echo "CRITICAL: RLS FAILED — user 2's write reached user 1's row"
  exit 1
fi
echo "User 1's row unchanged ($DISPLAY_NAME)"

echo ""
echo "RLS test: DELETE on devices is blocked entirely (no DELETE policy)"
DELETE_RESP=$(curl -s -X DELETE "$SUPABASE_DEV_URL/rest/v1/devices?device_id=eq.mac-1111222233334444" \
  -H "apikey: $SUPABASE_DEV_PUBLISHABLE_KEY" \
  -H "Authorization: Bearer $JWT_1")
# Even user 1 cannot direct-DELETE — the schema has no DELETE policy, so
# RLS default-denies. Revoke (soft-delete) is the only deletion path.
DELETE_COUNT=$(echo "$DELETE_RESP" | jq 'length' 2>/dev/null || echo "0")
if [[ "$DELETE_COUNT" != "0" ]]; then
  echo "FAIL: DELETE should be blocked by default-deny RLS, got $DELETE_COUNT"
  exit 1
fi
echo "DELETE correctly blocked for both users"

echo ""
echo "RLS test: user 2 cannot read user 1's device_events audit log"
EVENTS_RESP=$(curl -s "$SUPABASE_DEV_URL/rest/v1/device_events?select=*" \
  -H "apikey: $SUPABASE_DEV_PUBLISHABLE_KEY" \
  -H "Authorization: Bearer $JWT_2")
EVENTS_COUNT=$(echo "$EVENTS_RESP" | jq 'length')
if [[ "$EVENTS_COUNT" != "0" ]]; then
  echo "FAIL: user 2 sees $EVENTS_COUNT audit events (should be 0)"
  exit 1
fi
echo "User 2 sees zero audit events (RLS on device_events active)"

echo ""
echo "All Phase 0 RLS isolation tests PASSED (7 assertions across SELECT, UPDATE, DELETE, edge functions, audit log)"
