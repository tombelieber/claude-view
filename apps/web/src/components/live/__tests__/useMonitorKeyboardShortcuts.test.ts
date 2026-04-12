import { renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// Mock the monitor store
const mockStore = {
  expandedPaneId: null as string | null,
  selectedPaneId: null as string | null,
  expandPane: vi.fn(),
  selectPane: vi.fn(),
  pinnedPaneIds: new Set<string>(),
  hiddenPaneIds: new Set<string>(),
  pinPane: vi.fn(),
  unpinPane: vi.fn(),
  hidePane: vi.fn(),
  displayMode: 'chat' as string,
  setDisplayMode: vi.fn(),
  gridOverride: null,
  setGridOverride: vi.fn(),
}

vi.mock('../../../store/monitor-store', () => ({
  useMonitorStore: Object.assign(
    (selector: (s: typeof mockStore) => unknown) => selector(mockStore),
    { getState: () => mockStore },
  ),
}))

const { useMonitorKeyboardShortcuts } = await import('../useMonitorKeyboardShortcuts')

function fireKey(key: string, mods: Partial<KeyboardEvent> = {}) {
  const event = new KeyboardEvent('keydown', {
    key,
    bubbles: true,
    cancelable: true,
    ...mods,
  })
  document.dispatchEvent(event)
  return event
}

function createMockPanel(id = 'panel-1') {
  return {
    id,
    focus: vi.fn(),
    api: {
      maximize: vi.fn(),
      isMaximized: vi.fn(() => false),
      exitMaximized: vi.fn(),
      close: vi.fn(),
      setActive: vi.fn(),
    },
    group: {
      panels: [] as unknown[],
      api: { setActive: vi.fn() },
    },
  }
}

function createMockDockviewApi(panels: ReturnType<typeof createMockPanel>[] = []) {
  return {
    panels,
    activePanel: panels[0] ?? null,
    getPanel: vi.fn((id: string) => panels.find((p) => p.id === id)),
    addPanel: vi.fn(),
    hasMaximizedGroup: vi.fn(() => false),
    exitMaximizedGroup: vi.fn(),
    groups: panels.map((p) => p.group),
  }
}

function createSession(id: string) {
  return {
    id,
    status: 'working',
    agentState: { group: 'autonomous' },
  }
}

describe('useMonitorKeyboardShortcuts', () => {
  beforeEach(() => {
    mockStore.expandedPaneId = null
    mockStore.selectedPaneId = null
    mockStore.expandPane.mockClear()
    mockStore.selectPane.mockClear()
    mockStore.displayMode = 'chat'
    mockStore.setDisplayMode.mockClear()
    mockStore.gridOverride = null
    mockStore.setGridOverride.mockClear()
    mockStore.pinnedPaneIds = new Set()
    mockStore.hiddenPaneIds = new Set()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  // --- Existing shortcuts (regression tests) ---

  describe('existing shortcuts', () => {
    it('Escape closes expanded pane', () => {
      mockStore.expandedPaneId = 'p1'
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          onZoomToggle: vi.fn(),
        }),
      )

      fireKey('Escape')
      expect(mockStore.expandPane).toHaveBeenCalledWith(null)
    })

    it('number key selects pane by position in custom mode', () => {
      const panel = createMockPanel('s1')
      const api = createMockDockviewApi([panel])
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [createSession('s1')] as never,
          layoutMode: 'custom',
          dockviewApi: api as never,
          onZoomToggle: vi.fn(),
        }),
      )

      fireKey('1')
      expect(panel.focus).toHaveBeenCalled()
    })

    it('Ctrl+Shift+G switches to auto-grid mode', () => {
      const onLayoutModeChange = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          onLayoutModeChange,
          onZoomToggle: vi.fn(),
        }),
      )

      fireKey('G', { ctrlKey: true, shiftKey: true })
      expect(onLayoutModeChange).toHaveBeenCalledWith('auto-grid')
    })
  })

  // --- New shortcuts ---

  describe('Ctrl+Shift+Enter — zoom toggle', () => {
    it('calls onZoomToggle when pressed', () => {
      const onZoomToggle = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          layoutMode: 'custom',
          onZoomToggle,
        }),
      )

      fireKey('Enter', { ctrlKey: true, shiftKey: true })
      expect(onZoomToggle).toHaveBeenCalledTimes(1)
    })

    it('does not call onZoomToggle when disabled', () => {
      const onZoomToggle = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: false,
          sessions: [],
          onZoomToggle,
        }),
      )

      fireKey('Enter', { ctrlKey: true, shiftKey: true })
      expect(onZoomToggle).not.toHaveBeenCalled()
    })
  })

  describe('Ctrl+D — split right', () => {
    it('calls onSplitRight when pressed in custom mode', () => {
      const onSplitRight = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          layoutMode: 'custom',
          onSplitRight,
          onZoomToggle: vi.fn(),
        }),
      )

      fireKey('d', { ctrlKey: true })
      expect(onSplitRight).toHaveBeenCalledTimes(1)
    })

    it('does not fire when input is focused', () => {
      const onSplitRight = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          layoutMode: 'custom',
          onSplitRight,
          onZoomToggle: vi.fn(),
        }),
      )

      // Simulate input focus
      const input = document.createElement('input')
      document.body.appendChild(input)
      input.focus()

      fireKey('d', { ctrlKey: true })
      expect(onSplitRight).not.toHaveBeenCalled()

      document.body.removeChild(input)
    })
  })

  describe('Ctrl+Shift+D — split down', () => {
    it('calls onSplitDown when pressed', () => {
      const onSplitDown = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          layoutMode: 'custom',
          onSplitDown,
          onZoomToggle: vi.fn(),
        }),
      )

      fireKey('D', { ctrlKey: true, shiftKey: true })
      expect(onSplitDown).toHaveBeenCalledTimes(1)
    })
  })

  describe('Ctrl+[ / Ctrl+] — previous/next tab', () => {
    it('Ctrl+[ calls onPrevTab', () => {
      const onPrevTab = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          layoutMode: 'custom',
          onPrevTab,
          onZoomToggle: vi.fn(),
        }),
      )

      fireKey('[', { ctrlKey: true })
      expect(onPrevTab).toHaveBeenCalledTimes(1)
    })

    it('Ctrl+] calls onNextTab', () => {
      const onNextTab = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          layoutMode: 'custom',
          onNextTab,
          onZoomToggle: vi.fn(),
        }),
      )

      fireKey(']', { ctrlKey: true })
      expect(onNextTab).toHaveBeenCalledTimes(1)
    })
  })

  describe('Ctrl+Alt+Arrows — directional navigation', () => {
    it('Ctrl+Alt+ArrowRight calls onNavigate with right', () => {
      const onNavigate = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          layoutMode: 'custom',
          onNavigate,
          onZoomToggle: vi.fn(),
        }),
      )

      fireKey('ArrowRight', { ctrlKey: true, altKey: true })
      expect(onNavigate).toHaveBeenCalledWith('right')
    })

    it('Ctrl+Alt+ArrowLeft calls onNavigate with left', () => {
      const onNavigate = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          layoutMode: 'custom',
          onNavigate,
          onZoomToggle: vi.fn(),
        }),
      )

      fireKey('ArrowLeft', { ctrlKey: true, altKey: true })
      expect(onNavigate).toHaveBeenCalledWith('left')
    })

    it('Ctrl+Alt+ArrowUp calls onNavigate with up', () => {
      const onNavigate = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          layoutMode: 'custom',
          onNavigate,
          onZoomToggle: vi.fn(),
        }),
      )

      fireKey('ArrowUp', { ctrlKey: true, altKey: true })
      expect(onNavigate).toHaveBeenCalledWith('up')
    })

    it('Ctrl+Alt+ArrowDown calls onNavigate with down', () => {
      const onNavigate = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          layoutMode: 'custom',
          onNavigate,
          onZoomToggle: vi.fn(),
        }),
      )

      fireKey('ArrowDown', { ctrlKey: true, altKey: true })
      expect(onNavigate).toHaveBeenCalledWith('down')
    })
  })

  describe('Ctrl+Shift+W — close active pane', () => {
    it('calls onClosePane when pressed', () => {
      const onClosePane = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          layoutMode: 'custom',
          onClosePane,
          onZoomToggle: vi.fn(),
        }),
      )

      fireKey('W', { ctrlKey: true, shiftKey: true })
      expect(onClosePane).toHaveBeenCalledTimes(1)
    })
  })

  describe('Ctrl+Shift+= — equalize splits', () => {
    it('calls onEqualize when pressed', () => {
      const onEqualize = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          layoutMode: 'custom',
          onEqualize,
          onZoomToggle: vi.fn(),
        }),
      )

      fireKey('=', { ctrlKey: true, shiftKey: true })
      expect(onEqualize).toHaveBeenCalledTimes(1)
    })
  })

  describe('shortcut guards', () => {
    it('ignores all shortcuts when not enabled', () => {
      const onZoomToggle = vi.fn()
      const onSplitRight = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: false,
          sessions: [],
          onZoomToggle,
          onSplitRight,
        }),
      )

      fireKey('Enter', { ctrlKey: true, shiftKey: true })
      fireKey('d', { ctrlKey: true })
      expect(onZoomToggle).not.toHaveBeenCalled()
      expect(onSplitRight).not.toHaveBeenCalled()
    })

    it('skips Ctrl+D when input is focused but still handles Escape', () => {
      mockStore.expandedPaneId = 'p1'
      const onSplitRight = vi.fn()
      renderHook(() =>
        useMonitorKeyboardShortcuts({
          enabled: true,
          sessions: [],
          onSplitRight,
          onZoomToggle: vi.fn(),
        }),
      )

      const input = document.createElement('input')
      document.body.appendChild(input)
      input.focus()

      fireKey('d', { ctrlKey: true })
      expect(onSplitRight).not.toHaveBeenCalled()

      fireKey('Escape')
      expect(mockStore.expandPane).toHaveBeenCalledWith(null)

      document.body.removeChild(input)
    })
  })
})
