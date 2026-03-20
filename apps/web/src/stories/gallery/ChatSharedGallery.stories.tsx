import type { Meta, StoryObj } from '@storybook/react-vite'
import { DayDivider } from '../../components/conversation/DayDivider'
import { Markdown } from '../../components/conversation/blocks/shared/Markdown'
import { MessageTimestamp } from '../../components/conversation/blocks/shared/MessageTimestamp'
import { ToolChip } from '../../components/conversation/blocks/shared/ToolChip'
import { ToolDetail } from '../../components/conversation/blocks/shared/ToolDetail'
import { toolExecutions } from '../fixtures'

const NOW = Math.floor(Date.now() / 1000)

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="space-y-2">
      <h3 className="text-[11px] font-mono font-semibold uppercase tracking-widest text-gray-400 dark:text-gray-500 border-b border-gray-200 dark:border-gray-700 pb-1">
        {title}
      </h3>
      <div className="space-y-3">{children}</div>
    </div>
  )
}

function Label({ children }: { children: React.ReactNode }) {
  return (
    <span className="text-[9px] font-mono text-gray-400 dark:text-gray-500 uppercase tracking-wide">
      {children}
    </span>
  )
}

function Gallery() {
  return (
    <div className="w-[640px] max-w-full space-y-8">
      <Section title="ToolChip — all states">
        <div className="flex flex-wrap gap-2">
          <ToolChip execution={toolExecutions.bashRunning} />
          <ToolChip execution={toolExecutions.bashComplete} />
          <ToolChip execution={toolExecutions.bashError} />
          <ToolChip execution={toolExecutions.readComplete} />
          <ToolChip execution={toolExecutions.editComplete} />
          <ToolChip execution={toolExecutions.grepRunning} />
        </div>
      </Section>

      <Section title="ToolDetail — all states">
        <div className="space-y-3">
          <Label>bash running</Label>
          <ToolDetail execution={toolExecutions.bashRunning} />
          <Label>bash complete</Label>
          <ToolDetail execution={toolExecutions.bashComplete} />
          <Label>bash error</Label>
          <ToolDetail execution={toolExecutions.bashError} />
          <Label>read complete</Label>
          <ToolDetail execution={toolExecutions.readComplete} />
          <Label>edit complete</Label>
          <ToolDetail execution={toolExecutions.editComplete} />
        </div>
      </Section>

      <Section title="MessageTimestamp">
        <div className="flex items-center gap-6">
          <div>
            <Label>recent</Label>
            <MessageTimestamp timestamp={NOW - 60} />
          </div>
          <div>
            <Label>hours ago</Label>
            <MessageTimestamp timestamp={NOW - 3600 * 3} />
          </div>
          <div>
            <Label>align right</Label>
            <MessageTimestamp timestamp={NOW - 300} align="right" />
          </div>
          <div>
            <Label>no timestamp</Label>
            <MessageTimestamp timestamp={undefined} />
          </div>
        </div>
      </Section>

      <Section title="DayDivider">
        <DayDivider label="Today" />
        <DayDivider label="Yesterday" />
        <DayDivider label="Monday" />
        <DayDivider label="Sat, Mar 15" />
      </Section>

      <Section title="Markdown">
        <Label>inline formatting</Label>
        <Markdown content="Text with **bold**, *italic*, `inline code`, and [links](https://example.com)." />
        <Label>code block</Label>
        <Markdown content={'```rust\nfn main() {\n    println!("Hello!");\n}\n```'} />
        <Label>table</Label>
        <Markdown
          content={'| Module | Lines |\n|--------|-------|\n| auth.rs | 120 |\n| session.rs | 90 |'}
        />
        <Label>blockquote</Label>
        <Markdown content="> **Note**: Cache TTL should match JWT expiry." />
      </Section>
    </div>
  )
}

const meta = {
  title: 'Gallery/Chat Shared',
  component: Gallery,
  tags: [],
  parameters: { layout: 'padded' },
} satisfies Meta<typeof Gallery>

export default meta
type Story = StoryObj<typeof meta>

export const AllVariants: Story = {}
