---
status: approved
date: 2026-02-16
purpose: ADR for relay hosting provider selection and re-evaluation policy
---

# ADR: Relay Hosting for Mobile PWA (Fly.io v1)

## Context

The mobile PWA architecture requires a cloud relay for long-lived WebSocket connections between desktop daemon and phone PWA. Payloads are end-to-end encrypted, and the relay is designed to be zero-knowledge.

This ADR updates the hosting decision context from `docs/plans/2026-02-12-mobile-pwa-design.md`.

## Decision

For v1, use **Fly.io** as the default hosting provider for the relay service.

## Why

1. Fastest path to production for the current Rust relay design.
2. Lower operational overhead than self-managed VM stacks.
3. Good fit for always-on WebSocket relay with container-based deploys.
4. Preserves multi-region expansion option without redesigning transport.

## Options Considered

| Option | Summary | Decision |
|--------|---------|----------|
| Fly.io | Best overall trade-off for speed + ops + acceptable cost at early scale | **Selected for v1** |
| Cloudflare Workers + Durable Objects | See detailed analysis below | Not selected for v1 |
| Hetzner Cloud (VM + LB) | Lowest raw infrastructure cost, but higher ops burden and weaker global footprint | Candidate if cost dominates |
| AWS API Gateway WebSocket | Mature managed offering, but high message + connection-minute cost profile for this pattern | Not selected |
| GCP Cloud Run (WebSocket) | Good managed runtime, but per-instance cost and reconnect constraints reduce fit for this relay shape | Not selected |
| Azure Web PubSub | Purpose-built managed realtime service, but pricing/units and lock-in are less favorable for current needs | Not selected |

### Cloudflare Durable Objects: Why Not v1

Cloudflare DO with hibernation looks attractive on paper (idle WebSockets cost ~$0), but has significant trade-offs for a real-time relay:

| Aspect | Impact on Relay Use Case |
|--------|--------------------------|
| **Cold start latency** | Hibernated DO takes 50-200ms to wake. Real-time session notifications would feel laggy. Fly.io is always hot (0ms). |
| **Per-request cost uncertainty** | $0.15/million requests. 10K users × 10 msg/min × 60 × 24 × 30 = 4.3B requests = $645/mo. Cost scales with message volume, not predictable flat rate. |
| **Memory limit** | 128MB per DO. Connection registry + rate limiter + pending pairs may be tight at scale. Fly.io gives 256MB-1GB. |
| **Single-colo pinning** | DO is pinned to one colo. Users on other continents get RTT latency. Fly.io allows multi-region with voluntary migration. |
| **Platform lock-in** | DO-specific APIs for WebSocket hibernation, state management. Migration would require significant rewrite. Fly.io uses standard containers. |

**When to reconsider:** If idle cost becomes dominant (e.g., 100K+ users with very low message frequency), the hibernation model may become cost-advantageous despite latency trade-offs.

## Scale Cost Snapshot (Directional)

Assumptions: 24/7 connections, low message payloads, and relay node capacity roughly 2,000 to 5,000 concurrent connections per 2 vCPU / 4 GB node. Numbers are directional planning inputs, not billing guarantees.

| Concurrent Connections | Fly (USD/mo, compute only) | Hetzner (EUR/mo, compute + LB) | Cloudflare DO (USD/mo, hibernation model) |
|------------------------|----------------------------|----------------------------------|--------------------------------------------|
| 10 | 22.22 | 10.08 | 5.00 |
| 100 | 22.22 | 10.08 | 5.00 |
| 1,000 | 22.22 | 10.08 | 5.17 |
| 10,000 | 44.44-111.10 | 14.17-26.44 | 9.84 |
| 100,000 | 444.40-1111.00 | 87.79-210.49 | 99.77 |

Important: currency, included traffic, and pricing dimensions differ by provider, so this table is a relative planning guide, not a strict apples-to-apples quote.

## Re-Evaluation Policy (Trigger-Based)

Re-open hosting decision if any trigger fires:

1. **Scale trigger:** 30-day rolling concurrent connections exceed **10,000**.
2. **Cost trigger:** relay infrastructure spend exceeds **$300/month** for **2 consecutive months**.
3. **Latency trigger:** relay p95 message delivery latency exceeds **250ms** for **2 consecutive weeks** in primary user regions.
4. **Reliability trigger:** more than **2 Sev-2 incidents/month** attributable to hosting platform limits.
5. **Lock-in trigger:** required product capability cannot be implemented without provider-specific protocol or runtime behavior.

## Portability Guardrails

Keep these constraints to make migration cheap:

1. Deploy relay as a standard container image.
2. Keep wire protocol provider-agnostic (plain WebSocket + existing E2E payload format).
3. Isolate platform config in env vars and deploy manifests.
4. Avoid provider-specific state semantics in core relay logic.

## Next Review

Review this ADR when the first trigger is met, or by **2026-06-01**, whichever comes first.
