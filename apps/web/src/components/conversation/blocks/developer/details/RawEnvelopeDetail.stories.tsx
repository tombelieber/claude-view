import type { Meta, StoryObj } from '@storybook/react-vite'
import { rawJsonFixtures } from '../../../../../stories/fixtures-developer'
import { RawEnvelopeDetail } from './RawEnvelopeDetail'

const meta = {
  title: 'Developer/Details/RawEnvelopeDetail',
  component: RawEnvelopeDetail,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[480px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof RawEnvelopeDetail>

export default meta
type Story = StoryObj<typeof meta>

export const WithFields: Story = {
  args: { rawJson: rawJsonFixtures.full },
}

export const WithRenderedKeysFiltered: Story = {
  args: {
    rawJson: rawJsonFixtures.full,
    renderedKeys: [
      'parentUuid',
      'logicalParentUuid',
      'isSidechain',
      'agentId',
      'uuid',
      'messageId',
      'sessionId',
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
    ],
  },
}

export const Empty: Story = {
  args: { rawJson: rawJsonFixtures.empty },
}

export const NullRawJson: Story = {
  args: { rawJson: null },
}
