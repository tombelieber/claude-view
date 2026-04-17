#!/usr/bin/env bun
// scripts/bench/api.ts
//
// Minimal baseline API latency bench. Replaces the "Today (est.)" column
// in the CQRS stats redesign plan's §13 performance table with measured
// p50/p99/max against a running claude-view dev server.
//
// Usage:
//   bun run scripts/bench/api.ts                  # 100 runs, default URL
//   RUNS=500 bun run scripts/bench/api.ts         # 500 runs per endpoint
//   CLAUDE_VIEW_URL=http://... bun run scripts/bench/api.ts
//   OUT=bench.json bun run scripts/bench/api.ts   # explicit output path
//
// Prerequisite: the server must be running (bun dev or cargo run).

import { $ } from 'bun'

const URL = process.env.CLAUDE_VIEW_URL ?? 'http://localhost:47892'
const RUNS = Number(process.env.RUNS ?? 100)
const WARMUP = Number(process.env.WARMUP ?? 10)

type EndpointDef = {
  name: string
  path: string | ((ctx: Ctx) => string)
}

type Ctx = {
  sessionId: string
}

const ENDPOINTS: EndpointDef[] = [
  { name: 'GET /api/sessions?limit=30', path: '/api/sessions?limit=30' },
  { name: 'GET /api/sessions?sort=tokens', path: '/api/sessions?sort=tokens' },
  { name: 'GET /api/sessions/:id', path: (c) => `/api/sessions/${c.sessionId}` },
  { name: 'GET /api/stats/dashboard', path: '/api/stats/dashboard' },
  { name: 'GET /api/contributions?range=90d', path: '/api/contributions?range=90d' },
  { name: 'GET /api/insights/categories', path: '/api/insights/categories' },
]

function percentile(percent: number, samples: number[]): number {
  if (samples.length === 0) return Number.NaN
  const sorted = [...samples].sort((a, b) => a - b)
  const idx = Math.ceil((percent / 100) * sorted.length) - 1
  return sorted[Math.max(0, Math.min(idx, sorted.length - 1))]
}

const REQUEST_TIMEOUT_MS = Number(process.env.REQUEST_TIMEOUT_MS ?? 10_000)

async function timeRequest(url: string): Promise<number> {
  const ctrl = new AbortController()
  const timer = setTimeout(() => ctrl.abort(), REQUEST_TIMEOUT_MS)
  const start = performance.now()
  try {
    const res = await fetch(url, { signal: ctrl.signal })
    await res.arrayBuffer()
    if (!res.ok) throw new Error(`HTTP ${res.status} on ${url}`)
    return performance.now() - start
  } catch (err) {
    if ((err as Error).name === 'AbortError') {
      throw new Error(
        `timeout >${REQUEST_TIMEOUT_MS}ms on ${url} — is the server busy? (server at 290% CPU during indexing will give garbage numbers)`,
      )
    }
    throw err
  } finally {
    clearTimeout(timer)
  }
}

type Stats = {
  p50: number
  p99: number
  max: number
  mean: number
  samples: number[]
}

async function bench(endpoint: EndpointDef, ctx: Ctx): Promise<Stats> {
  const path = typeof endpoint.path === 'function' ? endpoint.path(ctx) : endpoint.path
  const url = `${URL}${path}`

  for (let i = 0; i < WARMUP; i++) await timeRequest(url)

  const samples: number[] = []
  for (let i = 0; i < RUNS; i++) {
    samples.push(await timeRequest(url))
  }

  return {
    p50: percentile(50, samples),
    p99: percentile(99, samples),
    max: Math.max(...samples),
    mean: samples.reduce((a, b) => a + b, 0) / samples.length,
    samples,
  }
}

async function findSessionId(): Promise<string> {
  const res = await fetch(`${URL}/api/sessions?limit=1`)
  if (!res.ok) throw new Error(`probe failed: HTTP ${res.status}`)
  const data = (await res.json()) as { sessions?: Array<{ id?: string }> }
  const id = data?.sessions?.[0]?.id
  if (!id)
    throw new Error(
      'no sessions returned from /api/sessions?limit=1 — bench needs at least one session',
    )
  return id
}

async function checkServerReachable(): Promise<void> {
  const ctrl = new AbortController()
  const timer = setTimeout(() => ctrl.abort(), 5_000)
  try {
    const res = await fetch(`${URL}/api/sessions?limit=1`, { signal: ctrl.signal })
    if (!res.ok) throw new Error(`HTTP ${res.status}`)
  } catch (err) {
    clearTimeout(timer)
    console.error(`\nServer not reachable at ${URL}:`)
    const msg =
      (err as Error).name === 'AbortError'
        ? 'timeout >5s (server may be indexing — wait for quiet and retry)'
        : (err as Error).message
    console.error(`  ${msg}`)
    console.error('\nStart the server first (if not running):')
    console.error('  bun dev                                   # full dev stack')
    console.error('  ./scripts/cq run -p claude-view-server    # server only')
    console.error('')
    console.error('Override with CLAUDE_VIEW_URL=http://... if the port differs.')
    process.exit(1)
  }
  clearTimeout(timer)
}

function format(ms: number): string {
  if (!Number.isFinite(ms)) return 'n/a'
  if (ms < 10) return `${ms.toFixed(2)}ms`
  if (ms < 100) return `${ms.toFixed(1)}ms`
  return `${ms.toFixed(0)}ms`
}

async function main() {
  console.log(`claude-view API baseline bench`)
  console.log(`  url:    ${URL}`)
  console.log(`  runs:   ${RUNS} per endpoint`)
  console.log(`  warmup: ${WARMUP}`)

  await checkServerReachable()
  const sessionId = await findSessionId()
  console.log(`  sid:    ${sessionId}`)
  console.log('')

  const rows: Array<{ name: string; p50: number; p99: number; max: number; mean: number }> = []

  for (const ep of ENDPOINTS) {
    process.stdout.write(`  ${ep.name.padEnd(40)} `)
    try {
      const r = await bench(ep, { sessionId })
      console.log(`p50=${format(r.p50)} p99=${format(r.p99)} max=${format(r.max)}`)
      rows.push({ name: ep.name, p50: r.p50, p99: r.p99, max: r.max, mean: r.mean })
    } catch (err) {
      console.log(`FAIL (${(err as Error).message})`)
    }
  }

  console.log('')
  console.log('| Endpoint | p50 | p99 | max | mean |')
  console.log('|---|---|---|---|---|')
  for (const r of rows) {
    console.log(
      `| \`${r.name}\` | ${format(r.p50)} | ${format(r.p99)} | ${format(r.max)} | ${format(r.mean)} |`,
    )
  }

  const commit = (await $`git rev-parse HEAD`.text()).trim()
  const outPath = process.env.OUT ?? `bench-api-${commit.slice(0, 8)}-${Date.now()}.json`
  const payload = {
    url: URL,
    runs: RUNS,
    warmup: WARMUP,
    commit,
    timestamp: new Date().toISOString(),
    results: rows,
  }
  await Bun.write(outPath, JSON.stringify(payload, null, 2))
  console.log('')
  console.log(`Written: ${outPath}`)
  console.log('')
  console.log(
    'Paste the table above into §13 "Today (measured)" of the plan and reference this commit + JSON file.',
  )
}

main().catch((e) => {
  console.error(e)
  process.exit(1)
})
