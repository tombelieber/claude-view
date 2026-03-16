import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { beforeAll, describe, expect, it } from 'vitest'

const sidecarRoot = resolve(fileURLToPath(import.meta.url), '../..')

describe('release artifact requirements', () => {
  beforeAll(() => {
    if (!existsSync(resolve(sidecarRoot, 'dist', 'index.js'))) {
      execFileSync('bun', ['run', 'build'], { cwd: sidecarRoot, stdio: 'pipe' })
    }
  })

  it('dist/index.js exists after build', () => {
    expect(existsSync(resolve(sidecarRoot, 'dist', 'index.js'))).toBe(true)
  })

  it('bundle does not dynamically read package.json at runtime', () => {
    const bundle = readFileSync(resolve(sidecarRoot, 'dist', 'index.js'), 'utf-8')
    const dynamicPkgJson = /readFileSync\([^)]*package\.json/
    expect(bundle).not.toMatch(dynamicPkgJson)
  })

  it('sourcemap exists alongside bundle', () => {
    expect(existsSync(resolve(sidecarRoot, 'dist', 'index.js.map'))).toBe(true)
  })
})
