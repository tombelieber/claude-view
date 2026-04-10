import { build } from 'esbuild'
import { mkdirSync, rmSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

// Keep the sidecar artifact self-contained, but avoid the tsup/esbuild service
// crash path that was killing `bun preview` before the bundle even started.
const sidecarRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const distDir = resolve(sidecarRoot, 'dist')
const outfile = resolve(distDir, 'index.js')

rmSync(distDir, { force: true, recursive: true })
mkdirSync(distDir, { recursive: true })

await build({
  entryPoints: [resolve(sidecarRoot, 'src/index.ts')],
  outfile,
  bundle: true,
  platform: 'node',
  format: 'esm',
  target: 'node20',
  sourcemap: true,
  minify: false,
  external: ['bufferutil', 'utf-8-validate'],
  banner: {
    js: [
      '// claude-view sidecar — self-contained bundle, no node_modules required',
      '// Create a CJS-compatible require for Node built-ins (ws uses require("events") etc.)',
      'import { createRequire as __createRequire } from "node:module";',
      'const require = __createRequire(import.meta.url);',
    ].join('\n'),
  },
  logLevel: 'info',
})
