import { spawnSync } from 'node:child_process'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

export function validateReleaseVersion(version) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error('Expected a plain major.minor.patch release version')
  }
  return version
}

export function requireBinaryVersion(result, version) {
  if (result.error || result.status !== 0) {
    throw new Error(`npx did not exit successfully: ${result.error?.message ?? result.status}`)
  }
  const expected = `claude-view ${version}`
  if (!result.stdout.split(/\r?\n/).some((line) => line.trim() === expected)) {
    throw new Error(`Missing exact binary version line: ${expected}`)
  }
}

export function checkPublishedNpm(version) {
  validateReleaseVersion(version)
  // A maintainer checkout has a same-named private workspace. Run the actual
  // customer command outside it, and retain all cold-install progress output.
  const cwd = mkdtempSync(join(tmpdir(), 'claude-view-published-npm-'))
  const command = (name, args) => {
    const result = spawnSync(name, args, {
      cwd,
      encoding: 'utf8',
      timeout: 120_000,
      maxBuffer: 4 * 1024 * 1024,
    })
    if (result.stdout) process.stdout.write(result.stdout)
    if (result.stderr) process.stderr.write(result.stderr)
    return result
  }
  try {
    const binary = command('npx', ['--yes', `claude-view@${version}`, '--version'])
    requireBinaryVersion(binary, version)
    const plugin = command('npm', ['view', `@claude-view/plugin@${version}`, 'version'])
    if (plugin.error || plugin.status !== 0 || plugin.stdout.trim() !== version) {
      throw new Error('Published plugin version check failed')
    }
    console.info(`OK: published launcher and plugin both match ${version}`)
  } finally {
    rmSync(cwd, { recursive: true, force: true })
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    checkPublishedNpm(process.argv[2] ?? '')
  } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}
