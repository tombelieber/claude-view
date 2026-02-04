import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { SessionCard } from './SessionCard'
import type { SessionInfo } from '../types/generated/SessionInfo'

// Helper to create a mock SessionInfo with default values
const createMockSession = (overrides?: Partial<SessionInfo>): SessionInfo => ({
  id: 'session-1',
  project: 'test-project',
  projectPath: '/path/to/project',
  filePath: '/path/to/session.jsonl',
  modifiedAt: BigInt(Math.floor(Date.now() / 1000)),
  sizeBytes: BigInt(1024),
  preview: 'This is a test session preview',
  lastMessage: 'This is the last message',
  filesTouched: [],
  skillsUsed: [],
  toolCounts: { edit: 0, read: 0, bash: 0, write: 0 },
  messageCount: 5,
  turnCount: 3,
  isSidechain: false,
  deepIndexed: true,
  userPromptCount: 3,
  apiCallCount: 3,
  toolCallCount: 0,
  filesRead: [],
  filesEdited: [],
  filesReadCount: 0,
  filesEditedCount: 0,
  reeditedFilesCount: 0,
  durationSeconds: 0,
  commitCount: 0,
  thinkingBlockCount: 0,
  apiErrorCount: 0,
  compactionCount: 0,
  agentSpawnCount: 0,
  bashProgressCount: 0,
  hookProgressCount: 0,
  mcpProgressCount: 0,
  parseVersion: 1,
  linesAdded: 0,
  linesRemoved: 0,
  locSource: 0,
  ...overrides,
})

