-- pairing_offers is a service-role-only table by design (see mobile-connect
-- revamp design spec §4.2). RLS is enabled, but we intentionally do NOT
-- expose a SELECT/INSERT/UPDATE/DELETE policy for `authenticated` or `anon`
-- — edge functions use the secret key to access this table, bypassing RLS.
--
-- Postgres default-denies with "RLS enabled + no matching policy", so
-- behavior today is correct. However, Supabase's security advisor flags
-- "rls_enabled_no_policy" as an INFO on this table, which muddies the
-- advisor panel and forces every subsequent audit to re-explain the intent.
--
-- Fix: add an explicit DENY policy for both authenticated + anon. This is
-- a documentation-as-code annotation that the advisor recognises as "yes,
-- this is locked down on purpose". Runtime behaviour is unchanged: both
-- roles still see zero rows and cannot write.

CREATE POLICY "pairing_offers_deny_anon_authenticated"
  ON public.pairing_offers
  AS RESTRICTIVE
  FOR ALL
  TO authenticated, anon
  USING (false)
  WITH CHECK (false);

COMMENT ON TABLE public.pairing_offers IS
  'Pending pairing tokens minted by pair-offer edge function. Service-role-only: authenticated/anon cannot read or write. Claimed via claim_pairing RPC. GC by pairing-offers-gc cron job every minute.';
