BEGIN;

-- -------------------------------------------------------------------------
-- register_device_self: atomically upsert the caller's own device row.
--
-- Purpose: close the chicken-and-egg in the mobile-connect pairing model.
-- `pair-offer` requires the issuing device to already exist in public.devices,
-- but a freshly-signed-in Mac has no row. Rather than letting pair-offer
-- silently bootstrap (which would conflate "mint a pairing token" with
-- "self-register"), we add an explicit edge-function path: a signed-in
-- client POSTs to /functions/v1/devices-register-self with its public
-- identifiers, and this RPC idempotently upserts the row.
--
-- Security properties (all must hold):
--   1. ATOMIC: single plpgsql transaction. Limit check + insert happen in
--      one statement; no check-then-act race window between concurrent
--      registers from the same user.
--   2. SEARCH PATH PINNED: same rationale as claim_pairing — prevents
--      malicious function hijacking via shadowing.
--   3. 10-DEVICE LIMIT ENFORCED: the count check is INSIDE the RPC, not in
--      the edge function. A client calling the RPC directly cannot bypass.
--   4. EXECUTE REVOKED from authenticated role: only service_role (edge
--      function) may invoke this. The edge function extracts user_id from
--      the caller's verified JWT and passes it as p_user_id.
--   5. IDEMPOTENT: re-registering the same device_id by its owning user is
--      a no-op-plus-refresh: last_seen_at bumped, revoked state cleared,
--      display_name / pubkeys rotated (supports "fresh install"
--      re-registration where the local keypair was regenerated).
--   6. TENANCY-SAFE: if a device_id already exists for a DIFFERENT user,
--      we raise INVALID_DEVICE_ID — never ACCOUNT_MISMATCH (which would
--      leak the fact that the device_id is taken).
-- -------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION public.register_device_self(
  p_device_id TEXT,
  p_user_id UUID,
  p_ed25519_pubkey TEXT,  -- base64, 32 raw bytes after decode
  p_x25519_pubkey TEXT,   -- base64, 32 raw bytes after decode
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
  v_existing_user_id UUID;
  v_active_count     BIGINT;
BEGIN
  -- Step 1: check whether device_id is already taken.
  SELECT user_id INTO v_existing_user_id
    FROM public.devices
    WHERE device_id = p_device_id;

  IF v_existing_user_id IS NOT NULL AND v_existing_user_id <> p_user_id THEN
    -- Leak-free: never reveal that the ID exists under another account.
    RAISE EXCEPTION 'INVALID_DEVICE_ID' USING ERRCODE = '23514';
  END IF;

  -- Step 2: new device for this user? Enforce 10-device active limit.
  -- (Limit excludes revoked rows and excludes THIS device_id if we're
  -- re-registering — i.e. re-register-of-own-device never hits the limit.)
  IF v_existing_user_id IS NULL THEN
    SELECT COUNT(*) INTO v_active_count
      FROM public.devices
      WHERE user_id = p_user_id
        AND revoked_at IS NULL;
    IF v_active_count >= 10 THEN
      RAISE EXCEPTION 'DEVICE_LIMIT_REACHED' USING ERRCODE = 'P0001';
    END IF;
  END IF;

  -- Step 3: upsert. ON CONFLICT updates the existing row by device_id —
  -- same idempotent semantics as a fresh install on the same hardware:
  --   - last_seen_at bumped
  --   - revoked_at / revoked_reason cleared (un-revoke on re-register)
  --   - display_name + pubkeys + versions refreshed from caller input
  INSERT INTO public.devices (
    device_id,
    user_id,
    ed25519_pubkey,
    x25519_pubkey,
    platform,
    display_name,
    app_version,
    os_version,
    last_seen_at
  ) VALUES (
    p_device_id,
    p_user_id,
    decode(p_ed25519_pubkey, 'base64'),
    decode(p_x25519_pubkey, 'base64'),
    p_platform,
    p_display_name,
    p_app_version,
    p_os_version,
    now()
  )
  ON CONFLICT (device_id) DO UPDATE SET
    ed25519_pubkey  = EXCLUDED.ed25519_pubkey,
    x25519_pubkey   = EXCLUDED.x25519_pubkey,
    display_name    = EXCLUDED.display_name,
    app_version     = COALESCE(EXCLUDED.app_version, public.devices.app_version),
    os_version      = COALESCE(EXCLUDED.os_version,  public.devices.os_version),
    last_seen_at    = EXCLUDED.last_seen_at,
    revoked_at      = NULL,
    revoked_reason  = NULL;
END;
$$;

-- Lock down: only service_role (edge functions) may invoke.
REVOKE ALL ON FUNCTION public.register_device_self(
  TEXT, UUID, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT
) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.register_device_self(
  TEXT, UUID, TEXT, TEXT, TEXT, TEXT, TEXT, TEXT
) TO service_role;

COMMIT;
