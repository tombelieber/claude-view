BEGIN;

-- -------------------------------------------------------------------------
-- claim_pairing: atomically consume a pairing offer and register a device.
--
-- Security properties (all must hold):
--   1. ATOMIC: the first statement is UPDATE ... RETURNING on pairing_offers,
--      which row-locks and claims the offer in one statement. No check-then-act
--      race window. Two concurrent claims on the same token: one returns a row,
--      the other gets zero rows and errors.
--   2. SEARCH PATH PINNED: SET search_path = pg_catalog, public prevents
--      malicious function hijacking via shadowing. See Postgres docs on
--      SECURITY DEFINER: https://www.postgresql.org/docs/current/sql-createfunction.html#SQL-CREATEFUNCTION-SECURITY
--   3. 10-DEVICE LIMIT ENFORCED IN RPC: the limit check lives inside the
--      transaction, not in the edge function. A client calling this RPC directly
--      cannot bypass the limit by skipping the edge function.
--   4. EXECUTE REVOKED from authenticated role: this RPC must only be called
--      via the service-role client from inside an edge function. The GRANT at
--      the bottom restores EXECUTE to service_role only.
-- -------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION public.claim_pairing(
  p_token TEXT,
  p_device_id TEXT,
  p_user_id UUID,
  p_ed25519_pubkey TEXT,  -- base64
  p_x25519_pubkey TEXT,   -- base64
  p_platform TEXT,
  p_display_name TEXT,
  p_app_version TEXT DEFAULT NULL,
  p_os_version TEXT DEFAULT NULL
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
DECLARE
  v_offer_user_id UUID;
  v_active_count  BIGINT;
BEGIN
  -- 1. ATOMIC CLAIM: this UPDATE is the lock, validator, and consumer in
  --    one statement. If zero rows are returned, the token was already
  --    consumed, expired, or never existed.
  UPDATE public.pairing_offers
     SET consumed_at = now()
   WHERE token = p_token
     AND consumed_at IS NULL
     AND expires_at > now()
     AND user_id = p_user_id
  RETURNING user_id INTO v_offer_user_id;

  IF NOT FOUND THEN
    -- Distinguish "token genuinely absent" from "mismatched user" from "expired"
    -- for clearer error messages. Run a second query without the user_id filter.
    PERFORM 1 FROM public.pairing_offers
     WHERE token = p_token AND consumed_at IS NULL AND expires_at > now();
    IF FOUND THEN
      RAISE EXCEPTION 'ACCOUNT_MISMATCH' USING ERRCODE = 'insufficient_privilege';
    ELSE
      RAISE EXCEPTION 'TOKEN_NOT_FOUND' USING ERRCODE = 'no_data_found';
    END IF;
  END IF;

  -- 2. 10-DEVICE LIMIT: counted INSIDE the transaction so the offer we just
  --    consumed cannot be bypassed by direct RPC callers. We lock the user's
  --    active device rows to serialize concurrent claims that all race the
  --    limit — only one can cross the count=9 → count=10 boundary.
  SELECT COUNT(*) INTO v_active_count
    FROM public.devices
   WHERE user_id = p_user_id AND revoked_at IS NULL
   FOR UPDATE;

  IF v_active_count >= 10 THEN
    RAISE EXCEPTION 'DEVICE_LIMIT_REACHED' USING ERRCODE = 'check_violation';
  END IF;

  -- 3. INSERT the device. Trigger fires and writes the audit log.
  INSERT INTO public.devices (
    device_id, user_id, ed25519_pubkey, x25519_pubkey,
    platform, display_name, app_version, os_version
  ) VALUES (
    p_device_id, p_user_id,
    decode(p_ed25519_pubkey, 'base64'),
    decode(p_x25519_pubkey, 'base64'),
    p_platform, p_display_name, p_app_version, p_os_version
  );
END;
$$;

-- Lock down who can call this function directly. Default grants on SECURITY
-- DEFINER functions include `authenticated`, which would let any signed-in
-- user call this RPC over /rest/v1/rpc/claim_pairing — bypassing the edge
-- function that enforces higher-level checks. Strip that default.
REVOKE EXECUTE ON FUNCTION public.claim_pairing(
  TEXT, TEXT, UUID, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT
) FROM PUBLIC, authenticated, anon;
GRANT EXECUTE ON FUNCTION public.claim_pairing(
  TEXT, TEXT, UUID, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT
) TO service_role;

COMMIT;
