import type { Meta, StoryObj } from '@storybook/react-vite'
import { ApiErrorDetail } from '../../components/conversation/blocks/developer/details/ApiErrorDetail'
import { HookMetadataDetail } from '../../components/conversation/blocks/developer/details/HookMetadataDetail'
import { MessageLineageDetail } from '../../components/conversation/blocks/developer/details/MessageLineageDetail'
import { RawEnvelopeDetail } from '../../components/conversation/blocks/developer/details/RawEnvelopeDetail'
import { RetryDetail } from '../../components/conversation/blocks/developer/details/RetryDetail'
import { StopReasonDetail } from '../../components/conversation/blocks/developer/details/StopReasonDetail'
import { ThinkingMetadataDetail } from '../../components/conversation/blocks/developer/details/ThinkingMetadataDetail'
import { rawJsonFixtures } from '../fixtures-developer'

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
    <div className="w-[520px] max-w-full space-y-8">
      <Section title="ApiErrorDetail">
        <Label>with error</Label>
        <ApiErrorDetail rawJson={rawJsonFixtures.withApiError} />
        <Label>no error</Label>
        <ApiErrorDetail rawJson={rawJsonFixtures.empty} />
      </Section>

      <Section title="StopReasonDetail">
        <Label>end_turn</Label>
        <StopReasonDetail rawJson={rawJsonFixtures.withStopReason} />
        <Label>max_tokens — prevented</Label>
        <StopReasonDetail rawJson={rawJsonFixtures.withStopReasonPrevented} />
      </Section>

      <Section title="ThinkingMetadataDetail">
        <Label>with metadata</Label>
        <ThinkingMetadataDetail rawJson={rawJsonFixtures.withThinkingMetadata} />
      </Section>

      <Section title="RetryDetail">
        <Label>retrying (2/5 in 5000ms)</Label>
        <RetryDetail rawJson={rawJsonFixtures.withRetry} />
      </Section>

      <Section title="HookMetadataDetail">
        <Label>hooks ok</Label>
        <HookMetadataDetail rawJson={rawJsonFixtures.withHooks} />
        <Label>hooks with errors</Label>
        <HookMetadataDetail rawJson={rawJsonFixtures.withHookErrors} />
      </Section>

      <Section title="MessageLineageDetail">
        <Label>with lineage (click to expand)</Label>
        <MessageLineageDetail rawJson={rawJsonFixtures.withLineage} />
      </Section>

      <Section title="RawEnvelopeDetail">
        <Label>with extra fields (click to expand)</Label>
        <RawEnvelopeDetail
          rawJson={{ ...rawJsonFixtures.full, extraField: 'value', debugInfo: { step: 3 } }}
          renderedKeys={[]}
        />
        <Label>with rendered keys filtered</Label>
        <RawEnvelopeDetail
          rawJson={rawJsonFixtures.full}
          renderedKeys={[
            'stopReason',
            'preventedContinuation',
            'hasOutput',
            'apiError',
            'thinkingMetadata',
            'retryInMs',
            'retryAttempt',
            'maxRetries',
            'hookCount',
            'hookInfos',
            'hookErrors',
          ]}
        />
      </Section>
    </div>
  )
}

const meta = {
  title: 'Gallery/Developer Details',
  component: Gallery,
  tags: [],
  parameters: { layout: 'padded' },
} satisfies Meta<typeof Gallery>

export default meta
type Story = StoryObj<typeof meta>

export const AllVariants: Story = {}
