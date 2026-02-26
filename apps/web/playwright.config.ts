import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  timeout: 180000, // 3 minute timeout (projects endpoint scans filesystem)
  use: {
    baseURL: 'http://localhost:47892',
    trace: 'on-first-retry',
    actionTimeout: 15000, // 15s timeout for actions
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'cargo run -p claude-view-server',
    url: 'http://localhost:47892/api/health',
    reuseExistingServer: !process.env.CI,
    timeout: 120000,
  },
})