describe('SessionCard', () => {
  describe('Phase A: Session Card Enhancement - AC-1: Branch Badge Display', () => {
    it('1.1: should show branch badge with GitBranch icon when gitBranch is set', () => {
      const session = createMockSession({
        gitBranch: 'feature/auth',
      })

      const { container } = render(<SessionCard session={session} />)

      // Check for branch badge text
      expect(screen.getByText('feature/auth')).toBeInTheDocument()

      // Check for GitBranch icon (lucide-react renders svg with specific class)
      const icon = container.querySelector('svg.lucide-git-branch')
      expect(icon).toBeInTheDocument()
    })

    it('1.2: should hide badge when gitBranch is null', () => {
      const session = createMockSession({
        gitBranch: null,
      })

      const { container } = render(<SessionCard session={session} />)

      // Branch badge should not be rendered
      const icon = container.querySelector('svg.lucide-git-branch')
      expect(icon).not.toBeInTheDocument()
    })

    it('1.3: should truncate long branch names with tooltip', () => {
      const longBranch = 'feature/very-long-branch-name-that-exceeds-twenty-characters'
      const session = createMockSession({
        gitBranch: longBranch,
      })

      const { container } = render(<SessionCard session={session} />)

      // Badge should have title attribute with full name
      const badge = container.querySelector('[title="' + longBranch + '"]')
      expect(badge).toBeInTheDocument()

      // Badge should have truncate class
      expect(badge?.querySelector('.truncate')).toBeInTheDocument()
    })

    it('1.4: should use muted pill style with proper contrast', () => {
      const session = createMockSession({
        gitBranch: 'main',
      })

      const { container } = render(<SessionCard session={session} />)

      const badge = container.querySelector('[title="main"]')
      expect(badge).toBeInTheDocument()

      // Should have muted background and text colors
      expect(badge?.className).toContain('bg-gray-100')
      expect(badge?.className).toContain('text-gray-500')
      expect(badge?.className).toContain('dark:bg-gray-800')
      expect(badge?.className).toContain('dark:text-gray-400')
    })

    it('1.5: should render badge before time range in header row', () => {
      const session = createMockSession({
        gitBranch: 'feature/test',
        durationSeconds: 2700, // 45 minutes
      })

      const { container } = render(<SessionCard session={session} />)

      // Get the header row
      const header = container.querySelector('article > div:first-child')
      expect(header).toBeInTheDocument()

      // Branch badge should be in left section, time should be in right section
      const leftSection = header?.querySelector('div:first-child')
      const rightSection = header?.querySelector('div:last-child')

      expect(leftSection?.textContent).toContain('feature/test')
      expect(rightSection?.textContent).toMatch(/min/)
    })
  })

  describe('Phase A: Session Card Enhancement - AC-3: Top Files Display', () => {
    it('3.1: should show all 3 files when exactly 3 files edited', () => {
      const session = createMockSession({
        filesEdited: [
          '/src/auth.ts',
          '/src/middleware.ts',
          '/test/auth.test.ts',
        ],
        filesEditedCount: 3,
      })

      const { container } = render(<SessionCard session={session} />)

      // Check for FileEdit icon (renders as lucide-file-pen)
      const icon = container.querySelector('svg.lucide-file-pen')
      expect(icon).toBeInTheDocument()

      // Check for basenames (not full paths)
      expect(screen.getByText('auth.ts')).toBeInTheDocument()
      expect(screen.getByText('middleware.ts')).toBeInTheDocument()
      expect(screen.getByText('auth.test.ts')).toBeInTheDocument()

      // Should not show "+N more"
      expect(screen.queryByText(/\+\d+ more/)).not.toBeInTheDocument()
    })

    it('3.2: should show 3 files + overflow indicator for 8 files', () => {
      const session = createMockSession({
        filesEdited: [
          '/src/auth.ts',
          '/src/middleware.ts',
          '/test/auth.test.ts',
          '/src/types.ts',
          '/src/utils.ts',
          '/src/api.ts',
          '/src/db.ts',
          '/src/server.ts',
        ],
        filesEditedCount: 8,
      })

      render(<SessionCard session={session} />)

      // Should show first 3
      expect(screen.getByText('auth.ts')).toBeInTheDocument()
      expect(screen.getByText('middleware.ts')).toBeInTheDocument()
      expect(screen.getByText('auth.test.ts')).toBeInTheDocument()

      // Should show "+5 more"
      expect(screen.getByText('+5 more')).toBeInTheDocument()
    })

    it('3.3: should not render top files row when no files edited', () => {
      const session = createMockSession({
        filesEdited: [],
        filesEditedCount: 0,
      })

      const { container } = render(<SessionCard session={session} />)

      // FileEdit icon should not be present
      const icon = container.querySelector('svg.lucide-file-pen')
      expect(icon).not.toBeInTheDocument()
    })

    it('3.4: should extract basename from full path', () => {
      const session = createMockSession({
        filesEdited: [
          '/src/components/Button.tsx',
          '/deeply/nested/folder/structure/file.ts',
          'filename-only.js', // Edge case: no path separators
        ],
        filesEditedCount: 3,
      })

      render(<SessionCard session={session} />)

      // Should show only basenames
      expect(screen.getByText('Button.tsx')).toBeInTheDocument()
      expect(screen.getByText('file.ts')).toBeInTheDocument()
      expect(screen.getByText('filename-only.js')).toBeInTheDocument()

      // Should NOT show full paths
      expect(screen.queryByText('/src/components/Button.tsx')).not.toBeInTheDocument()
    })

    it('3.5: should show files separated by middot', () => {
      const session = createMockSession({
        filesEdited: ['/src/a.ts', '/src/b.ts', '/src/c.ts'],
        filesEditedCount: 3,
      })

      const { container } = render(<SessionCard session={session} />)

      // Check for middot separators (rendered as "·")
      const topFilesRow = container.querySelector('svg.lucide-file-pen')?.parentElement
      expect(topFilesRow).toBeTruthy()
      expect(topFilesRow!.textContent).toContain('·')
    })

    it('3.6: should render top files row between metrics and footer', () => {
      const session = createMockSession({
        filesEdited: ['/src/a.ts', '/src/b.ts'],
        filesEditedCount: 2,
        userPromptCount: 5,
        commitCount: 2,
      })

      const { container } = render(<SessionCard session={session} />)

      // Find all major sections
      const article = container.querySelector('article')
      const sections = Array.from(article?.children || [])

      // Should have: header, preview, metrics, top-files, footer
      expect(sections.length).toBeGreaterThanOrEqual(4)

      // Top files should appear after metrics (containing "prompts") and before footer (containing "commits")
      const html = container.innerHTML
      const metricsIndex = html.indexOf('prompt')
      const topFilesIndex = html.indexOf('lucide-file-pen')
      const footerIndex = html.indexOf('lucide-git-commit-horizontal')

      expect(metricsIndex).toBeGreaterThan(-1)
      expect(topFilesIndex).toBeGreaterThan(-1)
      expect(footerIndex).toBeGreaterThan(-1)
      expect(topFilesIndex).toBeGreaterThan(metricsIndex)
      expect(topFilesIndex).toBeLessThan(footerIndex)
    })
  })

  describe('Edge cases and accessibility', () => {
    it('should handle null session gracefully', () => {
      const { container } = render(<SessionCard session={null} />)

      expect(screen.getByText('Session data unavailable')).toBeInTheDocument()
      expect(container.querySelector('article')).toBeInTheDocument()
    })

    it('should handle undefined session gracefully', () => {
      const { container } = render(<SessionCard session={undefined} />)

      expect(screen.getByText('Session data unavailable')).toBeInTheDocument()
    })

    it('should have cursor-pointer class for clickable card', () => {
      const session = createMockSession()
      const { container } = render(<SessionCard session={session} />)

      const article = container.querySelector('article')
      expect(article?.className).toContain('cursor-pointer')
    })

    it('should have sufficient color contrast in dark mode', () => {
      const session = createMockSession({
        gitBranch: 'feature/test',
        filesEdited: ['/src/test.ts'],
        filesEditedCount: 1,
      })

      const { container } = render(<SessionCard session={session} />)

      // Branch badge should have dark mode classes
      const badge = container.querySelector('[title="feature/test"]')
      expect(badge?.className).toContain('dark:bg-gray-800')
      expect(badge?.className).toContain('dark:text-gray-400')

      // Top files should have dark mode classes
      const topFiles = container.querySelector('svg.lucide-file-pen')?.parentElement
      expect(topFiles).toBeTruthy()
      expect(topFiles!.className).toContain('dark:text-gray-400')
    })

    it('should handle empty strings in filesEdited array', () => {
      const session = createMockSession({
        filesEdited: ['', '/src/valid.ts', ''],
        filesEditedCount: 3,
      })

      const { container } = render(<SessionCard session={session} />)

      // Should still render without crashing
      expect(container.querySelector('svg.lucide-file-pen')).toBeInTheDocument()
    })

    it('should handle very long file names', () => {
      const longFileName = 'a'.repeat(200) + '.ts'
      const session = createMockSession({
        filesEdited: [`/src/${longFileName}`],
        filesEditedCount: 1,
      })

      const { container } = render(<SessionCard session={session} />)

      // Should render without breaking layout
      expect(container.querySelector('svg.lucide-file-pen')).toBeInTheDocument()
    })

    it('should apply selected styles when isSelected is true', () => {
      const session = createMockSession()
      const { container } = render(<SessionCard session={session} isSelected={true} />)

      const article = container.querySelector('article')
      expect(article?.className).toContain('bg-blue-50')
      expect(article?.className).toContain('border-blue-500')
    })

    it('should show project display name when provided', () => {
      const session = createMockSession()
      render(<SessionCard session={session} projectDisplayName="My Project" />)

      expect(screen.getByText('My Project')).toBeInTheDocument()
    })

    it('should handle both gitBranch and filesEdited together', () => {
      const session = createMockSession({
        gitBranch: 'feature/auth',
        filesEdited: ['/src/auth.ts', '/src/middleware.ts'],
        filesEditedCount: 2,
      })

      const { container } = render(<SessionCard session={session} />)

      // Both features should be present
      expect(container.querySelector('svg.lucide-git-branch')).toBeInTheDocument()
      expect(container.querySelector('svg.lucide-file-pen')).toBeInTheDocument()
      expect(screen.getByText('feature/auth')).toBeInTheDocument()
      expect(screen.getByText('auth.ts')).toBeInTheDocument()
    })
  })

  describe('XSS prevention', () => {
    it('should escape malicious branch names', () => {
      const session = createMockSession({
        gitBranch: '<script>alert("XSS")</script>',
      })

      render(<SessionCard session={session} />)

      // React should auto-escape the script tag
      expect(screen.getByText(/<script>alert\("XSS"\)<\/script>/)).toBeInTheDocument()

      // Should not execute script
      const { container } = render(<SessionCard session={session} />)
      const scripts = container.querySelectorAll('script')
      expect(scripts.length).toBe(0)
    })

    it('should escape malicious file names', () => {
      const session = createMockSession({
        filesEdited: ['/src/<img src=x onerror="alert(1)">.ts'],
        filesEditedCount: 1,
      })

      render(<SessionCard session={session} />)

      // React should auto-escape
      const { container } = render(<SessionCard session={session} />)
      const imgs = container.querySelectorAll('img')
      expect(imgs.length).toBe(0)
    })
  })

  describe('Phase C: LOC Display - AC-2: Lines of Code Display', () => {
    it('2.1: should show +100 / -20 with green/red colors', () => {
      const session = createMockSession({
        linesAdded: 100,
        linesRemoved: 20,
        locSource: 1, // tool estimate
      })

      const { container } = render(<SessionCard session={session} />)

      // Check for green text with +100
      const greenText = screen.getByText('+100')
      expect(greenText).toBeInTheDocument()
      expect(greenText.className).toContain('text-green-600')
      expect(greenText.className).toContain('dark:text-green-400')

      // Check for red text with -20
      const redText = screen.getByText('-20')
      expect(redText).toBeInTheDocument()
      expect(redText.className).toContain('text-red-600')
      expect(redText.className).toContain('dark:text-red-400')

      // Check for separator
      expect(screen.getByText('/')).toBeInTheDocument()

      // Should not show git icon for tool estimate
      const gitIcons = container.querySelectorAll('svg.lucide-git-commit-horizontal')
      // Filter out the commit badge icon (which appears in footer)
      const locGitIcons = Array.from(gitIcons).filter(icon => {
        const parent = icon.closest('div')
        return parent?.className.includes('mt-2') && parent?.className.includes('text-xs')
      })
      expect(locGitIcons.length).toBe(0)
    })

    it('2.2: should show ±0 in muted gray when no changes', () => {
      const session = createMockSession({
        linesAdded: 0,
        linesRemoved: 0,
        locSource: 1, // computed but no changes
      })

      render(<SessionCard session={session} />)

      const zeroText = screen.getByText('±0')
      expect(zeroText).toBeInTheDocument()
      expect(zeroText.className).toContain('text-gray-400')
      expect(zeroText.className).toContain('dark:text-gray-500')
    })

    it('2.3: should show GitCommit icon for git verified LOC', () => {
      const session = createMockSession({
        linesAdded: 50,
        linesRemoved: 10,
        locSource: 2, // git verified
      })

      const { container } = render(<SessionCard session={session} />)

      // Check for git commit icon in LOC row
      const gitIcons = container.querySelectorAll('svg.lucide-git-commit-horizontal')
      expect(gitIcons.length).toBeGreaterThan(0)

      // Find the icon in the LOC row (not in footer)
      const locGitIcon = Array.from(gitIcons).find(icon => {
        const parent = icon.closest('div')
        return parent?.className.includes('mt-2') && parent?.className.includes('text-xs')
      })
      expect(locGitIcon).toBeTruthy()
    })

    it('2.4: should not show git icon for tool estimate', () => {
      const session = createMockSession({
        linesAdded: 50,
        linesRemoved: 10,
        locSource: 1, // tool estimate
      })

      const { container } = render(<SessionCard session={session} />)

      // Get all git commit icons
      const gitIcons = container.querySelectorAll('svg.lucide-git-commit-horizontal')

      // Filter to only LOC row icons (not footer commit badge)
      const locGitIcons = Array.from(gitIcons).filter(icon => {
        const parent = icon.closest('div')
        return parent?.className.includes('mt-2') && parent?.className.includes('text-xs') && !parent?.className.includes('border')
      })

      expect(locGitIcons.length).toBe(0)
    })

    it('2.5: should format large numbers with K suffix', () => {
      const session = createMockSession({
        linesAdded: 1234,
        linesRemoved: 340,
        locSource: 1,
      })

      render(<SessionCard session={session} />)

      // formatNumber(1234) -> "1.2K"
      expect(screen.getByText('+1.2K')).toBeInTheDocument()
      // formatNumber(340) -> "340" (not large enough for K)
      expect(screen.getByText('-340')).toBeInTheDocument()
    })

    it('2.6: should not render LOC row when locSource is 0 (not computed)', () => {
      const session = createMockSession({
        linesAdded: 0,
        linesRemoved: 0,
        locSource: 0, // not computed
      })

      const { container } = render(<SessionCard session={session} />)

      // Should not show ±0 or any LOC display
      expect(screen.queryByText('±0')).not.toBeInTheDocument()
      expect(screen.queryByText(/^\+/)).not.toBeInTheDocument()
    })

    it('2.7: should handle very large numbers with M suffix', () => {
      const session = createMockSession({
        linesAdded: 2500000,
        linesRemoved: 1200000,
        locSource: 2,
      })

      render(<SessionCard session={session} />)

      // formatNumber(2500000) -> "2.5M"
      expect(screen.getByText('+2.5M')).toBeInTheDocument()
      // formatNumber(1200000) -> "1.2M"
      expect(screen.getByText('-1.2M')).toBeInTheDocument()
    })

    it('2.8: should render LOC row between metrics and top files', () => {
      const session = createMockSession({
        linesAdded: 100,
        linesRemoved: 20,
        locSource: 1,
        userPromptCount: 5,
        filesEdited: ['/src/test.ts'],
        filesEditedCount: 1,
      })

      const { container } = render(<SessionCard session={session} />)

      // Find order of elements in HTML
      const html = container.innerHTML
      const metricsIndex = html.indexOf('5 prompts')
      const locIndex = html.indexOf('+100')
      const topFilesIndex = html.indexOf('lucide-file-pen')

      expect(metricsIndex).toBeGreaterThan(-1)
      expect(locIndex).toBeGreaterThan(-1)
      expect(topFilesIndex).toBeGreaterThan(-1)

      // LOC should be after metrics and before top files
      expect(locIndex).toBeGreaterThan(metricsIndex)
      expect(locIndex).toBeLessThan(topFilesIndex)
    })

    it('2.9: should handle only additions (no deletions)', () => {
      const session = createMockSession({
        linesAdded: 500,
        linesRemoved: 0,
        locSource: 1,
      })

      render(<SessionCard session={session} />)

      expect(screen.getByText('+500')).toBeInTheDocument()
      expect(screen.getByText('-0')).toBeInTheDocument()
    })

    it('2.10: should handle only deletions (no additions)', () => {
      const session = createMockSession({
        linesAdded: 0,
        linesRemoved: 250,
        locSource: 1,
      })

      render(<SessionCard session={session} />)

      // When linesAdded = 0 but linesRemoved > 0, still show as change
      expect(screen.getByText('+0')).toBeInTheDocument()
      expect(screen.getByText('-250')).toBeInTheDocument()
    })

    it('2.11: should work with git verified icon and large numbers together', () => {
      const session = createMockSession({
        linesAdded: 5000,
        linesRemoved: 2300,
        locSource: 2, // git verified
      })

      const { container } = render(<SessionCard session={session} />)

      // Check for formatted numbers
      expect(screen.getByText('+5.0K')).toBeInTheDocument()
      expect(screen.getByText('-2.3K')).toBeInTheDocument()

      // Check for git icon
      const gitIcons = container.querySelectorAll('svg.lucide-git-commit-horizontal')
      const locGitIcon = Array.from(gitIcons).find(icon => {
        const parent = icon.closest('div')
        return parent?.className.includes('mt-2') && parent?.className.includes('text-xs')
      })
      expect(locGitIcon).toBeTruthy()
    })
  })

  describe('Existing functionality (regression tests)', () => {
    it('should still show commit count badge', () => {
      const session = createMockSession({
        commitCount: 3,
      })

      const { container } = render(<SessionCard session={session} />)

      expect(container.querySelector('svg.lucide-git-commit-horizontal')).toBeInTheDocument()
      expect(screen.getByText(/3 commits/)).toBeInTheDocument()
    })

    it('should still show skills used', () => {
      const session = createMockSession({
        skillsUsed: ['tdd', 'commit'],
      })

      render(<SessionCard session={session} />)

      expect(screen.getByText('tdd')).toBeInTheDocument()
      expect(screen.getByText('commit')).toBeInTheDocument()
    })

    it('should still show time range when duration is set', () => {
      const now = Math.floor(Date.now() / 1000)
      const session = createMockSession({
        modifiedAt: BigInt(now),
        durationSeconds: 2700, // 45 minutes
      })

      render(<SessionCard session={session} />)

      expect(screen.getByText(/45 min/)).toBeInTheDocument()
    })

    it('should still show metrics: prompts, tokens, files', () => {
      const session = createMockSession({
        userPromptCount: 12,
        totalInputTokens: BigInt(30000),
        totalOutputTokens: BigInt(15000),
        filesEditedCount: 8,
      })

      render(<SessionCard session={session} />)

      expect(screen.getByText(/12 prompts/)).toBeInTheDocument()
      expect(screen.getByText(/45\.0K tokens/)).toBeInTheDocument()
      expect(screen.getByText(/8 files/)).toBeInTheDocument()
    })
  })
})
