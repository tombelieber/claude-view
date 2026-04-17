# Phase rehearsal scripts

Per-phase rehearsal logic for `scripts/rehearse-phase.sh`.

Each file in this directory is named `<phase>.sh` and must define three bash functions:

- `phase_forward_migrations` — apply the phase's forward migrations to `$REHEARSAL_DB`.
- `phase_smoke_test` — boot the app pointed at `$ACTIVE_DB` and verify it works against the migrated schema.
- `phase_rollback_check` — boot the app pointed at `$ACTIVE_DB` (the restored snapshot) under the `rehearsal-<phase>-pre-<stamp>` tag — verifies rollback is viable.

The orchestrator `scripts/rehearse-phase.sh` sources the phase file, then drives steps 1–6 of §11.6.

## Registering a new phase

1. Create `scripts/rehearse/<phase>.sh` defining the three functions above.
2. Run `./scripts/rehearse-phase.sh <phase>` (dry-run) to verify the flow.
3. Run `./scripts/rehearse-phase.sh <phase> --apply` to execute for real.

Phases with no script registered cannot be rehearsed — which is the point: no rehearsal, no production execution.
