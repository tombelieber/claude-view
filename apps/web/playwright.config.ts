import path from 'path'
import { defineConfig, devices } from '@playwright/test'

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
    command: `cd ${path.resolve(__dirname, '../..')} && cargo run -p claude-view-server`,
    env: { STATIC_DIR: path.resolve(__dirname, 'dist') },
    url: 'http://localhost:47892/api/health',
    reuseExistingServer: !process.env.CI,
    timeout: 120000,
  },
})
