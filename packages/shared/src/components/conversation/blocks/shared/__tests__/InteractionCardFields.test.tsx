import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { PermissionCard } from '../PermissionCard'
import { ElicitationCard } from '../ElicitationCard'
import { AskUserQuestionCard } from '../AskUserQuestionCard'
import type {
  PermissionRequest,
  Elicitation,
  AskQuestion,
} from '../../../../../types/sidecar-protocol'

// ── PermissionCard ────────────────────────────────────────────────────────────

function makePermission(overrides: Partial<PermissionRequest> = {}): PermissionRequest {
  return {
    type: 'permission_request',
    requestId: 'req-1',
    toolName: 'Bash',
    toolInput: { command: 'echo test' },
    toolUseID: 'tu-abc-def-0001',
    timeoutMs: 30000,
    ...overrides,
  }
}

describe('PermissionCard', () => {
  it('renders toolName', () => {
    render(<PermissionCard permission={makePermission()} />)
    expect(screen.getByText('Bash')).toBeInTheDocument()
  })

  it('renders toolUseID', () => {
    render(<PermissionCard permission={makePermission()} />)
    expect(screen.getByText(/tu-abc-def-0001/)).toBeInTheDocument()
  })

  it('renders blockedPath when present', () => {
    render(<PermissionCard permission={makePermission({ blockedPath: '/etc/passwd' })} />)
    expect(screen.getByText(/\/etc\/passwd/)).toBeInTheDocument()
  })

  it('does not render blockedPath element when absent', () => {
    render(<PermissionCard permission={makePermission()} />)
    expect(screen.queryByText(/Blocked:/)).not.toBeInTheDocument()
  })

  it('renders agentID when present', () => {
    render(<PermissionCard permission={makePermission({ agentID: 'agent-xyz-789' })} />)
    expect(screen.getByText(/agent-xyz-789/)).toBeInTheDocument()
  })

  it('does not render agentID element when absent', () => {
    render(<PermissionCard permission={makePermission()} />)
    expect(screen.queryByText(/Agent:/)).not.toBeInTheDocument()
  })

  it('renders decisionReason when present', () => {
    render(
      <PermissionCard
        permission={makePermission({ decisionReason: 'This command modifies system files' })}
      />,
    )
    expect(screen.getByText('This command modifies system files')).toBeInTheDocument()
  })

  it('renders resolved state label when resolved prop is provided', () => {
    render(<PermissionCard permission={makePermission()} resolved={{ allowed: true }} />)
    expect(screen.getByText('Allowed')).toBeInTheDocument()
  })

  it('renders Denied label when resolved with allowed=false', () => {
    render(<PermissionCard permission={makePermission()} resolved={{ allowed: false }} />)
    expect(screen.getByText('Denied')).toBeInTheDocument()
  })

  it('renders Allow and Deny buttons when onRespond is provided', () => {
    render(<PermissionCard permission={makePermission()} onRespond={() => {}} />)
    expect(screen.getByText('Allow')).toBeInTheDocument()
    expect(screen.getByText('Deny')).toBeInTheDocument()
  })

  it('does not render action buttons when onRespond is absent', () => {
    render(<PermissionCard permission={makePermission()} />)
    expect(screen.queryByText('Allow')).not.toBeInTheDocument()
    expect(screen.queryByText('Deny')).not.toBeInTheDocument()
  })
})

// ── ElicitationCard ───────────────────────────────────────────────────────────

function makeElicitation(overrides: Partial<Elicitation> = {}): Elicitation {
  return {
    type: 'elicitation',
    requestId: 'elicit-1',
    toolName: 'ReadFile',
    toolInput: { path: '/tmp/data.json', mode: 'utf-8' },
    prompt: 'Please confirm the file path',
    ...overrides,
  }
}

