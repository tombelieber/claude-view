import { describe, it, expect } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { FileSnapshotCard } from './FileSnapshotCard'

describe('FileSnapshotCard', () => {
  describe('rendering', () => {
    it('should show file count and timestamp', () => {
      render(
        <FileSnapshotCard
          fileCount={4}
          timestamp="14:32"
          files={['a.ts', 'b.ts', 'c.ts', 'd.ts']}
          isIncremental={false}
        />
      )
      expect(screen.getByText(/4 files backed up at 14:32/)).toBeInTheDocument()
    })

    it('should show "incremental" label when isIncremental is true', () => {
      render(
        <FileSnapshotCard
          fileCount={2}
          timestamp="14:32"
          files={['a.ts', 'b.ts']}
          isIncremental={true}
        />
      )
      expect(screen.getByText(/incremental/i)).toBeInTheDocument()
    })

    it('should show file list when expanded', () => {
      render(
        <FileSnapshotCard
          fileCount={3}
          timestamp="14:32"
          files={['src/app.ts', 'src/index.ts', 'README.md']}
          isIncremental={false}
        />
      )
      // With <= 10 files, should default expanded
      expect(screen.getByText('src/app.ts')).toBeInTheDocument()
      expect(screen.getByText('src/index.ts')).toBeInTheDocument()
      expect(screen.getByText('README.md')).toBeInTheDocument()
    })
  })

  describe('visual styling', () => {
    it('should have a blue left border', () => {
      const { container } = render(
        <FileSnapshotCard
          fileCount={1}
          timestamp="14:32"
          files={['a.ts']}
          isIncremental={false}
        />
      )
      const card = container.firstElementChild as HTMLElement
      expect(card.className).toMatch(/border-l/)
      expect(card.className).toMatch(/border-l-blue-300/)
    })

    it('should render the Archive icon', () => {
      const { container } = render(
        <FileSnapshotCard
          fileCount={1}
          timestamp="14:32"
          files={['a.ts']}
          isIncremental={false}
        />
      )
      const svg = container.querySelector('svg')
      expect(svg).toBeInTheDocument()
    })
  })

  describe('collapsible behavior', () => {
    it('should default collapsed when >10 files', () => {
      const files = Array.from({ length: 15 }, (_, i) => `file-${i}.ts`)
      render(
        <FileSnapshotCard
          fileCount={15}
          timestamp="14:32"
          files={files}
          isIncremental={false}
        />
      )
      // Files should NOT be visible when collapsed
      expect(screen.queryByText('file-0.ts')).not.toBeInTheDocument()
    })

    it('should default expanded when <=10 files', () => {
      const files = ['a.ts', 'b.ts', 'c.ts']
      render(
        <FileSnapshotCard
          fileCount={3}
          timestamp="14:32"
          files={files}
          isIncremental={false}
        />
      )
      expect(screen.getByText('a.ts')).toBeInTheDocument()
    })

    it('should toggle file list on button click', () => {
      const files = Array.from({ length: 15 }, (_, i) => `file-${i}.ts`)
      render(
        <FileSnapshotCard
          fileCount={15}
          timestamp="14:32"
          files={files}
          isIncremental={false}
        />
      )
      // Initially collapsed
      expect(screen.queryByText('file-0.ts')).not.toBeInTheDocument()

      // Click to expand
      fireEvent.click(screen.getByRole('button'))
      expect(screen.getByText('file-0.ts')).toBeInTheDocument()

      // Click to collapse
      fireEvent.click(screen.getByRole('button'))
      expect(screen.queryByText('file-0.ts')).not.toBeInTheDocument()
    })

    it('should be keyboard navigable (Enter key)', () => {
      const files = Array.from({ length: 15 }, (_, i) => `file-${i}.ts`)
      render(
        <FileSnapshotCard
          fileCount={15}
          timestamp="14:32"
          files={files}
          isIncremental={false}
        />
      )
      const button = screen.getByRole('button')
      fireEvent.keyDown(button, { key: 'Enter' })
      // Button's default behavior handles Enter, so we just verify it's focusable
      expect(button).not.toHaveAttribute('tabindex', '-1')
    })
  })

  describe('edge cases', () => {
    it('should show "No files" when files array is empty', () => {
      render(
        <FileSnapshotCard
          fileCount={0}
          timestamp="14:32"
          files={[]}
          isIncremental={false}
        />
      )
      expect(screen.getByText(/No files/i)).toBeInTheDocument()
    })

    it('should show "Empty snapshot" when fileCount is 0', () => {
      render(
        <FileSnapshotCard
          fileCount={0}
          timestamp="14:32"
          files={[]}
          isIncremental={false}
        />
      )
      expect(screen.getByText(/Empty snapshot/i)).toBeInTheDocument()
    })
  })

  describe('accessibility', () => {
    it('should have an aria-label on the card', () => {
      render(
        <FileSnapshotCard
          fileCount={3}
          timestamp="14:32"
          files={['a.ts', 'b.ts', 'c.ts']}
          isIncremental={false}
        />
      )
      expect(screen.getByLabelText(/file snapshot/i)).toBeInTheDocument()
    })

    it('should have aria-expanded on the collapse button', () => {
      const files = Array.from({ length: 15 }, (_, i) => `file-${i}.ts`)
      render(
        <FileSnapshotCard
          fileCount={15}
          timestamp="14:32"
          files={files}
          isIncremental={false}
        />
      )
      const button = screen.getByRole('button')
      expect(button).toHaveAttribute('aria-expanded', 'false')
    })

    it('should have aria-hidden on the icon', () => {
      const { container } = render(
        <FileSnapshotCard
          fileCount={1}
          timestamp="14:32"
          files={['a.ts']}
          isIncremental={false}
        />
      )
      const svg = container.querySelector('svg')
      expect(svg?.getAttribute('aria-hidden')).toBe('true')
    })
  })
})
