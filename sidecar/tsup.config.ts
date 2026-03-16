import { defineConfig } from 'tsup'

export default defineConfig({
  entry: ['src/index.ts'],
  format: ['esm'],
  target: 'node20',
  clean: true,
  noExternal: [/.*/],
  external: ['bufferutil', 'utf-8-validate'],
  platform: 'node',
  splitting: false,
  minify: false,
  sourcemap: true,
  banner: {
    js: '// claude-view sidecar — self-contained bundle, no node_modules required',
  },
})
