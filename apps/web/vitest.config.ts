import { existsSync } from 'node:fs'
import path from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

const enterprisePath = path.resolve(__dirname, '../../private/enterprise/web')
const hasEnterprise = existsSync(enterprisePath)

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@enterprise': hasEnterprise
        ? enterprisePath
        : path.resolve(__dirname, './src/enterprise-stubs'),
      '@': path.resolve(__dirname, './src'),
    },
    dedupe: ['react', 'react-dom'],
  },
  test: {
    globals: true,
    environment: 'happy-dom',
    setupFiles: ['./src/test-setup.ts'],
    exclude: ['**/node_modules/**', '**/e2e/**', '**/.claude/**', '**/.worktrees/**'],
  },
})
