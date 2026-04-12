import { X } from 'lucide-react'

/** Session status -> Tailwind dot color (working=green, paused=amber, done=gray). */
function statusDotColor(status: string | null): string {
  switch (status) {
    case 'spawning':
      return 'bg-blue-500'
    case 'working':
      return 'bg-green-500'
    case 'paused':
      return 'bg-amber-500'
    default:
      return 'bg-gray-300 dark:bg-gray-600'
  }
}

export interface TabContentProps {
  title: string
  status: string | null
  agentStateGroup: string | null
  isTmux: boolean
  onClose: (e: React.MouseEvent) => void
  onMiddleClick: (e: React.MouseEvent) => void
  /** Override dot color (e.g. CLI terminal uses static emerald). */
  dotColorOverride?: string
}

export function TabContent({
  title,
  status,
  agentStateGroup,
  isTmux,
  onClose,
  onMiddleClick,
  dotColorOverride,
}: TabContentProps) {
  const dotColor = dotColorOverride ?? statusDotColor(status)
  const isAutonomous = agentStateGroup === 'autonomous'
  const showPulse =
    !dotColorOverride && (status === 'spawning' || (isAutonomous && status === 'working'))

  return (
    <div
      className="group flex items-center gap-1.5 px-3 h-full text-xs cursor-pointer"
      onMouseDown={onMiddleClick}
    >
      <div className="flex-shrink-0 relative inline-flex">
        {showPulse && (
          <span
            className={`absolute inline-flex w-2 h-2 rounded-full opacity-60 motion-safe:animate-live-ring ${dotColor}`}
          />
        )}
        <span
          className={`relative inline-flex w-2 h-2 rounded-full ${dotColor} ${showPulse ? 'motion-safe:animate-live-breathe' : ''}`}
        />
      </div>
      <span className="truncate max-w-[120px]">{title}</span>
      <button
        type="button"
        onClick={onClose}
        title={isTmux ? 'Kill CLI session' : undefined}
        className="ml-auto w-4 h-4 flex items-center justify-center rounded-sm text-gray-400 dark:text-gray-500 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-700"
      >
        <X className="w-3 h-3" />
      </button>
    </div>
  )
}
