// Bounded, local-only Docker qualification. All data, containers and volumes
// belong to a unique disposable Compose project; no host Claude data is used.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { createServer } from 'node:net'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const fixture = mkdtempSync(join(tmpdir(), 'claude-view-docker-'))
const projects = join(fixture, 'projects')
const project = `claude-view-test-${randomUUID().slice(0, 8)}`
const sessionId = randomUUID()
const encoded = 'C--Users-Docker-Tester-project'
mkdirSync(join(projects, encoded), { recursive: true })
writeFileSync(join(fixture, '.credentials.json'), 'synthetic-credential-must-not-be-mounted')
const transcript = join(projects, encoded, `${sessionId}.jsonl`)
const records = [
  {
    type: 'user',
    uuid: randomUUID(),
    sessionId,
    timestamp: '2026-10-20T00:00:00Z',
    cwd: 'C:\\Users\\Docker Tester\\project',
    version: '2.1.296',
    message: { role: 'user', content: 'Docker synthetic transcript verification' },
  },
  {
    type: 'assistant',
    uuid: randomUUID(),
    sessionId,
    timestamp: '2026-10-20T00:00:01Z',
    cwd: 'C:\\Users\\Docker Tester\\project',
    version: '2.1.296',
    message: {
      id: randomUUID(),
      role: 'assistant',
      model: 'claude-sonnet-4-6',
      content: [{ type: 'text', text: 'Docker synthetic response' }],
      usage: { input_tokens: 10, output_tokens: 5 },
    },
  },
]
const original = `${records.map((r) => JSON.stringify(r)).join('\n')}\n`
writeFileSync(transcript, original)

const listener = createServer()
await new Promise((done) => listener.listen(0, '127.0.0.1', done))
const port = listener.address().port
await new Promise((done) => listener.close(done))
const env = { ...process.env, CLAUDE_PROJECTS_DIR: projects, CLAUDE_VIEW_PORT: String(port) }
const composeArgs = ['compose', '-f', join(repo, 'docker-compose.yml'), '-p', project]
function docker(args, { allowFailure = false, ...options } = {}) {
  const result = spawnSync('docker', args, {
    cwd: repo,
    env,
    encoding: 'utf8',
    timeout: 180_000,
    maxBuffer: 8 * 1024 * 1024,
    ...options,
  })
  if (!allowFailure && (result.status !== 0 || result.error)) {
    throw new Error(
      `docker ${args.slice(0, 3).join(' ')} failed: ${result.error?.message ?? result.stderr}`,
    )
  }
  return result
}
function compose(args, options) {
  return docker([...composeArgs, ...args], options)
}
const base = `http://127.0.0.1:${port}`
async function api(path, options = {}) {
  const response = await fetch(base + path, { signal: AbortSignal.timeout(5000), ...options })
  assert.equal(response.status, 200, `${path}: ${await response.clone().text()}`)
  return response.json()
}
async function ready(expectedId = sessionId) {
  for (let attempt = 0; attempt < 60; attempt++) {
    try {
      const sessions = await api('/api/sessions')
      if (sessions.sessions.some((s) => s.id === expectedId)) return
    } catch {
      /* Startup/indexing is asynchronous; inspect the terminal result below. */
    }
    await new Promise((done) => setTimeout(done, 500))
  }
  throw new Error('Container did not index the synthetic transcript within 30 seconds')
}

