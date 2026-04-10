import { execSync } from 'node:child_process'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig, devices } from '@playwright/test'

const webDir = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(webDir, '../..')

function resolveCommonRoot() {
  const gitCommonDir = execSync('git rev-parse --git-common-dir', {
    cwd: repoRoot,
    encoding: 'utf8',
  }).trim()
  const absoluteGitCommonDir = path.isAbsolute(gitCommonDir)
    ? gitCommonDir
    : path.resolve(repoRoot, gitCommonDir)
  return path.resolve(absoluteGitCommonDir, '..')
}

const commonRoot = resolveCommonRoot()

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  timeout: 180000,
  use: {
    baseURL: process.env.PW_BASE_URL ?? 'http://localhost:5173',
    trace: 'on-first-retry',
    actionTimeout: 15000,
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: [
    {
      command: `cd ${repoRoot} && ./scripts/cq run -p claude-view-server`,
      env: {
        STATIC_DIR: path.resolve(webDir, 'dist'),
        CARGO_INCREMENTAL: '0',
        CARGO_TARGET_DIR: path.join(commonRoot, 'target-playwright'),
      },
      url: 'http://localhost:47892/api/health',
      reuseExistingServer: !process.env.CI,
      timeout: 300000,
    },
    {
      command: 'cd ../../sidecar && bun run src/index.ts',
      url: 'http://localhost:3001/health',
      reuseExistingServer: !process.env.CI,
      timeout: 30000,
    },
  ],
})
