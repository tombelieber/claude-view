#!/usr/bin/env bun
/**
 * Full session lifecycle tracker.
 *
 * Captures every observable event with all identifiers:
 *   pid, sessionId, tmux name, timestamp, trigger point.
 *
 * Events tracked:
 *   FILE_CREATED    — {pid}.json appears in ~/.claude/sessions/
 *   TMUX_LINKED     — tmux cv-* session detected, pane PID matches
 *   TMUX_KILLED     — tmux cv-* session disappeared (tab closed / DELETE API)
 *   PROCESS_EXITED  — kill(pid,0) fails, process no longer running
 *   FILE_DELETED    — {pid}.json removed from disk
 *
 * Each event logs: { event, ts, pid, sessionId, tmuxName, elapsed, detail }
 *
 * Usage: bun scripts/watch-pid-lifecycle.ts
 */

import { execFileSync } from 'node:child_process'
import {
  appendFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  watch,
} from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

// ── paths ──

const SESSIONS_DIR = join(homedir(), '.claude', 'sessions')
const LOG_DIR = join(homedir(), '.claude-view', 'debug')
const LOG_FILE = join(LOG_DIR, 'pid-lifecycle.log')
const JSONL_FILE = join(LOG_DIR, 'pid-lifecycle.jsonl')

if (!existsSync(LOG_DIR)) mkdirSync(LOG_DIR, { recursive: true })

// ── state ──

interface SessionIdentity {
  pid: string
  sessionId: string | null
  tmuxName: string | null
  kind: string | null
  entrypoint: string | null
  cwd: string | null
}

interface TrackedSession extends SessionIdentity {
  createdAt: number
  events: EventRecord[]
}

interface EventRecord {
  event: string
  ts: string
  epochMs: number
  pid: string
  sessionId: string | null
  tmuxName: string | null
  processAlive: boolean
  fileExists: boolean
  elapsedSinceCreated: string
  elapsedSincePrev: string
  detail: string
}

const tracked = new Map<string, TrackedSession>()
const tmuxSessions = new Map<string, string>() // tmux name → pid

// ── helpers ──

function ts(): string {
  return new Date().toISOString()
}

function nowMs(): number {
  return Date.now()
}

function elapsed(fromMs: number): string {
  const ms = Date.now() - fromMs
  if (ms < 1000) return `${ms}ms`
  if (ms < 60000) return `${(ms / 1000).toFixed(2)}s`
  return `${(ms / 60000).toFixed(1)}m`
}

function gap(fromMs: number, toMs: number): string {
  const ms = toMs - fromMs
  if (ms < 1000) return `${ms}ms`
  if (ms < 60000) return `${(ms / 1000).toFixed(2)}s`
  return `${(ms / 60000).toFixed(1)}m`
}

