import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { test } from 'node:test'
import { fetchRelease } from './download-release.mjs'

const archive = Buffer.from('synthetic release archive')
const digest = createHash('sha256').update(archive).digest('hex')

test('fetches and verifies the exact supported release artifact', async () => {
  for (const [arch, platform] of [
    ['x64', 'linux-x64'],
    ['arm64', 'linux-arm64'],
  ]) {
    const urls = []
    const result = await fetchRelease('0.46.0', arch, async (url) => {
      urls.push(url)
      return new Response(
        url.endsWith('checksums.txt') ? `${digest}  claude-view-${platform}.tar.gz\n` : archive,
      )
    })
    assert.deepEqual(result.archive, archive)
    assert.equal(result.sha256, digest)
    assert.deepEqual(urls, [
      'https://github.com/tombelieber/claude-view/releases/download/v0.46.0/checksums.txt',
      `https://github.com/tombelieber/claude-view/releases/download/v0.46.0/claude-view-${platform}.tar.gz`,
    ])
  }
})

test('rejects unavailable or missing checksums before downloading an artifact', async () => {
  for (const response of [
    new Response('', { status: 404 }),
    new Response(`${digest}  other.tar.gz`),
  ]) {
    let requests = 0
    await assert.rejects(
      fetchRelease('0.46.0', 'arm64', async () => {
        requests++
        return response
      }),
      /checksums unavailable|Missing checksum/,
    )
    assert.equal(requests, 1)
  }
})

test('rejects a corrupted download', async () => {
  await assert.rejects(
    fetchRelease(
      '0.46.0',
      'x64',
      async (url) =>
        new Response(
          url.endsWith('checksums.txt')
            ? `${'0'.repeat(64)}  claude-view-linux-x64.tar.gz`
            : archive,
        ),
    ),
    /Checksum mismatch/,
  )
})

test('rejects invalid versions and unsupported architectures without a request', async () => {
  const noFetch = () => {
    throw new Error('Unexpected request')
  }
  await assert.rejects(fetchRelease('../other', 'x64', noFetch), /exact release version/)
  await assert.rejects(fetchRelease('0.46.0', 'ppc64', noFetch), /Unsupported Docker architecture/)
  await assert.rejects(
    fetchRelease('0.46.0', 'constructor', noFetch),
    /Unsupported Docker architecture/,
  )
})
