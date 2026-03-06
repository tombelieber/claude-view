---
status: pending
date: 2026-03-06
phase: K1
depends_on: K
---

# Codex Parsing/Indexing Statistical Verification Protocol

**Purpose:** Define a reproducible, evidence-based protocol to verify Codex parsing/indexing/live outputs with real usage data and zero fabricated claims.

## Non-Negotiable Rules

1. No fabricated data for correctness claims.
2. Real usage traces must be consented and anonymized before use.
3. Every reported metric must be reproducible from stored artifacts.
4. If data provenance is missing, that sample is excluded from claims.

## Corpus Construction

## Data Sources

1. Codex historical rollouts from `~/.codex/sessions/**/rollout-*.jsonl`
2. Codex live traces captured from `codex exec --json`
3. Optional synthetic edge fixtures (marked `synthetic`) for parser hardening only

## Sampling Strategy

Use stratified sampling by:

1. session length bucket: short, medium, long
2. tool usage: no tool vs tool-heavy
3. live complexity: low event-rate vs bursty event-rate

Minimum target sample counts:

1. historical sessions: 250
2. live sessions: 50
3. tool-heavy sessions: 80

## Provenance Manifest (required per sample)

Each sample entry must include:

1. sample ID
2. source type (`real` or `synthetic`)
3. original capture path
4. capture timestamp
5. anonymization version
6. raw SHA256
7. anonymized SHA256
8. parser/indexer run ID that consumed it

## Metrics and Thresholds

## Parsing Integrity Metrics

1. `parse_success_rate = parsed_sessions / total_sessions`
   - threshold: >= 99.5%
2. `session_id_match_rate`
   - threshold: 100%
3. `event_type_coverage`
   - threshold: all known event classes observed in corpus have mapping
4. `unknown_event_ratio`
   - threshold: <= 1.0% and non-increasing over 3 runs

## Indexing Integrity Metrics

1. `idempotency_diff_count` between two consecutive full reindexes
   - threshold: 0
2. `field_accuracy` for key indexed fields vs source evidence:
   - `source`, `source_session_id`, `started_at`, `primary_model`, token fields
   - threshold: >= 99.0% per field
3. `duplicate_identity_conflicts`
   - threshold: 0

## Live Monitor Metrics

1. `live_ingest_lag_ms` (P95)
   - threshold: <= 3000 ms
2. `event_dedupe_accuracy`
   - threshold: >= 99.0%
3. `state_transition_validity`
   - threshold: >= 99.0% on labeled traces
4. `degraded_mode_recovery_time` (P95)
   - threshold: <= 10 s

## Statistical Analysis Methods

1. Confidence intervals:
   - Wilson 95% CI for rate metrics (`success_rate`, `field_accuracy`)
2. Drift detection:
   - PSI or KL divergence on event-type distribution across weekly runs
3. Regression detection:
   - compare current run vs baseline with non-overlapping CI rule
4. Outlier handling:
   - report outliers, do not silently remove; exclusions need explicit reason

## Verification Workflow (TDD-aligned)

1. RED:
   - add failing contract tests and expected metric assertions
2. GREEN:
   - implement minimal parsing/indexing/live changes
3. REFACTOR:
   - run full verification protocol and ensure thresholds pass
4. EVIDENCE SIGN-OFF:
   - generate signed report with run IDs, hashes, and threshold table

## Required Artifacts Per Run

Store under `artifacts/codex-evidence/reports/<run-id>/`:

1. `manifest.json` (sample provenance)
2. `metrics.json` (all computed metrics)
3. `threshold-eval.json` (pass/fail per metric)
4. `regression-diff.json` (vs previous baseline)
5. `summary.md` (human-readable conclusions)

## Anti-Fabrication Audit Checks

Run these checks before merge:

1. sample IDs in metrics must exist in manifest
2. manifest hashes must match files on disk
3. report values must be derivable from raw result tables
4. any manual metric override fails the gate
5. synthetic-only claims fail the gate

## Release Gates

## Gate 1: Dev Complete

1. all new tests exist and passed after prior failing evidence
2. parser/indexer thresholds pass on local validation subset

## Gate 2: Staging Verification

1. full corpus protocol run completed
2. all threshold checks pass
3. unknown event ratio within threshold

## Gate 3: Canary

1. feature flag enabled for controlled cohort
2. live lag/state metrics remain in bounds for 7 days
3. no P0/P1 integrity incidents

## Gate 4: GA

1. two consecutive successful weekly verification runs
2. no unresolved schema drift blocker
3. formal sign-off by engineering owner

## Failure Policy

Any threshold breach requires:

1. open blocking issue with run ID
2. rollback or feature-flag disable for affected path
3. rerun protocol after fix before re-enable

## Report Template (must be used)

1. run metadata
2. corpus composition table
3. parser/indexer/live metric table
4. CI and drift analysis
5. pass/fail gates
6. unresolved risks and next actions
