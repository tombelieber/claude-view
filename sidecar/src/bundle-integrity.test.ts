import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { beforeAll, describe, expect, it } from 'vitest'

const sidecarRoot = resolve(fileURLToPath(import.meta.url), '../..')

describe('bundle integrity', () => {
  beforeAll(() => {
    execFileSync('bun', ['run', 'build'], { cwd: sidecarRoot, stdio: 'pipe' })
    if (!existsSync(resolve(sidecarRoot, 'dist', 'index.js'))) {
      throw new Error('Build did not produce dist/index.js — check tsup config')
    }
  })

  it('dist/index.js contains no external import/require for bundled deps', () => {
    const bundle = readFileSync(resolve(sidecarRoot, 'dist', 'index.js'), 'utf-8')

    const mustBeInlined = [
      '@anthropic-ai/claude-agent-sdk',
      'hono',
      '@hono/node-server',
      'ws',
      'yaml',
      'zod',
    ]

    for (const dep of mustBeInlined) {
      const escaped = dep.replace(/\//g, '\\/')
      // Match only real import/require statements (start of line), not JSDoc comments
      const esmPattern = new RegExp(`^import\\s+.*from\\s+["']${escaped}["']`, 'm')
      const cjsPattern = new RegExp(
        `(?:^|[;=])\\s*(?:const|let|var)?\\s*\\S*\\s*=?\\s*require\\(["']${escaped}["']\\)`,
      )

      expect(bundle, `external ESM import found for "${dep}"`).not.toMatch(esmPattern)
      expect(bundle, `external CJS require found for "${dep}"`).not.toMatch(cjsPattern)
    }
  })

  it('dist/index.js only has external imports for node builtins', () => {
    const bundle = readFileSync(resolve(sidecarRoot, 'dist', 'index.js'), 'utf-8')

    // Match only real import statements at start of line (not JSDoc/comments)
    const importPattern = /^import\s+.*from\s+["']([^"']+)["']/gm
    const requirePattern = /\b__require\(["']([^"']+)["']\)/g
    const externals = new Set<string>()

    let match: RegExpExecArray | null
    while ((match = importPattern.exec(bundle)) !== null) {
      if (!match[1].startsWith('.')) externals.add(match[1])
    }
    while ((match = requirePattern.exec(bundle)) !== null) {
      if (!match[1].startsWith('.')) externals.add(match[1])
    }

    const nodeBuiltins = new Set([
      'fs',
      'fs/promises',
      'path',
      'os',
      'crypto',
      'http',
      'http2',
      'https',
      'net',
      'url',
      'stream',
      'events',
      'util',
      'child_process',
      'assert',
      'tty',
      'buffer',
      'querystring',
      'zlib',
      'tls',
      'dns',
      'dgram',
      'readline',
      'process',
      'node:fs',
      'node:fs/promises',
      'node:path',
      'node:os',
      'node:crypto',
      'node:http',
      'node:http2',
      'node:https',
      'node:net',
      'node:url',
      'node:stream',
      'node:events',
      'node:util',
      'node:child_process',
      'node:assert',
      'node:tty',
      'node:buffer',
      'node:querystring',
      'node:zlib',
      'node:tls',
      'node:dns',
      'node:dgram',
      'node:readline',
      'node:worker_threads',
      'node:async_hooks',
      'node:perf_hooks',
      'node:process',
      'bufferutil',
      'utf-8-validate',
    ])

    for (const ext of externals) {
      expect(
        nodeBuiltins.has(ext),
        `Non-builtin external found in bundle: "${ext}" — must be inlined or explicitly allowed`,
      ).toBe(true)
    }
  })

  it('bundle size is under 3 MB', () => {
    const bundle = readFileSync(resolve(sidecarRoot, 'dist', 'index.js'))
    const sizeMB = bundle.length / (1024 * 1024)
    expect(sizeMB).toBeLessThan(3)
  })

  it('bundle does not contain .node native addon references', () => {
    const bundle = readFileSync(resolve(sidecarRoot, 'dist', 'index.js'), 'utf-8')
    const nativePattern = /\.node["']/
    expect(bundle, 'Bundle references a .node native addon').not.toMatch(nativePattern)
  })
})
