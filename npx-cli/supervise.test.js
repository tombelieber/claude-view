// Unit tests for the npx-cli crash supervisor decision logic.
// Pure functions — no process spawning — so they run fast and hermetically
// under plain `node npx-cli/supervise.test.js`.

const assert = require('assert')
const { EventEmitter } = require('events')
const { classifyExit, shouldRestart, superviseServer } = require('./supervise')

function test(name, fn) {
  fn()
  console.log(`PASS: ${name}`)
}

// Deterministic, synchronous fake `spawn`: returns a fresh EventEmitter per
// call so the test can drive 'exit'/'error' events by hand and observe whether
// the supervisor re-spawns.
function makeFakeSpawn() {
  const children = []
  const spawn = () => {
    const child = new EventEmitter()
    child.kill = () => {}
    children.push(child)
    return child
  }
  return {
    spawn,
    children,
    count: () => children.length,
    last: () => children[children.length - 1],
  }
}

// --- classifyExit: maps (code, signal) to an exit class ---

test('clean exit (code 0) is "normal"', () => {
  assert.strictEqual(classifyExit(0, null), 'normal')
})

test('non-zero exit code is "crash"', () => {
  assert.strictEqual(classifyExit(1, null), 'crash')
  assert.strictEqual(classifyExit(134, null), 'crash') // 128+SIGABRT style code
})

test('user-initiated stop signals are "user-stop" (never restart)', () => {
  assert.strictEqual(classifyExit(null, 'SIGINT'), 'user-stop')
  assert.strictEqual(classifyExit(null, 'SIGTERM'), 'user-stop')
  assert.strictEqual(classifyExit(null, 'SIGHUP'), 'user-stop')
})

test('crash signals (OOM-kill SIGKILL, abort SIGABRT, segfault) are "crash"', () => {
  assert.strictEqual(classifyExit(null, 'SIGKILL'), 'crash') // OS OOM-killer
  assert.strictEqual(classifyExit(null, 'SIGABRT'), 'crash') // panic=abort
  assert.strictEqual(classifyExit(null, 'SIGSEGV'), 'crash')
})

// --- shouldRestart: bounded restart within a rolling window ---

test('restarts a crash when no recent crashes', () => {
  assert.strictEqual(
    shouldRestart({
      exitClass: 'crash',
      crashTimes: [],
      now: 1000,
      maxRestarts: 5,
      windowMs: 60000,
    }),
    true,
  )
})

test('never restarts a normal exit', () => {
  assert.strictEqual(
    shouldRestart({
      exitClass: 'normal',
      crashTimes: [],
      now: 1000,
      maxRestarts: 5,
      windowMs: 60000,
    }),
    false,
  )
})

test('never restarts a user-stop', () => {
  assert.strictEqual(
    shouldRestart({
      exitClass: 'user-stop',
      crashTimes: [],
      now: 1000,
      maxRestarts: 5,
      windowMs: 60000,
    }),
    false,
  )
})

test('gives up after maxRestarts crashes within the window', () => {
  // 5 crashes already happened in the last 60s; a 6th must NOT restart.
  const crashTimes = [1000, 2000, 3000, 4000, 5000]
  assert.strictEqual(
    shouldRestart({ exitClass: 'crash', crashTimes, now: 6000, maxRestarts: 5, windowMs: 60000 }),
    false,
  )
})

test('restarts again once old crashes age out of the window', () => {
  // 5 crashes, but they are all older than the 60s window relative to now.
  const crashTimes = [1000, 2000, 3000, 4000, 5000]
  assert.strictEqual(
    shouldRestart({ exitClass: 'crash', crashTimes, now: 70000, maxRestarts: 5, windowMs: 60000 }),
    true,
  )
})

// --- superviseServer: the restart loop (injected spawn/clock/exit) ---

test('respawns once on a crash, then propagates the subsequent clean exit', () => {
  const f = makeFakeSpawn()
  const exits = []
  superviseServer({
    spawn: f.spawn,
    maxRestarts: 5,
    windowMs: 60000,
    now: () => 1000,
    log: () => {},
    onExit: (d) => exits.push(d),
  })
  f.last().emit('exit', 1, null) // crash → should restart
  f.last().emit('exit', 0, null) // clean → should propagate, no restart
  assert.strictEqual(f.count(), 2, 'should have spawned twice (initial + 1 restart)')
  assert.deepStrictEqual(exits, [{ code: 0, signal: null }])
})

test('never respawns on a clean exit', () => {
  const f = makeFakeSpawn()
  const exits = []
  superviseServer({
    spawn: f.spawn,
    maxRestarts: 5,
    windowMs: 60000,
    now: () => 1,
    log: () => {},
    onExit: (d) => exits.push(d),
  })
  f.last().emit('exit', 0, null)
  assert.strictEqual(f.count(), 1, 'clean exit must not respawn')
  assert.deepStrictEqual(exits, [{ code: 0, signal: null }])
})

test('never respawns on a deliberate stop signal', () => {
  const f = makeFakeSpawn()
  const exits = []
  superviseServer({
    spawn: f.spawn,
    maxRestarts: 5,
    windowMs: 60000,
    now: () => 1,
    log: () => {},
    onExit: (d) => exits.push(d),
  })
  f.last().emit('exit', null, 'SIGTERM')
  assert.strictEqual(f.count(), 1, 'SIGTERM must not respawn')
  assert.deepStrictEqual(exits, [{ code: null, signal: 'SIGTERM' }])
})

test('gives up after maxRestarts crashes within the window', () => {
  const f = makeFakeSpawn()
  const exits = []
  superviseServer({
    spawn: f.spawn,
    maxRestarts: 2,
    windowMs: 60000,
    now: () => 5000,
    log: () => {},
    onExit: (d) => exits.push(d),
  })
  f.last().emit('exit', 1, null) // crash 1 → restart (1/2)
  f.last().emit('exit', 1, null) // crash 2 → restart (2/2)
  f.last().emit('exit', 1, null) // crash 3 → budget exhausted → propagate
  assert.strictEqual(f.count(), 3, 'initial + exactly 2 restarts, then give up')
  assert.deepStrictEqual(exits, [{ code: 1, signal: null }])
})

test('a spawn error is surfaced and not retried', () => {
  const f = makeFakeSpawn()
  const exits = []
  superviseServer({
    spawn: f.spawn,
    maxRestarts: 5,
    windowMs: 60000,
    now: () => 1,
    log: () => {},
    onExit: (d) => exits.push(d),
  })
  f.last().emit('error', new Error('ENOENT'))
  assert.strictEqual(f.count(), 1, 'spawn error must not respawn')
  assert.strictEqual(exits.length, 1)
  assert.strictEqual(exits[0].code, 1)
})

console.log('\nAll supervise.js tests passed.')
