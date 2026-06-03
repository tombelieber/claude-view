// Crash-supervisor decision logic for the npx-cli launcher.
//
// The launcher spawns the Rust `claude-view` server as a child. Historically
// the wrapper relayed the child's exit verbatim — so ANY abnormal death of the
// server (a panic=abort SIGABRT, an OS OOM-kill SIGKILL, a non-zero exit) left
// the user staring at a dead app until they manually restarted. This module
// makes the launcher self-heal: restart the server on an abnormal exit, bounded
// to `maxRestarts` within a rolling `windowMs` so a hard crash-loop gives up
// loudly instead of thrashing forever.
//
// Pure functions, no side effects — the runtime loop in index.js injects the
// clock and the crash history so the policy is fully testable.

// Signals that mean "someone deliberately asked us to stop" — propagate, never
// restart. Everything else that kills the process (SIGKILL/SIGABRT/SIGSEGV/...)
// is treated as a crash worth recovering from.
const USER_STOP_SIGNALS = new Set(['SIGINT', 'SIGTERM', 'SIGHUP'])

/**
 * Classify a child exit into one of: 'normal' | 'user-stop' | 'crash'.
 * @param {number|null} code   exit code (null when killed by a signal)
 * @param {string|null} signal terminating signal name (null on normal exit)
 */
function classifyExit(code, signal) {
  if (signal) {
    return USER_STOP_SIGNALS.has(signal) ? 'user-stop' : 'crash'
  }
  return code === 0 ? 'normal' : 'crash'
}

/**
 * Decide whether to restart, given the recent crash history.
 * Only crashes are restartable, and only while fewer than `maxRestarts`
 * crashes have occurred within the trailing `windowMs`.
 * @returns {boolean}
 */
function shouldRestart({ exitClass, crashTimes, now, maxRestarts, windowMs }) {
  if (exitClass !== 'crash') return false
  const recent = crashTimes.filter((t) => now - t < windowMs)
  return recent.length < maxRestarts
}

/**
 * Run a child process under a bounded crash-supervisor.
 *
 * Dependency-injected so the restart loop is testable without real processes:
 *   - `spawn()`   -> returns a child (an EventEmitter emitting 'exit'/'error')
 *   - `now()`     -> current epoch ms (injected clock)
 *   - `log(msg)`  -> visible progress/diagnostic line
 *   - `onExit({code, signal})` -> terminal disposition (caller maps to process exit)
 *
 * Restarts only abnormal deaths, at most `maxRestarts` within `windowMs`;
 * beyond that it gives up and propagates the last disposition.
 *
 * @returns {{ getChild: () => any, crashCount: () => number }}
 */
function superviseServer({ spawn, maxRestarts, windowMs, now, log, onExit }) {
  const crashTimes = []
  let child = null

  const launch = () => {
    // Per-launch guard: a failed spawn can emit BOTH 'error' and 'exit'; we
    // must act on exactly one of them.
    let settled = false
    const settle = (disposition, restart) => {
      if (settled) return
      settled = true
      if (restart) {
        launch()
      } else {
        onExit(disposition)
      }
    }

    child = spawn()

    child.on('error', (err) => {
      // Spawn-level failure (binary missing / not executable) is a permanent
      // install problem, not a runtime crash — surface it, don't retry.
      log(`\nFailed to launch claude-view server: ${err.message}`)
      settle({ code: 1, signal: null }, false)
    })

    child.on('exit', (code, signal) => {
      const exitClass = classifyExit(code, signal)
      if (exitClass === 'crash') {
        const t = now()
        if (shouldRestart({ exitClass, crashTimes, now: t, maxRestarts, windowMs })) {
          crashTimes.push(t)
          const reason = signal ? `signal ${signal}` : `exit code ${code}`
          log(
            `\nclaude-view server died (${reason}) — restarting ` +
              `(${crashTimes.length}/${maxRestarts} within ${Math.round(windowMs / 1000)}s)...`,
          )
          settle(null, true)
          return
        }
        log(
          `\nclaude-view server crashed ${maxRestarts} times within ` +
            `${Math.round(windowMs / 1000)}s — giving up.`,
        )
      }
      settle({ code, signal }, false)
    })
  }

  launch()
  return { getChild: () => child, crashCount: () => crashTimes.length }
}

module.exports = { classifyExit, shouldRestart, superviseServer, USER_STOP_SIGNALS }
