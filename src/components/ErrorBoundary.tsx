import React, { ReactNode, ErrorInfo } from 'react'

interface Props {
  children: ReactNode
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
    // Log to console for debugging (in production, you'd send to error tracking)
    console.error('Error caught by ErrorBoundary:', error, errorInfo)
  }

  render() {
    if (this.state.hasError) {
      const errorMessage = this.state.error?.message || 'Unknown error'

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
