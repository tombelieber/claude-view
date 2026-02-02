import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ErrorBoundary } from './ErrorBoundary'

// Suppress console.error for this test suite
const originalError = console.error
beforeEach(() => {
  console.error = vi.fn()
})

afterEach(() => {
  console.error = originalError
})

// Component that throws an error
const ThrowingComponent = ({ shouldThrow }: { shouldThrow: boolean }) => {
  if (shouldThrow) {
    throw new Error('Test error message')
  }
  return <div>Safe content</div>
}

describe('ErrorBoundary', () => {
  describe('XSS prevention: error message rendering', () => {
    it('should safely render error messages with script tags', () => {
      const ThrowXSSError = () => {
        throw new Error('<script>alert("XSS")</script>')
      }

      render(
        <ErrorBoundary>
          <ThrowXSSError />
        </ErrorBoundary>
      )

      // Error message should appear as text (escaped), not executed
      expect(screen.getByText(/<script>alert\("XSS"\)<\/script>/)).toBeInTheDocument()
    })

    it('should safely render error messages with HTML injection attempts', () => {
      const ThrowHTMLError = () => {
        throw new Error('<img src=x onerror="alert(1)">')
      }

      render(
        <ErrorBoundary>
          <ThrowHTMLError />
        </ErrorBoundary>
      )

      // Should render as text, not as HTML
      expect(screen.getByText(/<img src=x onerror="alert\(1\)">/)).toBeInTheDocument()
    })

    it('should safely render error messages with onclick handlers', () => {
      const ThrowHandlerError = () => {
        throw new Error('onclick="malicious()"')
      }

      render(
        <ErrorBoundary>
          <ThrowHandlerError />
        </ErrorBoundary>
      )

      // Should appear as text, not bound as event handler
      expect(screen.getByText(/onclick="malicious\(\)"/)).toBeInTheDocument()
    })

    it('should not allow event handlers on error display elements', () => {
      const ThrowError = () => {
        throw new Error('Error with <b>HTML</b>')
      }

      const { container } = render(
        <ErrorBoundary>
          <ThrowError />
        </ErrorBoundary>
      )

      // Find the error message container and verify no onclick/onerror handlers
      const errorText = screen.getByText(/Error with/)
      expect(errorText.getAttribute('onclick')).toBeNull()
      expect(errorText.getAttribute('onerror')).toBeNull()
      expect(errorText.getAttribute('onmouseover')).toBeNull()
    })
  })

  describe('Happy path: Error catching and display', () => {
    it('should catch errors from child components', () => {
      render(
        <ErrorBoundary>
          <ThrowingComponent shouldThrow={true} />
        </ErrorBoundary>
      )

      expect(screen.getByText(/Test error message/)).toBeInTheDocument()
    })

    it('should render children when no error occurs', () => {
      render(
        <ErrorBoundary>
          <ThrowingComponent shouldThrow={false} />
        </ErrorBoundary>
      )

      expect(screen.getByText('Safe content')).toBeInTheDocument()
    })

    it('should display error details in fallback UI', () => {
      render(
        <ErrorBoundary>
          <ThrowingComponent shouldThrow={true} />
        </ErrorBoundary>
      )

      // Should show error indicator/boundary message
      expect(screen.getByText('Something went wrong')).toBeInTheDocument()
    })

    it('should provide a way to identify the error boundary triggered', () => {
      const { container } = render(
        <ErrorBoundary>
          <ThrowingComponent shouldThrow={true} />
        </ErrorBoundary>
      )

      // Error boundary container should be identifiable
      const errorContainer = container.querySelector('[data-testid="error-boundary"]')
      expect(errorContainer).toBeInTheDocument()
    })
  })

  describe('Edge cases: Error message formats', () => {
    it('should handle errors with null message', () => {
      const ThrowNullError = () => {
        throw { message: null }
      }

      const { container } = render(
        <ErrorBoundary>
          <ThrowNullError />
        </ErrorBoundary>
      )

      // Should render without crashing
      expect(container).toBeInTheDocument()
      expect(screen.getByText('Something went wrong')).toBeInTheDocument()
    })

    it('should handle errors with undefined message', () => {
      const ThrowUndefinedError = () => {
        throw new Error()
      }

      const { container } = render(
        <ErrorBoundary>
          <ThrowUndefinedError />
        </ErrorBoundary>
      )

      // Should render without crashing
      expect(container).toBeInTheDocument()
      expect(screen.getByText('Something went wrong')).toBeInTheDocument()
    })

    it('should handle very long error messages', () => {
      const longMessage = 'A'.repeat(5000)
      const ThrowLongError = () => {
        throw new Error(longMessage)
      }

      const { container } = render(
        <ErrorBoundary>
          <ThrowLongError />
        </ErrorBoundary>
      )

      // Should render without DOM performance issues
      expect(container).toBeInTheDocument()
    })

    it('should handle errors with special characters and unicode', () => {
      const SpecialCharError = () => {
        throw new Error('Error with special chars: < > & " \' ñ 中文 🔒')
      }

      render(
        <ErrorBoundary>
          <SpecialCharError />
        </ErrorBoundary>
      )

      // Should render all characters safely
      expect(screen.getByText(/Error with special chars/)).toBeInTheDocument()
    })
  })

  describe('Multiple errors: Subsequent render cycles', () => {
    it('should maintain error state across re-renders', () => {
      const { rerender } = render(
        <ErrorBoundary>
          <ThrowingComponent shouldThrow={true} />
        </ErrorBoundary>
      )

      expect(screen.getByText(/Test error message/)).toBeInTheDocument()

      // Re-render with error still present
      rerender(
        <ErrorBoundary>
          <ThrowingComponent shouldThrow={true} />
        </ErrorBoundary>
      )

      // Error should still be displayed
      expect(screen.getByText(/Test error message/)).toBeInTheDocument()
    })
  })
})

describe('Integration: ErrorBoundary wraps messages', () => {
  it('should catch a crashing MessageTyped without killing the list', () => {
    // Simulate: first child crashes, second child survives
    const Crasher = () => { throw new Error('boom') }

    const { container } = render(
      <div>
        <ErrorBoundary>
          <Crasher />
        </ErrorBoundary>
        <ErrorBoundary>
          <div data-testid="survivor">I survived</div>
        </ErrorBoundary>
      </div>
    )

    // Crashed message shows boundary, sibling still renders
    expect(screen.getByTestId('error-boundary')).toBeInTheDocument()
    expect(screen.getByTestId('survivor')).toBeInTheDocument()
  })
})
