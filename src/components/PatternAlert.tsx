import { useState, useEffect } from 'react'
import { FlaskConical } from 'lucide-react'

interface PatternAlertData {
  pattern: string
  count: number
  tip: string
}

export function PatternAlert() {
  const [alert, setAlert] = useState<PatternAlertData | null>(null)
  const [dismissed, setDismissed] = useState(false)

  useEffect(() => {
    fetch('/api/facets/pattern-alert')
      .then(r => r.json())
      .then(data => {
        if (data.pattern) {
          const dismissedKey = `pattern-alert-${data.pattern}-${new Date().toISOString().slice(0, 10)}`
          if (!localStorage.getItem(dismissedKey)) {
            setAlert(data)
          }
        }
      })
      .catch(() => {}) // silent fail — pattern alert is non-critical
  }, [])

  if (!alert || dismissed) return null

  const handleDismiss = () => {
    const key = `pattern-alert-${alert.pattern}-${new Date().toISOString().slice(0, 10)}`
    localStorage.setItem(key, '1')
    setDismissed(true)
  }

  return (
    <div className="fixed bottom-4 right-4 max-w-sm rounded-lg border border-amber-500/30 bg-white dark:bg-gray-900 p-4 shadow-lg z-50">
      <div className="flex items-center gap-1.5 mb-1">
        <p className="text-sm font-medium text-gray-900 dark:text-gray-100">
          {alert.count} of your last sessions were &quot;{alert.pattern}&quot;
        </p>
        <span className="inline-flex items-center gap-0.5 px-1 py-0 text-[9px] font-medium rounded-full border border-amber-300 dark:border-amber-700 text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/40 flex-shrink-0">
          <FlaskConical className="w-2 h-2" />
          Experimental
        </span>
      </div>
      <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">{alert.tip}</p>
      <button
        onClick={handleDismiss}
        className="mt-2 text-xs text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 cursor-pointer"
      >
        Dismiss
      </button>
    </div>
  )
}
