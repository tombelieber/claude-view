import path from 'path'
import { fileURLToPath } from 'url'
import { defineConfig, devices } from '@playwright/test'

const webDir = path.dirname(fileURLToPath(import.meta.url))

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  timeout: 180000,
  use: {
    baseURL: 'http://localhost:47892',
    trace: 'on-first-retry',
    actionTimeout: 15000,
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: `cd ${path.resolve(webDir, '../..')} && cargo run -p claude-view-server`,
    env: {
      STATIC_DIR: path.resolve(webDir, 'dist'),
      CARGO_TARGET_DIR: path.resolve(webDir, '../../target-playwright'),
    },
    url: 'http://localhost:47892/api/health',
    reuseExistingServer: !process.env.CI,
    timeout: 300000,
  },
})
