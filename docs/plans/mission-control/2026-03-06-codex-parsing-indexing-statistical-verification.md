---
status: approved
date: 2026-03-06
phase: K-verify
depends_on: K
---

# Codex Parsing/Indexing Statistical Verification Protocol (Evidence-Only)

**Purpose:** Define how parsing/indexing correctness is verified using real usage evidence, with zero fabricated datasets.

## Hard Rules

1. Use real Codex session data only (consented, de-identified).
2. Synthetic fixtures are allowed only for corruption/fault-injection tests, and must be tagged `synthetic_fault`.
3. Any fixture without provenance hash chain is invalid.
4. Validation reports must be reproducible from committed commands and manifests.

---

## Data Provenance Standard

Each fixture requires:

1. `fixture_id`
2. `source_type` (`real_user_usage` or `synthetic_fault`)
3. `collection_method`
4. `collected_at`
5. `raw_sha256`
6. `sanitized_sha256`
7. `sanitization_profile`
8. `owner_approval_ref` (or consent record ID)

Reject fixture if any required field is missing.

---

## Dataset Design

1. **Development set (70%)**: iterative RED/GREEN tests.
2. **Holdout set (30%)**: untouched until release-candidate validation.
3. **Stratification dimensions:**
- session length quantiles
- tool-call density
- compacted vs non-compacted sessions
- aborted/failed turns
- model/provider variety

Minimum sample target per release:

1. 200 real sessions total.
2. At least 30 sessions per major stratum.

---

## Ground Truth Construction

1. Use raw Codex JSONL as source-of-truth evidence.
2. Build an independent reference extractor script (separate from production parser path).
3. Compare production parser/indexer outputs against reference outputs on same fixture IDs.
4. Store row-level diff artifacts for every mismatch.

---

## Metrics and Statistical Checks

### Core Accuracy Metrics

1. Session discovery recall.
2. Session indexing precision.
3. Message count absolute error.
4. Tool-call count absolute error.
5. Token total relative error.
6. Timestamp ordering violation rate.

### Acceptance Thresholds

1. Discovery recall >= 99.5%.
2. Indexing precision >= 99.5%.
3. Message count median absolute error <= 0.
4. Tool-call count median absolute error <= 0.
5. Token relative error p95 <= 1.0%.
6. Ordering violation rate <= 0.1%.

### Inference/Confidence

1. Compute 95% bootstrap confidence intervals for each metric.
2. Gate passes only if lower CI bound still meets threshold (where applicable).
3. Gate fails on any threshold breach in holdout set.

---

## Live Monitor Verification

1. End-to-end latency p95 from event ingestion to UI update <= 2.5s.
2. Duplicate-event handling error rate <= 0.1%.
3. Reconnect recovery success >= 99%.
4. Degraded-mode false-positive rate <= 1%.

---

## Anti-Fabrication Controls

1. CI check: reject unmanifested fixtures.
2. CI check: reject fixtures with changed content but unchanged hash metadata.
3. CI check: reject `real_user_usage` fixtures lacking consent/provenance references.
4. Weekly audit: random fixture sampling with manual provenance verification.

---

## Required Artifacts Per Release

1. `artifacts/codex-evidence/fixture-manifest.json`
2. `artifacts/codex-evidence/reference-output.json`
3. `artifacts/codex-evidence/prod-output.json`
4. `artifacts/codex-evidence/diff-report.json`
5. `artifacts/codex-evidence/stat-summary.md`
6. `artifacts/codex-evidence/live-monitor-summary.md`

---

## Go/No-Go Rules

1. **GO** only if all holdout thresholds pass and Claude regressions are clean.
2. **NO-GO** if any parsing/indexing metric breaches threshold or provenance checks fail.
3. Any no-go requires root-cause report and re-run of full holdout validation.
