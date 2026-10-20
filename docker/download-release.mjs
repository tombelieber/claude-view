import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

const releaseRoot = 'https://github.com/tombelieber/claude-view/releases/download'

export async function fetchRelease(version, arch, fetchImpl = fetch) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error('Expected an exact release version')
  const platform = new Map([
    ['x64', 'linux-x64'],
    ['arm64', 'linux-arm64'],
  ]).get(arch)
  if (!platform) throw new Error(`Unsupported Docker architecture: ${arch}`)
  const artifact = `claude-view-${platform}.tar.gz`
  const base = `${releaseRoot}/v${version}`
  const checksums = await fetchImpl(`${base}/checksums.txt`)
  if (!checksums.ok) throw new Error(`Release checksums unavailable: HTTP ${checksums.status}`)
  const expected = (await checksums.text())
    .split('\n')
    .map((line) => line.match(/^([0-9a-f]{64}) {2}(.+)$/))
    .find((match) => match?.[2] === artifact)?.[1]
  if (!expected) throw new Error(`Missing checksum for ${artifact}`)

  const response = await fetchImpl(`${base}/${artifact}`)
  if (!response.ok) throw new Error(`Release artifact unavailable: HTTP ${response.status}`)
  const archive = Buffer.from(await response.arrayBuffer())
  if (createHash('sha256').update(archive).digest('hex') !== expected) {
    throw new Error(`Checksum mismatch for ${artifact}`)
  }
  return { archive, artifact, sha256: expected }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const { version } = JSON.parse(readFileSync(new URL('./package.json', import.meta.url)))
  const { archive, artifact, sha256 } = await fetchRelease(version, process.arch)
  mkdirSync('/app', { recursive: true })
  writeFileSync('/download/release.tar.gz', archive)
  try {
    execFileSync('tar', ['xzf', '/download/release.tar.gz', '-C', '/app'])
  } finally {
    rmSync('/download/release.tar.gz', { force: true })
  }
  console.info(`Verified ${artifact} (v${version}, sha256:${sha256})`)
}
