import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ChatTeamTranscriptBlock } from '../TeamTranscriptBlock'

const speakers = [
  { id: 's1', displayName: 'Agent A', color: '#ff0000', stance: 'advocate' },
  { id: 's2', displayName: 'Agent B', color: '#0000ff' },
]

const fixture = {
  type: 'team_transcript' as const,
  id: 'tt-1',
  teamName: 'Test Team',
  description: 'A test team',
  speakers,
  entries: [
    { kind: 'agent_message', teammateId: 's1', text: 'Hello world', lineIndex: 0 },
    { kind: 'moderator_narration', text: 'The debate begins', isVerdict: false, lineIndex: 1 },
    { kind: 'moderator_relay', to: 's2', message: 'Please respond', lineIndex: 2 },
    {
      kind: 'task_event',
      subject: 'Review code',
      status: 'completed',
      owner: 'Agent A',
      lineIndex: 3,
    },
    { kind: 'team_lifecycle', event: 'team_started', lineIndex: 4 },
    {
      kind: 'protocol',
      teammateId: 's1',
      msgType: 'thinking',
      raw: { data: 'test' },
      lineIndex: 5,
    },
    { kind: 'unknown_kind', lineIndex: 6 },
  ],
}

describe('ChatTeamTranscriptBlock', () => {
  it('renders the team name in the header', () => {
    render(<ChatTeamTranscriptBlock block={fixture} />)
    expect(screen.getByText('Test Team')).toBeInTheDocument()
  })

  it('renders the team description', () => {
    render(<ChatTeamTranscriptBlock block={fixture} />)
    expect(screen.getByText('A test team')).toBeInTheDocument()
  })

  it('renders all speaker display names', () => {
    render(<ChatTeamTranscriptBlock block={fixture} />)
    expect(screen.getAllByText('Agent A').length).toBeGreaterThan(0)
    expect(screen.getByText('Agent B')).toBeInTheDocument()
  })

  it('renders speaker stance when present', () => {
    render(<ChatTeamTranscriptBlock block={fixture} />)
    expect(screen.getByText('advocate')).toBeInTheDocument()
  })

  it('renders agent_message kind — speaker name and text', () => {
    render(<ChatTeamTranscriptBlock block={fixture} />)
    expect(screen.getByText(/Hello world/)).toBeInTheDocument()
  })

  it('renders moderator_narration kind', () => {
    render(<ChatTeamTranscriptBlock block={fixture} />)
    expect(screen.getByText('The debate begins')).toBeInTheDocument()
  })

  it('renders moderator_relay kind with to + message', () => {
    render(<ChatTeamTranscriptBlock block={fixture} />)
    expect(screen.getByText(/Please respond/)).toBeInTheDocument()
    expect(screen.getByText(/Agent B/)).toBeInTheDocument()
  })

  it('renders task_event kind — subject, status badge, and owner', () => {
    render(<ChatTeamTranscriptBlock block={fixture} />)
    expect(screen.getByText('Review code')).toBeInTheDocument()
    expect(screen.getByText('completed')).toBeInTheDocument()
  })

  it('renders team_lifecycle kind — event text', () => {
    render(<ChatTeamTranscriptBlock block={fixture} />)
    expect(screen.getByText('team_started')).toBeInTheDocument()
  })

  it('renders protocol kind — msgType badge', () => {
    render(<ChatTeamTranscriptBlock block={fixture} />)
    expect(screen.getByText('thinking')).toBeInTheDocument()
  })

  it('renders unknown kind via CollapsibleJson fallback', () => {
    render(<ChatTeamTranscriptBlock block={fixture} />)
    expect(screen.getByText('unknown_kind')).toBeInTheDocument()
  })

  it('renders "No entries" message when entries array is empty', () => {
    render(<ChatTeamTranscriptBlock block={{ ...fixture, entries: [] }} />)
    expect(screen.getByText('No entries')).toBeInTheDocument()
  })

  it('renders without crash when speakers array is empty', () => {
    render(<ChatTeamTranscriptBlock block={{ ...fixture, speakers: [] }} />)
    expect(screen.getByText('Test Team')).toBeInTheDocument()
  })

  it('renders moderator_narration with verdict styling when isVerdict=true', () => {
    const verdictFixture = {
      ...fixture,
      entries: [
        { kind: 'moderator_narration', text: 'Final verdict!', isVerdict: true, lineIndex: 0 },
      ],
    }
    render(<ChatTeamTranscriptBlock block={verdictFixture} />)
    const verdictEl = screen.getByText('Final verdict!')
    expect(verdictEl.className).toContain('amber')
  })
})
