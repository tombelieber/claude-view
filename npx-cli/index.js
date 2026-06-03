#!/usr/bin/env node

const { execFileSync, spawn } = require('child_process')
const fs = require('fs')
const path = require('path')
const os = require('os')
const https = require('https')
const zlib = require('zlib')
const { classifyExit, superviseServer } = require('./supervise')

const VERSION = require('./package.json').version
const REPO = 'tombelieber/claude-view'
const BINARY_NAME = process.platform === 'win32' ? 'claude-view.exe' : 'claude-view'

// --- Platform detection ---

const PLATFORM_MAP = {
  'darwin-arm64': { artifact: 'claude-view-darwin-arm64.tar.gz', ext: 'tar.gz' },
  'darwin-x64': { artifact: 'claude-view-darwin-x64.tar.gz', ext: 'tar.gz' },
  'linux-x64': { artifact: 'claude-view-linux-x64.tar.gz', ext: 'tar.gz' },
  'linux-arm64': { artifact: 'claude-view-linux-arm64.tar.gz', ext: 'tar.gz' },
  'win32-x64': { artifact: 'claude-view-win32-x64.zip', ext: 'zip' },
}

const platformKey = `${process.platform}-${process.arch}`
const platformInfo = PLATFORM_MAP[platformKey]

if (!platformInfo) {
  console.error(
    `Error: Unsupported platform "${process.platform}" with architecture "${process.arch}".\n` +
      `Supported: macOS (arm64, x64), Linux (arm64, x64), Windows (x64).`,
  )
  process.exit(1)
}

// --- Cache paths ---

const cacheDir = path.join(os.homedir(), '.cache', 'claude-view')
const binDir = path.join(cacheDir, 'bin')
const versionFile = path.join(cacheDir, 'version')
const binaryPath = path.join(binDir, BINARY_NAME)
const distDir = path.join(binDir, 'dist')

// --- Helpers ---

function download(url) {
  return new Promise((resolve, reject) => {
    const request = https.get(url, (res) => {
      // Follow redirects (GitHub releases redirect to S3/CDN)
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        return download(res.headers.location).then(resolve, reject)
      }
      if (res.statusCode !== 200) {
        reject(new Error(`Download failed: HTTP ${res.statusCode} from ${url}`))
        res.resume()
        return
      }
      const chunks = []
      res.on('data', (chunk) => chunks.push(chunk))
      res.on('end', () => resolve(Buffer.concat(chunks)))
      res.on('error', reject)
    })
    request.on('error', reject)
  })
}

function extractTarGz(buffer, destDir) {
  // Use system tar — available on macOS, Linux, and modern Windows (tar ships with Win10+)
  fs.mkdirSync(destDir, { recursive: true })
  const tmpFile = path.join(os.tmpdir(), `claude-view-${Date.now()}.tar.gz`)
  fs.writeFileSync(tmpFile, buffer)
  try {
    execFileSync('tar', ['xzf', tmpFile, '-C', destDir], { stdio: 'pipe' })
  } finally {
    fs.unlinkSync(tmpFile)
  }
}

function extractZip(buffer, destDir) {
  // Use system tar on Windows 10+ (supports zip) or PowerShell as fallback
  fs.mkdirSync(destDir, { recursive: true })
  const tmpFile = path.join(os.tmpdir(), `claude-view-${Date.now()}.zip`)
  fs.writeFileSync(tmpFile, buffer)
  try {
    if (process.platform === 'win32') {
      execFileSync(
        'powershell',
        ['-Command', `Expand-Archive -Force -Path '${tmpFile}' -DestinationPath '${destDir}'`],
        { stdio: 'pipe' },
      )
    } else {
      execFileSync('unzip', ['-o', tmpFile, '-d', destDir], { stdio: 'pipe' })
    }
  } finally {
    fs.unlinkSync(tmpFile)
  }
}

function downloadChecksums(version) {
  const url = `https://github.com/${REPO}/releases/download/v${version}/checksums.txt`
  return download(url)
    .then((buf) => {
      const map = {}
      const lines = buf.toString('utf-8').split('\n')
      for (const line of lines) {
        // Format: "<64-hex-chars>  <filename>" (two spaces between hash and filename)
        const match = line.match(/^([0-9a-f]{64}) {2}(.+)$/)
        if (match) {
          map[match[2]] = match[1]
        }
      }
      return map
    })
    .catch(() => null) // Graceful fallback for older releases without checksums
}

function verifyChecksum(buffer, expectedHash) {
  const crypto = require('crypto')
  const actualHash = crypto.createHash('sha256').update(buffer).digest('hex')
  if (actualHash !== expectedHash) {
    console.error(`Checksum verification failed.`)
    console.error(`  Expected: ${expectedHash}`)
    console.error(`  Actual:   ${actualHash}`)
    process.exit(1)
  }
}

// --- Main ---

