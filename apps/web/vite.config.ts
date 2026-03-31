import { readFileSync } from 'fs'
import path from 'path'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const rootPkg = JSON.parse(readFileSync(path.resolve(__dirname, '../../package.json'), 'utf-8'))

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(rootPkg.version),
    __APP_BUILD_DATE__: JSON.stringify(new Date().toISOString().split('T')[0]),
  },
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 5173,
    host: true,
    proxy: {
      // Sidecar TCP :3001 — must come before /api catch-all
      '/ws/chat': {
        target: 'ws://localhost:3001',
        ws: true,
        rewriteWsOrigin: true,
      },
      '/api/sidecar': {
        target: 'http://localhost:3001',
      },
      // Main server :47892
      '/api/live/sessions': {
        target: 'http://localhost:47892',
        ws: true,
      },
      '/api': 'http://localhost:47892',
    },
  },
  build: {
    outDir: 'dist',
  },
})