function isPidAlive(pid: number): boolean {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

function fileStillExists(pid: string): boolean {
  return existsSync(join(SESSIONS_DIR, `${pid}.json`))
}

function readPidJson(pid: string): Partial<SessionIdentity> | null {
  const path = join(SESSIONS_DIR, `${pid}.json`)
  try {
    const raw = readFileSync(path, 'utf-8')
    const json = JSON.parse(raw)
    return {
      sessionId: json.sessionId ?? null,
      kind: json.kind ?? null,
      entrypoint: json.entrypoint ?? null,
      cwd: json.cwd ?? null,
    }
  } catch {
    return null
  }
}

// ── logging ──

function logText(msg: string): void {
  const line = `[${ts()}] ${msg}`
  try {
    appendFileSync(LOG_FILE, `${line}\n`)
  } catch {}
}

function logEvent(session: TrackedSession, event: string, detail: string): EventRecord {
  const now = nowMs()
  const prevEvent = session.events[session.events.length - 1]
  const prevMs = prevEvent ? prevEvent.epochMs : session.createdAt

  const record: EventRecord = {
    event,
    ts: ts(),
    epochMs: now,
    pid: session.pid,
    sessionId: session.sessionId,
    tmuxName: session.tmuxName,
    processAlive: isPidAlive(Number(session.pid)),
    fileExists: fileStillExists(session.pid),
    elapsedSinceCreated: elapsed(session.createdAt),
    elapsedSincePrev: gap(prevMs, now),
    detail,
  }

  session.events.push(record)

  // Human-readable log
  const icon =
    event === 'FILE_CREATED'
      ? '🟢'
      : event === 'TMUX_LINKED'
        ? '🔗'
        : event === 'TMUX_KILLED'
          ? '🚪'
          : event === 'PROCESS_EXITED'
            ? '💀'
            : event === 'FILE_DELETED'
              ? '🔴'
              : '⚡'

  logText(
    `${icon} ${event} | pid=${record.pid} sessionId=${record.sessionId ?? '?'} ` +
      `tmux=${record.tmuxName ?? '?'} | ` +
      `process=${record.processAlive ? 'alive' : 'DEAD'} file=${record.fileExists ? 'exists' : 'GONE'} | ` +
      `+${record.elapsedSincePrev} (total ${record.elapsedSinceCreated}) | ${detail}`,
  )

  // Machine-readable JSONL
  try {
    appendFileSync(JSONL_FILE, `${JSON.stringify(record)}\n`)
  } catch {}

  return record
}

function printSummary(session: TrackedSession): void {
  logText(`   ┌─── LIFECYCLE SUMMARY pid=${session.pid} ───`)
  logText(
    `   │ identity: sessionId=${session.sessionId} tmux=${session.tmuxName} ` +
      `kind=${session.kind} entrypoint=${session.entrypoint}`,
  )
  logText(`   │ cwd: ${session.cwd}`)

  for (let i = 0; i < session.events.length; i++) {
    const e = session.events[i]
    const prefix = i === session.events.length - 1 ? '└' : '├'
    logText(
      `   ${prefix}─ [${i + 1}] ${e.event.padEnd(15)} ${e.ts}  +${e.elapsedSincePrev.padEnd(8)} ` +
        `process=${e.processAlive ? 'alive' : 'DEAD'} file=${e.fileExists ? 'yes' : 'no'}`,
    )
  }

  // Gap analysis
  const events = session.events
  const findEvent = (name: string) => events.find((e) => e.event === name)
  const created = findEvent('FILE_CREATED')
  const tmuxKilled = findEvent('TMUX_KILLED')
  const processDead = findEvent('PROCESS_EXITED')
  const fileDeleted = findEvent('FILE_DELETED')

  if (tmuxKilled || processDead || fileDeleted) {
    logText('   ┌─── GAP ANALYSIS ───')
    if (created && tmuxKilled)
      logText(`   │ CREATED → TMUX_KILLED    = ${gap(created.epochMs, tmuxKilled.epochMs)}`)
    if (tmuxKilled && processDead)
      logText(`   │ TMUX_KILLED → PROC_EXIT  = ${gap(tmuxKilled.epochMs, processDead.epochMs)}`)
    if (tmuxKilled && fileDeleted)
      logText(`   │ TMUX_KILLED → FILE_DEL   = ${gap(tmuxKilled.epochMs, fileDeleted.epochMs)}`)
    if (processDead && fileDeleted)
      logText(`   │ PROC_EXIT → FILE_DEL     = ${gap(processDead.epochMs, fileDeleted.epochMs)}`)
    if (created && fileDeleted)
      logText(`   └ CREATED → FILE_DEL (total) = ${gap(created.epochMs, fileDeleted.epochMs)}`)
  }
}

// ── tmux scanner ──

function getTmuxSessions(): Map<string, string> {
  const result = new Map<string, string>()
  try {
    const output = execFileSync('tmux', ['list-sessions', '-F', '#{session_name}'], {
      encoding: 'utf-8',
      timeout: 2000,
      stdio: ['pipe', 'pipe', 'pipe'],
    }).trim()
    if (!output) return result

    for (const name of output.split('\n')) {
      if (!name.startsWith('cv-')) continue
      try {
        const pid = execFileSync('tmux', ['list-panes', '-t', name, '-F', '#{pane_pid}'], {
          encoding: 'utf-8',
          timeout: 2000,
          stdio: ['pipe', 'pipe', 'pipe'],
        }).trim()
        if (pid) result.set(name, pid)
      } catch {}
    }
  } catch {}
  return result
}

// ── initial scan ──

function scanExisting(): void {
  if (!existsSync(SESSIONS_DIR)) {
    logText(`Sessions dir not found: ${SESSIONS_DIR}`)
    return
  }

  const files = readdirSync(SESSIONS_DIR).filter((f) => f.endsWith('.json'))
  logText(`Initial scan: ${files.length} session file(s)`)

  for (const file of files) {
    const match = file.match(/^(\d+)\.json$/)
    if (!match) continue
    const pid = match[1]

    const fullPath = join(SESSIONS_DIR, file)
    try {
      const stat = statSync(fullPath)
      const alive = isPidAlive(Number(pid))
      const info = readPidJson(pid)

      const session: TrackedSession = {
        pid,
        sessionId: info?.sessionId ?? null,
        tmuxName: null,
        kind: info?.kind ?? null,
        entrypoint: info?.entrypoint ?? null,
        cwd: info?.cwd ?? null,
        createdAt: stat.birthtimeMs,
        events: [],
      }
      tracked.set(pid, session)

      logText(
        `  PRE-EXISTING pid=${pid} sessionId=${session.sessionId} ` +
          `kind=${session.kind} entrypoint=${session.entrypoint} ` +
          `alive=${alive} age=${elapsed(stat.birthtimeMs)}`,
      )
    } catch {
      logText(`  pid=${pid} stat failed`)
    }
  }

  // Link existing tmux sessions
  const currentTmux = getTmuxSessions()
  for (const [name, pid] of currentTmux) {
    tmuxSessions.set(name, pid)
    const entry = tracked.get(pid)
    if (entry) {
      entry.tmuxName = name
      logText(`  pid=${pid} → tmux=${name}`)
    }
  }
}

// ── pollers ──

function startPollers(): void {
  // Tmux poller — 200ms for tight timing
  setInterval(() => {
    const current = getTmuxSessions()

    // New tmux sessions
    for (const [name, pid] of current) {
      if (!tmuxSessions.has(name)) {
        tmuxSessions.set(name, pid)
        const entry = tracked.get(pid)
        if (entry) {
          entry.tmuxName = name
          // Also try to fill in sessionId if we didn't have it
          if (!entry.sessionId) {
            const info = readPidJson(pid)
            if (info?.sessionId) entry.sessionId = info.sessionId
          }
          logEvent(entry, 'TMUX_LINKED', `tmux=${name} pane_pid=${pid}`)
        }
      }
    }

    // Disappeared tmux sessions → tab closed
    for (const [name, pid] of tmuxSessions) {
      if (!current.has(name)) {
        const entry = tracked.get(pid)
        if (entry && !entry.events.some((e) => e.event === 'TMUX_KILLED')) {
          // Read pid.json NOW before it gets deleted — enrich identity
          if (!entry.sessionId) {
            const info = readPidJson(pid)
            if (info?.sessionId) entry.sessionId = info.sessionId
          }
          logEvent(entry, 'TMUX_KILLED', `tmux=${name} (tab closed / DELETE API called)`)
        }
        tmuxSessions.delete(name)
      }
    }
  }, 200)

  // Process death poller — 150ms
  setInterval(() => {
    for (const [pid, entry] of tracked) {
      if (!isPidAlive(Number(pid)) && !entry.events.some((e) => e.event === 'PROCESS_EXITED')) {
        logEvent(entry, 'PROCESS_EXITED', `kill(${pid}, 0) failed — process gone`)
        printSummary(entry)
        tracked.delete(pid)
      }
    }
  }, 150)
}

// ── filesystem watcher ──

function startFsWatcher(): void {
  if (!existsSync(SESSIONS_DIR)) {
    mkdirSync(SESSIONS_DIR, { recursive: true })
  }

  const watcher = watch(SESSIONS_DIR, (eventType, filename) => {
    if (!filename || !filename.endsWith('.json')) return
    const match = filename.match(/^(\d+)\.json$/)
    if (!match) return
    const pid = match[1]

    const fullPath = join(SESSIONS_DIR, filename)
    const exists = existsSync(fullPath)

    if (eventType === 'rename') {
      if (exists) {
        // ── FILE_CREATED ──
        if (!tracked.has(pid)) {
          const info = readPidJson(pid)
          const session: TrackedSession = {
            pid,
            sessionId: info?.sessionId ?? null,
            tmuxName: null,
            kind: info?.kind ?? null,
            entrypoint: info?.entrypoint ?? null,
            cwd: info?.cwd ?? null,
            createdAt: nowMs(),
            events: [],
          }
          tracked.set(pid, session)
          logEvent(
            session,
            'FILE_CREATED',
            `sessionId=${session.sessionId} kind=${session.kind} entrypoint=${session.entrypoint} cwd=${session.cwd}`,
          )
        }
      } else {
        // ── FILE_DELETED ──
        const entry = tracked.get(pid)
        if (entry) {
          logEvent(entry, 'FILE_DELETED', '{pid}.json removed from disk')
          // Keep tracking to observe when process ACTUALLY exits
          if (isPidAlive(Number(pid))) {
            logText(
              '   ⏳ Process still alive after file delete — continuing to track until PROCESS_EXITED',
            )
          } else {
            logEvent(entry, 'PROCESS_EXITED', 'process already dead at file delete time')
            printSummary(entry)
            tracked.delete(pid)
          }
        } else {
          logText(`🔴 FILE_DELETED pid=${pid} (untracked pre-existing session)`)
        }
      }
    } else if (eventType === 'change') {
      const entry = tracked.get(pid)
      if (entry) {
        // Re-read in case content changed
        const info = readPidJson(pid)
        if (info?.sessionId && !entry.sessionId) entry.sessionId = info.sessionId
        logEvent(entry, 'FILE_MODIFIED', 'pid.json content updated')
      }
    }
  })

  process.on('SIGINT', () => {
    logText('── Watcher stopped (SIGINT) ──')
    watcher.close()
    process.exit(0)
  })
  process.on('SIGTERM', () => {
    logText('── Watcher stopped (SIGTERM) ──')
    watcher.close()
    process.exit(0)
  })
}

// ── main ──

logText('')
logText('══════════════════════════════════════════════════════════')
logText('  pid.json full lifecycle tracker v3')
logText(`  sessions dir : ${SESSIONS_DIR}`)
logText(`  log (human)  : ${LOG_FILE}`)
logText(`  log (jsonl)  : ${JSONL_FILE}`)
logText('  events: FILE_CREATED → TMUX_LINKED → TMUX_KILLED → PROCESS_EXITED → FILE_DELETED')
logText('  each event: { pid, sessionId, tmuxName, timestamp, processAlive, fileExists, gaps }')
logText('══════════════════════════════════════════════════════════')

scanExisting()
startFsWatcher()
startPollers()

logText('Ready. Full lifecycle tracking active.\n')
