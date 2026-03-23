import type { Meta, StoryObj } from '@storybook/react-vite'
import { permissionRequests } from '../../../../stories/fixtures'
import { PermissionCard } from './PermissionCard'

const meta = {
  title: 'Chat/Cards/PermissionCard',
  component: PermissionCard,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[480px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof PermissionCard>

export default meta
type Story = StoryObj<typeof meta>

export const BashCommand: Story = {
  args: {
    permission: permissionRequests.bash,
    onRespond: () => {},
  },
}

export const FileEdit: Story = {
  args: {
    permission: permissionRequests.edit,
    onRespond: () => {},
  },
}

export const WithAlwaysAllow: Story = {
  args: {
    permission: permissionRequests.write,
    onRespond: () => {},
    onAlwaysAllow: () => {},
  },
}

export const Allowed: Story = {
  args: {
    permission: permissionRequests.bash,
    resolved: { allowed: true },
  },
}

export const Denied: Story = {
  args: {
    permission: permissionRequests.bash,
    resolved: { allowed: false },
  },
}

export const Pending: Story = {
  args: {
    permission: permissionRequests.bash,
    onRespond: () => {},
    isPending: true,
  },
}
