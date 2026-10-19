import assert from 'node:assert/strict'
import test from 'node:test'
import { requireBinaryVersion, validateReleaseVersion } from './check-published-npm.mjs'

test('cold install progress before the binary version is accepted', () => {
  requireBinaryVersion(
    { status: 0, stdout: '\nDownloading claude-view v0.45.1...\nclaude-view 0.45.1\n' },
    '0.45.1',
  )
})
test('an npm warning or download banner is not a binary version', () => {
  assert.throws(() =>
    requireBinaryVersion({ status: 0, stdout: 'Downloading claude-view v0.45.1...\n' }, '0.45.1'),
  )
  assert.throws(() =>
    requireBinaryVersion(
      { status: 0, stdout: 'npm warn package claude-view@0.45.1\nclaude-view 0.45.0\n' },
      '0.45.1',
    ),
  )
})
test('a failed or timed out installer cannot pass on matching output', () => {
  assert.throws(() => requireBinaryVersion({ status: 1, stdout: 'claude-view 0.45.1\n' }, '0.45.1'))
  assert.throws(() =>
    requireBinaryVersion(
      { status: null, error: new Error('timed out'), stdout: 'claude-view 0.45.1\n' },
      '0.45.1',
    ),
  )
})
test('manual recovery accepts only a plain release version', () => {
  assert.equal(validateReleaseVersion('0.45.1'), '0.45.1')
  for (const value of ['', 'main', 'v0.45.1', '0.45.1; echo invalid', '../0.45.1']) {
    assert.throws(() => validateReleaseVersion(value))
  }
})
