import path from 'path'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
    dedupe: ['react', 'react-dom'],
  },
  test: {
    globals: true,
    environment: 'happy-dom',
    setupFiles: ['./src/test-setup.ts'],
    exclude: ['**/node_modules/**', '**/e2e/**', '**/.claude/**', '**/.worktrees/**'],
    server: {
      deps: {
        // @lobehub/fluent-emoji ships ESM with bare directory imports
        // (e.g., `export { default } from "./FluentEmoji"` pointing to a directory).
        // Node's ESM resolver rejects these. Inlining lets Vite resolve them.
        inline: ['@lobehub/fluent-emoji', '@lobehub/ui'],
      },
    },
  },
})
