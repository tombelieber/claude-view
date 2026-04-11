import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ChatTurnBoundary } from '../TurnBoundary'
import type { TurnBoundaryBlock } from '../../../../../types/blocks'

function makeBlock(overrides: Partial<TurnBoundaryBlock> = {}): TurnBoundaryBlock {
  return {
    type: 'turn_boundary',
    id: 'tb-1',
    success: true,
    totalCostUsd: 0,
    numTurns: 1,
    durationMs: 500,
    usage: {},
    modelUsage: {},
    permissionDenials: [],
    stopReason: null,
    ...overrides,
  }
}

describe('ChatTurnBoundary', () => {
  it('renders without crash for minimal block', () => {
    render(<ChatTurnBoundary block={makeBlock()} />)
  })

  it('does not render Details toggle when no detail fields present', () => {
    render(<ChatTurnBoundary block={makeBlock()} />)
    expect(screen.queryByText('Details')).not.toBeInTheDocument()
  })

  it('renders Details toggle when stopReason is present', () => {
    render(<ChatTurnBoundary block={makeBlock({ stopReason: 'end_turn' })} />)
    expect(screen.getByText('Details')).toBeInTheDocument()
  })

  it('Details section is hidden by default', () => {
    render(<ChatTurnBoundary block={makeBlock({ stopReason: 'end_turn' })} />)
    // stopReason label not visible until expanded
    expect(screen.queryByText('Stop:')).not.toBeInTheDocument()
  })

  it('expands Details section on click and shows stopReason badge', () => {
    render(<ChatTurnBoundary block={makeBlock({ stopReason: 'end_turn' })} />)
    fireEvent.click(screen.getByText('Details'))
    expect(screen.getByText('Stop:')).toBeInTheDocument()
    expect(screen.getByText('end_turn')).toBeInTheDocument()
  })

  it('shows fastModeState badge after expanding', () => {
    render(<ChatTurnBoundary block={makeBlock({ stopReason: null, fastModeState: 'on' })} />)
    fireEvent.click(screen.getByText('Details'))
    expect(screen.getByText('Fast mode:')).toBeInTheDocument()
    expect(screen.getByText('on')).toBeInTheDocument()
  })

  it('shows durationApiMs after expanding', () => {
    render(
      <ChatTurnBoundary
        block={makeBlock({ stopReason: null, fastModeState: undefined, durationApiMs: 2500 })}
      />,
    )
    fireEvent.click(screen.getByText('Details'))
    expect(screen.getByText('API duration:')).toBeInTheDocument()
    expect(screen.getByText('2.5s')).toBeInTheDocument()
  })

  it('shows result text after expanding', () => {
    render(<ChatTurnBoundary block={makeBlock({ result: 'Task completed successfully' })} />)
    fireEvent.click(screen.getByText('Details'))
    expect(screen.getByText('Result:')).toBeInTheDocument()
    expect(screen.getByText('Task completed successfully')).toBeInTheDocument()
  })

  it('shows error subtype badge after expanding', () => {
    render(
      <ChatTurnBoundary
        block={makeBlock({
          success: false,
          error: { subtype: 'error_max_turns', messages: ['Exceeded turn limit'] },
        })}
      />,
    )
    fireEvent.click(screen.getByText('Details'))
    expect(screen.getByText('error_max_turns')).toBeInTheDocument()
  })

  it('shows error messages list after expanding', () => {
    render(
      <ChatTurnBoundary
        block={makeBlock({
          success: false,
          error: {
            subtype: 'error_during_execution',
            messages: ['Something went wrong', 'Secondary error'],
          },
        })}
      />,
    )
    fireEvent.click(screen.getByText('Details'))
    expect(screen.getByText('Something went wrong')).toBeInTheDocument()
    expect(screen.getByText('Secondary error')).toBeInTheDocument()
  })

  it('shows permissionDenials with toolUseId after expanding', () => {
    render(
      <ChatTurnBoundary
        block={makeBlock({
          permissionDenials: [
            { toolName: 'Bash', toolUseId: 'tu-abc-1234', toolInput: { command: 'rm -rf' } },
          ],
        })}
      />,
    )
    fireEvent.click(screen.getByText('Details'))
    expect(screen.getByText('Bash')).toBeInTheDocument()
    expect(screen.getByText(/tu-abc-1234/)).toBeInTheDocument()
  })

  it('shows structuredOutput CollapsibleJson after expanding', () => {
    render(
      <ChatTurnBoundary block={makeBlock({ structuredOutput: { result: 'ok', score: 0.9 } })} />,
    )
    fireEvent.click(screen.getByText('Details'))
    expect(screen.getByText('structuredOutput')).toBeInTheDocument()
  })

  it('shows hookInfos CollapsibleJson after expanding', () => {
    render(
      <ChatTurnBoundary block={makeBlock({ hookInfos: [{ hookName: 'my-hook', exitCode: 0 }] })} />,
    )
    fireEvent.click(screen.getByText('Details'))
    expect(screen.getByText(/hookInfos/)).toBeInTheDocument()
  })

  it('collapses Details section on second click', () => {
    render(<ChatTurnBoundary block={makeBlock({ stopReason: 'end_turn' })} />)
    const btn = screen.getByText('Details')
    fireEvent.click(btn)
    expect(screen.getByText('Stop:')).toBeInTheDocument()
    fireEvent.click(btn)
    expect(screen.queryByText('Stop:')).not.toBeInTheDocument()
  })

  it('renders hook error when hookErrors present', () => {
    render(
      <ChatTurnBoundary block={makeBlock({ hookErrors: ['Hook script failed with exit 1'] })} />,
    )
    expect(screen.getByText('Hook script failed with exit 1')).toBeInTheDocument()
  })

  it('renders preventedContinuation notice', () => {
    render(<ChatTurnBoundary block={makeBlock({ preventedContinuation: true, hookCount: 3 })} />)
    expect(screen.getByText('Hook blocked continuation')).toBeInTheDocument()
    expect(screen.getByText('3 hooks ran')).toBeInTheDocument()
  })

  it('shows max turns label in divider for error_max_turns error', () => {
    render(
      <ChatTurnBoundary
        block={makeBlock({
          success: false,
          numTurns: 5,
          error: { subtype: 'error_max_turns', messages: [] },
        })}
      />,
    )
    expect(screen.getByText('Max turns (5)')).toBeInTheDocument()
  })
})
