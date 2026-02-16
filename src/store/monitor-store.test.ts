import { describe, it, expect, beforeEach } from 'vitest'
import { useMonitorStore } from './monitor-store'

describe('useMonitorStore', () => {
  // Reset the store before each test to avoid cross-test pollution
  beforeEach(() => {
    useMonitorStore.setState({
      gridOverride: null,
      compactHeaders: false,
      selectedPaneId: null,
      expandedPaneId: null,
      pinnedPaneIds: new Set<string>(),
      hiddenPaneIds: new Set<string>(),
      paneMode: {},
    })
  })

  describe('initial state', () => {
    it('has null gridOverride', () => {
      expect(useMonitorStore.getState().gridOverride).toBeNull()
    })

    it('has compactHeaders false', () => {
      expect(useMonitorStore.getState().compactHeaders).toBe(false)
    })

    it('has null selectedPaneId', () => {
      expect(useMonitorStore.getState().selectedPaneId).toBeNull()
    })

    it('has null expandedPaneId', () => {
      expect(useMonitorStore.getState().expandedPaneId).toBeNull()
    })

    it('has empty pinnedPaneIds set', () => {
      expect(useMonitorStore.getState().pinnedPaneIds.size).toBe(0)
    })

    it('has empty hiddenPaneIds set', () => {
      expect(useMonitorStore.getState().hiddenPaneIds.size).toBe(0)
    })

    it('has empty paneMode record', () => {
      expect(Object.keys(useMonitorStore.getState().paneMode)).toHaveLength(0)
    })
  })

  describe('setGridOverride', () => {
    it('updates gridOverride with cols and rows', () => {
      useMonitorStore.getState().setGridOverride({ cols: 3, rows: 2 })

      expect(useMonitorStore.getState().gridOverride).toEqual({ cols: 3, rows: 2 })
    })

    it('sets gridOverride to null for auto mode', () => {
      useMonitorStore.getState().setGridOverride({ cols: 3, rows: 2 })
      useMonitorStore.getState().setGridOverride(null)

      expect(useMonitorStore.getState().gridOverride).toBeNull()
    })
  })

  describe('setCompactHeaders', () => {
    it('sets compactHeaders to true', () => {
      useMonitorStore.getState().setCompactHeaders(true)

      expect(useMonitorStore.getState().compactHeaders).toBe(true)
    })

    it('sets compactHeaders to false', () => {
      useMonitorStore.getState().setCompactHeaders(true)
      useMonitorStore.getState().setCompactHeaders(false)

      expect(useMonitorStore.getState().compactHeaders).toBe(false)
    })
  })

  describe('selectPane', () => {
    it('sets selectedPaneId', () => {
      useMonitorStore.getState().selectPane('pane-abc')

      expect(useMonitorStore.getState().selectedPaneId).toBe('pane-abc')
    })

    it('clears selectedPaneId with null', () => {
      useMonitorStore.getState().selectPane('pane-abc')
      useMonitorStore.getState().selectPane(null)

      expect(useMonitorStore.getState().selectedPaneId).toBeNull()
    })
  })

  describe('expandPane', () => {
    it('sets expandedPaneId', () => {
      useMonitorStore.getState().expandPane('pane-xyz')

      expect(useMonitorStore.getState().expandedPaneId).toBe('pane-xyz')
    })

    it('clears expandedPaneId with null', () => {
      useMonitorStore.getState().expandPane('pane-xyz')
      useMonitorStore.getState().expandPane(null)

      expect(useMonitorStore.getState().expandedPaneId).toBeNull()
    })
  })

  describe('pinPane', () => {
    it('adds to pinnedPaneIds set', () => {
      useMonitorStore.getState().pinPane('pane-1')

      expect(useMonitorStore.getState().pinnedPaneIds.has('pane-1')).toBe(true)
    })

    it('can pin multiple panes', () => {
      useMonitorStore.getState().pinPane('pane-1')
      useMonitorStore.getState().pinPane('pane-2')

      const pinned = useMonitorStore.getState().pinnedPaneIds
      expect(pinned.has('pane-1')).toBe(true)
      expect(pinned.has('pane-2')).toBe(true)
      expect(pinned.size).toBe(2)
    })

    it('is idempotent - pinning same pane twice does not duplicate', () => {
      useMonitorStore.getState().pinPane('pane-1')
      useMonitorStore.getState().pinPane('pane-1')

      expect(useMonitorStore.getState().pinnedPaneIds.size).toBe(1)
    })
  })

  describe('unpinPane', () => {
    it('removes from pinnedPaneIds set', () => {
      useMonitorStore.getState().pinPane('pane-1')
      useMonitorStore.getState().pinPane('pane-2')
      useMonitorStore.getState().unpinPane('pane-1')

      const pinned = useMonitorStore.getState().pinnedPaneIds
      expect(pinned.has('pane-1')).toBe(false)
      expect(pinned.has('pane-2')).toBe(true)
    })

    it('is safe to unpin a pane that is not pinned', () => {
      useMonitorStore.getState().unpinPane('nonexistent')

      expect(useMonitorStore.getState().pinnedPaneIds.size).toBe(0)
    })
  })

  describe('hidePane', () => {
    it('adds to hiddenPaneIds set', () => {
      useMonitorStore.getState().hidePane('pane-1')

      expect(useMonitorStore.getState().hiddenPaneIds.has('pane-1')).toBe(true)
    })

    it('can hide multiple panes', () => {
      useMonitorStore.getState().hidePane('pane-1')
      useMonitorStore.getState().hidePane('pane-2')

      const hidden = useMonitorStore.getState().hiddenPaneIds
      expect(hidden.has('pane-1')).toBe(true)
      expect(hidden.has('pane-2')).toBe(true)
      expect(hidden.size).toBe(2)
    })
  })

  describe('showPane', () => {
    it('removes from hiddenPaneIds set', () => {
      useMonitorStore.getState().hidePane('pane-1')
      useMonitorStore.getState().hidePane('pane-2')
      useMonitorStore.getState().showPane('pane-1')

      const hidden = useMonitorStore.getState().hiddenPaneIds
      expect(hidden.has('pane-1')).toBe(false)
      expect(hidden.has('pane-2')).toBe(true)
    })

    it('is safe to show a pane that is not hidden', () => {
      useMonitorStore.getState().showPane('nonexistent')

      expect(useMonitorStore.getState().hiddenPaneIds.size).toBe(0)
    })
  })

  describe('setPaneMode', () => {
    it('sets mode for a pane', () => {
      useMonitorStore.getState().setPaneMode('pane-1', 'rich')

      expect(useMonitorStore.getState().paneMode['pane-1']).toBe('rich')
    })

    it('can set different modes for different panes', () => {
      useMonitorStore.getState().setPaneMode('pane-1', 'rich')
      useMonitorStore.getState().setPaneMode('pane-2', 'raw')

      const modes = useMonitorStore.getState().paneMode
      expect(modes['pane-1']).toBe('rich')
      expect(modes['pane-2']).toBe('raw')
    })

    it('overwrites existing mode', () => {
      useMonitorStore.getState().setPaneMode('pane-1', 'raw')
      useMonitorStore.getState().setPaneMode('pane-1', 'rich')

      expect(useMonitorStore.getState().paneMode['pane-1']).toBe('rich')
    })
  })
})
