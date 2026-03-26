#!/usr/bin/env bun
/**
 * Generates SKILL.md files from .tmpl templates, injecting a shared preamble
 * and auto-generated tool list.
 *
 * Usage:  bun run scripts/gen-skill-docs.ts
 *
 * For each skills/X/SKILL.md.tmpl:
 *   1. Replace {{PREAMBLE}} with contents of scripts/preamble.md
 *   2. Replace {{AVAILABLE_TOOLS}} with a formatted tool table grouped by tag
 *   3. Write to skills/X/SKILL.md
 *
 * Skills WITHOUT a .tmpl file keep their SKILL.md untouched.
 */

import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { SSE_OPERATION_IDS, HAND_WRITTEN_TAGS, toSnakeCase, makeToolName } from './shared.js'

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

const ROOT = dirname(dirname(new URL(import.meta.url).pathname))
const SPEC_PATH = join(ROOT, 'scripts', 'openapi.json')
const PREAMBLE_PATH = join(ROOT, 'scripts', 'preamble.md')
const SKILLS_DIR = join(ROOT, 'skills')

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface OpenAPISpec {
  paths: Record<string, Record<string, OpenAPIOperation>>
}

interface OpenAPIOperation {
  tags?: string[]
  summary?: string
  description?: string
  operationId?: string
}

interface ToolEntry {
  name: string
  description: string
}

interface ToolGroup {
  label: string
  sortOrder: number
  tools: ToolEntry[]
}

// ---------------------------------------------------------------------------
// Hand-written tool definitions (mirror of src/tools/*.ts)
// ---------------------------------------------------------------------------

const HAND_WRITTEN_GROUPS: ToolGroup[] = [
  {
    label: 'Session Tools',
    sortOrder: 0,
    tools: [
      { name: 'list_sessions', description: 'List sessions with filters' },
      { name: 'get_session', description: 'Get session details' },
      { name: 'search_sessions', description: 'Full-text search across sessions' },
    ],
  },
  {
    label: 'Stats Tools',
    sortOrder: 1,
    tools: [
      { name: 'get_stats', description: 'Dashboard statistics' },
      { name: 'get_fluency_score', description: 'AI Fluency Score (0-100)' },
      { name: 'get_token_stats', description: 'Token usage statistics' },
    ],
  },
  {
    label: 'Live Tools',
    sortOrder: 2,
    tools: [
      { name: 'list_live_sessions', description: 'Currently running sessions' },
      { name: 'get_live_summary', description: 'Aggregate live session summary' },
    ],
  },
]

// HAND_WRITTEN_TAGS, toSnakeCase, makeToolName imported from ./shared.js

// ---------------------------------------------------------------------------
// OpenAPI tool extraction
// ---------------------------------------------------------------------------

function cleanDescription(summary: string, description: string, path: string, method: string): string {
  let desc = summary || description || `${method.toUpperCase()} ${path}`
  // Collapse newlines and excess whitespace into single spaces
  desc = desc.replace(/\n/g, ' ').replace(/\s+/g, ' ').trim()
  desc = desc.replace(/^(GET|POST|PUT|DELETE|PATCH)\s+\/api\/\S+\s*[-—]\s*/i, '').trim()
  if (!desc || /^(GET|POST|PUT|DELETE|PATCH)\s+\/api\//i.test(desc)) {
    desc = path.split('/').filter(Boolean).slice(1).join(' ') || 'Unknown'
  }
  // Take first sentence only, truncate at 80 chars
  const firstSentence = desc.split(/\.\s/)[0]
  // Strip pipe characters which break markdown table formatting
  const cleaned = firstSentence.replace(/\|/g, '/')
  return cleaned.length > 80 ? `${cleaned.slice(0, 77)}...` : cleaned
}

function capitalizeTag(tag: string): string {
  return tag
    .split(/[_-]/)
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ')
}

