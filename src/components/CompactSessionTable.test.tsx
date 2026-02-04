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
  describe('AC-7.3: Table structure and columns', () => {
    it('renders table with all 9 required columns', () => {
      renderTable()

      // Check for all column headers
      expect(screen.getByRole('columnheader', { name: /time/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /branch/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /preview/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /prompts/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /tokens/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /files/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /loc/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /commits/i })).toBeInTheDocument()
      expect(screen.getByRole('columnheader', { name: /duration/i })).toBeInTheDocument()
    })

    it('renders correct number of data rows', () => {
      renderTable()

      const rows = screen.getAllByRole('row')
      // 1 header row + 2 data rows
      expect(rows).toHaveLength(3)
    })
  })

  describe('AC-7.4: Row navigation', () => {
    it('navigates to session detail on row click', () => {
      renderTable()

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1] // Skip header row

      // Row should be clickable
      expect(firstDataRow).toHaveClass('cursor-pointer')

      // Row should have links (accessibility) - each cell has a link
      const links = within(firstDataRow).getAllByRole('link')
      expect(links.length).toBeGreaterThan(0)
      // All links should point to the same session
      links.forEach(link => {
        expect(link).toHaveAttribute('href', expect.stringContaining('/project/test-project/session/'))
      })
    })

    it('highlights row on hover', () => {
      renderTable()

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]

      // Should have hover classes
      expect(firstDataRow).toHaveClass('hover:bg-gray-50')
    })
  })

  describe('AC-7.5: Column sorting', () => {
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
            sortColumn="tokens"
            sortDirection="asc"
          />
        </BrowserRouter>
      )

      const tokensHeader = screen.getByRole('columnheader', { name: /tokens/i })

      // Should have aria-sort attribute
      expect(tokensHeader).toHaveAttribute('aria-sort', 'ascending')
    })

    it('Preview column is not sortable', () => {
      renderTable()

      const previewHeader = screen.getByRole('columnheader', { name: /preview/i })

      // Should not have a button inside
      expect(within(previewHeader).queryByRole('button')).not.toBeInTheDocument()

      // Should have aria-sort="none" to indicate it's not sortable
      expect(previewHeader).toHaveAttribute('aria-sort', 'none')
    })
  })

  describe('Data formatting', () => {
    it('formats time as date + time range', () => {
      renderTable([mockSession])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]

      // Should show date prefix (Today, Yesterday, or Jan 26)
      const timeCell = within(firstDataRow).getAllByRole('cell')[0]
      expect(timeCell.textContent).toMatch(/(Today|Yesterday|\w+ \d+)/)

      // Should show time range
      expect(timeCell.textContent).toMatch(/\d+:\d+ (AM|PM)/)
    })

    it('formats branch with truncation', () => {
      renderTable([mockSession])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const branchCell = within(firstDataRow).getAllByRole('cell')[1]

      expect(branchCell.textContent).toContain('feature/test')
    })

    it('formats tokens with K suffix', () => {
      renderTable([mockSession])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const tokensCell = within(firstDataRow).getAllByRole('cell')[4]

      // 25000 + 20000 = 45000 -> "45K"
      expect(tokensCell.textContent).toBe('45K')
    })

    it('formats LOC as +N / -N', () => {
      renderTable([mockSession])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const locCell = within(firstDataRow).getAllByRole('cell')[6]

      expect(locCell.textContent).toContain('+150')
      expect(locCell.textContent).toContain('-45')
    })

    it('formats duration in minutes', () => {
      renderTable([mockSession])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const durationCell = within(firstDataRow).getAllByRole('cell')[8]

      // 2700 seconds = 45 minutes
      expect(durationCell.textContent).toBe('45m')
    })

    it('truncates long preview text', () => {
      const longPreview = 'This is a very long preview text that should be truncated to fit within the table cell without breaking the layout or causing overflow issues'
      renderTable([{ ...mockSession, preview: longPreview }])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const previewCell = within(firstDataRow).getAllByRole('cell')[2]

      // Should have truncation class on span inside link
      const link = previewCell.firstElementChild as HTMLElement
      const span = link.firstElementChild as HTMLElement
      expect(span).toHaveClass('truncate')
    })
  })

  describe('Accessibility', () => {
    it('uses tabular-nums for numeric columns', () => {
      renderTable()

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]

      // Check prompts column (index 3)
      const promptsCell = within(firstDataRow).getAllByRole('cell')[3]
      expect(promptsCell).toHaveClass('tabular-nums')

      // Check tokens column (index 4)
      const tokensCell = within(firstDataRow).getAllByRole('cell')[4]
      expect(tokensCell).toHaveClass('tabular-nums')
    })

    it('table has proper semantic structure', () => {
      renderTable()

      const table = screen.getByRole('table')
      expect(table).toBeInTheDocument()

      // Should have thead and tbody
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

  describe('Edge cases', () => {
    it('handles empty sessions array', () => {
      renderTable([])

      const rows = screen.getAllByRole('row')
      // Only header row
      expect(rows).toHaveLength(1)
    })

    it('handles session without branch', () => {
      renderTable([{ ...mockSession, gitBranch: null }])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const branchCell = within(firstDataRow).getAllByRole('cell')[1]

      // Should show placeholder
      expect(branchCell.textContent).toBe('--')
    })

    it('handles session without LOC data', () => {
      renderTable([{ ...mockSession, linesAdded: 0, linesRemoved: 0 }])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const locCell = within(firstDataRow).getAllByRole('cell')[6]

      expect(locCell.textContent).toBe('--')
    })

    it('handles session without commits', () => {
      renderTable([{ ...mockSession, commitCount: 0 }])

      const rows = screen.getAllByRole('row')
      const firstDataRow = rows[1]
      const commitsCell = within(firstDataRow).getAllByRole('cell')[7]

      expect(commitsCell.textContent).toBe('--')
    })
  })
})
