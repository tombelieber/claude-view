---
status: pending
date: 2026-03-06
phase: K
depends_on: K
---

# Codex Parsing/Indexing Evidence and Statistical Verification Protocol

## Purpose

Define a reproducible, non-fabricated verification protocol for Codex parsing/indexing integrity and live-monitor normalization quality.

## Evidence Source Policy

1. Production-quality claims must use real usage data samples from approved sources.
2. Synthetic data may be used only for deterministic edge-case unit tests.
3. Synthetic data must not be used as the sole basis for parser/indexer quality acceptance.
4. Every verification run must provide immutable run metadata and checksums.

## Data Handling and Privacy

1. Store only hashed session identifiers in evidence artifacts.
2. Remove or mask user-identifying free text where required by policy.
3. Keep raw source files in approved secure storage only; do not commit raw transcripts.
4. Commit only aggregate stats, hashed IDs, manifests, and reproducible summaries.

## Sampling Design

## Population

1. Sessions with `source=codex` available in the selected verification window.
2. Include both completed and interrupted sessions.

## Stratification Dimensions

1. Recency buckets:
   - Last 7 days
   - 8-30 days
   - 31-90 days
2. Session length buckets:
   - Short
   - Medium
   - Long
3. Tool-call density:
   - None
   - Low
   - High
4. Error state:
   - No parser/index flags
   - Parser/index warning/error flagged

## Sample Size Rules

1. Minimum total sample size: 400 sessions per verification run.
2. Minimum per high-risk stratum: 50 sessions when population permits.
3. Use fixed random seed recorded in manifest for reproducibility.
4. If population is smaller than target size, run full-population verification and record that condition.

## Metrics and Invariants

## Parsing Integrity Metrics

1. Required-field completeness rate.
2. Role-mapping correctness rate.
3. Chronological ordering validity rate.
4. Token-count consistency rate when token snapshots exist.

## Indexing Integrity Metrics

1. Session reconciliation mismatch rate between source rows and indexed rows.
2. Duplicate canonical ID rate.
3. Source-collision rate (`source`, `source_session_id` uniqueness).
4. Derived metric consistency rate (counts/tokens/cost fields where applicable).

## Live Normalization Metrics

1. Event deduplication correctness rate.
2. Out-of-order handling correctness rate.
3. Event drop rate.
4. End-to-end monitor update latency (p50/p95).

## Acceptance Thresholds

1. Required-field completeness:
   - Point estimate >= 99.5%
   - 95% CI lower bound >= 99.0%
2. Role-mapping correctness:
   - Point estimate >= 98.0%
   - 95% CI lower bound >= 97.0%
3. Source-collision rate:
   - Must equal 0
4. Reconciliation mismatch rate:
   - <= 0.1%
5. Live event drop rate:
   - <= 0.5%
6. Live p95 latency:
   - <= 3.0 seconds under verification load profile

## Confidence and Statistical Checks

1. Use Wilson score intervals for key binomial quality metrics.
2. Report confidence intervals in every stats report; never report only point estimates.
3. Fail verification when lower confidence bound violates threshold.
4. Include effect size or absolute delta against previous baseline for each metric.

## Drift Detection

1. Run drift checks on core feature distributions per release candidate:
   - Role distribution
   - Event type distribution
   - Tool-call density distribution
   - Parse-error category distribution
2. Compute PSI for bucketed features.
3. Use KS tests for continuous metrics where applicable.
4. Trigger investigation when:
   - PSI >= 0.2 for any core distribution
   - Statistically significant degradation with practical impact beyond accepted deltas

## Reproducible Artifact Contract

Every verification run must include:

1. `manifest.json`
   - run_id
   - git SHA
   - script/version SHA
   - UTC timestamp
   - sample seed
   - population and sampled counts
2. `sample_ids_hashed.csv`
   - hashed session identifiers and stratum labels
3. `parser_outputs.parquet`
   - normalized parser outputs needed for recomputation
4. `index_reconciliation.json`
   - row-level and aggregate reconciliation summary
5. `stats_report.md`
   - metrics, confidence intervals, threshold pass/fail, drift results
6. `checksums.sha256`
   - checksums for all produced artifacts

## TDD Gate Integration

1. RED gate:
   - Add failing tests for new/changed invariants first.
2. GREEN gate:
   - Make tests pass with minimal implementation change.
3. REFACTOR gate:
   - Refactor with unchanged outcomes and rerun full verification.
4. Evidence gate:
   - PR is blocked without valid artifact package and threshold pass.

## Release Gate Policy

1. Dev gate:
   - Test suites pass and artifact package generated successfully.
2. Staging gate:
   - Full statistical verification pass on approved sample.
3. Canary gate:
   - No drift alerts above threshold and no integrity regressions.
4. GA gate:
   - Two consecutive successful verification runs on release candidate builds.

## Failure Handling

1. Any missing artifact invalidates the run.
2. Any checksum mismatch invalidates the run.
3. Any threshold failure blocks release.
4. Any unverifiable metric claim is rejected.

