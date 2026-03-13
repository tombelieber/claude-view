import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { ChatPalette } from './ChatPalette'
import type { PaletteSection } from './palette-items'

// Stub LucideIcon — ChatPalette renders icons but we don't need real SVGs
// biome-ignore lint/suspicious/noExplicitAny: test stub for LucideIcon
const MockIcon = (() => null) as any

const mockSections: PaletteSection[] = [
  {
    label: 'Context',
    items: [
      { type: 'action', label: 'Clear conversation', icon: MockIcon, onSelect: vi.fn() },
      {
        type: 'action',
        label: 'Attach file',
        icon: MockIcon,
        onSelect: vi.fn(),
        disabled: true,
        hint: 'coming soon',
      },
    ],
  },
  {
    label: 'Model',
    items: [
      {
        type: 'submenu',
        label: 'Switch model',
        icon: MockIcon,
        current: 'Sonnet 4.6',
        items: [
          { id: 'claude-opus-4-6', label: 'Claude Opus 4.6', active: false },
          { id: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6', active: true },
        ],
        onSelect: vi.fn(),
        warning: 'Switching model resumes session.',
      },
    ],
  },
  {
    label: 'Slash Commands',
    items: [
      { type: 'command', name: 'commit', description: 'Create a git commit', onSelect: vi.fn() },
      {
        type: 'command',
        name: 'test',
        description: 'Run the project test suite',
        onSelect: vi.fn(),
      },
    ],
  },
]

describe('ChatPalette', () => {
  it('renders all section headers', () => {
    render(<ChatPalette sections={mockSections} filter="" onClose={vi.fn()} />)
    expect(screen.getByText('Context')).toBeTruthy()
    expect(screen.getByText('Model')).toBeTruthy()
    expect(screen.getByText('Slash Commands')).toBeTruthy()
  })

  it('renders action items with labels', () => {
    render(<ChatPalette sections={mockSections} filter="" onClose={vi.fn()} />)
    expect(screen.getByText('Clear conversation')).toBeTruthy()
  })

  it('renders disabled items with hint', () => {
    render(<ChatPalette sections={mockSections} filter="" onClose={vi.fn()} />)
    expect(screen.getByText('coming soon')).toBeTruthy()
  })

  it('does not fire onSelect for disabled action items', async () => {
    const user = userEvent.setup()
    render(<ChatPalette sections={mockSections} filter="" onClose={vi.fn()} />)
    await user.click(screen.getByText('Attach file'))
    const attachItem = mockSections[0].items[1]
    expect(attachItem.type === 'action' && attachItem.onSelect).not.toHaveBeenCalled()
  })

  it('fires onSelect for enabled action items', async () => {
    const user = userEvent.setup()
    render(<ChatPalette sections={mockSections} filter="" onClose={vi.fn()} />)
    await user.click(screen.getByText('Clear conversation'))
    const clearItem = mockSections[0].items[0]
    expect(clearItem.type === 'action' && clearItem.onSelect).toHaveBeenCalledOnce()
  })

  it('renders submenu items with current value', () => {
    render(<ChatPalette sections={mockSections} filter="" onClose={vi.fn()} />)
    expect(screen.getByText('Sonnet 4.6')).toBeTruthy()
  })

  it('clicking submenu item opens submenu view', async () => {
    const user = userEvent.setup()
    render(<ChatPalette sections={mockSections} filter="" onClose={vi.fn()} />)
    await user.click(screen.getByText('Switch model'))
    expect(screen.getByText('Claude Opus 4.6')).toBeTruthy()
    expect(screen.getByText('Claude Sonnet 4.6')).toBeTruthy()
  })

  it('submenu shows warning text', async () => {
    const user = userEvent.setup()
    render(<ChatPalette sections={mockSections} filter="" onClose={vi.fn()} />)
    await user.click(screen.getByText('Switch model'))
    expect(screen.getByText(/resumes session/)).toBeTruthy()
  })

  it('submenu back button returns to main view', async () => {
    const user = userEvent.setup()
    render(<ChatPalette sections={mockSections} filter="" onClose={vi.fn()} />)
    await user.click(screen.getByText('Switch model'))
    const backBtn = screen.getByRole('button', { name: /back/i })
    await user.click(backBtn)
    expect(screen.getByText('Context')).toBeTruthy()
  })

  it('selecting submenu option fires onSelect with id', async () => {
    const user = userEvent.setup()
    render(<ChatPalette sections={mockSections} filter="" onClose={vi.fn()} />)
    await user.click(screen.getByText('Switch model'))
    await user.click(screen.getByText('Claude Opus 4.6'))
    const switchItem = mockSections[1].items[0]
    expect(switchItem.type === 'submenu' && switchItem.onSelect).toHaveBeenCalledWith(
      'claude-opus-4-6',
    )
  })

  it('renders command items with name and description', () => {
    render(<ChatPalette sections={mockSections} filter="" onClose={vi.fn()} />)
    expect(screen.getByText('/commit')).toBeTruthy()
    expect(screen.getByText('Create a git commit')).toBeTruthy()
  })

  it('filters items by label/name when filter is set', () => {
    render(<ChatPalette sections={mockSections} filter="commit" onClose={vi.fn()} />)
    expect(screen.getByText('/commit')).toBeTruthy()
    expect(screen.queryByText('/test')).toBeNull()
  })

  it('Escape key calls onClose', async () => {
    const onClose = vi.fn()
    const user = userEvent.setup()
    render(<ChatPalette sections={mockSections} filter="" onClose={onClose} />)
    await user.keyboard('{Escape}')
    expect(onClose).toHaveBeenCalledOnce()
  })
})
