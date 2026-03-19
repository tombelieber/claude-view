import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const rootPkg = JSON.parse(readFileSync(path.resolve(__dirname, '../../package.json'), 'utf-8'))

const enterprisePath = path.resolve(__dirname, '../../private/enterprise/web')
const hasEnterprise = existsSync(enterprisePath)

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(rootPkg.version),
  },
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@enterprise': hasEnterprise
        ? enterprisePath
        : path.resolve(__dirname, './src/enterprise-stubs'),
      '@': path.resolve(__dirname, './src'),
    },
    dedupe: ['react', 'react-dom', '@tanstack/react-query', 'react-router-dom'],
  },
  server: {
    port: 5173,
    host: true,
    proxy: {
      '/api/live/sessions': {
        target: 'http://localhost:47892',
        ws: true,
      },
      '/api/control/sessions': {
        target: 'http://localhost:47892',
        ws: true,
      },
      '/api/control/connect': {
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
