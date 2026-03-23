import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

const BANNED_MODULES = [
  'RichPane',
  'RichTerminalPane',
  'ActionLogTab',
  'ActionFilterChips',
  'SubAgentDrillDown',
  'ViewModeControls',
  'CompactChatTab',
  'use-live-session-messages',
  'use-paired-messages',
  'compute-category-counts',
  'message-to-rich',
  'hook-events-to-messages',
  'pending-queue',
  'use-subagent-stream',
]

/** Recursively collect .ts/.tsx files, skipping tests/stories */
function collectProdFiles(dir: string): string[] {
  const files: string[] = []
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name)
    if (entry.isDirectory() && !entry.name.includes('node_modules')) {
      files.push(...collectProdFiles(full))
    } else if (
      entry.isFile() &&
      /\.(ts|tsx)$/.test(entry.name) &&
      !entry.name.includes('.test.') &&
      !entry.name.includes('.spec.') &&
      !entry.name.includes('.stories.') &&
      !entry.name.includes('regression')
    ) {
      files.push(full)
    }
  }
  return files
}

describe('dead code removal regression', () => {
  const prodFiles = collectProdFiles(join(__dirname, '../../..'))

  for (const banned of BANNED_MODULES) {
    it(`no production import of "${banned}"`, () => {
      const offenders = prodFiles.filter((f) => {
        const content = readFileSync(f, 'utf-8')
        return content.includes(banned)
      })
      expect(
        offenders,
        `Found "${banned}" in: ${offenders.map((f) => f.split('apps/web/src/')[1]).join(', ')}`,
      ).toEqual([])
    })
  }
})