let cid
try {
  const config = JSON.parse(compose(['config', '--format', 'json']).stdout)
  const service = config.services['claude-view']
  assert.equal(service.ports[0].host_ip, '127.0.0.1')
  const windows = JSON.parse(
    compose(['config', '--format', 'json'], {
      env: { ...env, CLAUDE_PROJECTS_DIR: 'C:/Users/Docker Tester/.claude/projects' },
    }).stdout,
  )
  assert.match(windows.services['claude-view'].volumes[0].source, /^C:\/Users\/Docker Tester/)

  // Build through the documented entry point, including fail-closed checksums.
  compose(['up', '--build', '-d', '--wait', '--wait-timeout', '90'])
  cid = compose(['ps', '-q', 'claude-view']).stdout.trim()
  const inspected = JSON.parse(docker(['inspect', cid]).stdout)[0]
  const image = JSON.parse(docker(['image', 'inspect', inspected.Image]).stdout)[0]
  if (process.env.DOCKER_DEFAULT_PLATFORM) {
    assert.equal(`linux/${image.Architecture}`, process.env.DOCKER_DEFAULT_PLATFORM)
  }
  assert.equal(inspected.Config.User, 'node')
  assert.equal(inspected.HostConfig.ReadonlyRootfs, true)
  assert.deepEqual(inspected.HostConfig.CapDrop, ['ALL'])
  assert(inspected.HostConfig.SecurityOpt.includes('no-new-privileges:true'))
  assert.deepEqual(inspected.HostConfig.PortBindings['47892/tcp'], [
    { HostIp: '127.0.0.1', HostPort: String(port) },
  ])
  assert.equal(inspected.Mounts.find((m) => m.Destination === '/sessions/projects').RW, false)
  assert.equal(inspected.State.Health.Status, 'healthy')
  await ready()
  const version = JSON.parse(readFileSync(join(repo, 'npx-cli/package.json'))).version
  assert.equal((await api('/api/health')).version, version)
  const messages = await api(`/api/sessions/${sessionId}/messages?format=block`)
  assert.match(JSON.stringify(messages), /Docker synthetic transcript verification/)
  assert.match(JSON.stringify(messages), /Docker synthetic response/)
  assert.equal((await api(`/api/sessions/${sessionId}`)).configDir, '/sessions')
  const newId = randomUUID()
  writeFileSync(
    join(projects, encoded, `${newId}.jsonl`),
    `${records
      .map((r) =>
        JSON.stringify({
          ...r,
          sessionId: newId,
          uuid: randomUUID(),
        }),
      )
      .join('\n')}\n`,
  )
  await ready(newId)
  assert.match(JSON.stringify(await api('/api/export/sessions?format=json')), new RegExp(sessionId))
  const page = await fetch(base)
  assert.equal(page.status, 200)
  const html = await page.text()
  const asset = html.match(/src="(\/assets\/[^" ]+\.js)"/)?.[1]
  assert(asset, 'Bundled frontend entry point must be present')
  assert.equal((await fetch(base + asset)).status, 200)
  assert.equal(
    (
      await fetch(`${base}/api/sessions`, { headers: { Origin: 'https://example.invalid' } })
    ).headers.get('access-control-allow-origin'),
    null,
  )

  docker([
    'exec',
    cid,
    'node',
    '--input-type=module',
    '--eval',
    `
    import assert from 'node:assert/strict'; import fs from 'node:fs';
    assert.notEqual(process.getuid(), 0);
    for (const path of ['/sessions/projects/write-probe', '/app/write-probe']) {
      assert.throws(() => fs.writeFileSync(path, 'blocked'), e => ['EROFS', 'EACCES'].includes(e.code));
    }
    assert.equal(fs.existsSync('/sessions/.credentials.json'), false);
    assert.equal(fs.existsSync('/data/home/.claude/.credentials.json'), false);
    fs.writeFileSync('/data/persistence-probe', 'preserved');
  `,
  ])
  assert.match(docker(['exec', cid, 'claude', '--version']).stdout, /2\.1\.296/)
  assert.deepEqual(await api('/api/sidecar/sessions'), { active: [] })
  await api('/api/settings', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ llmModel: 'sonnet', llmTimeoutSecs: 61 }),
  })
  compose(['down'])
  compose(['up', '-d', '--wait', '--wait-timeout', '90'])
  await ready()
  cid = compose(['ps', '-q', 'claude-view']).stdout.trim()
  assert.equal((await api('/api/settings')).llmTimeoutSecs, 61)
  assert.equal(docker(['exec', cid, 'cat', '/data/persistence-probe']).stdout, 'preserved')
  assert.equal(readFileSync(transcript, 'utf8'), original, 'Host transcript must remain unchanged')
  assert.equal(
    readFileSync(join(fixture, '.credentials.json'), 'utf8'),
    'synthetic-credential-must-not-be-mounted',
  )
  compose(['down'])
  const missing = join(fixture, 'does-not-exist')
  const rejected = compose(['up', '-d', '--no-build'], {
    allowFailure: true,
    env: { ...env, CLAUDE_PROJECTS_DIR: missing },
  })
  assert.notEqual(rejected.status, 0, 'A missing host directory must fail closed')
  assert.match(rejected.stderr, /bind source path does not exist/)
  assert.equal(existsSync(missing), false)
  console.info(
    JSON.stringify({
      version,
      platform: inspected.Platform,
      architecture: image.Architecture,
      nonRoot: true,
      localhostOnly: true,
      readOnlyHost: true,
      hostCredentialsExcluded: true,
      transcriptRead: true,
      newTranscriptDiscovered: true,
      export: true,
      frontend: true,
      sidecar: true,
      persistedAcrossRecreate: true,
      windowsComposePath: true,
      missingSourceRejected: true,
    }),
  )
} catch (error) {
  process.stderr.write(
    compose(['logs', '--no-color', '--tail', '50'], { allowFailure: true }).stdout,
  )
  throw error
} finally {
  const cleanup = compose(['down', '-v', '--remove-orphans'], { allowFailure: true })
  rmSync(fixture, { recursive: true, force: true })
  if (cleanup.status !== 0) {
    console.error(`Task-owned Compose cleanup failed: ${cleanup.stderr}`)
    process.exitCode = 1
  }
  docker(['image', 'rm', `${project}-claude-view`], { allowFailure: true })
}
