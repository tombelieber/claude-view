-- =========================================================================
-- Mobile Connect Revamp — 2026-04-16
-- Replaces the legacy pairing system (relay DashMap + Mac JSON files).
-- All device state lives here, gated by RLS on auth.uid().
-- See docs/plans/2026-04-16-mobile-connect-revamp-design.md for full context.
-- =========================================================================

BEGIN;

-- -------------------------------------------------------------------------
-- Extensions
-- -------------------------------------------------------------------------
CREATE EXTENSION IF NOT EXISTS pg_cron;

-- -------------------------------------------------------------------------
-- devices: authoritative registry of all paired devices per Supabase user
-- -------------------------------------------------------------------------
CREATE TABLE public.devices (
  device_id         TEXT PRIMARY KEY
                    CONSTRAINT device_id_format
                    CHECK (device_id ~ '^(mac|ios|android|web)-[0-9a-f]{16}$'),
  user_id           UUID NOT NULL
                    REFERENCES auth.users(id) ON DELETE CASCADE,
  ed25519_pubkey    BYTEA NOT NULL
                    CONSTRAINT ed25519_pubkey_length CHECK (octet_length(ed25519_pubkey) = 32),
  x25519_pubkey     BYTEA NOT NULL
                    CONSTRAINT x25519_pubkey_length CHECK (octet_length(x25519_pubkey) = 32),
  platform          TEXT NOT NULL
                    CHECK (platform IN ('mac', 'ios', 'android', 'web')),
  display_name      TEXT NOT NULL
                    CHECK (char_length(display_name) BETWEEN 1 AND 80),
  app_version       TEXT,
  os_version        TEXT,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_seen_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  revoked_at        TIMESTAMPTZ,
  revoked_reason    TEXT CHECK (
                     revoked_reason IS NULL OR
                     revoked_reason IN ('user_action', 'bulk_terminate', 'inactivity_gc', 'admin')
                   ),
  last_ip           INET,
  last_user_agent   TEXT
);

CREATE UNIQUE INDEX devices_user_id_device_id_active_idx
  ON public.devices (user_id, device_id)
  WHERE revoked_at IS NULL;

CREATE INDEX devices_user_id_active_idx
  ON public.devices (user_id)
  WHERE revoked_at IS NULL;

CREATE INDEX devices_last_seen_at_active_idx
  ON public.devices (last_seen_at)
  WHERE revoked_at IS NULL;

-- -------------------------------------------------------------------------
-- device_events: append-only audit log, populated by triggers only
-- -------------------------------------------------------------------------
CREATE TABLE public.device_events (
  id               BIGSERIAL PRIMARY KEY,
  device_id        TEXT NOT NULL,
  user_id          UUID NOT NULL,
  event            TEXT NOT NULL CHECK (event IN (
                    'paired', 'unpaired', 'revoked', 'expired',
                    'connected', 'disconnected', 'renamed'
                   )),
  metadata         JSONB NOT NULL DEFAULT '{}'::jsonb,
  occurred_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX device_events_user_id_occurred_idx
  ON public.device_events (user_id, occurred_at DESC);

CREATE INDEX device_events_device_id_idx
  ON public.device_events (device_id);

-- -------------------------------------------------------------------------
-- pairing_offers: short-lived pairing tokens, TTL 5 minutes
-- -------------------------------------------------------------------------
CREATE TABLE public.pairing_offers (
  token            TEXT PRIMARY KEY
                   CONSTRAINT token_format CHECK (char_length(token) = 43),
  user_id          UUID NOT NULL
                   REFERENCES auth.users(id) ON DELETE CASCADE,
  issuing_device_id TEXT NOT NULL,
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at       TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '5 minutes'),
  consumed_at      TIMESTAMPTZ
);

CREATE INDEX pairing_offers_user_id_idx ON public.pairing_offers (user_id);
CREATE INDEX pairing_offers_expires_at_idx ON public.pairing_offers (expires_at);

