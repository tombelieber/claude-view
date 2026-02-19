import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { QuestionCard } from './QuestionCard'

const singleQuestion = {
  questions: [
    {
      question: 'Which auth method should we use?',
      header: 'Auth',
      options: [
        { label: 'JWT tokens', description: 'Stateless auth with JSON web tokens' },
        { label: 'OAuth 2.0', description: 'Delegated auth via OAuth' },
        { label: 'Session cookies', description: 'Server-side session storage' },
      ],
      multiSelect: false,
    },
  ],
}

const multiQuestion = {
  questions: [
    { question: 'First question?', header: 'Q1', options: [{ label: 'A', description: 'a' }, { label: 'B', description: 'b' }], multiSelect: false },
    { question: 'Second question?', header: 'Q2', options: [{ label: 'C', description: 'c' }], multiSelect: false },
  ],
}

describe('QuestionCard', () => {
  it('renders nothing when context has no questions', () => {
    const { container } = render(<QuestionCard context={{}} />)
    expect(container.firstChild).toBeNull()
  })

  it('renders nothing when context is undefined', () => {
    const { container } = render(<QuestionCard context={undefined} />)
    expect(container.firstChild).toBeNull()
  })

  it('shows the question text', () => {
    render(<QuestionCard context={singleQuestion} />)
    expect(screen.getByText('Which auth method should we use?')).toBeInTheDocument()
  })

  it('shows option labels as read-only chips', () => {
    render(<QuestionCard context={singleQuestion} />)
    expect(screen.getByText('JWT tokens')).toBeInTheDocument()
    expect(screen.getByText('OAuth 2.0')).toBeInTheDocument()
    expect(screen.getByText('Session cookies')).toBeInTheDocument()
  })

  it('shows "Answer in terminal" footer', () => {
    render(<QuestionCard context={singleQuestion} />)
    expect(screen.getByText(/answer in terminal/i)).toBeInTheDocument()
  })

  it('shows multi-question badge when questions.length > 1', () => {
    render(<QuestionCard context={multiQuestion} />)
    expect(screen.getByText(/1 of 2/)).toBeInTheDocument()
  })

  it('does not show multi-question badge for single question', () => {
    render(<QuestionCard context={singleQuestion} />)
    expect(screen.queryByText(/1 of/)).not.toBeInTheDocument()
  })

  it('limits displayed options to 4', () => {
    const manyOptions = {
      questions: [{
        question: 'Pick one?',
        header: 'Test',
        options: [
          { label: 'Opt1', description: '' },
          { label: 'Opt2', description: '' },
          { label: 'Opt3', description: '' },
          { label: 'Opt4', description: '' },
          { label: 'Opt5', description: '' },
        ],
        multiSelect: false,
      }],
    }
    render(<QuestionCard context={manyOptions} />)
    expect(screen.getByText('Opt1')).toBeInTheDocument()
    expect(screen.getByText('Opt4')).toBeInTheDocument()
    expect(screen.queryByText('Opt5')).not.toBeInTheDocument()
  })
})
