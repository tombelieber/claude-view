import type { ModelOption } from '@/hooks/use-models'
import type { SessionCapabilities } from '@/hooks/use-session-capabilities'
import { describe, expect, it, vi } from 'vitest'
import { KNOWN_COMMAND_DESCRIPTIONS, buildPaletteSections } from './palette-items'

const mockModelOptions: ModelOption[] = [
  { id: 'claude-opus-4-6', label: 'Claude Opus 4.6' },
  { id: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6' },
  { id: 'claude-haiku-4-5', label: 'Claude Haiku 4.5' },
]

const noopCallbacks = {
  onModelSwitch: vi.fn(),
  onPaletteModeChange: vi.fn(),
  onCommand: vi.fn(),
  onAgent: vi.fn(),
  onClear: vi.fn(),
  onCompact: vi.fn(),
}

describe('KNOWN_COMMAND_DESCRIPTIONS', () => {
  it('has descriptions for all well-known commands', () => {
    const required = [
      'commit',
      'clear',
      'compact',
      'test',
      'review',
      'debug',
      'help',
      'cost',
      'status',
      'deploy',
    ]
    for (const cmd of required) {
      expect(KNOWN_COMMAND_DESCRIPTIONS[cmd]).toBeDefined()
      expect(KNOWN_COMMAND_DESCRIPTIONS[cmd].length).toBeGreaterThan(0)
    }
  })

  it('all descriptions are non-empty strings', () => {
    for (const [, desc] of Object.entries(KNOWN_COMMAND_DESCRIPTIONS)) {
      expect(typeof desc).toBe('string')
      expect(desc.trim().length).toBeGreaterThan(0)
    }
  })
})

describe('buildPaletteSections', () => {
  const baseCapabilities: SessionCapabilities = {
    model: 'claude-sonnet-4-6',
    permissionMode: 'default',
    slashCommands: ['commit', 'test', 'review', 'custom-skill'],
    mcpServers: [{ name: 'github', status: 'connected' }],
    skills: [],
    agents: [],
  }

  it('returns Context, Model, Customize, and Slash Commands sections', () => {
    const sections = buildPaletteSections(baseCapabilities, mockModelOptions, noopCallbacks, {
      sessionActive: true,
      isStreaming: false,
    })
    const labels = sections.map((s) => s.label)
    expect(labels).toEqual(['Context', 'Model', 'Customize', 'MCP Servers', 'Commands'])
  })

  it('hides Slash Commands section when no commands from sidecar', () => {
    const caps = { ...baseCapabilities, slashCommands: [] }
    const sections = buildPaletteSections(caps, mockModelOptions, noopCallbacks, {
      sessionActive: true,
      isStreaming: false,
    })
    expect(sections.find((s) => s.label === 'Commands')).toBeUndefined()
  })

  it('disables resume actions when session is not live', () => {
    const sections = buildPaletteSections(baseCapabilities, mockModelOptions, noopCallbacks, {
      sessionActive: false,
      isStreaming: false,
    })
    const modelSection = sections.find((s) => s.label === 'Model')
    expect(modelSection).toBeDefined()
    const switchItem = modelSection?.items.find(
      (i) => i.type === 'submenu' && i.label.includes('Switch'),
    )
    expect(switchItem).toBeDefined()
    expect(switchItem?.type === 'submenu' && switchItem.items.length).toBeGreaterThan(0)
  })

  it('disables resume actions when streaming', () => {
    const sections = buildPaletteSections(baseCapabilities, mockModelOptions, noopCallbacks, {
      sessionActive: true,
      isStreaming: true,
    })
    const customizeSection = sections.find((s) => s.label === 'Customize')
    expect(customizeSection).toBeDefined()
    const permItem = customizeSection?.items.find(
      (i) => i.type === 'submenu' && i.label.includes('Permissions'),
    )
    expect(permItem).toBeDefined()
  })

  it('shows current model label in Switch model submenu', () => {
    const sections = buildPaletteSections(baseCapabilities, mockModelOptions, noopCallbacks, {
      sessionActive: true,
      isStreaming: false,
    })
    const modelSection = sections.find((s) => s.label === 'Model')
    expect(modelSection).toBeDefined()
    const switchItem = modelSection?.items.find((i) => i.type === 'submenu')
    expect(switchItem?.type === 'submenu' && switchItem.current).toContain('Sonnet')
  })

  it('shows current permission mode in Permissions submenu', () => {
    const sections = buildPaletteSections(baseCapabilities, mockModelOptions, noopCallbacks, {
      sessionActive: true,
      isStreaming: false,
    })
    const customizeSection = sections.find((s) => s.label === 'Customize')
    expect(customizeSection).toBeDefined()
    const permItem = customizeSection?.items.find(
      (i) => i.type === 'submenu' && i.label.includes('Permissions'),
    )
    expect(permItem?.type === 'submenu' && permItem.current).toBe('default')
  })

  it('merges known descriptions with dynamic slash commands', () => {
    const sections = buildPaletteSections(baseCapabilities, mockModelOptions, noopCallbacks, {
      sessionActive: true,
      isStreaming: false,
    })
    const cmdSection = sections.find((s) => s.label === 'Commands')
    expect(cmdSection).toBeDefined()
    const commitItem = cmdSection?.items.find((i) => i.type === 'command' && i.name === 'commit')
    expect(commitItem?.type === 'command' && commitItem.description.length).toBeGreaterThan(0)
    const customItem = cmdSection?.items.find(
      (i) => i.type === 'command' && i.name === 'custom-skill',
    )
    expect(customItem?.type === 'command' && customItem.description).toBe('')
  })

  it('MCP servers section shows individual servers with status badge', () => {
    const sections = buildPaletteSections(baseCapabilities, mockModelOptions, noopCallbacks, {
      sessionActive: true,
      isStreaming: false,
    })
    const mcpSection = sections.find((s) => s.label === 'MCP Servers')
    expect(mcpSection).toBeDefined()
    expect(mcpSection?.items).toHaveLength(1)
    const githubItem = mcpSection?.items[0]
    expect(githubItem?.type === 'link' && githubItem.label).toBe('github')
    expect(githubItem?.type === 'link' && githubItem.badge).toBe('connected')
  })

  it('attach file action is disabled with hint', () => {
    const sections = buildPaletteSections(baseCapabilities, mockModelOptions, noopCallbacks, {
      sessionActive: true,
      isStreaming: false,
    })
    const contextSection = sections.find((s) => s.label === 'Context')
    expect(contextSection).toBeDefined()
    const attachItem = contextSection?.items.find(
      (i) => i.type === 'action' && i.label.includes('Attach'),
    )
    expect(attachItem?.type === 'action' && attachItem.disabled).toBe(true)
    expect(attachItem?.type === 'action' && attachItem.hint).toBe('coming soon')
  })

  it('builds model submenu items from modelOptions parameter', () => {
    const sections = buildPaletteSections(baseCapabilities, mockModelOptions, noopCallbacks, {
      sessionActive: true,
      isStreaming: false,
    })
    const modelSection = sections.find((s) => s.label === 'Model')
    expect(modelSection).toBeDefined()
    const switchItem = modelSection?.items.find(
      (i) => i.type === 'submenu' && i.label.includes('Switch'),
    )
    expect(switchItem?.type === 'submenu' && switchItem.items).toHaveLength(3)
    expect(switchItem?.type === 'submenu' && switchItem.items[0].label).toBe('Claude Opus 4.6')
  })
})
