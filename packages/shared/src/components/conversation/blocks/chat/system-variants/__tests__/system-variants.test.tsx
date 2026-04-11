import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { AiTitlePill } from '../ai-title'
import { AttachmentPill } from '../attachment'
import { CommandOutputBlock } from '../command-output'
import { FileHistorySnapshotPill } from '../file-history-snapshot'
import { FilesSavedPill } from '../files-saved'
import { HookEventPill } from '../hook-event'
import { LastPromptPill } from '../last-prompt'
import { LocalCommandBlock } from '../local-command'
import { PermissionModeChangePill } from '../permission-mode-change'
import { ScheduledTaskFirePill } from '../scheduled-task-fire'
import { SessionInitPill } from '../session-init'
import { SessionStatusPill } from '../session-status'
import { UnknownSystemPill } from '../unknown'
import { WorktreeStatePill } from '../worktree-state'

// ── SessionInitPill ───────────────────────────────────────────────────────────

describe('SessionInitPill', () => {
  it('renders model and permissionMode', () => {
    render(
      <SessionInitPill
        data={{
          type: 'session_init',
          model: 'claude-sonnet-4-5',
          permissionMode: 'default',
          tools: [],
          mcpServers: [],
          slashCommands: [],
          claudeCodeVersion: '1.0',
          cwd: '/tmp',
          agents: [],
          skills: [],
          outputStyle: 'stream',
        }}
      />,
    )
    expect(screen.getByText(/claude-sonnet-4-5/)).toBeInTheDocument()
    expect(screen.getByText(/default/)).toBeInTheDocument()
  })

  it('renders without crash when data fields are undefined', () => {
    // @ts-expect-error intentional partial data for resilience test
    render(<SessionInitPill data={{}} />)
    // Should not throw
  })
})

// ── SessionStatusPill ─────────────────────────────────────────────────────────

describe('SessionStatusPill', () => {
  it('renders status text', () => {
    render(
      <SessionStatusPill
        data={{ type: 'session_status', status: 'compacting', permissionMode: undefined }}
      />,
    )
    expect(screen.getByText('compacting')).toBeInTheDocument()
  })

  it('renders permissionMode badge when present', () => {
    render(
      <SessionStatusPill
        data={{ type: 'session_status', status: null, permissionMode: 'bypassPermissions' }}
      />,
    )
    expect(screen.getByText('bypassPermissions')).toBeInTheDocument()
  })

  it('renders idle fallback when status is null', () => {
    render(<SessionStatusPill data={{ type: 'session_status', status: null }} />)
    expect(screen.getByText('idle')).toBeInTheDocument()
  })
})

// ── HookEventPill ─────────────────────────────────────────────────────────────

describe('HookEventPill', () => {
  it('renders hookName and phase', () => {
    render(
      <HookEventPill
        data={{
          type: 'hook_event',
          phase: 'response',
          hookId: 'h1',
          hookName: 'my-hook',
          hookEventName: 'PreToolUse',
          outcome: 'success',
        }}
      />,
    )
    expect(screen.getByText(/my-hook/)).toBeInTheDocument()
    expect(screen.getByText(/response/)).toBeInTheDocument()
  })

  it('applies red text for error outcome', () => {
    const { container } = render(
      <HookEventPill
        data={{
          type: 'hook_event',
          phase: 'response',
          hookId: 'h1',
          hookName: 'bad-hook',
          hookEventName: 'PostToolUse',
          outcome: 'error',
        }}
      />,
    )
    const div = container.querySelector('div')
    expect(div?.className).toContain('text-red-500')
  })

  it('does not apply red text for success outcome', () => {
    const { container } = render(
      <HookEventPill
        data={{
          type: 'hook_event',
          phase: 'response',
          hookId: 'h1',
          hookName: 'good-hook',
          hookEventName: 'PreToolUse',
          outcome: 'success',
        }}
      />,
    )
    const div = container.querySelector('div')
    expect(div?.className).not.toContain('text-red-500')
  })
})

// ── FilesSavedPill ────────────────────────────────────────────────────────────

describe('FilesSavedPill', () => {
  it('renders saved count', () => {
    render(
      <FilesSavedPill
        data={{
          type: 'files_saved',
          files: [{ filename: 'a.ts', fileId: '1' }],
          failed: [],
          processedAt: '',
        }}
      />,
    )
    expect(screen.getByText('1 saved')).toBeInTheDocument()
  })

  it('renders failed count when present', () => {
    render(
      <FilesSavedPill
        data={{
          type: 'files_saved',
          files: [{ filename: 'a.ts', fileId: '1' }],
          failed: [{ filename: 'b.ts', error: 'oops' }],
          processedAt: '',
        }}
      />,
    )
    expect(screen.getByText('1 saved, 1 failed')).toBeInTheDocument()
  })

  it('renders zero counts gracefully', () => {
    render(
      <FilesSavedPill data={{ type: 'files_saved', files: [], failed: [], processedAt: '' }} />,
    )
    expect(screen.getByText('0 saved')).toBeInTheDocument()
  })
})