function extractGeneratedToolGroups(spec: OpenAPISpec): ToolGroup[] {
  const byTag = new Map<string, ToolEntry[]>()

  for (const [path, methods] of Object.entries(spec.paths)) {
    for (const [method, op] of Object.entries(methods)) {
      if (!['get', 'post', 'put', 'delete', 'patch'].includes(method)) continue

      const operation = op as OpenAPIOperation
      const tag = (operation.tags?.[0] ?? 'misc').toLowerCase()
      const operationId = operation.operationId ?? ''

      if (SSE_OPERATION_IDS.has(operationId)) continue
      if (HAND_WRITTEN_TAGS.has(tag)) continue

      const toolName = makeToolName(tag, operationId)
      const desc = cleanDescription(
        operation.summary ?? '',
        operation.description ?? '',
        path,
        method,
      )

      if (!byTag.has(tag)) byTag.set(tag, [])
      byTag.get(tag)!.push({ name: toolName, description: desc })
    }
  }

  const groups: ToolGroup[] = []
  const sortedTags = [...byTag.keys()].sort()

  for (let i = 0; i < sortedTags.length; i++) {
    const tag = sortedTags[i]
    groups.push({
      label: `${capitalizeTag(tag)} Tools`,
      sortOrder: 100 + i,
      tools: byTag.get(tag)!,
    })
  }

  return groups
}

// ---------------------------------------------------------------------------
// Build the tool table markdown
// ---------------------------------------------------------------------------

function buildToolTable(groups: ToolGroup[]): string {
  const sorted = [...groups].sort((a, b) => a.sortOrder - b.sortOrder)

  const lines: string[] = [
    '## Available Tools',
    '',
    '| Tool | Description |',
    '|------|-------------|',
  ]

  for (const group of sorted) {
    lines.push(`| **${group.label}** | |`)
    for (const tool of group.tools) {
      lines.push(`| \`${tool.name}\` | ${tool.description} |`)
    }
  }

  return lines.join('\n')
}

// ---------------------------------------------------------------------------
// Template processing
// ---------------------------------------------------------------------------

function processTemplate(templateContent: string, preamble: string, toolTable: string): string {
  let result = templateContent
  result = result.replace('{{PREAMBLE}}', preamble.trim())
  result = result.replace('{{AVAILABLE_TOOLS}}', toolTable)
  return result
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  // Read preamble
  if (!existsSync(PREAMBLE_PATH)) {
    console.error(`ERROR: preamble.md not found at ${PREAMBLE_PATH}`)
    process.exit(1)
  }
  const preamble = readFileSync(PREAMBLE_PATH, 'utf-8')

  // Read OpenAPI spec and build tool groups
  if (!existsSync(SPEC_PATH)) {
    console.error(`ERROR: openapi.json not found at ${SPEC_PATH}`)
    process.exit(1)
  }
  const spec = JSON.parse(readFileSync(SPEC_PATH, 'utf-8')) as OpenAPISpec
  const generatedGroups = extractGeneratedToolGroups(spec)
  const allGroups = [...HAND_WRITTEN_GROUPS, ...generatedGroups]
  const toolTable = buildToolTable(allGroups)

  // Process each skill directory
  const skillDirs = readdirSync(SKILLS_DIR, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => d.name)

  let generated = 0
  let skipped = 0

  for (const skill of skillDirs) {
    const tmplPath = join(SKILLS_DIR, skill, 'SKILL.md.tmpl')
    const outPath = join(SKILLS_DIR, skill, 'SKILL.md')

    if (!existsSync(tmplPath)) {
      skipped++
      continue
    }

    const template = readFileSync(tmplPath, 'utf-8')
    const output = processTemplate(template, preamble, toolTable)
    writeFileSync(outPath, output)
    generated++
    console.log(`  wrote ${skill}/SKILL.md`)
  }

  console.log(`\nGenerated ${generated} SKILL.md files, skipped ${skipped} (no .tmpl)`)
}

main()
