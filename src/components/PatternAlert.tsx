import { useState, useEffect } from 'react'

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
    <div className="fixed bottom-4 right-4 max-w-sm rounded-lg border border-amber-500/30 bg-card p-4 shadow-lg z-50">
      <p className="text-sm font-medium">
        {alert.count} of your last sessions were &quot;{alert.pattern}&quot;
      </p>
      <p className="text-sm text-muted-foreground mt-1">{alert.tip}</p>
      <button
        onClick={handleDismiss}
        className="mt-2 text-xs text-muted-foreground hover:text-foreground"
      >
        Dismiss
      </button>
    </div>
  )
}
