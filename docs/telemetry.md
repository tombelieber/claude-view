# Telemetry contract

Official desktop releases send explicit, anonymous product events to PostHog's
US ingestion endpoint. Source builds contain no key and send nothing. Existing
`CLAUDE_VIEW_TELEMETRY=0`, CI, and saved opt-out behavior is retained.

## Shared OSS destination

The official release workflow compiles `POSTHOG_OSS_API_KEY` into the binary as
`POSTHOG_API_KEY`. It must be the shared OSS project's **ingestion-only** key;
never embed a personal API key. Keep the legacy Actions secret available for
rollback. A custom build can supply its own ingestion key at compile time.

Every local-app event carries immutable `product: claude_view`, `schema_version: 1`,
`deployment: local_app`, and `source: server | web`. The existing `surface` and
`action` vocabulary is retained. `environment: production` distinguishes real
usage from reserved synthetic `qualification:` install IDs. Optional relay and
share-worker emitters also stamp the product but have separate deployments and
service identities; they must not be counted as desktop installations.

The persisted install UUID and consent file are not rewritten on upgrade. Both
browser and server use `claude_view:<existing UUID>` on the wire. Browser storage
uses a product-specific namespace. Sign-in does not call `identify` or `alias`,
and no email/account profile is sent. Person profiles, replay, automatic page
capture, performance capture and geolocation are disabled. The browser boundary
retains only enumerated event names and coarse, allowlisted properties; it drops
SDK URLs/referrers, DOM, plugin names and profile writes.

## Historical continuity

The previous claude-view project and its dashboards remain intact. Old binaries
keep sending there; only releases built with the shared destination send to the
new project. There is no destructive migration, event backfill, identity alias
or cross-project deduplication. Acquisition and first-seen charts in the shared
project start a new observation window; they are **not** historical signup counts.
Upgrade does not replay one-time install or activation events.

Each product has its own dashboard. Every event-reading SQL source and every
native query must contain `product = claude_view`, `deployment = local_app` and
`environment = production`, even when the dashboard has the same filters. Never
copy another project's cohort/person IDs. A shared project shares access,
settings, retention and quota; product filters are not access control.

Core success, complete coverage and account-level retention remain unknown until
their evidence and definitions exist. Anonymous IDs measure installations, not
necessarily people. Synthetic qualification events never populate usage charts.