describe('ElicitationCard', () => {
  it('renders the prompt text', () => {
    render(<ElicitationCard elicitation={makeElicitation()} />)
    expect(screen.getByText('Please confirm the file path')).toBeInTheDocument()
  })

  it('renders toolName', () => {
    render(<ElicitationCard elicitation={makeElicitation()} />)
    expect(screen.getByText('ReadFile')).toBeInTheDocument()
  })

  it('renders CollapsibleJson label for toolInput', () => {
    render(<ElicitationCard elicitation={makeElicitation()} />)
    expect(screen.getByText('Tool Input')).toBeInTheDocument()
  })

  it('renders Submit button when onSubmit provided', () => {
    render(<ElicitationCard elicitation={makeElicitation()} onSubmit={() => {}} />)
    expect(screen.getByText('Submit')).toBeInTheDocument()
  })

  it('does not render Submit button when onSubmit absent', () => {
    render(<ElicitationCard elicitation={makeElicitation()} />)
    expect(screen.queryByText('Submit')).not.toBeInTheDocument()
  })

  it('renders input field when not resolved', () => {
    render(<ElicitationCard elicitation={makeElicitation()} onSubmit={() => {}} />)
    expect(screen.getByPlaceholderText('Type your response...')).toBeInTheDocument()
  })

  it('does not render input field when resolved', () => {
    render(<ElicitationCard elicitation={makeElicitation()} resolved={true} />)
    expect(screen.queryByPlaceholderText('Type your response...')).not.toBeInTheDocument()
  })

  it('renders Submitted state label when resolved', () => {
    render(<ElicitationCard elicitation={makeElicitation()} resolved={true} />)
    expect(screen.getByText('Submitted')).toBeInTheDocument()
  })
})

// ── AskUserQuestionCard ───────────────────────────────────────────────────────

function makeQuestion(overrides: Partial<AskQuestion> = {}): AskQuestion {
  return {
    type: 'ask_question',
    requestId: 'q-1',
    questions: [
      {
        question: 'Which approach do you prefer?',
        header: 'Architecture Choice',
        options: [
          { label: 'Option A', description: 'First approach', markdown: '**Bold Option A**' },
          { label: 'Option B', description: 'Second approach' },
        ],
        multiSelect: false,
      },
    ],
    ...overrides,
  }
}

describe('AskUserQuestionCard', () => {
  it('renders the question text', () => {
    render(<AskUserQuestionCard question={makeQuestion()} />)
    expect(screen.getByText('Which approach do you prefer?')).toBeInTheDocument()
  })

  it('renders the question header', () => {
    render(<AskUserQuestionCard question={makeQuestion()} />)
    expect(screen.getByText('Architecture Choice')).toBeInTheDocument()
  })

  it('renders option labels', () => {
    render(<AskUserQuestionCard question={makeQuestion()} />)
    expect(screen.getByText('Option A')).toBeInTheDocument()
    expect(screen.getByText('Option B')).toBeInTheDocument()
  })

  it('renders option descriptions', () => {
    render(<AskUserQuestionCard question={makeQuestion()} />)
    expect(screen.getByText('First approach')).toBeInTheDocument()
    expect(screen.getByText('Second approach')).toBeInTheDocument()
  })

  it('renders option markdown content', () => {
    render(<AskUserQuestionCard question={makeQuestion()} />)
    // Markdown renders **Bold Option A** as <strong>Bold Option A</strong>
    const bold = screen.getByText('Bold Option A')
    expect(bold.tagName.toLowerCase()).toBe('strong')
  })

  it('renders Other... option always', () => {
    render(<AskUserQuestionCard question={makeQuestion()} />)
    expect(screen.getByText('Other...')).toBeInTheDocument()
  })

  it('renders "Single selection only" hint for non-multiSelect', () => {
    render(<AskUserQuestionCard question={makeQuestion()} />)
    expect(screen.getByText('Single selection only')).toBeInTheDocument()
  })

  it('renders "Multiple selections allowed" hint for multiSelect', () => {
    render(
      <AskUserQuestionCard
        question={makeQuestion({
          questions: [
            {
              question: 'Pick all that apply',
              header: '',
              options: [
                { label: 'X', description: '' },
                { label: 'Y', description: '' },
              ],
              multiSelect: true,
            },
          ],
        })}
      />,
    )
    expect(screen.getByText('Multiple selections allowed')).toBeInTheDocument()
  })

  it('renders Submit button when onAnswer provided', () => {
    render(<AskUserQuestionCard question={makeQuestion()} onAnswer={() => {}} />)
    expect(screen.getByText('Submit')).toBeInTheDocument()
  })

  it('does not render Submit button when onAnswer absent', () => {
    render(<AskUserQuestionCard question={makeQuestion()} />)
    expect(screen.queryByText('Submit')).not.toBeInTheDocument()
  })

  it('renders Answered state label when answered prop is true', () => {
    render(<AskUserQuestionCard question={makeQuestion()} answered={true} />)
    expect(screen.getByText('Answered')).toBeInTheDocument()
  })

  it('renders selected answer text when answered and selectedAnswers provided', () => {
    render(
      <AskUserQuestionCard
        question={makeQuestion()}
        answered={true}
        selectedAnswers={{ 'Which approach do you prefer?': 'Option A' }}
      />,
    )
    expect(screen.getByText('Answer: Option A')).toBeInTheDocument()
  })
})
