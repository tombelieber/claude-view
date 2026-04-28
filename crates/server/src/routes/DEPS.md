# Route Dependency Profiles

> Generated for ISP migration planning. Each route's actual AppState field usage
> determines which ISP trait it should be bounded on.

| Route | Fields Used | Target ISP Trait |
|-------|-------------|------------------|
| classify | db, classify, jobs, shutdown | ClassifyDeps |
| coaching | rules_dir | CoachingDeps |
| contributions | db, pricing | DbDeps + PricingDeps |
| hooks | db, live_sessions, live_manager, debug_hooks_log | LiveDeps |
| indexing | indexing, shutdown | IndexingDeps |
| insights | db | DbDeps |
| live | live_sessions, live_tx, live_manager, pricing, recently_closed, shutdown | LiveDeps |
| plugins | db, registry | DbDeps + RegistryDeps |
| reports | db | DbDeps |
| sessions | db, live_sessions, pricing | DbDeps + LiveDeps |
| stats | db, pricing | DbDeps + PricingDeps |
| statusline | transcript_to_session, debug_statusline_log | StatuslineDeps |
| sync | db, git_sync, indexing, registry, shutdown | SyncDeps (FullDeps) |
| system | db, indexing, telemetry | SystemDeps |
| terminal | db, live_sessions, recently_closed, terminal_connections, hook_event_channels | LiveDeps + TerminalDeps |
| turns | db | DbDeps |
| workflow_samples | (none) | NoDeps |
