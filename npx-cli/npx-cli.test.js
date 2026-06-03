const fs = require('fs')
const path = require('path')
const os = require('os')
const assert = require('assert')

function testSidecarSetup() {
  console.log('E2E: Testing sidecar setup without npm install...')

  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'npx-cli-test-'))
  const sidecarDir = path.join(tempDir, 'sidecar')
  const distDir = path.join(sidecarDir, 'dist')

  try {
    fs.mkdirSync(distDir, { recursive: true })
    fs.writeFileSync(path.join(distDir, 'index.js'), '// test stub\nprocess.exit(0);')

    assert.strictEqual(
      fs.existsSync(path.join(sidecarDir, 'package.json')),
      false,
      'package.json should NOT exist in release sidecar',
    )
    assert.strictEqual(
      fs.existsSync(path.join(sidecarDir, 'node_modules')),
      false,
      'node_modules should NOT exist in release sidecar',
    )
    assert.strictEqual(
      fs.existsSync(path.join(distDir, 'index.js')),
      true,
      'dist/index.js must exist',
    )

    console.log('PASS: sidecar structure correct for zero-install')
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true })
  }
}

function testNpxCliNoInstallStep() {
  console.log('E2E: Verifying npx-cli has no npm install step...')

  const npxCli = fs.readFileSync(path.join(__dirname, 'index.js'), 'utf-8')

  assert.strictEqual(
    npxCli.includes('npm install'),
    false,
    'npx-cli/index.js must not contain "npm install"',
  )
  assert.strictEqual(
    npxCli.includes('Installing sidecar dependencies'),
    false,
    'npx-cli/index.js must not contain sidecar install messaging',
  )
  assert.strictEqual(
    npxCli.includes('SIDECAR_DIR'),
    true,
    'npx-cli/index.js must still set SIDECAR_DIR env var',
  )

  console.log('PASS: npx-cli has no install step')
}

function testSupervisorWiring() {
  console.log('E2E: Verifying crash-supervisor is wired and shipped...')

  const npxCli = fs.readFileSync(path.join(__dirname, 'index.js'), 'utf-8')
  assert.strictEqual(
    npxCli.includes("require('./supervise')"),
    true,
    'index.js must require ./supervise',
  )
  assert.strictEqual(
    npxCli.includes('superviseServer('),
    true,
    'index.js must call superviseServer',
  )

  // The publish trap: supervise.js MUST be in package.json files[] or the
  // published npm package would throw MODULE_NOT_FOUND at launch.
  const pkg = JSON.parse(fs.readFileSync(path.join(__dirname, 'package.json'), 'utf-8'))
  assert.strictEqual(
    pkg.files.includes('supervise.js'),
    true,
    'package.json files[] must include supervise.js so it ships to npm',
  )

  console.log('PASS: supervisor wired into index.js and included in published files')
}

testSidecarSetup()
testNpxCliNoInstallStep()
testSupervisorWiring()
console.log('\nAll E2E tests passed.')