// ── CommandOutputBlock ────────────────────────────────────────────────────────

describe('CommandOutputBlock', () => {
  it('renders content text', () => {
    render(<CommandOutputBlock data={{ type: 'command_output', content: 'hello output' }} />)
    expect(screen.getByText('hello output')).toBeInTheDocument()
  })

  it('renders without crash when content is undefined', () => {
    // @ts-expect-error intentional partial data for resilience test
    render(<CommandOutputBlock data={{}} />)
  })
})

// ── LocalCommandBlock ─────────────────────────────────────────────────────────

describe('LocalCommandBlock', () => {
  it('strips XML wrapper tags from content', () => {
    render(
      <LocalCommandBlock
        data={{
          type: 'system',
          subtype: 'local_command',
          content: '<local-command-stdout>stripped content</local-command-stdout>',
        }}
      />,
    )
    expect(screen.getByText('stripped content')).toBeInTheDocument()
    expect(screen.queryByText('<local-command-stdout>')).not.toBeInTheDocument()
  })

  it('renders plain content without tags', () => {
    render(
      <LocalCommandBlock
        data={{ type: 'system', subtype: 'local_command', content: 'plain output' }}
      />,
    )
    expect(screen.getByText('plain output')).toBeInTheDocument()
  })

  it('handles undefined content gracefully', () => {
    // @ts-expect-error intentional partial data
    render(<LocalCommandBlock data={{ type: 'system', subtype: 'local_command' }} />)
  })
})

// ── FileHistorySnapshotPill ───────────────────────────────────────────────────

describe('FileHistorySnapshotPill', () => {
  it('renders file count from files array', () => {
    render(
      <FileHistorySnapshotPill
        data={{
          type: 'file-history-snapshot',
          files: [{ path: 'a' }, { path: 'b' }],
          isSnapshotUpdate: false,
        }}
      />,
    )
    expect(screen.getByText('2 file(s) snapshot')).toBeInTheDocument()
  })

  it('appends update suffix when isSnapshotUpdate is true', () => {
    render(
      <FileHistorySnapshotPill
        data={{
          type: 'file-history-snapshot',
          files: [{ path: 'a' }],
          isSnapshotUpdate: true,
        }}
      />,
    )
    expect(screen.getByText('1 file(s) snapshot (update)')).toBeInTheDocument()
  })

  it('renders zero when no files', () => {
    render(
      <FileHistorySnapshotPill
        data={{ type: 'file-history-snapshot', files: [], isSnapshotUpdate: false }}
      />,
    )
    expect(screen.getByText('0 file(s) snapshot')).toBeInTheDocument()
  })
})

// ── AiTitlePill ───────────────────────────────────────────────────────────────

describe('AiTitlePill', () => {
  it('renders the aiTitle text', () => {
    render(<AiTitlePill data={{ type: 'ai_title', aiTitle: 'My Session Title' }} />)
    expect(screen.getByText('My Session Title')).toBeInTheDocument()
  })

  it('renders nothing visible when aiTitle is undefined', () => {
    // @ts-expect-error intentional partial data
    render(<AiTitlePill data={{}} />)
    // Should not throw
  })
})

// ── LastPromptPill ────────────────────────────────────────────────────────────

describe('LastPromptPill', () => {
  it('renders prompt text under 100 chars as-is', () => {
    render(<LastPromptPill data={{ type: 'last_prompt', lastPrompt: 'Short prompt' }} />)
    expect(screen.getByText('Short prompt')).toBeInTheDocument()
  })

  it('truncates prompt at 100 chars with ellipsis', () => {
    const long = 'x'.repeat(150)
    render(<LastPromptPill data={{ type: 'last_prompt', lastPrompt: long }} />)
    const displayed = screen.getByText(/x+\u2026/)
    expect(displayed.textContent?.length).toBeLessThanOrEqual(102) // 100 + ellipsis char
  })

  it('renders without crash when lastPrompt is undefined', () => {
    // @ts-expect-error intentional partial data
    render(<LastPromptPill data={{}} />)
  })
})

// ── WorktreeStatePill ─────────────────────────────────────────────────────────

describe('WorktreeStatePill', () => {
  it('renders worktree name and branch', () => {
    render(
      <WorktreeStatePill
        data={{
          type: 'worktree_state',
          worktreeSession: {
            worktreeName: 'feature-branch',
            worktreeBranch: 'feat/new-feature',
          },
        }}
      />,
    )
    expect(screen.getByText(/feature-branch/)).toBeInTheDocument()
    expect(screen.getByText(/feat\/new-feature/)).toBeInTheDocument()
  })

  it('renders without crash when worktreeSession is undefined', () => {
    // @ts-expect-error intentional partial data
    render(<WorktreeStatePill data={{}} />)
  })
})

// ── AttachmentPill — async_hook_response ──────────────────────────────────────

