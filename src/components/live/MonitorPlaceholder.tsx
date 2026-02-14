import { Monitor } from 'lucide-react'

export function MonitorPlaceholder() {
  return (
    <div className="flex flex-col items-center justify-center py-20 text-center">
      <div className="w-16 h-16 rounded-2xl bg-slate-800/50 border border-slate-700 flex items-center justify-center mb-4">
        <Monitor className="w-8 h-8 text-slate-500" />
      </div>
      <h3 className="text-sm font-medium text-slate-300 mb-1">
        Live Monitor
      </h3>
      <p className="text-xs text-slate-500 max-w-xs">
        Real-time terminal output view coming in Phase C.
      </p>
    </div>
  )
}
