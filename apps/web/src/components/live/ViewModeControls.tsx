import { ViewModeToggle } from '@claude-view/shared'
import { useMonitorStore } from '../../store/monitor-store'

interface ViewModeControlsProps {
  className?: string
}

/**
 * Web app wrapper: connects shared ViewModeToggle to zustand store.
 */
export function ViewModeControls({ className }: ViewModeControlsProps) {
  const verboseMode = useMonitorStore((s) => s.verboseMode)
  const toggleVerbose = useMonitorStore((s) => s.toggleVerbose)
  const richRenderMode = useMonitorStore((s) => s.richRenderMode)
  const setRichRenderMode = useMonitorStore((s) => s.setRichRenderMode)

  return (
    <ViewModeToggle
      verboseMode={verboseMode}
      onToggleVerbose={toggleVerbose}
      richRenderMode={richRenderMode}
      onSetRichRenderMode={setRichRenderMode}
      className={className}
    />
  )
}
