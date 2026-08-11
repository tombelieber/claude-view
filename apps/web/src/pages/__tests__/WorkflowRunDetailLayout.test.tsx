// @vitest-environment happy-dom
import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { WORKFLOW_DETAIL_WIDTH_KEY, WorkflowRunDetailLayout } from '../WorkflowRunDetailPage'

function setViewport(width: number) {
  Object.defineProperty(window, 'innerWidth', {
    configurable: true,
    writable: true,
    value: width,
  })
}

function renderLayout() {
  return render(
    <WorkflowRunDetailLayout
      activity={<main>Activity content</main>}
      detail={<aside>Agent detail content</aside>}
    />,
  )
}

beforeEach(() => {
  localStorage.clear()
  setViewport(1280)
  vi.restoreAllMocks()
})

describe('WorkflowRunDetailLayout', () => {
  it('renders a draggable splitter and widens the agent detail pane', () => {
    renderLayout()

    const splitter = screen.getByRole('separator', { name: 'Resize agent detail panel' })
    expect(splitter).toBeInTheDocument()
    expect(splitter).toHaveAttribute('aria-valuemin', '340')
    expect(splitter).toHaveAttribute('aria-valuemax', '664')
    expect(splitter).toHaveAttribute('aria-valuenow', '460')
    expect(splitter).toHaveAttribute('aria-valuetext', 'Agent detail width 460px')
    expect(screen.getByTestId('workflow-run-detail-layout')).toHaveStyle({
      gridTemplateColumns: 'minmax(0, 1fr) 8px 460px',
    })

    fireEvent.pointerDown(splitter, { clientX: 900 })
    fireEvent.pointerMove(window, { clientX: 780 })
    fireEvent.pointerUp(window)

    expect(screen.getByTestId('workflow-run-detail-layout')).toHaveStyle({
      gridTemplateColumns: 'minmax(0, 1fr) 8px 580px',
    })
    expect(splitter).toHaveAttribute('aria-valuenow', '580')
    expect(splitter).toHaveAttribute('aria-valuetext', 'Agent detail width 580px')
    expect(localStorage.getItem(WORKFLOW_DETAIL_WIDTH_KEY)).toBe('580')
  })

  it('clamps the agent detail pane to a usable minimum width', () => {
    renderLayout()

    const splitter = screen.getByRole('separator', { name: 'Resize agent detail panel' })
    fireEvent.pointerDown(splitter, { clientX: 900 })
    fireEvent.pointerMove(window, { clientX: 1200 })
    fireEvent.pointerUp(window)

    expect(screen.getByTestId('workflow-run-detail-layout')).toHaveStyle({
      gridTemplateColumns: 'minmax(0, 1fr) 8px 340px',
    })
    expect(localStorage.getItem(WORKFLOW_DETAIL_WIDTH_KEY)).toBe('340')
  })

  it('restores a persisted panel width and resets it on double click', () => {
    localStorage.setItem(WORKFLOW_DETAIL_WIDTH_KEY, '560')
    renderLayout()

    const splitter = screen.getByRole('separator', { name: 'Resize agent detail panel' })
    expect(screen.getByTestId('workflow-run-detail-layout')).toHaveStyle({
      gridTemplateColumns: 'minmax(0, 1fr) 8px 560px',
    })

    fireEvent.doubleClick(splitter)

    expect(screen.getByTestId('workflow-run-detail-layout')).toHaveStyle({
      gridTemplateColumns: 'minmax(0, 1fr) 8px 460px',
    })
    expect(localStorage.getItem(WORKFLOW_DETAIL_WIDTH_KEY)).toBe('460')
  })

  it('clamps a persisted wide panel when the viewport shrinks', () => {
    localStorage.setItem(WORKFLOW_DETAIL_WIDTH_KEY, '700')
    const { rerender } = renderLayout()

    expect(screen.getByTestId('workflow-run-detail-layout')).toHaveStyle({
      gridTemplateColumns: 'minmax(0, 1fr) 8px 664px',
    })

    setViewport(1100)
    fireEvent.resize(window)
    rerender(
      <WorkflowRunDetailLayout
        activity={<main>Activity content</main>}
        detail={<aside>Agent detail content</aside>}
      />,
    )

    expect(screen.getByTestId('workflow-run-detail-layout')).toHaveStyle({
      gridTemplateColumns: 'minmax(0, 1fr) 8px 484px',
    })
  })

  it('supports keyboard resizing and reset on the splitter', () => {
    renderLayout()

    const splitter = screen.getByRole('separator', { name: 'Resize agent detail panel' })
    fireEvent.keyDown(splitter, { key: 'ArrowLeft' })

    expect(screen.getByTestId('workflow-run-detail-layout')).toHaveStyle({
      gridTemplateColumns: 'minmax(0, 1fr) 8px 484px',
    })
    expect(localStorage.getItem(WORKFLOW_DETAIL_WIDTH_KEY)).toBe('484')

    fireEvent.keyDown(splitter, { key: 'Enter' })

    expect(screen.getByTestId('workflow-run-detail-layout')).toHaveStyle({
      gridTemplateColumns: 'minmax(0, 1fr) 8px 460px',
    })
    expect(localStorage.getItem(WORKFLOW_DETAIL_WIDTH_KEY)).toBe('460')
  })

  it('stacks panes on narrow screens instead of reserving a cramped right rail', () => {
    setViewport(900)
    renderLayout()

    expect(screen.queryByRole('separator', { name: 'Resize agent detail panel' })).toBeNull()
    expect(screen.getByTestId('workflow-run-detail-layout')).toHaveStyle({
      gridTemplateColumns: 'minmax(0, 1fr)',
    })
  })

  it('keeps the preferred detail width when opening narrow and later resizing wide', () => {
    setViewport(900)
    const { rerender } = renderLayout()

    setViewport(1280)
    fireEvent.resize(window)
    rerender(
      <WorkflowRunDetailLayout
        activity={<main>Activity content</main>}
        detail={<aside>Agent detail content</aside>}
      />,
    )

    expect(screen.getByTestId('workflow-run-detail-layout')).toHaveStyle({
      gridTemplateColumns: 'minmax(0, 1fr) 8px 460px',
    })
  })
})
