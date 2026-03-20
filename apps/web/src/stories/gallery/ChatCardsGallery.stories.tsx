import type { Meta, StoryObj } from '@storybook/react-vite'
import { InteractiveCardShell } from '../../components/chat/cards/InteractiveCardShell'
import { AskUserQuestionCard } from '../../components/conversation/blocks/shared/AskUserQuestionCard'
import { ElicitationCard } from '../../components/conversation/blocks/shared/ElicitationCard'
import { PermissionCard } from '../../components/conversation/blocks/shared/PermissionCard'
import { PlanApprovalCard } from '../../components/conversation/blocks/shared/PlanApprovalCard'
import { askQuestions, elicitations, permissionRequests, planApprovals } from '../fixtures'

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
      <Section title="PermissionCard">
        <Label>bash command — pending</Label>
        <PermissionCard permission={permissionRequests.bash} onRespond={() => {}} />
        <Label>file edit — with reason</Label>
        <PermissionCard permission={permissionRequests.edit} onRespond={() => {}} />
        <Label>write — with always allow</Label>
        <PermissionCard
          permission={permissionRequests.write}
          onRespond={() => {}}
          onAlwaysAllow={() => {}}
        />
        <Label>resolved — allowed</Label>
        <PermissionCard permission={permissionRequests.bash} resolved={{ allowed: true }} />
        <Label>resolved — denied</Label>
        <PermissionCard permission={permissionRequests.bash} resolved={{ allowed: false }} />
      </Section>

      <Section title="AskUserQuestionCard">
        <Label>single select</Label>
        <AskUserQuestionCard question={askQuestions.singleSelect} onAnswer={() => {}} />
        <Label>multi select</Label>
        <AskUserQuestionCard question={askQuestions.multiSelect} onAnswer={() => {}} />
        <Label>answered</Label>
        <AskUserQuestionCard
          question={askQuestions.singleSelect}
          answered
          selectedAnswers={{
            'Which authentication strategy should I use for this service?': 'JWT with RSA-256',
          }}
        />
      </Section>

      <Section title="PlanApprovalCard">
        <Label>pending</Label>
        <PlanApprovalCard plan={planApprovals.simple} onApprove={() => {}} />
        <Label>approved</Label>
        <PlanApprovalCard plan={planApprovals.simple} resolved={{ approved: true }} />
        <Label>rejected</Label>
        <PlanApprovalCard plan={planApprovals.simple} resolved={{ approved: false }} />
      </Section>

      <Section title="ElicitationCard">
        <Label>pending</Label>
        <ElicitationCard elicitation={elicitations.simple} onSubmit={() => {}} />
        <Label>submitted</Label>
        <ElicitationCard elicitation={elicitations.simple} resolved />
      </Section>

      <Section title="InteractiveCardShell — variants">
        <div className="grid grid-cols-2 gap-3">
          <div>
            <Label>permission</Label>
            <InteractiveCardShell variant="permission" header="Permission Required">
              <p className="text-xs text-gray-700 dark:text-gray-300">Content</p>
            </InteractiveCardShell>
          </div>
          <div>
            <Label>question</Label>
            <InteractiveCardShell variant="question" header="Question">
              <p className="text-xs text-gray-700 dark:text-gray-300">Content</p>
            </InteractiveCardShell>
          </div>
          <div>
            <Label>plan</Label>
            <InteractiveCardShell variant="plan" header="Plan Approval">
              <p className="text-xs text-gray-700 dark:text-gray-300">Content</p>
            </InteractiveCardShell>
          </div>
          <div>
            <Label>elicitation</Label>
            <InteractiveCardShell variant="elicitation" header="Input Requested">
              <p className="text-xs text-gray-700 dark:text-gray-300">Content</p>
            </InteractiveCardShell>
          </div>
        </div>
        <div className="grid grid-cols-3 gap-3">
          <div>
            <Label>resolved success</Label>
            <InteractiveCardShell
              variant="permission"
              header="Done"
              resolved={{ label: 'Allowed', variant: 'success' }}
            >
              <p className="text-xs">OK</p>
            </InteractiveCardShell>
          </div>
          <div>
            <Label>resolved denied</Label>
            <InteractiveCardShell
              variant="permission"
              header="Done"
              resolved={{ label: 'Denied', variant: 'denied' }}
            >
              <p className="text-xs">No</p>
            </InteractiveCardShell>
          </div>
          <div>
            <Label>resolved neutral</Label>
            <InteractiveCardShell
              variant="elicitation"
              header="Done"
              resolved={{ label: 'Submitted', variant: 'neutral' }}
            >
              <p className="text-xs">OK</p>
            </InteractiveCardShell>
          </div>
        </div>
      </Section>
    </div>
  )
}

const meta = {
  title: 'Gallery/Chat Cards',
  component: Gallery,
  tags: [],
  parameters: { layout: 'padded' },
} satisfies Meta<typeof Gallery>

export default meta
type Story = StoryObj<typeof meta>

export const AllVariants: Story = {}
