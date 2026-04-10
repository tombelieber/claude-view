import { describe, expect, test } from 'bun:test'
import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

const PLUGIN_ROOT = join(import.meta.dir, '..', '..')

describe('skill generation', () => {
  const skillDirs = ['daily-cost', 'session-recap', 'standup', 'system-monitor', 'live', 'usage']

  for (const skill of skillDirs) {
    test(`${skill}/SKILL.md exists and has preamble`, () => {
      const path = join(PLUGIN_ROOT, 'skills', skill, 'SKILL.md')
      expect(existsSync(path)).toBe(true)
      const content = readFileSync(path, 'utf-8')
      expect(content).toContain('claude-view MCP server')
      expect(content).toContain('Available Tools')
      expect(content).not.toContain('{{PREAMBLE}}')
      expect(content).not.toContain('{{AVAILABLE_TOOLS}}')
    })

    test(`${skill}/SKILL.md.tmpl has template placeholders`, () => {
      const path = join(PLUGIN_ROOT, 'skills', skill, 'SKILL.md.tmpl')
      expect(existsSync(path)).toBe(true)
      const content = readFileSync(path, 'utf-8')
      expect(content).toContain('{{PREAMBLE}}')
      expect(content).toContain('{{AVAILABLE_TOOLS}}')
    })
  }

  test('generated SKILL.md contains tool table with hand-written tools', () => {
    const content = readFileSync(join(PLUGIN_ROOT, 'skills', 'daily-cost', 'SKILL.md'), 'utf-8')
    expect(content).toContain('list_sessions')
    expect(content).toContain('get_stats')
    expect(content).toContain('list_live_sessions')
    expect(content).toContain('get_live_summary')
  })

  test('generated SKILL.md contains generated tools', () => {
    const content = readFileSync(join(PLUGIN_ROOT, 'skills', 'daily-cost', 'SKILL.md'), 'utf-8')
    expect(content).toContain('projects_list_projects')
    expect(content).toContain('contributions_get_contributions')
  })

  test('generated SKILL.md preserves YAML frontmatter', () => {
    const content = readFileSync(join(PLUGIN_ROOT, 'skills', 'daily-cost', 'SKILL.md'), 'utf-8')
    expect(content.startsWith('---\n')).toBe(true)
    expect(content).toContain('name: daily-cost')
    expect(content).toContain('description:')
  })

  test('tool table has correct markdown format', () => {
    const content = readFileSync(join(PLUGIN_ROOT, 'skills', 'standup', 'SKILL.md'), 'utf-8')
    expect(content).toContain('| Tool | Description |')
    expect(content).toContain('|------|-------------|')
    expect(content).toContain('| **Session Tools** | |')
    expect(content).toContain('| **Stats Tools** | |')
    expect(content).toContain('| **Live Tools** | |')
  })

  describe('new priority skills', () => {
    test('system-monitor SKILL.md has expected sections', () => {
      const content = readFileSync(
        join(PLUGIN_ROOT, 'skills', 'system-monitor', 'SKILL.md'),
        'utf-8',
      )
      expect(content).toContain('name: system-monitor')
      expect(content).toContain('# System Monitor')
      expect(content).toContain('monitor_snapshot')
      expect(content).toContain('system_get_system_status')
      expect(content).toContain('## Available Tools')
    })

    test('live SKILL.md has expected sections', () => {
      const content = readFileSync(join(PLUGIN_ROOT, 'skills', 'live', 'SKILL.md'), 'utf-8')
      expect(content).toContain('name: live')
      expect(content).toContain('# Live Session Status')
      expect(content).toContain('list_live_sessions')
      expect(content).toContain('get_live_summary')
      expect(content).toContain('live_get_live_session')
      expect(content).toContain('## Available Tools')
    })

    test('usage SKILL.md has expected sections', () => {
      const content = readFileSync(join(PLUGIN_ROOT, 'skills', 'usage', 'SKILL.md'), 'utf-8')
      expect(content).toContain('name: usage')
      expect(content).toContain('# Usage Report')
      expect(content).toContain('get_stats')
      expect(content).toContain('get_token_stats')
      expect(content).toContain('stats_get_trends')
      expect(content).toContain('## Available Tools')
    })

    const phantomEndpoints = ['handle_hook', 'bind_control', 'get_session_parsed']
    const newSkills = ['system-monitor', 'live', 'usage']

    for (const skill of newSkills) {
      test(`${skill} has no phantom internal endpoints`, () => {
        const content = readFileSync(join(PLUGIN_ROOT, 'skills', skill, 'SKILL.md'), 'utf-8')
        for (const endpoint of phantomEndpoints) {
          expect(content).not.toContain(endpoint)
        }
      })
    }
  })
})
