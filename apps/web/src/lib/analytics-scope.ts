// Shared helpers for rendering the analytics "scope" disclosure line.
//
// `dataScope` + `sessionBreakdown` are guaranteed present by the generated ts-rs
// contract (the backend serializes `AnalyticsScopeMeta` with non-Option fields),
// so these helpers operate on the required generated types — no optional-chaining
// or `?? fallback` guards are needed. This module replaces the per-component
// hand-written optional `ScopeMeta` forks that previously contradicted the
// contract.
import type { AnalyticsDataScope } from '../types/generated/AnalyticsDataScope'
import type { AnalyticsScopeMeta } from '../types/generated/AnalyticsScopeMeta'

export function scopeLabel(scope: AnalyticsDataScope): string {
  return scope === 'primary_plus_subagent_work'
    ? 'primary + subagent work'
    : 'primary sessions only'
}

export function resolveSessionBreakdown(meta: AnalyticsScopeMeta) {
  const { primarySessions, sidechainSessions, otherSessions, totalObservedSessions } =
    meta.sessionBreakdown
  return {
    primarySessions,
    sidechainSessions,
    otherSessions,
    totalObservedSessions,
  }
}
