// sidecar/src/sidecar-standalone.integration.test.ts
// Integration test: proves the sidecar starts and responds to a health check
// from an isolated temp directory with ONLY dist/ (no node_modules).

import { type ChildProcess, execFileSync, spawn } from 'node:child_process'
import { cpSync, existsSync, mkdtempSync, rmSync } from 'node:fs'
import { request } from 'node:http'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'

const sidecarRoot = resolve(fileURLToPath(import.meta.url), '../..')

describe('sidecar standalone (no node_modules)', () => {
  let tempDir: string
  let sidecarProcess: ChildProcess | null = null
  const socketPath = `/tmp/claude-view-sidecar-standalone-test-${process.pid}.sock`
  let processExited = false
  let exitCode: number | null = null

  beforeAll(() => {
    // Build the bundle
    execFileSync('bun', ['run', 'build'], { cwd: sidecarRoot, stdio: 'pipe' })

    // Copy only dist/ into an isolated temp directory
    tempDir = mkdtempSync(join(tmpdir(), 'sidecar-standalone-'))
    cpSync(resolve(sidecarRoot, 'dist'), join(tempDir, 'dist'), { recursive: true })

    // Verify no node_modules exists in the temp dir
    expect(existsSync(join(tempDir, 'node_modules'))).toBe(false)
  })

  afterAll(() => {
    if (sidecarProcess && !processExited) {
      sidecarProcess.kill('SIGTERM')
    }
    sidecarProcess = null
    if (tempDir) rmSync(tempDir, { recursive: true, force: true })
    rmSync(socketPath, { force: true })
  })

  it('starts and responds to health check without node_modules', async () => {
    sidecarProcess = spawn('node', [join(tempDir, 'dist', 'index.js')], {
      env: {
        ...process.env,
        SIDECAR_SOCKET: socketPath,
        CLAUDECODE: undefined,
        ANTHROPIC_API_KEY: undefined,
      },
      stdio: 'pipe',
    })

    sidecarProcess.on('exit', (code) => {
      processExited = true
      exitCode = code
    })

    let stderr = ''
    sidecarProcess.stderr?.on('data', (chunk: Buffer) => {
      stderr += chunk.toString()
    })

    // Wait for socket file to appear (max 5s)
    const startTime = Date.now()
    while (Date.now() - startTime < 5000) {
      if (processExited) {
        throw new Error(
          `Sidecar crashed on startup with exit code ${exitCode}.\n` +
            `This likely means a bundled dependency failed to load.\n` +
            `stderr: ${stderr}`,
        )
      }
      if (existsSync(socketPath)) break
      await new Promise((r) => setTimeout(r, 100))
    }

    expect(existsSync(socketPath), 'Socket file was not created within 5s').toBe(true)

    // Health check with retries
    let lastError: Error | null = null
    for (let attempt = 0; attempt < 5; attempt++) {
      try {
        const response = await new Promise<{ statusCode: number; body: string }>(
          (resolve, reject) => {
            const req = request({ socketPath, path: '/health', method: 'GET' }, (res) => {
              let body = ''
              res.on('data', (chunk) => {
                body += chunk
              })
              res.on('end', () => {
                resolve({ statusCode: res.statusCode ?? 0, body })
              })
            })
            req.on('error', reject)
            req.setTimeout(2000, () => {
              req.destroy()
              reject(new Error('Timeout'))
            })
            req.end()
          },
        )

        expect(response.statusCode).toBe(200)

        // Verify response body is valid JSON with expected shape
        const parsed = JSON.parse(response.body)
        expect(parsed.status).toBe('ok')
        expect(typeof parsed.uptime).toBe('number')
        return
      } catch (e) {
        lastError = e as Error
        await new Promise((r) => setTimeout(r, 200))
      }
    }
    throw new Error(`Health check failed after 5 attempts: ${lastError?.message}`)
  }, 15000)
})