describe('AttachmentPill async_hook_response', () => {
  it('renders hookName and hookEvent badge', () => {
    render(
      <AttachmentPill
        data={{
          attachment: {
            type: 'async_hook_response',
            hookName: 'my-async-hook',
            hookEvent: 'PostToolUse',
            exitCode: 0,
          },
        }}
      />,
    )
    expect(screen.getByText('my-async-hook')).toBeInTheDocument()
    expect(screen.getByText('PostToolUse')).toBeInTheDocument()
  })

  it('renders exit code badge when exitCode is non-zero', () => {
    render(
      <AttachmentPill
        data={{
          attachment: {
            type: 'async_hook_response',
            hookName: 'hook',
            hookEvent: 'PreToolUse',
            exitCode: 1,
          },
        }}
      />,
    )
    expect(screen.getByText('exit 1')).toBeInTheDocument()
  })

  it('does not render exit code badge when exitCode is 0', () => {
    render(
      <AttachmentPill
        data={{
          attachment: {
            type: 'async_hook_response',
            hookName: 'hook',
            hookEvent: 'PreToolUse',
            exitCode: 0,
          },
        }}
      />,
    )
    expect(screen.queryByText(/exit 0/)).not.toBeInTheDocument()
  })
})

// ── AttachmentPill — file type ────────────────────────────────────────────────

describe('AttachmentPill file type', () => {
  it('renders added and removed counts', () => {
    render(
      <AttachmentPill
        data={{
          attachment: {
            type: 'file',
            addedNames: ['a.ts', 'b.ts'],
            removedNames: ['c.ts'],
            addedLines: 42,
          },
        }}
      />,
    )
    expect(screen.getByText(/2 added, 1 removed/)).toBeInTheDocument()
  })

  it('renders addedLines when present', () => {
    render(
      <AttachmentPill
        data={{
          attachment: {
            type: 'file',
            addedNames: ['a.ts'],
            removedNames: [],
            addedLines: 10,
          },
        }}
      />,
    )
    expect(screen.getByText('+10 lines')).toBeInTheDocument()
  })
})

// ── AttachmentPill — generic type ─────────────────────────────────────────────

describe('AttachmentPill generic type', () => {
  it('renders type badge and CollapsibleJson label', () => {
    render(
      <AttachmentPill
        data={{
          attachment: {
            type: 'custom_payload',
            someData: 'value',
          },
        }}
      />,
    )
    expect(screen.getByText('custom_payload')).toBeInTheDocument()
    expect(screen.getByText('Attachment')).toBeInTheDocument()
  })
})

// ── AttachmentPill — null attachment guard ────────────────────────────────────

describe('AttachmentPill null attachment', () => {
  it('renders fallback when attachment is null', () => {
    render(<AttachmentPill data={{}} />)
    expect(screen.getByText('attachment')).toBeInTheDocument()
  })

  it('renders fallback when data is empty object', () => {
    render(<AttachmentPill data={{ attachment: null }} />)
    expect(screen.getByText('attachment')).toBeInTheDocument()
  })
})

// ── PermissionModeChangePill ──────────────────────────────────────────────────

describe('PermissionModeChangePill', () => {
  it('renders permission mode label and badge', () => {
    render(<PermissionModeChangePill data={{ permissionMode: 'bypassPermissions' }} />)
    expect(screen.getByText('Permission mode:')).toBeInTheDocument()
    expect(screen.getByText('bypassPermissions')).toBeInTheDocument()
  })

  it('renders label even when permissionMode is empty', () => {
    render(<PermissionModeChangePill data={{}} />)
    expect(screen.getByText('Permission mode:')).toBeInTheDocument()
  })
})

// ── ScheduledTaskFirePill ─────────────────────────────────────────────────────

describe('ScheduledTaskFirePill', () => {
  it('renders content text', () => {
    render(<ScheduledTaskFirePill data={{ content: 'Run daily backup' }} />)
    expect(screen.getByText('Run daily backup')).toBeInTheDocument()
  })

  it('renders default text when content is absent', () => {
    render(<ScheduledTaskFirePill data={{}} />)
    expect(screen.getByText('Scheduled task fired')).toBeInTheDocument()
  })
})

// ── UnknownSystemPill ─────────────────────────────────────────────────────────

describe('UnknownSystemPill', () => {
  it('renders sdkType label when provided', () => {
    render(<UnknownSystemPill data={{}} sdkType="some_future_type" />)
    expect(screen.getByText('some_future_type')).toBeInTheDocument()
  })

  it('renders data type field as label when sdkType absent', () => {
    render(<UnknownSystemPill data={{ type: 'mystery_event', value: 42 }} />)
    expect(screen.getByText('mystery_event')).toBeInTheDocument()
  })

  it('renders CollapsibleJson for the data', () => {
    render(<UnknownSystemPill data={{ key: 'val' }} />)
    expect(screen.getByText('data')).toBeInTheDocument()
  })

  it('falls back to "unknown" label when neither sdkType nor type provided', () => {
    render(<UnknownSystemPill data={{}} />)
    expect(screen.getByText('unknown')).toBeInTheDocument()
  })
})