-- -------------------------------------------------------------------------
-- RLS: devices
-- -------------------------------------------------------------------------
ALTER TABLE public.devices ENABLE ROW LEVEL SECURITY;

CREATE POLICY devices_select_own
  ON public.devices FOR SELECT
  TO authenticated
  USING (user_id = auth.uid());

CREATE POLICY devices_update_own_display_name
  ON public.devices FOR UPDATE
  TO authenticated
  USING (user_id = auth.uid())
  WITH CHECK (user_id = auth.uid());

-- INSERT and DELETE only through edge functions (service role).

-- -------------------------------------------------------------------------
-- RLS: device_events (read-only for users)
-- -------------------------------------------------------------------------
ALTER TABLE public.device_events ENABLE ROW LEVEL SECURITY;

CREATE POLICY device_events_select_own
  ON public.device_events FOR SELECT
  TO authenticated
  USING (user_id = auth.uid());

-- -------------------------------------------------------------------------
-- RLS: pairing_offers (no client access, edge-function-only)
-- -------------------------------------------------------------------------
ALTER TABLE public.pairing_offers ENABLE ROW LEVEL SECURITY;
-- no policies = default-deny for all operations

-- -------------------------------------------------------------------------
-- Trigger: devices audit log population
-- -------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION public.tg_devices_audit()
RETURNS TRIGGER
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
BEGIN
  IF TG_OP = 'INSERT' THEN
    INSERT INTO public.device_events (device_id, user_id, event, metadata)
    VALUES (
      NEW.device_id, NEW.user_id, 'paired',
      jsonb_build_object(
        'platform', NEW.platform,
        'display_name', NEW.display_name,
        'app_version', NEW.app_version,
        'os_version', NEW.os_version
      )
    );
    RETURN NEW;

  ELSIF TG_OP = 'UPDATE' THEN
    IF OLD.revoked_at IS NULL AND NEW.revoked_at IS NOT NULL THEN
      INSERT INTO public.device_events (device_id, user_id, event, metadata)
      VALUES (
        NEW.device_id, NEW.user_id, 'revoked',
        jsonb_build_object('reason', NEW.revoked_reason)
      );
    END IF;
    IF OLD.display_name IS DISTINCT FROM NEW.display_name THEN
      INSERT INTO public.device_events (device_id, user_id, event, metadata)
      VALUES (
        NEW.device_id, NEW.user_id, 'renamed',
        jsonb_build_object('from', OLD.display_name, 'to', NEW.display_name)
      );
    END IF;
    RETURN NEW;

  ELSIF TG_OP = 'DELETE' THEN
    INSERT INTO public.device_events (device_id, user_id, event, metadata)
    VALUES (OLD.device_id, OLD.user_id, 'unpaired', '{}'::jsonb);
    RETURN OLD;
  END IF;

  RETURN NULL;
EXCEPTION WHEN OTHERS THEN
  -- Never block the underlying device write because audit failed.
  RAISE WARNING 'tg_devices_audit failed (%): %', SQLSTATE, SQLERRM;
  RETURN COALESCE(NEW, OLD);
END;
$$;

CREATE TRIGGER devices_audit
  AFTER INSERT OR DELETE ON public.devices
  FOR EACH ROW EXECUTE FUNCTION public.tg_devices_audit();

CREATE TRIGGER devices_audit_update
  AFTER UPDATE ON public.devices
  FOR EACH ROW
  WHEN (
    OLD.revoked_at IS DISTINCT FROM NEW.revoked_at OR
    OLD.display_name IS DISTINCT FROM NEW.display_name
  )
  EXECUTE FUNCTION public.tg_devices_audit();

-- -------------------------------------------------------------------------
-- Realtime: publish devices table for client subscriptions
-- -------------------------------------------------------------------------
ALTER PUBLICATION supabase_realtime ADD TABLE public.devices;

COMMIT;
