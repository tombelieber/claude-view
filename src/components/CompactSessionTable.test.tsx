import { describe, it, expect, vi } from 'vitest'
import { render, screen, within, fireEvent } from '@testing-library/react'
import { BrowserRouter } from 'react-router-dom'
import { CompactSessionTable } from './CompactSessionTable'
import type { SessionInfo } from '../hooks/use-projects'

const mockSession: SessionInfo = {
  id: 'test-session-1',
  project: 'test-project',
  projectPath: '/test/path',
  filePath: '/test/path/session.jsonl',
  modifiedAt: BigInt(Math.floor(Date.now() / 1000) - 3600), // 1 hour ago
  sizeBytes: BigInt(1024),
  preview: 'Test session preview text',
  lastMessage: 'Last message in session',
  filesTouched: ['file1.ts', 'file2.ts'],
  skillsUsed: ['skill1'],
  toolCounts: { edit: 5, read: 10, bash: 2, write: 3 },
  messageCount: 15,
  turnCount: 8,
  summary: null,
  gitBranch: 'feature/test',
  isSidechain: false,
  deepIndexed: true,
  totalInputTokens: BigInt(25000),
  totalOutputTokens: BigInt(20000),
  totalCacheReadTokens: null,
  totalCacheCreationTokens: null,
  turnCountApi: null,
  primaryModel: 'claude-opus-4',
  userPromptCount: 8,
  apiCallCount: 8,
  toolCallCount: 20,
  filesRead: ['file1.ts'],
  filesEdited: ['file1.ts', 'file2.ts'],
  filesReadCount: 1,
  filesEditedCount: 2,
  reeditedFilesCount: 0,
  durationSeconds: 2700, // 45 minutes
  commitCount: 2,
  thinkingBlockCount: 3,
  turnDurationAvgMs: null,
  turnDurationMaxMs: null,
  apiErrorCount: 0,
  compactionCount: 0,
  agentSpawnCount: 0,
  bashProgressCount: 0,
  hookProgressCount: 0,
  mcpProgressCount: 0,
  summaryText: null,
  linesAdded: 150,
  linesRemoved: 45,
  locSource: 1,
  parseVersion: 1,
}

const mockSessions: SessionInfo[] = [
  mockSession,
  {
    ...mockSession,
    id: 'test-session-2',
    preview: 'Another session preview',
    gitBranch: 'main',
    modifiedAt: BigInt(Math.floor(Date.now() / 1000) - 7200), // 2 hours ago
    durationSeconds: 1800, // 30 minutes
    userPromptCount: 5,
    totalInputTokens: BigInt(10000),
    totalOutputTokens: BigInt(8000),
    filesEditedCount: 3,
    linesAdded: 75,
    linesRemoved: 20,
    commitCount: 1,
  },
]

function renderTable(sessions: SessionInfo[] = mockSessions, onSort?: (column: string) => void) {
  return render(
    <BrowserRouter>
      <CompactSessionTable sessions={sessions} onSort={onSort || vi.fn()} sortColumn="time" sortDirection="desc" />
    </BrowserRouter>
  )
}

