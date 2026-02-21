# costUSD Parity with ccusage

## Goal

Match ccusage's cost calculation: use per-entry `costUSD` from JSONL (auto mode) instead of recalculating from aggregated tokens.

## Changes

### 1. Parser — extract `costUSD`

Add `cost_usd: Option<f64>` to `JsonlLine`. Parse `"costUSD"` from each JSONL entry's top level.

### 2. Accumulator — sum per session

Sum `costUSD` across all entries in a session. When `costUSD` is missing from an entry, calculate from that entry's tokens using LiteLLM rates (auto mode fallback).

### 3. DB — store per-session cost

Add `total_cost_usd REAL DEFAULT NULL` to sessions table. Bump `parse_version` to re-index.

### 4. API — aggregate from stored costs

`GET /api/stats/ai-generation` uses `SUM(total_cost_usd)` for the total. Itemized breakdown (input/output/cache split) stays tokens x LiteLLM rates since `costUSD` is a single number.

### 5. Frontend — no changes

`CostBreakdownCard` already displays `AggregateCostBreakdown`. Numbers become more accurate automatically.
