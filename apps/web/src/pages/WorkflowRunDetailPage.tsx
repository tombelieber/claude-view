import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { RunActivity } from '../components/workflows/RunActivity'
import { RunAgentDetailPanel } from '../components/workflows/RunAgentDetailPanel'
import { RunDetailHeader } from '../components/workflows/RunDetailHeader'
import { useWorkflowAgent, useWorkflowRun } from '../hooks/use-workflows'

export const WORKFLOW_DETAIL_WIDTH_KEY = 'claude-view:workflow-run-detail-width'

const DEFAULT_DETAIL_WIDTH = 460
const MIN_DETAIL_WIDTH = 340
const MOBILE_BREAKPOINT = 1024
const HORIZONTAL_PADDING = 64
const MIN_ACTIVITY_WIDTH = 520
const RESIZER_WIDTH = 8
const RESIZER_GAPS = 24
const KEYBOARD_RESIZE_STEP = 24
const KEYBOARD_RESIZE_LARGE_STEP = 80

function CenteredMessage({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 bg-gray-50 text-sm text-gray-500 dark:bg-black">
      {children}
    </div>
  )
}

function maxDetailWidth(viewportWidth = window.innerWidth): number {
  return Math.max(
    MIN_DETAIL_WIDTH,
    viewportWidth - HORIZONTAL_PADDING - MIN_ACTIVITY_WIDTH - RESIZER_WIDTH - RESIZER_GAPS,
  )
}

function clampDetailWidth(width: number, viewportWidth = window.innerWidth): number {
  const maxWidth = maxDetailWidth(viewportWidth)
  return Math.round(Math.min(maxWidth, Math.max(MIN_DETAIL_WIDTH, width)))
}

function getPreferredDetailWidth(): number {
  try {
    const stored = Number.parseInt(localStorage.getItem(WORKFLOW_DETAIL_WIDTH_KEY) ?? '', 10)
    if (Number.isFinite(stored)) {
      return Math.max(MIN_DETAIL_WIDTH, stored)
    }
  } catch (err) {
    console.debug('[WorkflowRunDetailPage] localStorage access failed:', err)
  }
  return DEFAULT_DETAIL_WIDTH
}

function useWorkflowViewportWidth() {
  const [viewportWidth, setViewportWidth] = useState(() => window.innerWidth)

  useEffect(() => {
    const onResize = () => setViewportWidth(window.innerWidth)
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  }, [])

  return viewportWidth
}