describe('CompactSessionTable', () => {
  describe('Table structure and columns', () => {
    it('renders table with all 7 required columns', () => {
      renderTable()

      expect(screen.getByRole('columnheader', { name: /time/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /branch/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /preview/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /activity/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /changes/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /commits/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /dur/i })).toBeInTheDocument()
    })

    it('does not render old Tokens or LOC columns', () => {
      renderTable()

      expect(screen.queryByRole('columnheader', { name: /^tokens$/i })).not.toBeInTheDocument()
      expect(screen.queryByRole('columnheader', { name: /^loc$/i })).not.toBeInTheDocument()
    })

    it('renders correct number of data rows', () => {
      renderTable()

      const rows = screen.getAllByRole('row')
      // 1 header row + 2 data rows
      expect(rows).toHaveLength(3)
    })
  })

  describe('Row navigation', () => {
    it('navigates to session detail on row click', () => {
      renderTable()

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]

      expect(firstDataRow).toHaveClass('cursor-pointer')

      const links = within(firstDataRow).getAllByRole('link')
      expect(links.length).toBeGreaterThan(0)
      links.forEach(link => {
        expect(link).toHaveAttribute('href', expect.stringContaining('/project/test-project/session/'))
      })
    })

    it('highlights row on hover with blue tint', () => {
      renderTable()

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]

      expect(firstDataRow).toHaveClass('hover:bg-blue-50/50')
    })
  })

  describe('Column sorting', () => {
    it('calls onSort when clicking sortable column header', () => {
      const onSort = vi.fn()
      renderTable(mockSessions, onSort)

      const timeHeader = screen.getByRole('columnheader', { name: /time/i })
      const button = within(timeHeader).getByRole('button')

      fireEvent.click(button)

      expect(onSort).toHaveBeenCalledWith('time')
    })

    it('shows sort arrow on sorted column', () => {
      render(
        <BrowserRouter>
          <CompactSessionTable
            sessions={mockSessions}
            onSort={vi.fn()}
            sortColumn="prompts"
            sortDirection="asc"
          />
        </BrowserRouter>
      )

      const activityHeader = screen.getByRole('columnheader', { name: /activity/i })
      expect(activityHeader).toHaveAttribute('aria-sort', 'ascending')
    })

    it('Preview column is not sortable', () => {
      renderTable()

      const previewHeader = screen.getByRole('columnheader', { name: /preview/i })

      expect(within(previewHeader).queryByRole('button')).not.toBeInTheDocument()
      expect(previewHeader).toHaveAttribute('aria-sort', 'none')
    })

    it('Activity column sorts by prompts key', () => {
      const onSort = vi.fn()
      renderTable(mockSessions, onSort)

      const activityHeader = screen.getByRole('columnheader', { name: /activity/i })
      const button = within(activityHeader).getByRole('button')

      fireEvent.click(button)

      expect(onSort).toHaveBeenCalledWith('prompts')
    })

    it('Changes column sorts by files key', () => {
      const onSort = vi.fn()
      renderTable(mockSessions, onSort)

      const changesHeader = screen.getByRole('columnheader', { name: /changes/i })
      const button = within(changesHeader).getByRole('button')

      fireEvent.click(button)

      expect(onSort).toHaveBeenCalledWith('files')
    })
  })

  describe('Data formatting', () => {
    it('formats time as date + time', () => {
      renderTable([mockSession])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]

      const timeCell = within(firstDataRow).getAllByRole('cell')[0]
      expect(timeCell.textContent).toMatch(/(Today|Yest\.|\w+ \d+)/)
      expect(timeCell.textContent).toMatch(/\d+:\d+ (AM|PM)/)
    })

    it('formats branch as pill badge', () => {
      renderTable([mockSession])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const branchCell = within(firstDataRow).getAllByRole('cell')[1]

      expect(branchCell.textContent).toContain('feature/test')

      // Branch pill should have rounded-md and bg-gray-100 classes
      const pill = branchCell.querySelector('.rounded-md')
      expect(pill).toBeInTheDocument()
      expect(pill).toHaveClass('bg-gray-100')
    })

    it('formats activity as prompts/tokens inline', () => {
      renderTable([mockSession])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const activityCell = within(firstDataRow).getAllByRole('cell')[3]

      // "8/45K" — prompt count, separator, token count
      expect(activityCell.textContent).toContain('8')
      expect(activityCell.textContent).toContain('45K')
    })

    it('formats changes as files + LOC inline', () => {
      renderTable([mockSession])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const changesCell = within(firstDataRow).getAllByRole('cell')[4]

      // "2f +150/-45"
      expect(changesCell.textContent).toContain('2f')
      expect(changesCell.textContent).toContain('+150')
      expect(changesCell.textContent).toContain('-45')
    })

    it('formats duration in minutes', () => {
      renderTable([mockSession])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const durationCell = within(firstDataRow).getAllByRole('cell')[6]

      // 2700 seconds = 45 minutes
      expect(durationCell.textContent).toBe('45m')
    })

    it('truncates long preview text', () => {
      const longPreview = 'This is a very long preview text that should be truncated to fit within the table cell without breaking the layout or causing overflow issues'
      renderTable([{ ...mockSession, preview: longPreview }])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const previewCell = within(firstDataRow).getAllByRole('cell')[2]

      const link = previewCell.firstElementChild as HTMLElement
      const span = link.firstElementChild as HTMLElement
      expect(span).toHaveClass('truncate')
    })

    it('shows commit badge with green background when commits > 0', () => {
      renderTable([mockSession])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const commitsCell = within(firstDataRow).getAllByRole('cell')[5]

      const badge = commitsCell.querySelector('.bg-emerald-50')
      expect(badge).toBeInTheDocument()
      expect(commitsCell.textContent).toContain('2')
    })
  })

  describe('Accessibility', () => {
    it('uses tabular-nums for numeric columns', () => {
      renderTable()

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]

      // Check Activity column (index 3)
      const activityCell = within(firstDataRow).getAllByRole('cell')[3]
      expect(activityCell).toHaveClass('tabular-nums')

      // Check Changes column (index 4)
      const changesCell = within(firstDataRow).getAllByRole('cell')[4]
      expect(changesCell).toHaveClass('tabular-nums')
    })

    it('table has proper semantic structure', () => {
      renderTable()

      const table = screen.getByRole('table')
      expect(table).toBeInTheDocument()

      const thead = table.querySelector('thead')
      const tbody = table.querySelector('tbody')
      expect(thead).toBeInTheDocument()
      expect(tbody).toBeInTheDocument()
    })

    it('column headers have scope attribute', () => {
      renderTable()

      const timeHeader = screen.getByRole('columnheader', { name: /time/i })
      expect(timeHeader).toHaveAttribute('scope', 'col')
    })
  })

  describe('Responsive design', () => {
    it('has overflow-x-auto wrapper for mobile', () => {
      const { container } = renderTable()

      const wrapper = container.firstChild
      expect(wrapper).toHaveClass('overflow-x-auto')
    })
  })

  describe('Empty state', () => {
    it('renders empty state when sessions array is empty', () => {
      renderTable([])

      expect(screen.getByText('No sessions found')).toBeInTheDocument()
      expect(screen.getByText('Try adjusting your filters')).toBeInTheDocument()
    })

    it('does not render table when sessions array is empty', () => {
      renderTable([])

      expect(screen.queryByRole('table')).not.toBeInTheDocument()
    })
  })

  describe('Edge cases', () => {
    it('handles session without branch', () => {
      renderTable([{ ...mockSession, gitBranch: null }])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const branchCell = within(firstDataRow).getAllByRole('cell')[1]

      expect(branchCell.textContent).toBe('--')
    })

    it('handles session without LOC data', () => {
      renderTable([{ ...mockSession, linesAdded: 0, linesRemoved: 0 }])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const changesCell = within(firstDataRow).getAllByRole('cell')[4]

      // Should still show file count but no LOC breakdown
      expect(changesCell.textContent).toContain('2f')
    })

    it('handles session without commits', () => {
      renderTable([{ ...mockSession, commitCount: 0 }])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const commitsCell = within(firstDataRow).getAllByRole('cell')[5]

      expect(commitsCell.textContent).toBe('--')
    })

    it('handles session with no files edited', () => {
      renderTable([{ ...mockSession, filesEditedCount: 0, linesAdded: 0, linesRemoved: 0 }])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const changesCell = within(firstDataRow).getAllByRole('cell')[4]

      expect(changesCell.textContent).toBe('--')
    })

    it('applies zebra striping on odd rows', () => {
      renderTable()

      const rows = screen.getAllByRole('row')
      // First data row (index 0) should not have zebra class
      expect(rows[1]).not.toHaveClass('bg-gray-50/40')
      // Second data row (index 1) should have zebra class
      expect(rows[2]).toHaveClass('bg-gray-50/40')
    })
  })
})
