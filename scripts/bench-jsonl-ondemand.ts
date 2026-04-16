#!/usr/bin/env bun
/**
 * One-off benchmark: is "JSONL on-demand read" viable for replacing the
 * current parse-to-DB indexer in claude-view?
 *
 * Measures:
 *   1. JSONL file size distribution across ~/.claude/projects
 *   2. Parse latency (read + JSON.parse per line) stratified by size bucket
 *   3. Current DB state (row counts + file size) for comparison
 *
 * NOT a regression benchmark — this is data-gathering to back an architectural
 * decision (see docs/plans/2026-04-16-token-reconciliation-parsing-pipeline-revamp.md).
 * Run once, read the numbers, make the call.
 *
 * Usage:  bun scripts/bench-jsonl-ondemand.ts
 */

import { readdirSync, statSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { homedir } from 'node:os'
import { Database } from 'bun:sqlite'

type FileSample = { path: string; bytes: number; hasSubagents: boolean }
type ParseResult = {
  bytes: number
  parseMs: number
  lineCount: number
  assistantCount: number
  errorCount: number
}

const BUCKETS: Array<[string, number, number]> = [
  ['< 10 KB', 0, 10_000],
  ['10-100 KB', 10_000, 100_000],
  ['100 KB-1 MB', 100_000, 1_000_000],
  ['1-10 MB', 1_000_000, 10_000_000],
  ['10-100 MB', 10_000_000, 100_000_000],
  ['>= 100 MB', 100_000_000, Infinity],
]

const bucketOf = (bytes: number): string =>
  BUCKETS.find(([, lo, hi]) => bytes >= lo && bytes < hi)?.[0] ?? '?'

const fmtBytes = (n: number): string => {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`
}

const pctOfSorted = (sorted: number[], p: number): number => {
  if (!sorted.length) return 0
  const idx = Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))
  return sorted[idx]
}

function walkJsonl(root: string): FileSample[] {
  const out: FileSample[] = []
  const stack = [root]
  while (stack.length) {
    const dir = stack.pop()!
    let entries
    try {
      entries = readdirSync(dir, { withFileTypes: true })
    } catch {
      continue
    }
    for (const e of entries) {
      const p = join(dir, e.name)
      if (e.isDirectory()) {
        stack.push(p)
        continue
      }
      if (!e.isFile() || !p.endsWith('.jsonl')) continue
      try {
        const st = statSync(p)
        const sidePath = p.slice(0, -'.jsonl'.length)
        let hasSub = false
        try {
          hasSub = statSync(join(sidePath, 'subagents')).isDirectory()
        } catch {
          // no sidecar — fine
        }
        out.push({ path: p, bytes: st.size, hasSubagents: hasSub })
      } catch {
        // unreadable stat — skip
      }
    }
  }
  return out
}

function parseFile(path: string): ParseResult {
  const buf = readFileSync(path)
  const start = performance.now()
  const text = buf.toString('utf8')
  let lineCount = 0
  let assistantCount = 0
  let errorCount = 0
  let pos = 0
  while (pos < text.length) {
    const nl = text.indexOf('\n', pos)
    const end = nl === -1 ? text.length : nl
    if (end > pos) {
      lineCount++
      try {
        const obj = JSON.parse(text.slice(pos, end))
        if (obj && typeof obj === 'object' && (obj as { type?: unknown }).type === 'assistant') {
          assistantCount++
        }
      } catch {
        errorCount++
      }
    }
    pos = end + 1
  }
  return {
    bytes: buf.length,
    parseMs: performance.now() - start,
    lineCount,
    assistantCount,
    errorCount,
  }
}

function sampleN<T>(arr: T[], n: number): T[] {
  if (arr.length <= n) return [...arr]
  const idxs = new Set<number>()
  while (idxs.size < n) idxs.add(Math.floor(Math.random() * arr.length))
  return [...idxs].map((i) => arr[i])
}

// --- main ---------------------------------------------------------------

console.log('\n=== claude-view JSONL on-demand read viability benchmark ===\n')
console.log('Hypothesis: reading + parsing JSONL on session open is fast enough')
console.log('            to replace the parse-to-DB indexer for session body.\n')

// Phase 1 — discovery
const root = join(homedir(), '.claude', 'projects')
console.log(`Phase 1 — discovery  (root: ${root})`)
const walkStart = performance.now()
const files = walkJsonl(root)
const walkMs = performance.now() - walkStart

if (files.length === 0) {
  console.log('  ⚠ No JSONL files found. Exiting.')
  process.exit(0)
}

type BucketStat = { count: number; bytes: number }
const bucketStats = new Map<string, BucketStat>()
for (const [name] of BUCKETS) bucketStats.set(name, { count: 0, bytes: 0 })
let totalBytes = 0
let subagentFiles = 0
for (const f of files) {
  const s = bucketStats.get(bucketOf(f.bytes))!
  s.count++
  s.bytes += f.bytes
  totalBytes += f.bytes
  if (f.hasSubagents) subagentFiles++
}

console.log(`  walked in            ${walkMs.toFixed(0)} ms`)
console.log(`  total files:         ${files.length.toLocaleString()}`)
console.log(`  total bytes:         ${fmtBytes(totalBytes)}`)
console.log(`  with subagents/:     ${subagentFiles.toLocaleString()}`)
console.log()
console.log('  size histogram:')
console.log('    bucket            count       bytes')
for (const [name] of BUCKETS) {
  const s = bucketStats.get(name)!
  console.log(
    `    ${name.padEnd(16)} ${s.count.toString().padStart(6)}  ${fmtBytes(s.bytes).padStart(10)}`,
  )
}
console.log()

const sortedSizes = [...files.map((f) => f.bytes)].sort((a, b) => a - b)
const maxSize = sortedSizes[sortedSizes.length - 1]
console.log('  size percentiles:')
console.log(`    p50: ${fmtBytes(pctOfSorted(sortedSizes, 50))}`)
console.log(`    p95: ${fmtBytes(pctOfSorted(sortedSizes, 95))}`)
console.log(`    p99: ${fmtBytes(pctOfSorted(sortedSizes, 99))}`)
console.log(`    max: ${fmtBytes(maxSize)}`)
console.log()

// Phase 2 — parse benchmark
const SAMPLES_PER_BUCKET = 10
console.log(`Phase 2 — parse benchmark  (${SAMPLES_PER_BUCKET} files per bucket, warm cache)\n`)

const byBucket = new Map<string, FileSample[]>()
for (const [name] of BUCKETS) byBucket.set(name, [])
for (const f of files) byBucket.get(bucketOf(f.bytes))!.push(f)

// Warm disk cache so the first bucket measurement isn't penalised by cold pages
const warmup = files.slice(0, Math.min(50, files.length))
for (const f of warmup) {
  try {
    readFileSync(f.path)
  } catch {
    // ignore unreadable
  }
}

console.log('  bucket            samples   p50 ms   p95 ms   max ms   MB/s (avg)   err-lines')
for (const [name] of BUCKETS) {
  const bf = byBucket.get(name)!
  if (bf.length === 0) {
    console.log(`    ${name.padEnd(16)} (empty)`)
    continue
  }
  const picked = sampleN(bf, SAMPLES_PER_BUCKET)
  const results: ParseResult[] = []
  for (const f of picked) {
    try {
      results.push(parseFile(f.path))
    } catch {
      // ignore individual parse blowups
    }
  }
  if (results.length === 0) {
    console.log(`    ${name.padEnd(16)} (parse errors)`)
    continue
  }
  const times = [...results.map((r) => r.parseMs)].sort((a, b) => a - b)
  const p50 = pctOfSorted(times, 50)
  const p95 = pctOfSorted(times, 95)
  const maxT = times[times.length - 1]
  const totalBytesSample = results.reduce((a, r) => a + r.bytes, 0)
  const totalMsSample = results.reduce((a, r) => a + r.parseMs, 0)
  const mbps = totalMsSample > 0 ? totalBytesSample / 1024 / 1024 / (totalMsSample / 1000) : 0
  const errTotal = results.reduce((a, r) => a + r.errorCount, 0)
  console.log(
    `    ${name.padEnd(16)} ${results.length.toString().padStart(6)}  ${p50.toFixed(1).padStart(7)}  ${p95.toFixed(1).padStart(7)}  ${maxT.toFixed(1).padStart(7)}  ${mbps.toFixed(0).padStart(10)}   ${errTotal.toString().padStart(8)}`,
  )
}
console.log()

// Phase 3 — DB stats
console.log('Phase 3 — current DB state  (~/.claude-view/claude-view.db)\n')
const dbPath = join(homedir(), '.claude-view', 'claude-view.db')
let dbBytes = 0
try {
  dbBytes = statSync(dbPath).size
} catch {
  console.log(`  ⚠ DB not found at ${dbPath}. Skipping phase 3.`)
  process.exit(0)
}

const db = new Database(dbPath, { readonly: true })
const safeCount = (sql: string): number => {
  try {
    return (db.query(sql).get() as { c: number }).c
  } catch {
    return -1
  }
}

const tables = [
  ['sessions', 'SELECT COUNT(*) AS c FROM sessions'],
  ['turns', 'SELECT COUNT(*) AS c FROM turns'],
  ['hook_events', 'SELECT COUNT(*) AS c FROM hook_events'],
  ['commits', 'SELECT COUNT(*) AS c FROM commits'],
  ['session_commits', 'SELECT COUNT(*) AS c FROM session_commits'],
  ['invocations', 'SELECT COUNT(*) AS c FROM invocations'],
  ['reports', 'SELECT COUNT(*) AS c FROM reports'],
] as const

console.log(`  db file size:        ${fmtBytes(dbBytes)}`)
for (const [label, sql] of tables) {
  const n = safeCount(sql)
  if (n < 0) continue
  console.log(`  ${label.padEnd(16)}     ${n.toLocaleString()}`)
}
console.log()
console.log(
  `  db / jsonl ratio:    ${((dbBytes / totalBytes) * 100).toFixed(1)}%   ` +
    `(${fmtBytes(dbBytes)} mirror vs ${fmtBytes(totalBytes)} source)`,
)
console.log()

db.close()

// --- verdict hints ------------------------------------------------------

console.log('=== Verdict hints ===\n')
console.log('If p95 parse latency for the bucket matching your p95 session size is')
console.log('< 100 ms, on-demand JSONL read is viable: replace the parse-to-DB indexer')
console.log('with a light session_index + on-demand reader. If p95 > 500 ms, the')
console.log('architecture is not viable without additional caching (mmap / binary cache).')
console.log()
console.log('Bun JSON.parse is ~1.5-2x slower than Rust serde_json — treat bun')
console.log('numbers as a conservative upper bound on the Rust implementation.\n')