export function WorkflowRunDetailLayout({
  activity,
  detail,
}: {
  activity: React.ReactNode
  detail: React.ReactNode
}) {
  const viewportWidth = useWorkflowViewportWidth()
  const isWide = viewportWidth >= MOBILE_BREAKPOINT
  const [preferredDetailWidth, setPreferredDetailWidth] = useState(getPreferredDetailWidth)
  const detailWidth = clampDetailWidth(preferredDetailWidth, viewportWidth)
  const detailMaxWidth = maxDetailWidth(viewportWidth)
  const detailWidthRef = useRef(detailWidth)
  const viewportWidthRef = useRef(viewportWidth)

  useEffect(() => {
    detailWidthRef.current = detailWidth
    viewportWidthRef.current = viewportWidth
  }, [detailWidth, viewportWidth])

  const persistDetailWidth = useCallback((nextWidth: number) => {
    try {
      localStorage.setItem(WORKFLOW_DETAIL_WIDTH_KEY, String(nextWidth))
    } catch (err) {
      console.debug('[WorkflowRunDetailPage] localStorage access failed:', err)
    }
  }, [])

  const setClampedDetailWidth = useCallback((nextWidth: number, viewportWidth?: number) => {
    const clamped = clampDetailWidth(nextWidth, viewportWidth ?? viewportWidthRef.current)
    detailWidthRef.current = clamped
    setPreferredDetailWidth(clamped)
    return clamped
  }, [])

  const handleResizeStart = useCallback(
    (event: React.PointerEvent<HTMLHRElement>) => {
      event.preventDefault()
      const startX = event.clientX
      const startWidth = detailWidthRef.current

      const onMove = (moveEvent: PointerEvent) => {
        const delta = startX - moveEvent.clientX
        setClampedDetailWidth(startWidth + delta)
      }

      const onUp = () => {
        persistDetailWidth(detailWidthRef.current)
        window.removeEventListener('pointermove', onMove)
        window.removeEventListener('pointerup', onUp)
      }

      window.addEventListener('pointermove', onMove)
      window.addEventListener('pointerup', onUp)
    },
    [persistDetailWidth, setClampedDetailWidth],
  )

  const handleReset = useCallback(() => {
    const nextWidth = setClampedDetailWidth(DEFAULT_DETAIL_WIDTH)
    persistDetailWidth(nextWidth)
  }, [persistDetailWidth, setClampedDetailWidth])

  const handleResizeKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLHRElement>) => {
      const step = event.shiftKey ? KEYBOARD_RESIZE_LARGE_STEP : KEYBOARD_RESIZE_STEP
      if (event.key === 'ArrowLeft') {
        event.preventDefault()
        persistDetailWidth(setClampedDetailWidth(detailWidthRef.current + step))
      } else if (event.key === 'ArrowRight') {
        event.preventDefault()
        persistDetailWidth(setClampedDetailWidth(detailWidthRef.current - step))
      } else if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault()
        handleReset()
      }
    },
    [handleReset, persistDetailWidth, setClampedDetailWidth],
  )

  return (
    <div
      data-testid="workflow-run-detail-layout"
      className="grid gap-y-5 px-8 py-6"
      style={{
        columnGap: isWide ? 12 : undefined,
        gridTemplateColumns: isWide
          ? `minmax(0, 1fr) ${RESIZER_WIDTH}px ${detailWidth}px`
          : 'minmax(0, 1fr)',
      }}
    >
      <div className="min-w-0 self-stretch">{activity}</div>
      {isWide && (
        <hr
          aria-label="Resize agent detail panel"
          aria-orientation="vertical"
          aria-valuemax={detailMaxWidth}
          aria-valuemin={MIN_DETAIL_WIDTH}
          aria-valuenow={detailWidth}
          aria-valuetext={`Agent detail width ${detailWidth}px`}
          className="m-0 h-full w-1 cursor-col-resize justify-self-center rounded-full border-0 bg-transparent transition-colors hover:bg-blue-300 focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:outline-none dark:hover:bg-blue-700"
          onDoubleClick={handleReset}
          onKeyDown={handleResizeKeyDown}
          onPointerDown={handleResizeStart}
          tabIndex={0}
        />
      )}
      <div className="min-w-0 self-stretch">{detail}</div>
    </div>
  )
}

export function WorkflowRunDetailPage() {
  const { sessionId = '', runId = '' } = useParams<{ sessionId: string; runId: string }>()
  const { data: detail, isLoading, isError } = useWorkflowRun(sessionId, runId)
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null)
  const activeAgentId = selectedAgentId ?? detail?.agents[0]?.agentId ?? null
  const { data: agentDetail } = useWorkflowAgent(sessionId, runId, activeAgentId)

  if (isLoading) {
    return <CenteredMessage>Loading workflow run...</CenteredMessage>
  }

  if (isError || !detail) {
    return (
      <CenteredMessage>
        <div>{isError ? 'Could not load this workflow run.' : 'Workflow run not found.'}</div>
        <Link to="/workflows" className="text-blue-600 hover:underline dark:text-blue-400">
          Back to workflows
        </Link>
      </CenteredMessage>
    )
  }

  return (
    <div className="flex h-full flex-col overflow-auto bg-gray-50 dark:bg-black">
      <RunDetailHeader run={detail.summary} />
      <WorkflowRunDetailLayout
        activity={
          <RunActivity
            detail={detail}
            activeAgentId={activeAgentId}
            onSelectAgent={setSelectedAgentId}
          />
        }
        detail={<RunAgentDetailPanel detail={detail} agentDetail={agentDetail} />}
      />
    </div>
  )
}