async function main() {
  // Check if cached version matches
  let needsDownload = true
  if (fs.existsSync(versionFile) && fs.existsSync(binaryPath)) {
    const cached = fs.readFileSync(versionFile, 'utf-8').trim()
    if (cached === VERSION) {
      needsDownload = false
    }
  }

  if (needsDownload) {
    const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${platformInfo.artifact}`
    console.log(`Downloading claude-view v${VERSION} for ${platformKey}...`)

    let buffer
    try {
      buffer = await download(url)
    } catch (err) {
      console.error(`\nFailed to download claude-view:\n  ${err.message}`)
      console.error(`\nURL: ${url}`)
      console.error(
        `\nCheck that release v${VERSION} exists at https://github.com/${REPO}/releases`,
      )
      process.exit(1)
    }

    // Verify checksum if available
    const checksums = await downloadChecksums(VERSION)
    if (checksums && checksums[platformInfo.artifact]) {
      verifyChecksum(buffer, checksums[platformInfo.artifact])
      console.log('Checksum verified.')
    }

    // Clean previous install
    fs.rmSync(binDir, { recursive: true, force: true })
    fs.mkdirSync(binDir, { recursive: true })

    // Extract
    try {
      if (platformInfo.ext === 'zip') {
        extractZip(buffer, binDir)
      } else {
        extractTarGz(buffer, binDir)
      }
    } catch (err) {
      console.error(`\nFailed to extract archive:\n  ${err.message}`)
      process.exit(1)
    }

    // Make binary executable (no-op on Windows)
    if (process.platform !== 'win32') {
      fs.chmodSync(binaryPath, 0o755)
    }

    // macOS Gatekeeper: remove quarantine flag from downloaded binary
    if (process.platform === 'darwin') {
      try {
        execFileSync('xattr', ['-dr', 'com.apple.quarantine', binDir], { stdio: 'pipe' })
      } catch {
        // Ignore — xattr may not be available or quarantine flag may not be set
      }
    }

    // Write version marker
    fs.mkdirSync(cacheDir, { recursive: true })
    fs.writeFileSync(versionFile, VERSION)

    console.log(`Installed to ${binDir}`)
  }

  // Verify binary exists
  if (!fs.existsSync(binaryPath)) {
    console.error(`Error: Binary not found at ${binaryPath}`)
    console.error('Try deleting ~/.claude-view/ and running again.')
    process.exit(1)
  }

  // Set STATIC_DIR so the server finds the frontend assets
  const env = { ...process.env, STATIC_DIR: distDir }

  // Sidecar bundle is self-contained (zero-install).
  // Just verify the entry point exists and set SIDECAR_DIR.
  const sidecarDir = path.join(binDir, 'sidecar')
  if (fs.existsSync(path.join(sidecarDir, 'dist', 'index.js'))) {
    env.SIDECAR_DIR = sidecarDir
  }

  // Run the server under a bounded crash-supervisor (see ./supervise.js).
  //
  // The server is long-running; an abnormal death — a panic=abort SIGABRT, an
  // OS out-of-memory SIGKILL, or a non-zero exit — used to leave the user with
  // a dead app until they manually relaunched. We now restart it on an abnormal
  // exit, bounded to MAX_RESTARTS within RESTART_WINDOW_MS so a hard crash-loop
  // gives up loudly instead of thrashing.
  //
  // Signal handling (intent unchanged):
  // - SIGINT: the terminal delivers it to the whole process group, so the child
  //   already receives it. The wrapper ignores it (so Node doesn't exit first)
  //   and treats the child's SIGINT death as a deliberate stop — never restart.
  // - SIGTERM/SIGHUP: forward to whichever child is currently running (it may
  //   not get them if they were sent to our PID only); also a deliberate stop.
  const MAX_RESTARTS = 5
  const RESTART_WINDOW_MS = 60_000

  // Prevent Node from exiting on SIGINT before the child does.
  const ignoreSigint = () => {}
  process.on('SIGINT', ignoreSigint)

  const supervisor = superviseServer({
    spawn: () => spawn(binaryPath, process.argv.slice(2), { stdio: 'inherit', env }),
    maxRestarts: MAX_RESTARTS,
    windowMs: RESTART_WINDOW_MS,
    now: () => Date.now(),
    log: (msg) => console.error(msg),
    onExit: ({ code, signal }) => {
      // Terminal disposition: restore default SIGINT behavior, then propagate
      // the child's fate faithfully so the parent shell sees the right status.
      process.removeListener('SIGINT', ignoreSigint)

      // Reaching here with a crash disposition means the supervisor exhausted
      // its restart budget — point the user at the evidence instead of dying
      // silently.
      if (classifyExit(code, signal) === 'crash') {
        const logDir = path.join(os.homedir(), '.claude-view', 'logs')
        console.error(
          `\nCheck ${path.join(logDir, 'crash-*.log')} for a panic backtrace, ` +
            `or your system log for an out-of-memory (jetsam) kill.`,
        )
      }

      if (signal) {
        // Re-signal ourselves so the parent shell sees 128 + signal number.
        process.kill(process.pid, signal)
      } else {
        process.exit(code ?? 1)
      }
    },
  })

  // Forward deliberate-stop signals to whichever child is currently running.
  for (const sig of ['SIGTERM', 'SIGHUP']) {
    process.on(sig, () => {
      const child = supervisor.getChild()
      if (child) child.kill(sig)
    })
  }
}

main()
