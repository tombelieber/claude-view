import React, { type ReactNode, type ErrorInfo } from 'react'

interface Props {
  children: ReactNode
  /** Compact inline fallback for per-block error isolation. */
  inline?: boolean
  /** Block ID for debugging — shown in compact fallback. */
  blockId?: string
}

interface State {
  hasError: boolean
  error?: Error
}

export class ErrorBoundary extends React.Component<Props, State> {
  constructor(props: Props) {
    super(props)
    this.state = { hasError: false }
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Error caught by ErrorBoundary:', error, errorInfo)
  }

  render() {
    if (this.state.hasError) {
      const errorMessage = this.state.error?.message || 'Unknown error'

      if (this.props.inline) {
        return (
          <div
            data-testid="error-boundary-inline"
            className="px-3 py-2 rounded border border-red-200 dark:border-red-900/50 bg-red-50/50 dark:bg-red-950/20 text-xs"
          >
            <span className="text-red-500 dark:text-red-400 font-medium">Block render error</span>
            {this.props.blockId && (
              <span className="text-red-400/60 dark:text-red-500/40 ml-1.5 font-mono">
                {this.props.blockId.slice(0, 12)}
              </span>
            )}
            <div className="text-red-400 dark:text-red-500/70 font-mono mt-0.5 break-words">
              {errorMessage}
            </div>
          </div>
        )
      }

      return (
        <div
          data-testid="error-boundary"
          style={{
            padding: '16px',
            margin: '8px',
            backgroundColor: '#fef2f2',
            border: '1px solid #fecaca',
            borderRadius: '4px',
          }}
        >
          <div style={{ fontWeight: 'bold', color: '#dc2626', marginBottom: '8px' }}>
            Something went wrong
          </div>
          <div
            style={{
              color: '#7f1d1d',
              fontSize: '14px',
              fontFamily: 'monospace',
              wordBreak: 'break-word',
              whiteSpace: 'pre-wrap',
            }}
          >
            {errorMessage}
          </div>
        </div>
      )
    }

    return this.props.children
  }
}
