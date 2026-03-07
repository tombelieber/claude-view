-- Waitlist table for early access signups with referral tracking
CREATE TABLE public.waitlist (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email TEXT NOT NULL,
  referral_code TEXT NOT NULL,
  referred_by TEXT,
  referral_count INTEGER NOT NULL DEFAULT 0,
  position INTEGER GENERATED ALWAYS AS IDENTITY,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

  CONSTRAINT waitlist_email_unique UNIQUE (email),
  CONSTRAINT waitlist_referral_code_unique UNIQUE (referral_code),
  CONSTRAINT waitlist_referred_by_fk FOREIGN KEY (referred_by) REFERENCES public.waitlist(referral_code)
);

-- Index for referral lookups
CREATE INDEX idx_waitlist_referral_code ON public.waitlist (referral_code);

-- RLS: anon can INSERT only (CF Function uses service_role key, bypasses RLS)
ALTER TABLE public.waitlist ENABLE ROW LEVEL SECURITY;

CREATE POLICY "anon_insert_only" ON public.waitlist
  FOR INSERT TO anon
  WITH CHECK (true);

-- RPC for atomic referral count increment.
-- SECURITY DEFINER runs as function owner (bypasses RLS). search_path pinned
-- to prevent search-path hijack (Supabase best practice for DEFINER functions).
CREATE OR REPLACE FUNCTION increment_referral_count(ref_code TEXT)
RETURNS void
LANGUAGE sql
SECURITY DEFINER
SET search_path = public
AS $$
  UPDATE public.waitlist
  SET referral_count = referral_count + 1
  WHERE referral_code = ref_code;
$$;

-- Lock down RPC access: only service_role can call this function.
-- Without this, anon/authenticated roles (public API keys) could call
-- POST /rest/v1/rpc/increment_referral_count with arbitrary ref_codes.
-- Ref: https://supabase.com/docs/guides/troubleshooting/how-can-i-revoke-execution-of-a-postgresql-function-2GYb0A
REVOKE EXECUTE ON FUNCTION increment_referral_count FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION increment_referral_count FROM anon;
REVOKE EXECUTE ON FUNCTION increment_referral_count FROM authenticated;
GRANT EXECUTE ON FUNCTION increment_referral_count TO service_role;
